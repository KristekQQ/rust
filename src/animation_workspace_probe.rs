//! Deterministic native diagnostic; timings and allocation observation are separate builds.
use crate::{gltf_model::ModelAsset, scene::SceneManager};
use std::{sync::Arc, time::Instant};

#[cfg(feature = "animation-allocation-probe")]
mod allocations {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        cell::Cell,
    };
    thread_local! { static COUNTER:Cell<(bool,u64,u64)>=const{Cell::new((false,0,0))}; }
    struct Observed;
    fn record(size: usize) {
        let _ = COUNTER.try_with(|c| {
            let (on, n, b) = c.get();
            if on {
                c.set((on, n + 1, b + size as u64));
            }
        });
    }
    unsafe impl GlobalAlloc for Observed {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            record(l.size());
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            record(l.size());
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            record(n);
            unsafe { System.realloc(p, l, n) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: Observed = Observed;
    pub fn begin() {
        COUNTER.with(|c| c.set((true, 0, 0)));
    }
    pub fn end() -> (u64, u64) {
        COUNTER.with(|c| {
            let (_, n, b) = c.get();
            c.set((false, n, b));
            (n, b)
        })
    }
}
fn assets() -> Vec<Arc<ModelAsset>> {
    ["blade-wraith", "core-tank", "drone-orbiter", "madmechanic"]
        .iter()
        .map(|slug| {
            let bytes = std::fs::read(format!(
                "{}/assets/models/rim/{slug}.glb",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            Arc::new(crate::gltf_loader::parse_model(&bytes, &[], &[]).unwrap())
        })
        .collect()
}
fn scene(assets: &[Arc<ModelAsset>], count: usize) -> (SceneManager, Vec<u32>) {
    let mut scene = SceneManager::default();
    let mut ids = Vec::with_capacity(count);
    let authored = [0, 0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3];
    for i in 0..count {
        let asset = assets[if count == 15 {
            authored[i]
        } else {
            i % assets.len()
        }]
        .clone();
        let clip = asset
            .animations
            .iter()
            .position(|c| c.name.as_deref() == Some("Idle"))
            .unwrap_or(0);
        let id = scene.add_model(asset).unwrap();
        scene.play_model_animation(id, clip, true, 1.).unwrap();
        ids.push(id);
    }
    (scene, ids)
}
fn digest(scene: &SceneManager, ids: &[u32]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |v: u32| {
        for byte in v.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    for &id in ids {
        let m = scene.model(id).unwrap();
        add(id);
        add(m.revision as u32);
        for trs in &m.pose.local_trs {
            for x in trs
                .translation
                .to_array()
                .into_iter()
                .chain(trs.rotation.to_array())
                .chain(trs.scale.to_array())
            {
                add(x.to_bits());
            }
        }
        for matrix in &m.pose.world_matrices {
            for x in matrix.to_cols_array() {
                add(x.to_bits());
            }
        }
        for weights in &m.pose.node_weights {
            for x in weights {
                add(x.to_bits());
            }
        }
        for skin in &m.pose.skin_matrices {
            for matrix in skin {
                for x in matrix.to_cols_array() {
                    add(x.to_bits());
                }
            }
        }
        for &flag in &m.pose.skin_world_space {
            add(flag as u32);
        }
        for &node in &m.pose.active_nodes {
            add(node as u32);
        }
        if let Some(p) = &m.playback {
            add(p.time.to_bits());
            add(p.playing as u32);
        }
    }
    hash
}
fn workspace_bytes(scene: &SceneManager, ids: &[u32]) -> usize {
    ids.iter()
        .map(|id| {
            scene
                .model(*id)
                .unwrap()
                .animation_workspace
                .retained_bytes()
        })
        .sum()
}
#[test]
#[ignore = "explicit release diagnostic only; not a publication gate benchmark"]
fn animation_workspace_probe() {
    let assets = assets();
    let mut rows = Vec::new();
    for count in [1, 15, 64] {
        let (mut scene, ids) = scene(&assets, count);
        for _ in 0..120 {
            scene.update(1. / 90.);
        }
        #[cfg(feature = "animation-allocation-probe")]
        allocations::begin();
        let started = Instant::now();
        for _ in 0..900 {
            scene.update(std::hint::black_box(1. / 90.));
            std::hint::black_box(&scene);
        }
        let elapsed = started.elapsed().as_secs_f64() * 1000.;
        #[cfg(feature = "animation-allocation-probe")]
        let counts = Some(allocations::end());
        #[cfg(not(feature = "animation-allocation-probe"))]
        let counts: Option<(u64, u64)> = None;
        let mut trace = Vec::new();
        for frame in 0..64 {
            scene.update(if frame % 7 == 0 { 1. / 120. } else { 1. / 90. });
            trace.push(format!("{:016x}", digest(&scene, &ids)));
        }
        rows.push(serde_json::json!({"models":count,"frames":900,"elapsedMs":elapsed,"msPerUpdate":elapsed/900.,"allocationObservation":cfg!(feature="animation-allocation-probe"),"allocations":counts.map(|v|v.0),"allocatedBytes":counts.map(|v|v.1),"retainedWorkspaceBytes":workspace_bytes(&scene,&ids),"trace":trace}));
    }
    let result = serde_json::json!({"scope":"native whole Scene::update; actual Rome rigid assets, Idle; no rendering/FPS/GPU inference","warmupUpdates":120,"rows":rows});
    if let Ok(path) = std::env::var("ANIMATION_PROBE_OUT") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        writeln!(file, "{}", serde_json::to_string_pretty(&result).unwrap()).unwrap();
    } else {
        println!("{}", result);
    }
}
