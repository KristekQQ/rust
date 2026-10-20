//! Opt-in Rust-owned combo rows. Assets remain shared; only live rows own instances.
use crate::{scene::SceneManager as Scene, xr_round::ComboEvent};
use glam::Vec3;
use serde_json::{json, Value};

struct Symbol {
    kind: String,
    asset: u32,
    variant: u32,
}
pub struct ComboDisplay {
    symbols: Vec<Symbol>,
    frame_asset: u32,
    origin: Vec3,
    spacing: f32,
    row_gap: f32,
    fruit_scale: f32,
    frame_scale: Vec3,
    seconds: f32,
    pub timer_panel: u32,
    pub score_panel: u32,
    awards: [u32; 3],
    current: Option<Row>,
    history: Vec<Row>,
}
struct Row {
    kind: String,
    count: u32,
    success: bool,
    points: u32,
    age: f32,
    icons: Vec<u32>,
    frames: Vec<u32>,
}
impl Row {
    fn new(kind: String, count: u32, success: bool, points: u32) -> Self {
        Self {
            kind,
            count,
            success,
            points,
            age: 0.,
            icons: vec![],
            frames: vec![],
        }
    }
    fn remove(&mut self, scene: &mut Scene) {
        for id in self.icons.drain(..).chain(self.frames.drain(..)) {
            scene.remove_object(id);
        }
    }
    fn value(&self, seconds: f32, label: u32) -> Value {
        json!({"kind":self.kind,"count":self.count,"success":self.success,"points":self.points,
            "age":self.age,"opacity":(1.-self.age/seconds).clamp(0.,1.),"icons":self.icons,"frames":self.frames,"label":label})
    }
}
impl ComboDisplay {
    pub fn configure(scene: &Scene, input: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(input).ok()?;
        let id = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
        let vector = |key: &str, default: [f32; 3]| -> Option<Vec3> {
            let Some(v) = v.get(key) else {
                return Some(Vec3::from(default));
            };
            let a = v.as_array()?;
            if a.len() != 3 {
                return None;
            }
            let out = Vec3::new(
                a[0].as_f64()? as f32,
                a[1].as_f64()? as f32,
                a[2].as_f64()? as f32,
            );
            out.is_finite().then_some(out)
        };
        let number = |key: &str, default: f32| -> Option<f32> {
            let n = v
                .get(key)
                .map_or(Some(default), |n| n.as_f64().map(|n| n as f32))?;
            (n.is_finite() && n > 0. && n <= 100.).then_some(n)
        };
        let mut symbols = vec![];
        for s in v["symbols"].as_array()? {
            let kind = s["kind"].as_str()?.to_owned();
            let asset = id(&s["asset"])?;
            let variant = id(&s["variant"])?;
            if kind.is_empty()
                || symbols.iter().any(|s: &Symbol| s.kind == kind)
                || !scene
                    .mesh_variants
                    .get(&asset)
                    .is_some_and(|a| a.registered && (variant as usize) < a.variants.len())
            {
                return None;
            }
            symbols.push(Symbol {
                kind,
                asset,
                variant,
            });
        }
        if symbols.is_empty() || symbols.len() > 64 {
            return None;
        }
        let frame_asset = id(&v["frameAsset"])?;
        if !scene
            .mesh_variants
            .get(&frame_asset)
            .is_some_and(|a| a.registered)
        {
            return None;
        }
        let timer_panel = id(&v["timerPanel"])?;
        let score_panel = id(&v["scorePanel"])?;
        let a = v["awardPanels"].as_array()?;
        if a.len() != 3 {
            return None;
        }
        let awards = [id(&a[0])?, id(&a[1])?, id(&a[2])?];
        let panels = [timer_panel, score_panel, awards[0], awards[1], awards[2]];
        for (i, p) in panels.iter().enumerate() {
            let expected = match i {
                0 => crate::info_panel::PanelKind::Countdown,
                1 => crate::info_panel::PanelKind::Score,
                _ => crate::info_panel::PanelKind::ComboAward,
            };
            if panels[..i].contains(p)
                || !scene
                    .info_panels
                    .get(p)
                    .is_some_and(|panel| panel.kind == expected)
            {
                return None;
            }
        }
        Some(Self {
            symbols,
            frame_asset,
            origin: vector("origin", [0., 1.5, -2.])?,
            spacing: number("spacing", 0.4)?,
            row_gap: number("rowGap", 0.4)?,
            fruit_scale: number("fruitScale", 0.1)?,
            frame_scale: vector("frameScale", [0.15, 1., 1.])?,
            seconds: number("historySeconds", 2.)?,
            timer_panel,
            score_panel,
            awards,
            current: None,
            history: vec![],
        })
    }
    pub fn reset(&mut self, scene: &mut Scene) {
        if let Some(mut r) = self.current.take() {
            r.remove(scene);
        }
        for r in &mut self.history {
            r.remove(scene);
        }
        self.history.clear();
        self.sync(scene);
    }
    pub fn event(&mut self, scene: &mut Scene, event: ComboEvent) {
        if let Some((kind, count)) = event.failed {
            if let Some(mut row) = self.current.take() {
                row.remove(scene);
            }
            self.history.insert(0, Row::new(kind, count, false, 0));
        }
        if let Some(mut row) = self.current.take() {
            row.remove(scene);
        }
        if event.success {
            self.history
                .insert(0, Row::new(event.kind, event.count, true, event.points));
        } else {
            self.current = Some(Row::new(event.kind, event.count, false, 0));
        }
        while self.history.len() > 3 {
            self.history.pop().unwrap().remove(scene);
        }
        self.sync(scene);
    }
    pub fn advance(&mut self, scene: &mut Scene, dt: f32) {
        if !dt.is_finite() || dt <= 0. || self.history.is_empty() {
            return;
        }
        for r in &mut self.history {
            r.age += dt;
        }
        self.history.retain_mut(|r| {
            if r.age >= self.seconds {
                r.remove(scene);
                false
            } else {
                true
            }
        });
        self.sync(scene);
    }
    fn sync(&mut self, scene: &mut Scene) {
        for (row_index, row) in self
            .current
            .iter_mut()
            .map(|r| (0, r))
            .chain(self.history.iter_mut().enumerate().map(|(i, r)| (i + 1, r)))
        {
            let Some(symbol) = self.symbols.iter().find(|s| s.kind == row.kind) else {
                continue;
            };
            while row.icons.len() < row.count.min(3) as usize {
                let Ok(id) = scene.add_mesh_variant(symbol.asset, symbol.variant) else {
                    break;
                };
                row.icons.push(id);
            }
            while row.frames.len() < 3 {
                let Ok(id) = scene.add_mesh_variant(self.frame_asset, 0) else {
                    break;
                };
                row.frames.push(id);
            }
            let opacity = if row_index == 0 {
                1.
            } else {
                (1. - row.age / self.seconds).clamp(0., 1.)
            };
            for (ids, scale, emission, icons) in [
                (&row.icons, Vec3::splat(self.fruit_scale), 3., true),
                (&row.frames, self.frame_scale, 7., false),
            ] {
                for (slot, &id) in ids.iter().enumerate() {
                    scene.set_object_position(
                        id,
                        self.origin
                            + Vec3::new(
                                (slot as f32 - 1.) * self.spacing,
                                -(row_index as f32) * self.row_gap,
                                0.,
                            ),
                    );
                    scene.set_object_scale(id, scale);
                    let unearned = !icons && slot >= row.count as usize;
                    scene.set_opacity(id, opacity * if unearned { 0.22 } else { 1. }, 8., 0., -1.);
                    scene.set_emission(
                        id,
                        emission * opacity * opacity * if unearned { 0.25 } else { 1. },
                    );
                    scene.set_render_options(
                        id,
                        crate::scene::RenderOptions {
                            casts_shadow: false,
                            receives_shadow: false,
                            ..Default::default()
                        },
                    );
                }
            }
        }
        for (index, id) in self.awards.iter().enumerate() {
            let row = self.history.get(index);
            let label = row.map_or(String::new(), |r| {
                if r.success {
                    format!("+{}", r.points)
                } else {
                    "PRERUSENO".into()
                }
            });
            if let Some(panel) = scene.info_panels.get_mut(id) {
                panel.update(vec![("", label)]);
            }
            let opacity = row.map_or(0., |r| (1. - r.age / self.seconds).clamp(0., 1.));
            scene.set_opacity(*id, opacity, 8., 0., -1.);
            scene.set_emission(*id, 4. * opacity * opacity);
        }
    }
    pub fn state(&self) -> String {
        json!({"current":self.current.as_ref().map(|r|r.value(self.seconds,0)),"history":self.history.iter().enumerate().map(|(i,r)|r.value(self.seconds,self.awards[i])).collect::<Vec<_>>()}).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Scene, ComboDisplay) {
        let mut scene = Scene::default();
        let asset = scene
            .register_mesh_variants(
                &[0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.],
                &[0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
                &[1., 0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0.],
                false,
            )
            .unwrap();
        let timer = scene.add_info_panel("countdown", 1., 0.3).unwrap();
        let score = scene.add_info_panel("score", 1., 0.3).unwrap();
        let awards: Vec<_> = (0..3)
            .map(|_| scene.add_info_panel("combo-award", 1., 0.2).unwrap())
            .collect();
        let config = json!({"symbols":[{"kind":"A","asset":asset,"variant":0},{"kind":"B","asset":asset,"variant":0},{"kind":"W","asset":asset,"variant":0}],"frameAsset":asset,"timerPanel":timer,"scorePanel":score,"awardPanels":awards});
        let display = ComboDisplay::configure(&scene, &config.to_string()).unwrap();
        (scene, display)
    }
    #[test]
    fn combo_display_mismatch_wild_success_fade_and_reset() {
        let (mut scene, mut display) = fixture();
        let mut score = crate::xr_round::ComboScore::default();
        for (id, kind, wild) in [
            (1, "A", false),
            (2, "W", true),
            (3, "B", false),
            (4, "W", true),
            (5, "B", false),
        ] {
            assert!(score.register(id, kind, 10, wild));
            display.event(&mut scene, score.hit_event(id).unwrap());
            if id == 2 {
                let row = display.current.as_ref().unwrap();
                assert_eq!(
                    (&*row.kind, row.count, row.icons.len(), row.frames.len()),
                    ("A", 2, 2, 3)
                );
            }
            if id == 3 {
                assert_eq!(
                    (
                        display.history[0].kind.as_str(),
                        display.history[0].count,
                        display.history[0].success
                    ),
                    ("A", 2, false)
                );
            }
        }
        assert!(display.current.is_none());
        assert_eq!(score.score, 10);
        assert_eq!(
            (
                display.history[0].kind.as_str(),
                display.history[0].points,
                display.history[0].success
            ),
            ("B", 10, true)
        );
        assert!(score.hit_event(5).is_none());
        let ids = display
            .history
            .iter()
            .flat_map(|r| r.icons.iter().chain(&r.frames))
            .copied()
            .collect::<Vec<_>>();
        display.advance(&mut scene, 1.);
        assert_eq!(display.history[0].value(2., 0)["opacity"], 0.5);
        for row in &display.history {
            for (slot, id) in row.icons.iter().enumerate() {
                assert_eq!(
                    scene.objects()[id].opacity,
                    if slot >= row.count as usize {
                        0.11
                    } else {
                        0.5
                    }
                );
            }
            assert_eq!(row.icons.len(), row.count as usize);
            for (slot, id) in row.frames.iter().enumerate() {
                assert_eq!(
                    scene.objects()[id].opacity,
                    if slot >= row.count as usize {
                        0.11
                    } else {
                        0.5
                    }
                );
            }
        }
        display.advance(&mut scene, 1.);
        assert!(display.history.is_empty());
        for id in ids {
            assert!(!scene.object_exists(id));
        }
        display.event(
            &mut scene,
            ComboEvent {
                failed: None,
                kind: "A".into(),
                count: 1,
                success: false,
                points: 0,
            },
        );
        display.reset(&mut scene);
        assert!(display.current.is_none() && display.history.is_empty());
        assert_eq!(scene.objects().len(), 5);
    }
    #[test]
    fn combo_display_bounds_history_and_shared_instance_lifetime() {
        let (mut scene, mut display) = fixture();
        for _ in 0..8 {
            display.event(
                &mut scene,
                ComboEvent {
                    failed: None,
                    kind: "A".into(),
                    count: 3,
                    success: true,
                    points: 10,
                },
            );
        }
        assert_eq!(display.history.len(), 3);
        assert_eq!(scene.variant_instances.len(), 18);
        display.event(
            &mut scene,
            ComboEvent {
                failed: None,
                kind: "B".into(),
                count: 2,
                success: false,
                points: 0,
            },
        );
        assert_eq!(scene.variant_instances.len(), 23);
        assert_eq!(scene.mesh_variants.len(), 1);
        display.reset(&mut scene);
        assert!(scene.variant_instances.is_empty());
    }
}
