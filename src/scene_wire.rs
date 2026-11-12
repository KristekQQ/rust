//! Experimental ordered binary scene transport between separate WASM memories.
//! NWR1/v2 is little endian: sequence/revisions, immutable asset deltas, then full
//! live object/asset references, lights and font tags. Apply every frame in order;
//! start a fresh mirror/publisher together after restarting the transport. glTF
//! assets and poses remain static and must be loaded identically on both sides.
use super::*;
use crate::{
    cutting_mesh::{MeshData, MeshStats},
    gltf_loader::{ImportedMesh, ImportedVertex},
    info_panel::{InfoPanel, PanelKind},
};
use publication::{PreparedVariant, RenderPublication, RenderPublisher};
use std::{collections::BTreeSet, sync::Arc};

const MAGIC: &[u8; 4] = b"NWR1";
const VERSION: u32 = 4;
const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_ITEMS: usize = 200_000;
const MAX_ARRAY: usize = 4_000_000;

#[derive(Default)]
pub struct WirePublisher {
    publisher: RenderPublisher,
    sequence: u64,
    triangles: BTreeMap<u32, MeshData>,
    variants: BTreeMap<u32, Arc<PreparedVariant>>,
    panels: BTreeMap<u32, (u64, bool)>,
}
struct VariantAsset {
    asset: MeshVariants,
    prepared: Arc<PreparedVariant>,
}
struct PanelAsset {
    panel: InfoPanel,
    #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
    glow: Option<Arc<crate::render::neon_glow::PreparedGlowMesh>>,
}
#[derive(Default)]
pub(super) struct WireState {
    sequence: u64,
    triangles: BTreeMap<u32, MeshData>,
    variants: BTreeMap<u32, VariantAsset>,
    panels: BTreeMap<u32, PanelAsset>,
}
impl WirePublisher {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn capture(&mut self, scene: &mut SceneManager) -> Result<Vec<u8>, String> {
        if scene.render_publication.is_mirror {
            return Err("Cannot publish a render mirror".into());
        }
        let p = self.publisher.capture(scene);
        self.capture_publication(&p)
    }
    /// Encode a complete immutable snapshot; skipped snapshots need not be sent.
    pub fn capture_publication(&mut self, p: &RenderPublication) -> Result<Vec<u8>, String> {
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Wire sequence exhausted")?;
        let mut w = Writer::default();
        w.raw(MAGIC);
        w.u32(VERSION);
        w.u64(sequence);
        w.u64(p.render_revision);
        w.u64(p.static_revision);
        w.u64(p.shadows.static_casters);
        w.u64(p.shadows.dynamic_casters);
        w.u32(p.mirror.unwrap_or(INVALID_ID));
        w.u32(p.next_id);
        for id in p.cut_feedback_rings {
            w.u32(id.unwrap_or(INVALID_ID));
        }
        let changed_triangles: Vec<_> = p
            .triangles
            .iter()
            .filter(|(id, t)| {
                !p.instances.contains_key(*id)
                    && !self
                        .triangles
                        .get(*id)
                        .is_some_and(|old| old.same_source(&t.mesh))
            })
            .collect();
        w.count(changed_triangles.len(), MAX_ITEMS);
        for (&id, t) in changed_triangles {
            w.id(id);
            put_mesh_data(&mut w, &t.mesh, None)?;
        }
        let changed_variants: Vec<_> = p
            .prepared
            .iter()
            .filter(|(id, a)| {
                !self
                    .variants
                    .get(*id)
                    .is_some_and(|old| Arc::ptr_eq(old, a))
            })
            .collect();
        w.count(changed_variants.len(), MAX_ITEMS);
        for (&id, a) in changed_variants {
            w.id(id);
            let digit = p
                .numeric
                .assets
                .as_ref()
                .and_then(|v| v.iter().position(|&x| x == id))
                .map(|v| v as u8);
            put_mesh_data(&mut w, &a.source, digit)?;
            put_mesh(&mut w, &a.mesh);
            put_colors(&mut w, &a.colors);
            let asset = &p.variants[&id];
            put_colors(&mut w, &asset.colors);
            w.count(asset.varying_triangles.len(), MAX_ARRAY);
            for &v in asset.varying_triangles.iter() {
                w.u32(v as u32);
            }
            w.count(asset.variants.len(), MAX_ITEMS);
        }
        let changed_panels: Vec<_> = p
            .panels
            .iter()
            .filter(|(id, panel)| {
                self.panels.get(*id) != Some(&(panel.revision, panel.font3d_bound))
            })
            .collect();
        w.count(changed_panels.len(), MAX_ITEMS);
        for (&id, panel) in changed_panels {
            w.id(id);
            w.u8(kind_tag(panel.kind));
            w.f32(panel.width);
            w.f32(panel.height);
            w.u64(panel.revision);
            w.bool(panel.font3d_bound);
            put_mesh(&mut w, &panel.mesh);
            #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
            {
                let glow = panel
                    .kind
                    .neon()
                    .then(|| crate::render::neon_glow::PreparedGlowMesh::new(&panel.mesh))
                    .flatten();
                put_glow(&mut w, glow.as_ref());
            }
            #[cfg(not(any(target_arch = "wasm32", feature = "native-render")))]
            w.bool(false);
        }
        w.count(p.objects.len(), MAX_ITEMS);
        for (&id, o) in &p.objects {
            w.id(id);
            put_object(&mut w, o);
        }
        w.count(p.triangles.len(), MAX_ITEMS);
        for (&id, t) in &p.triangles {
            w.id(id);
            w.bool(t.cuttable);
            w.vec3(t.cap_color);
            if let Some(&(handle, index)) = p.instances.get(&id) {
                w.bool(true);
                w.id(handle);
                w.u32(index);
            } else {
                w.bool(false);
            }
        }
        w.count(p.variants.len(), MAX_ITEMS);
        for (&id, a) in &p.variants {
            w.id(id);
            w.bool(a.registered);
        }
        w.count(p.panels.len(), MAX_ITEMS);
        for (&id, panel) in &p.panels {
            w.id(id);
            w.u64(panel.revision);
        }
        w.count(p.lights.len(), MAX_LIGHTS);
        for (&id, l) in &p.lights {
            w.id(id);
            w.vec3(l.position);
            w.vec3(l.color);
        }
        w.bool(p.numeric.assets.is_some());
        if let Some(assets) = p.numeric.assets {
            for id in assets {
                w.id(id);
            }
        }
        put_set(&mut w, &p.numeric.children);
        put_set(&mut w, &p.font_assets);
        put_set(&mut w, &p.font_children);
        put_set(&mut w, &p.font_texts);
        put_set(&mut w, &p.font_owners);
        w.count(p.cut_lineage.len(), MAX_ITEMS);
        for (&child, &(parent, relative)) in &p.cut_lineage {
            w.id(child);
            w.id(parent);
            for value in relative.to_cols_array() {
                w.f32(value);
            }
        }
        if !w.valid || w.bytes.len() > MAX_BYTES {
            return Err("Wire publication exceeds bounded schema".into());
        }
        // Preserve cache nodes for unchanged assets; commit only after validation.
        self.triangles
            .retain(|id, _| p.triangles.contains_key(id) && !p.instances.contains_key(id));
        for (&id, triangle) in &p.triangles {
            if !p.instances.contains_key(&id)
                && self
                    .triangles
                    .get(&id)
                    .is_none_or(|old| !old.same_source(&triangle.mesh))
            {
                self.triangles.insert(id, triangle.mesh.clone());
            }
        }
        self.variants = p.prepared.clone();
        self.panels.retain(|id, _| p.panels.contains_key(id));
        for (&id, panel) in &p.panels {
            let value = (panel.revision, panel.font3d_bound);
            if self.panels.get(&id) != Some(&value) {
                self.panels.insert(id, value);
            }
        }
        self.sequence = sequence;
        Ok(w.bytes)
    }
}

struct Writer {
    bytes: Vec<u8>,
    valid: bool,
}
impl Default for Writer {
    fn default() -> Self {
        Self {
            bytes: Vec::new(),
            valid: true,
        }
    }
}
impl Writer {
    fn raw(&mut self, v: &[u8]) {
        self.bytes.extend_from_slice(v);
    }
    fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.u8(v as u8);
    }
    fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }
    fn id(&mut self, v: u32) {
        if v == INVALID_ID {
            self.valid = false;
        }
        self.u32(v);
    }
    fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        if !v.is_finite() {
            self.valid = false;
        }
        self.u32(v.to_bits());
    }
    fn f64(&mut self, v: f64) {
        if !v.is_finite() {
            self.valid = false;
        }
        self.u64(v.to_bits());
    }
    fn vec3(&mut self, v: Vec3) {
        for f in v.to_array() {
            self.f32(f);
        }
    }
    fn count(&mut self, n: usize, max: usize) {
        if n > max || n > u32::MAX as usize {
            self.valid = false;
        }
        self.u32(n as u32);
    }
}
fn put_set(w: &mut Writer, ids: &BTreeSet<u32>) {
    w.count(ids.len(), MAX_ITEMS);
    for &id in ids {
        w.id(id);
    }
}
fn put_colors(w: &mut Writer, c: &[[f32; 4]]) {
    w.count(c.len(), MAX_ARRAY);
    for color in c {
        for &v in color {
            w.f32(v);
        }
    }
}
fn put_mesh(w: &mut Writer, m: &ImportedMesh) {
    w.vec3(Vec3::from(m.bounds_min));
    w.vec3(Vec3::from(m.bounds_max));
    w.count(m.vertices.len(), MAX_ARRAY);
    for v in &m.vertices {
        for a in [v.position, v.normal, v.color] {
            for f in a {
                w.f32(f);
            }
        }
    }
    w.count(m.indices.len(), MAX_ARRAY);
    for &i in &m.indices {
        w.u32(i);
    }
}
fn put_mesh_data(w: &mut Writer, m: &MeshData, digit: Option<u8>) -> Result<(), String> {
    let stats = m.stats();
    w.vec3(stats.min);
    w.vec3(stats.max);
    w.count(stats.triangles, MAX_ARRAY);
    w.count(stats.vertices, MAX_ARRAY);
    w.f64(stats.volume);
    put_mesh(w, m.prepared_render().ok_or("Unprepared worker mesh")?);
    #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
    if let Some(digit) = digit {
        let glow = crate::render::neon_glow::PreparedGlowMesh::new(
            &crate::info_panel::numeric_glyph_glow(digit),
        );
        put_glow(w, glow.as_ref());
    } else {
        put_glow(w, m.prepared_glow().and_then(|v| v.as_ref()));
    }
    #[cfg(not(any(target_arch = "wasm32", feature = "native-render")))]
    {
        let _ = digit;
        w.bool(false);
    }
    Ok(())
}
#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
fn put_glow(w: &mut Writer, g: Option<&crate::render::neon_glow::PreparedGlowMesh>) {
    w.bool(g.is_some());
    if let Some(g) = g {
        w.count(g.vertices.len(), MAX_ARRAY);
        for v in &g.vertices {
            for a in [v.position, v.color, v.normal] {
                for f in a {
                    w.f32(f);
                }
            }
        }
        w.count(g.indices.len(), MAX_ARRAY);
        for &i in &g.indices {
            w.u32(i);
        }
    }
}
fn put_object(w: &mut Writer, o: &SceneObject) {
    let (tag, id) = match o.mesh {
        MeshKind::Cube => (0, 0),
        MeshKind::Plane => (1, 0),
        MeshKind::Sphere => (2, 0),
        MeshKind::Imported(id) => (3, id),
    };
    w.u8(tag);
    w.u32(id);
    w.vec3(o.transform.position);
    w.vec3(o.transform.rotation);
    w.vec3(o.transform.scale);
    let r = o.render_options;
    w.bool(r.is_static);
    w.bool(r.casts_shadow);
    w.bool(r.receives_shadow);
    w.f32(r.reflectivity);
    w.bool(r.opaque_reorderable);
    for f in [
        o.emission,
        o.color_cutout,
        o.visual_fade,
        o.opacity,
        o.opaque_above,
        o.opacity_variation,
        o.opacity_coverage,
        o.cut_assist,
    ] {
        w.f32(f);
    }
    w.bool(o.visual_dissolve);
    w.bool(o.blade_contact_response);
    w.bool(o.local_bounds.is_some());
    if let Some((a, b)) = o.local_bounds {
        w.vec3(a);
        w.vec3(b);
    }
    let i = &o.render_data.instance;
    for a in i.model.iter().chain(i.normal_matrix.iter()).chain([
        &i.material,
        &i.emission,
        &i.surface_params,
    ]) {
        for &f in a {
            w.f32(f);
        }
    }
    w.vec3(o.render_data.center);
    w.vec3(o.render_data.half_extent);
}
fn kind_tag(k: PanelKind) -> u8 {
    match k {
        PanelKind::Scene => 0,
        PanelKind::Round => 1,
        PanelKind::Restart => 2,
        PanelKind::Diagnostics => 3,
        PanelKind::Text => 4,
        PanelKind::NumericText => 5,
        PanelKind::DebugText => 6,
        PanelKind::Countdown => 7,
        PanelKind::Score => 8,
        PanelKind::ComboAward => 9,
    }
}
fn read_kind(k: u8) -> Result<PanelKind, String> {
    Ok(match k {
        0 => PanelKind::Scene,
        1 => PanelKind::Round,
        2 => PanelKind::Restart,
        3 => PanelKind::Diagnostics,
        4 => PanelKind::Text,
        5 => PanelKind::NumericText,
        6 => PanelKind::DebugText,
        7 => PanelKind::Countdown,
        8 => PanelKind::Score,
        9 => PanelKind::ComboAward,
        _ => return Err("Invalid wire panel kind".into()),
    })
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.offset.checked_add(n).ok_or("Wire size overflow")?;
        let b = self
            .bytes
            .get(self.offset..end)
            .ok_or("Truncated wire publication")?;
        self.offset = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, String> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("Invalid wire boolean".into()),
        }
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn id(&mut self) -> Result<u32, String> {
        let id = self.u32()?;
        if id == INVALID_ID {
            Err("Invalid wire ID".into())
        } else {
            Ok(id)
        }
    }
    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32, String> {
        let v = f32::from_bits(self.u32()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err("Nonfinite wire float".into())
        }
    }
    fn f64(&mut self) -> Result<f64, String> {
        let v = f64::from_bits(self.u64()?);
        if v.is_finite() {
            Ok(v)
        } else {
            Err("Nonfinite wire float".into())
        }
    }
    fn vec3(&mut self) -> Result<Vec3, String> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    fn array4(&mut self) -> Result<[f32; 4], String> {
        Ok([self.f32()?, self.f32()?, self.f32()?, self.f32()?])
    }
    fn count(&mut self, max: usize, min_bytes: usize) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > max
            || n.checked_mul(min_bytes)
                .is_none_or(|b| b > self.bytes.len() - self.offset)
        {
            Err("Wire count exceeds bounds".into())
        } else {
            Ok(n)
        }
    }
}
fn insert<T>(map: &mut BTreeMap<u32, T>, id: u32, v: T) -> Result<(), String> {
    if map.insert(id, v).is_some() {
        Err("Duplicate wire ID".into())
    } else {
        Ok(())
    }
}
fn read_set(r: &mut Reader<'_>) -> Result<BTreeSet<u32>, String> {
    let n = r.count(MAX_ITEMS, 4)?;
    let mut set = BTreeSet::new();
    for _ in 0..n {
        if !set.insert(r.id()?) {
            return Err("Duplicate wire tag".into());
        }
    }
    Ok(set)
}
fn read_colors(r: &mut Reader<'_>) -> Result<Vec<[f32; 4]>, String> {
    let n = r.count(MAX_ARRAY, 16)?;
    (0..n).map(|_| r.array4()).collect()
}
fn read_indices(r: &mut Reader<'_>, vertices: usize) -> Result<Vec<u32>, String> {
    let n = r.count(MAX_ARRAY, 4)?;
    if n % 3 != 0 {
        return Err("Wire triangle index count".into());
    }
    let mut indices = Vec::with_capacity(n);
    for _ in 0..n {
        let i = r.u32()?;
        if i as usize >= vertices {
            return Err("Wire vertex index out of range".into());
        }
        indices.push(i);
    }
    Ok(indices)
}
fn read_mesh(r: &mut Reader<'_>) -> Result<ImportedMesh, String> {
    let min = r.vec3()?;
    let max = r.vec3()?;
    if min.cmpgt(max).any() {
        return Err("Invalid wire mesh bounds".into());
    }
    let n = r.count(MAX_ARRAY, 36)?;
    let mut vertices = Vec::with_capacity(n);
    for _ in 0..n {
        vertices.push(ImportedVertex {
            position: r.vec3()?.to_array(),
            normal: r.vec3()?.to_array(),
            color: r.vec3()?.to_array(),
        });
    }
    let indices = read_indices(r, n)?;
    Ok(ImportedMesh {
        vertices,
        indices,
        bounds_min: min.to_array(),
        bounds_max: max.to_array(),
        warnings: Vec::new(),
    })
}
// Headless decoders validate this payload; only render-enabled builds consume it.
#[cfg_attr(
    not(any(target_arch = "wasm32", feature = "native-render")),
    allow(dead_code)
)]
struct GlowPayload {
    vertices: Vec<[[f32; 3]; 3]>,
    indices: Vec<u32>,
}
fn read_glow(r: &mut Reader<'_>) -> Result<Option<GlowPayload>, String> {
    if !r.bool()? {
        return Ok(None);
    }
    let n = r.count(MAX_ARRAY, 36)?;
    let mut vertices = Vec::with_capacity(n);
    for _ in 0..n {
        vertices.push([
            r.vec3()?.to_array(),
            r.vec3()?.to_array(),
            r.vec3()?.to_array(),
        ]);
    }
    Ok(Some(GlowPayload {
        vertices,
        indices: read_indices(r, n)?,
    }))
}
#[cfg(any(target_arch = "wasm32", feature = "native-render"))]
fn prepare_decoded_glow(g: GlowPayload) -> crate::render::neon_glow::PreparedGlowMesh {
    crate::render::neon_glow::PreparedGlowMesh {
        vertices: g
            .vertices
            .into_iter()
            .map(|[position, color, normal]| crate::render::data::Vertex {
                position,
                color,
                normal,
            })
            .collect(),
        indices: g.indices,
    }
}
fn read_mesh_data(r: &mut Reader<'_>) -> Result<MeshData, String> {
    let min = r.vec3()?;
    let max = r.vec3()?;
    if min.cmpgt(max).any() {
        return Err("Invalid wire statistics bounds".into());
    }
    let triangles = r.count(MAX_ARRAY, 0)?;
    let vertices = r.count(MAX_ARRAY, 0)?;
    let volume = r.f64()?;
    if volume < 0. {
        return Err("Invalid wire mesh volume".into());
    }
    let render = read_mesh(r)?;
    let glow = read_glow(r)?;
    let mesh = MeshData::from_render_only(
        MeshStats {
            min,
            max,
            triangles,
            vertices,
            volume,
        },
        render,
    );
    #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
    mesh.set_prepared_glow(glow.map(prepare_decoded_glow));
    #[cfg(not(any(target_arch = "wasm32", feature = "native-render")))]
    let _ = glow;
    Ok(mesh)
}
fn read_object(r: &mut Reader<'_>) -> Result<SceneObject, String> {
    let tag = r.u8()?;
    let mesh_id = r.u32()?;
    let mesh = match tag {
        0 => MeshKind::Cube,
        1 => MeshKind::Plane,
        2 => MeshKind::Sphere,
        3 if mesh_id != INVALID_ID => MeshKind::Imported(mesh_id),
        _ => return Err("Invalid wire mesh kind".into()),
    };
    let transform = Transform::new(r.vec3()?, r.vec3()?, r.vec3()?);
    if !transform.is_valid() {
        return Err("Invalid wire transform".into());
    }
    let render_options = RenderOptions {
        is_static: r.bool()?,
        casts_shadow: r.bool()?,
        receives_shadow: r.bool()?,
        reflectivity: r.f32()?,
        opaque_reorderable: r.bool()?,
    };
    let emission = r.f32()?;
    let color_cutout = r.f32()?;
    let visual_fade = r.f32()?;
    let opacity = r.f32()?;
    let opaque_above = r.f32()?;
    let opacity_variation = r.f32()?;
    let opacity_coverage = r.f32()?;
    let cut_assist = r.f32()?;
    let visual_dissolve = r.bool()?;
    let blade_contact_response = r.bool()?;
    let local_bounds = if r.bool()? {
        let a = r.vec3()?;
        let b = r.vec3()?;
        if a.cmpgt(b).any() {
            return Err("Invalid wire object bounds".into());
        }
        Some((a, b))
    } else {
        None
    };
    let mut model = [[0.; 4]; 4];
    for a in &mut model {
        *a = r.array4()?;
    }
    let mut normal_matrix = [[0.; 4]; 4];
    for a in &mut normal_matrix {
        *a = r.array4()?;
    }
    let instance = crate::visibility::InstanceData {
        model,
        normal_matrix,
        material: r.array4()?,
        emission: r.array4()?,
        surface_params: r.array4()?,
    };
    let center = r.vec3()?;
    let half_extent = r.vec3()?;
    if half_extent.cmplt(Vec3::ZERO).any() {
        return Err("Invalid wire render extent".into());
    }
    Ok(SceneObject {
        transform,
        mesh,
        render_options,
        emission,
        color_cutout,
        visual_fade,
        visual_dissolve,
        opacity,
        opaque_above,
        opacity_variation,
        opacity_coverage,
        cut_assist,
        blade_contact_response,
        local_bounds,
        render_data: crate::visibility::ObjectRenderData {
            instance,
            center,
            half_extent,
        },
    })
}

struct TriangleRef {
    cuttable: bool,
    cap_color: Vec3,
    instance: Option<(u32, u32)>,
}
struct Frame {
    sequence: u64,
    render: u64,
    static_revision: u64,
    shadows: ShadowRevisions,
    mirror: Option<u32>,
    next_id: u32,
    cut_feedback_rings: [Option<u32>; 2],
    new_triangles: BTreeMap<u32, MeshData>,
    new_variants: BTreeMap<u32, VariantAsset>,
    new_panels: BTreeMap<u32, PanelAsset>,
    objects: Vec<(u32, SceneObject)>,
    triangles: Vec<(u32, TriangleRef)>,
    variants: Vec<(u32, bool)>,
    panels: Vec<(u32, u64)>,
    lights: Vec<(u32, Light)>,
    numeric_assets: Option<[u32; 10]>,
    numeric_children: BTreeSet<u32>,
    font_assets: BTreeSet<u32>,
    font_children: BTreeSet<u32>,
    font_texts: BTreeSet<u32>,
    font_owners: BTreeSet<u32>,
    cut_lineage: Vec<(u32, (u32, Mat4))>,
}
fn read_ordered<T>(
    r: &mut Reader<'_>,
    max: usize,
    read: impl Fn(&mut Reader<'_>) -> Result<T, String>,
) -> Result<Vec<(u32, T)>, String> {
    let n = r.count(max, 4)?;
    let mut values = Vec::with_capacity(n);
    let mut previous = None;
    for _ in 0..n {
        let id = r.id()?;
        if previous.is_some_and(|old| old >= id) {
            return Err("Wire IDs must be ordered and unique".into());
        }
        previous = Some(id);
        values.push((id, read(r)?));
    }
    Ok(values)
}
fn has<T>(values: &[(u32, T)], id: u32) -> bool {
    values.binary_search_by_key(&id, |(id, _)| *id).is_ok()
}
fn get<T>(values: &[(u32, T)], id: u32) -> Option<&T> {
    values
        .binary_search_by_key(&id, |(id, _)| *id)
        .ok()
        .map(|i| &values[i].1)
}
fn decode(bytes: &[u8]) -> Result<Frame, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Wire publication too large".into());
    }
    let mut r = Reader { bytes, offset: 0 };
    if r.take(4)? != MAGIC {
        return Err("Unsupported wire publication header".into());
    }
    let version = r.u32()?;
    if !(1..=VERSION).contains(&version) {
        return Err("Unsupported wire publication header".into());
    }
    let sequence = r.u64()?;
    let render = r.u64()?;
    let static_revision = r.u64()?;
    let shadows = ShadowRevisions {
        static_casters: r.u64()?,
        dynamic_casters: r.u64()?,
    };
    let mirror = match r.u32()? {
        INVALID_ID => None,
        id => Some(id),
    };
    let next_id = r.u32()?;
    let mut cut_feedback_rings = [None; 2];
    if version >= 2 {
        for id in &mut cut_feedback_rings {
            let value = r.u32()?;
            *id = (value != INVALID_ID).then_some(value);
        }
    }
    let mut new_triangles = BTreeMap::new();
    let n = r.count(MAX_ITEMS, 4)?;
    for _ in 0..n {
        let id = r.id()?;
        let mesh = read_mesh_data(&mut r)?;
        insert(&mut new_triangles, id, mesh)?;
    }
    let mut new_variants = BTreeMap::new();
    let n = r.count(MAX_ITEMS, 4)?;
    for _ in 0..n {
        let id = r.id()?;
        let source = read_mesh_data(&mut r)?;
        let mesh = read_mesh(&mut r)?;
        let colors = read_colors(&mut r)?;
        let original_colors = read_colors(&mut r)?;
        let n = r.count(MAX_ARRAY, 4)?;
        let mut varying = Vec::with_capacity(n);
        for _ in 0..n {
            let i = r.u32()? as usize;
            if i >= source.triangle_count() {
                return Err("Wire variant triangle outside base".into());
            }
            varying.push(i);
        }
        let variants = r.count(MAX_ITEMS, 0)?;
        if variants == 0
            || source.triangle_count() == 0
            || original_colors.len()
                != variants
                    .checked_mul(source.triangle_count())
                    .ok_or("Wire color size overflow")?
            || colors.len()
                != variants
                    .checked_mul(varying.len())
                    .ok_or("Wire varying color size overflow")?
        {
            return Err("Wire variant color shape mismatch".into());
        }
        if mesh.indices.len() != source.prepared_render().unwrap().indices.len() {
            return Err("Wire variant geometry mismatch".into());
        }
        let asset = MeshVariants {
            variants: vec![source.clone(); variants].into(),
            colors: original_colors.into(),
            registered: true,
            varying_triangles: varying.into(),
        };
        // Match native cross-thread publication ownership; WASM MeshData is local Rc.
        #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
        let prepared = Arc::new(PreparedVariant {
            source,
            mesh,
            colors,
        });
        insert(&mut new_variants, id, VariantAsset { asset, prepared })?;
    }
    let mut new_panels = BTreeMap::new();
    let n = r.count(MAX_ITEMS, 4)?;
    for _ in 0..n {
        let id = r.id()?;
        let kind = read_kind(r.u8()?)?;
        let width = r.f32()?;
        let height = r.f32()?;
        if !(0.01..=100.).contains(&width) || !(0.01..=100.).contains(&height) {
            return Err("Wire panel dimensions invalid".into());
        }
        let revision = r.u64()?;
        let bound = r.bool()?;
        let mesh = read_mesh(&mut r)?;
        let glow = read_glow(&mut r)?;
        let panel = InfoPanel::from_render(kind, width, height, mesh, revision, bound);
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        let glow = glow.map(prepare_decoded_glow).map(Arc::new);
        #[cfg(not(any(target_arch = "wasm32", feature = "native-render")))]
        let _ = glow;
        insert(
            &mut new_panels,
            id,
            PanelAsset {
                panel,
                #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
                glow,
            },
        )?;
    }
    let objects = read_ordered(&mut r, MAX_ITEMS, read_object)?;
    let triangles = read_ordered(&mut r, MAX_ITEMS, |r| {
        let cuttable = r.bool()?;
        let cap_color = r.vec3()?;
        let instance = if r.bool()? {
            Some((r.id()?, r.u32()?))
        } else {
            None
        };
        Ok(TriangleRef {
            cuttable,
            cap_color,
            instance,
        })
    })?;
    let variants = read_ordered(&mut r, MAX_ITEMS, |r| r.bool())?;
    let panels = read_ordered(&mut r, MAX_ITEMS, |r| r.u64())?;
    let lights = read_ordered(&mut r, MAX_LIGHTS, |r| {
        Ok(Light {
            position: r.vec3()?,
            color: r.vec3()?,
        })
    })?;
    let numeric_assets = if r.bool()? {
        let mut a = [0; 10];
        for id in &mut a {
            *id = r.id()?;
        }
        Some(a)
    } else {
        None
    };
    let numeric_children = read_set(&mut r)?;
    let font_assets = read_set(&mut r)?;
    let font_children = read_set(&mut r)?;
    let font_texts = read_set(&mut r)?;
    // v2 had no font-owner lifetime metadata. Empty owners conservatively
    // disable warmed halo retention without changing scene rendering.
    let font_owners = if version >= 3 {
        read_set(&mut r)?
    } else {
        BTreeSet::new()
    };
    let cut_lineage = if version >= 4 {
        read_ordered(&mut r, MAX_ITEMS, |r| {
            let parent = r.id()?;
            let mut values = [0.; 16];
            for value in &mut values {
                *value = r.f32()?;
            }
            let relative = Mat4::from_cols_array(&values);
            if relative.row(3) != glam::Vec4::W
                || !relative.determinant().is_finite()
                || relative.determinant().abs() < 1e-10
            {
                return Err("Invalid wire birth transform".into());
            }
            Ok((parent, relative))
        })?
    } else {
        Vec::new()
    };
    if r.offset != bytes.len() {
        return Err("Trailing wire publication bytes".into());
    }
    Ok(Frame {
        sequence,
        render,
        static_revision,
        shadows,
        mirror,
        next_id,
        cut_feedback_rings,
        new_triangles,
        new_variants,
        new_panels,
        objects,
        triangles,
        variants,
        panels,
        lights,
        numeric_assets,
        numeric_children,
        font_assets,
        font_children,
        font_texts,
        font_owners,
        cut_lineage,
    })
}
impl SceneManager {
    pub fn wire_publication_sequence(&self) -> u64 {
        self.wire_state.sequence
    }
    /// Transactional validation precedes all scene mutation. Sequence gaps are
    /// rejected because immutable asset buffers are sent incrementally.
    pub fn apply_wire_publication(
        &mut self,
        bytes: &[u8],
        local_ids: &[u32],
    ) -> Result<(), String> {
        self.apply_wire_publication_with_spawns(bytes, local_ids, &[])
    }
    /// Explicit [child, removed parent, ...] provenance for new topology IDs.
    /// Invalid mappings reject the entire publication; unknown old parents are harmless.
    pub fn apply_wire_publication_with_spawns(
        &mut self,
        bytes: &[u8],
        local_ids: &[u32],
        spawn_pairs: &[u32],
    ) -> Result<(), String> {
        let f = decode(bytes)?;
        if self.wire_state.sequence.checked_add(1) != Some(f.sequence) {
            return Err("Out-of-order wire publication".into());
        }
        let variant = |id: &u32| {
            f.new_variants
                .get(id)
                .or_else(|| self.wire_state.variants.get(id))
        };
        let triangle = |id: &u32| {
            f.new_triangles
                .get(id)
                .or_else(|| self.wire_state.triangles.get(id))
        };
        let panel = |id: &u32| {
            f.new_panels
                .get(id)
                .or_else(|| self.wire_state.panels.get(id))
        };
        for (id, _) in &f.variants {
            if variant(id).is_none() {
                return Err("Missing wire variant asset".into());
            }
        }
        for &id in f.new_variants.keys() {
            if !has(&f.variants, id) {
                return Err("Unreferenced wire variant delta".into());
            }
        }
        for &id in f.new_triangles.keys() {
            if get(&f.triangles, id).is_none_or(|t| t.instance.is_some()) {
                return Err("Unreferenced wire mesh delta".into());
            }
        }
        for &id in f.new_panels.keys() {
            if !has(&f.panels, id) {
                return Err("Unreferenced wire panel delta".into());
            }
        }
        for (id, t) in &f.triangles {
            let object = get(&f.objects, *id).ok_or("Wire triangle has no object")?;
            if let Some((handle, index)) = t.instance {
                if !has(&f.variants, handle)
                    || variant(&handle).is_none_or(|v| index as usize >= v.asset.variants.len())
                    || object.mesh != MeshKind::Imported(handle)
                {
                    return Err("Invalid wire variant instance".into());
                }
            } else if triangle(id).is_none() || object.mesh != MeshKind::Imported(*id) {
                return Err("Missing wire triangle asset".into());
            }
        }
        for (id, revision) in &f.panels {
            if !has(&f.objects, *id) || panel(id).is_none_or(|p| p.panel.revision != *revision) {
                return Err("Missing wire panel revision".into());
            }
        }
        if f.numeric_assets
            .as_ref()
            .is_some_and(|a| a.iter().any(|id| !has(&f.variants, *id)))
            || f.font_assets.iter().any(|id| !has(&f.variants, *id))
        {
            return Err("Missing wire font asset".into());
        }
        if f.numeric_children
            .iter()
            .chain(f.font_children.iter())
            .chain(f.font_texts.iter())
            .any(|id| !has(&f.objects, *id))
            || f.mirror.is_some_and(|id| !has(&f.objects, id))
        {
            return Err("Missing wire tagged object".into());
        }
        if f.cut_feedback_rings
            .iter()
            .flatten()
            .any(|id| !has(&f.triangles, *id))
        {
            return Err("Missing wire cut material mesh".into());
        }
        if !spawn_pairs.len().is_multiple_of(2) || spawn_pairs.len() / 2 > MAX_ITEMS {
            return Err("Invalid wire spawn pairs".into());
        }
        let mut spawns = BTreeMap::new();
        for &(child, (parent, relative)) in &f.cut_lineage {
            if child == parent || !has(&f.objects, child) || has(&f.objects, parent) {
                return Err("Invalid embedded wire spawn pair".into());
            }
            // Protected local objects never inherit remote presentation provenance.
            if !local_ids.contains(&child) && !local_ids.contains(&parent) {
                spawns.insert(child, (parent, relative));
            }
        }
        let mut explicit_children = BTreeSet::new();
        for pair in spawn_pairs.as_chunks::<2>().0 {
            let (child, parent) = (pair[0], pair[1]);
            if child == u32::MAX
                || parent == u32::MAX
                || child == parent
                || !has(&f.objects, child)
                || has(&f.objects, parent)
                || local_ids.contains(&child)
                || local_ids.contains(&parent)
                || !explicit_children.insert(child)
                || spawns.get(&child).is_some_and(|old| old.0 != parent)
            {
                return Err("Invalid or contradictory wire spawn pair".into());
            }
            spawns.entry(child).or_insert((parent, Mat4::IDENTITY));
        }
        // Capture before removed parents are pruned; never seed an existing ID.
        let spawn_seeds: BTreeMap<_, _> = spawns
            .into_iter()
            .filter(|(child, _)| {
                self.mirror_interpolation.enabled() && !self.objects.contains_key(child)
            })
            .filter_map(|(child, (parent, relative))| {
                self.objects
                    .get(&parent)
                    .cloned()
                    .map(|p| (child, (p, relative)))
            })
            .collect();
        // All references validated. Keep existing BTreeMap nodes for stable IDs.
        let changed_panels: BTreeSet<_> = f.new_panels.keys().copied().collect();
        let changed_triangles: BTreeSet<_> = f.new_triangles.keys().copied().collect();
        let changed_variants: BTreeSet<_> = f.new_variants.keys().copied().collect();
        self.mirror_interpolation
            .retain(|id| !local_ids.contains(&id) && has(&f.objects, id));
        self.wire_state.triangles.extend(f.new_triangles);
        self.wire_state.variants.extend(f.new_variants);
        self.wire_state.panels.extend(f.new_panels);
        self.objects
            .retain(|id, _| local_ids.contains(id) || has(&f.objects, *id));
        for (id, mut o) in f.objects {
            if local_ids.contains(&id) {
                continue;
            }
            let geometry_changed = changed_triangles.contains(&id)
                || changed_panels.contains(&id)
                || get(&f.triangles, id).is_some_and(|t| {
                    t.instance != self.variant_instances.get(&id).copied()
                        || t.instance
                            .is_some_and(|(handle, _)| changed_variants.contains(&handle))
                });
            if let Some((parent, relative)) = spawn_seeds.get(&id) {
                self.mirror_interpolation
                    .receive_spawn(id, parent, &mut o, *relative);
            } else {
                self.mirror_interpolation.receive(
                    id,
                    self.objects.get(&id),
                    &mut o,
                    geometry_changed,
                );
            }
            match self.objects.get_mut(&id) {
                Some(old) => *old = o,
                None => {
                    self.objects.insert(id, o);
                }
            }
        }
        self.triangle_meshes
            .retain(|id, _| local_ids.contains(id) || has(&f.triangles, *id));
        self.variant_instances.retain(|id, _| {
            local_ids.contains(id) || get(&f.triangles, *id).is_some_and(|t| t.instance.is_some())
        });
        for (id, t) in &f.triangles {
            if local_ids.contains(id) {
                continue;
            }
            let mesh = if let Some((handle, index)) = t.instance {
                self.variant_instances.insert(*id, (handle, index));
                self.wire_state.variants[&handle].asset.variants[0].clone()
            } else {
                self.wire_state.triangles[id].clone()
            };
            if let Some(old) = self.triangle_meshes.get_mut(id) {
                old.mesh = mesh;
                old.cuttable = t.cuttable;
                old.cap_color = t.cap_color;
            } else {
                self.triangle_meshes.insert(
                    *id,
                    TriangleObject {
                        mesh,
                        cuttable: t.cuttable,
                        cap_color: t.cap_color,
                    },
                );
            }
        }
        self.mesh_variants
            .retain(|id, _| local_ids.contains(id) || has(&f.variants, *id));
        self.render_publication
            .variants
            .retain(|id, _| local_ids.contains(id) || has(&f.variants, *id));
        for (id, registered) in &f.variants {
            if local_ids.contains(id) {
                continue;
            }
            let a = &self.wire_state.variants[id];
            let changed = self
                .mesh_variants
                .get(id)
                .is_none_or(|old| !old.variants[0].same_source(&a.asset.variants[0]));
            if changed {
                self.mesh_variants.insert(*id, a.asset.clone());
                self.render_publication
                    .variants
                    .insert(*id, a.prepared.clone());
            }
            self.mesh_variants.get_mut(id).unwrap().registered = *registered;
        }
        self.info_panels
            .retain(|id, _| local_ids.contains(id) || has(&f.panels, *id));
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        self.render_publication
            .panel_glow
            .retain(|id, _| local_ids.contains(id) || has(&f.panels, *id));
        for (id, revision) in &f.panels {
            if local_ids.contains(id) {
                continue;
            }
            if changed_panels.contains(id)
                || self
                    .info_panels
                    .get(id)
                    .is_none_or(|old| old.revision != *revision)
            {
                let a = &self.wire_state.panels[id];
                self.info_panels.insert(*id, a.panel.clone());
                #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
                self.render_publication
                    .panel_glow
                    .insert(*id, a.glow.clone());
            }
        }
        self.lights.retain(|id, _| has(&f.lights, *id));
        for (id, l) in f.lights {
            if let Some(old) = self.lights.get_mut(&id) {
                *old = l;
            } else {
                self.lights.insert(id, l);
            }
        }
        self.set_numeric_render_tags(f.numeric_assets, f.numeric_children);
        self.render_publication.font_assets = f.font_assets;
        self.render_publication.font_children = f.font_children;
        self.render_publication.font_texts = f.font_texts;
        self.render_publication.font_owners = f.font_owners;
        self.render_revision = self
            .render_revision
            .wrapping_add(f.render.wrapping_sub(self.render_publication.source_render));
        self.static_revision = self.static_revision.wrapping_add(
            f.static_revision
                .wrapping_sub(self.render_publication.source_static),
        );
        self.shadow_revisions.static_casters = self.shadow_revisions.static_casters.wrapping_add(
            f.shadows
                .static_casters
                .wrapping_sub(self.render_publication.source_shadows.static_casters),
        );
        self.shadow_revisions.dynamic_casters = self.shadow_revisions.dynamic_casters.wrapping_add(
            f.shadows
                .dynamic_casters
                .wrapping_sub(self.render_publication.source_shadows.dynamic_casters),
        );
        self.render_publication.source_render = f.render;
        self.render_publication.source_static = f.static_revision;
        self.render_publication.source_shadows = f.shadows;
        self.render_publication.cut_feedback_rings = f
            .cut_feedback_rings
            .map(|id| id.filter(|id| !local_ids.contains(id)));
        self.render_publication.is_mirror = true;
        self.mirror_object = f.mirror;
        self.next_id = self.next_id.max(f.next_id);
        self.wire_state.sequence = f.sequence;
        self.wire_state
            .triangles
            .retain(|id, _| get(&f.triangles, *id).is_some_and(|t| t.instance.is_none()));
        self.wire_state
            .variants
            .retain(|id, _| has(&f.variants, *id));
        self.wire_state.panels.retain(|id, _| has(&f.panels, *id));
        Ok(())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    #[test]
    fn binary_scaled_rotated_tracked_cut_canonicalizes_birth_affine_row() {
        let mut source = SceneManager::default();
        let (_, parent) = tetra(&mut source);
        let root = Vec3::new(0.3, 1., -0.8);
        source.set_object_transform(
            parent,
            root,
            Vec3::new(8.18, 5.726, 1.636),
            Vec3::splat(0.105),
        );
        source.set_cuttable(parent, true, Vec3::ONE).unwrap();
        let model = source.objects[&parent].transform.model();
        let cutter = source.add_cube(Vec3::ZERO);
        source
            .set_cutter(cutter, Some([Vec3::ZERO, Vec3::Z]))
            .unwrap();
        let offset = |y, z| model.transform_point3(Vec3::new(0.2, y, z)) - root;
        let history = vec![
            [offset(-1., -1.), offset(-1., 2.)],
            [offset(2., -1.), offset(2., 2.)],
        ];
        let mut publisher = WirePublisher::default();
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(200., 4.).unwrap();
        mirror
            .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
            .unwrap();
        let cut = source
            .cut_tracked_with(cutter, &BTreeMap::from([(parent, history)]))
            .unwrap();
        assert_eq!(cut.pieces.len(), 2);
        for &id in &cut.pieces {
            let relative = source.cut_lineage[&id].1;
            assert_eq!(relative.row(3), glam::Vec4::W);
            let birth = model * relative;
            let child = source.objects[&id].transform.model();
            for vertex in &source.triangle_meshes[&id].mesh.cpu().unwrap().positions {
                assert!(
                    birth
                        .transform_point3(*vertex)
                        .distance(child.transform_point3(*vertex))
                        < 1e-5
                );
            }
        }
        let bytes = publisher.capture(&mut source).unwrap();
        assert_eq!(decode(&bytes).unwrap().cut_lineage.len(), 2);
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        assert_eq!(mirror.wire_publication_sequence(), 2);
    }
    #[test]
    fn binary_tracked_recentered_birth_pose_survives_delayed_samples_and_recuts() {
        fn ribbon(scene: &SceneManager, id: u32, axis: usize) -> Vec<[Vec3; 2]> {
            let object = &scene.objects[&id];
            let (min, max) = scene.triangle_meshes[&id].mesh.bounds();
            let center = (min + max) * 0.5;
            let model = object.transform.model();
            let point = |a: f32, b: f32| {
                let p = if axis == 0 {
                    Vec3::new(center.x, a, b)
                } else {
                    Vec3::new(a, center.y, b)
                };
                model.transform_point3(p) - object.transform.position
            };
            vec![
                [point(-2., -2.), point(-2., 2.)],
                [point(2., -2.), point(2., 2.)],
            ]
        }
        for moving in [false, true] {
            let mut source = SceneManager::default();
            let (_, parent) = tetra(&mut source);
            source.set_cuttable(parent, true, Vec3::ONE).unwrap();
            let cutter = source.add_cube(Vec3::ZERO);
            source
                .set_cutter(cutter, Some([Vec3::ZERO, Vec3::Z]))
                .unwrap();
            let mut mirror = SceneManager::default();
            mirror.set_mirror_interpolation(200., 4.).unwrap();
            let mut publisher = WirePublisher::default();
            mirror
                .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
                .unwrap();
            if moving {
                source.set_object_position(parent, Vec3::X * 0.4);
                mirror
                    .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
                    .unwrap();
                mirror.update(0.1);
            }
            let displayed_parent =
                Mat4::from_cols_array_2d(&mirror.objects[&parent].render_data.instance.model);
            let history = ribbon(&source, parent, 0);
            let cut = source
                .cut_tracked_with(cutter, &BTreeMap::from([(parent, history)]))
                .unwrap();
            assert_eq!(cut.pieces.len(), 2);
            let births: BTreeMap<_, _> = cut
                .pieces
                .iter()
                .map(|&id| (id, source.objects[&id].transform.model()))
                .collect();
            assert!(cut.pieces.iter().all(|id| source.triangle_meshes[id]
                .mesh
                .cpu()
                .unwrap()
                .centroid()
                .length()
                < 1e-5));
            source.update(0.03); // Sample after motion: provenance must retain actual birth pivots.
            mirror
                .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
                .unwrap();
            for &id in &cut.pieces {
                let relative = source.cut_lineage[&id].1;
                let actual =
                    Mat4::from_cols_array_2d(&mirror.objects[&id].render_data.instance.model);
                let expected = displayed_parent * relative;
                assert!((actual.w_axis - expected.w_axis).length() < 1e-5);
                if !moving {
                    // Recentered child vertices reassemble at their authored birth world positions.
                    for &vertex in &source.triangle_meshes[&id].mesh.cpu().unwrap().positions {
                        assert!(
                            actual
                                .transform_point3(vertex)
                                .distance(births[&id].transform_point3(vertex))
                                < 1e-5
                        );
                    }
                }
            }
            let child = cut.pieces[0];
            let before = mirror.objects[&child].render_data.instance.model;
            mirror.update(0.05);
            assert_ne!(mirror.objects[&child].render_data.instance.model, before);
            // A different blade recuts a recentered child; provenance stays relative to that child.
            let cutter2 = source.add_cube(Vec3::ZERO);
            source
                .set_cutter(cutter2, Some([Vec3::ZERO, Vec3::Z]))
                .unwrap();
            let history = ribbon(&source, child, 1);
            let child_birth_model = source.objects[&child].transform.model();
            let recut = source
                .cut_tracked_with(cutter2, &BTreeMap::from([(child, history)]))
                .unwrap();
            assert_eq!(recut.pieces.len(), 2);
            let current_display =
                Mat4::from_cols_array_2d(&mirror.objects[&child].render_data.instance.model);
            for &id in &recut.pieces {
                let (parent_id, relative) = source.cut_lineage[&id];
                assert_eq!(parent_id, child);
                let expected_birth = child_birth_model * relative;
                assert!(
                    (expected_birth.w_axis - source.objects[&id].transform.model().w_axis).length()
                        < 1e-5
                );
            }
            source.update(0.02);
            mirror
                .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
                .unwrap();
            for &id in &recut.pieces {
                let expected = current_display * source.cut_lineage[&id].1;
                let actual =
                    Mat4::from_cols_array_2d(&mirror.objects[&id].render_data.instance.model);
                assert!((expected.w_axis - actual.w_axis).length() < 1e-5);
            }
        }
    }
    #[test]
    fn binary_cut_lineage_automatically_smooths_and_cleans_live_children() {
        let mut source = SceneManager::default();
        let (_, parent) = tetra(&mut source);
        source.set_cuttable(parent, true, Vec3::ONE).unwrap();
        let cutter = source.add_cube(Vec3::ZERO);
        source
            .set_cutter(
                cutter,
                Some([Vec3::new(0.2, -1., -1.), Vec3::new(0.2, 2., -1.)]),
            )
            .unwrap();
        let mut publisher = WirePublisher::default();
        let mut mirror = SceneManager::default();
        mirror.set_mirror_interpolation(200., 4.).unwrap();
        mirror
            .apply_wire_publication(&publisher.capture(&mut source).unwrap(), &[])
            .unwrap();
        let displayed = mirror.objects[&parent].render_data.instance.model;
        let cut = source.cut_with(cutter, Vec3::new(0., 0., 3.), 0.).unwrap();
        assert_eq!(cut.pieces.len(), 2);
        for &child in &cut.pieces {
            assert_eq!(source.cut_lineage[&child].0, parent);
            source.set_object_position(child, Vec3::X);
        }
        let bytes = publisher.capture(&mut source).unwrap();
        assert_eq!(decode(&bytes).unwrap().cut_lineage.len(), 2);
        let mut invalid_birth = bytes.clone();
        let len = invalid_birth.len();
        invalid_birth[len - 4..].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(mirror.apply_wire_publication(&invalid_birth, &[]).is_err());
        assert_eq!(mirror.wire_publication_sequence(), 1);
        // Automatic path: no supplemental spawn argument.
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        let child = cut.pieces[0];
        assert_eq!(mirror.objects[&child].render_data.instance.model, displayed);
        mirror.update(0.1);
        let x = mirror.objects[&child].render_data.instance.model[3][0];
        assert!((x - 0.5).abs() < 1e-5);
        // Persisted lineage survives another publication; explicit identical mapping merges.
        mirror
            .apply_wire_publication_with_spawns(
                &publisher.capture(&mut source).unwrap(),
                &[],
                &[child, parent],
            )
            .unwrap();
        assert!((mirror.objects[&child].render_data.instance.model[3][0] - x).abs() < 1e-5);
        source.remove_object(child);
        assert!(!source.cut_lineage.contains_key(&child));
        assert_eq!(
            decode(&publisher.capture(&mut source).unwrap())
                .unwrap()
                .cut_lineage
                .len(),
            1
        );
        source.clear_scene();
        assert!(source.cut_lineage.is_empty());
    }
    #[test]
    fn binary_legacy_versions_have_no_lineage_and_keep_their_header_layout() {
        let mut source = SceneManager::default();
        source.add_cube(Vec3::ZERO);
        let bytes = WirePublisher::default().capture(&mut source).unwrap();
        for version in [1u32, 2, 3] {
            let mut legacy = bytes[..bytes.len() - 4].to_vec(); // Empty v4 lineage.
            if version < 3 {
                legacy.truncate(legacy.len() - 4);
            } // Empty font owners.
            if version == 1 {
                legacy.drain(56..64);
            } // No cut material IDs.
            legacy[4..8].copy_from_slice(&version.to_le_bytes());
            let mut mirror = SceneManager::default();
            mirror.apply_wire_publication(&legacy, &[]).unwrap();
            assert_eq!(mirror.objects.len(), 1);
        }
    }
    #[test]
    fn borrowed_publication_matches_capture_and_skips_removed_geometry() {
        let mut worker = SceneManager::default();
        let (_, original) = tetra(&mut worker);
        let mut direct = WirePublisher::default();
        let mut borrowed = WirePublisher::default();
        let mut snapshots = RenderPublisher::default();
        let publication = snapshots.capture(&mut worker);
        let initial = borrowed.capture_publication(&publication).unwrap();
        assert_eq!(direct.capture(&mut worker).unwrap(), initial);
        let mut mirror = SceneManager::default();
        mirror.apply_wire_publication(&initial, &[]).unwrap();
        // Geometry created and removed in a skipped snapshot must not enter the cache.
        let transient = worker
            .add_triangle_mesh(crate::cutting::CutMesh::from_render_mesh(
                &crate::info_panel::numeric_glyph(1),
            ))
            .unwrap();
        drop(snapshots.capture(&mut worker));
        worker.remove_object(transient);
        worker.remove_object(original);
        let pieces: Vec<_> = (0..2)
            .map(|i| {
                let id = worker
                    .add_triangle_mesh(crate::cutting::CutMesh::from_render_mesh(
                        &crate::info_panel::numeric_glyph(2),
                    ))
                    .unwrap();
                worker.set_object_position(id, Vec3::new(i as f32, 1., -2.));
                id
            })
            .collect();
        let latest = snapshots.capture(&mut worker);
        let bytes = borrowed.capture_publication(&latest).unwrap();
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        assert!(!mirror.objects.contains_key(&original));
        assert!(!mirror.objects.contains_key(&transient));
        for id in pieces {
            assert_mesh(
                worker.triangle_meshes[&id].mesh.prepared_render().unwrap(),
                mirror.triangle_meshes[&id].mesh.prepared_render().unwrap(),
            );
            assert_eq!(
                mirror.objects[&id].transform.position,
                worker.objects[&id].transform.position
            );
        }
    }
    #[test]
    fn binary_publication_font_owners_and_legacy_defaults() {
        let mut worker = SceneManager::default();
        let font = worker.builtin_font3d("pixel").unwrap();
        let mut mirror = SceneManager::default();
        assert_eq!(mirror.builtin_font3d("pixel").unwrap(), font);
        let mut publisher = WirePublisher::default();
        let first = publisher.capture(&mut worker).unwrap();
        mirror.apply_wire_publication(&first, &[]).unwrap();
        assert!(mirror.render_publication.font_assets.is_empty());
        assert_eq!(
            mirror.render_publication.font_owners,
            BTreeSet::from([font])
        );
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        assert!(mirror.font3d_glow_fonts().contains(&font));
        // One owner: the appended set contains a u32 count and one u32 ID.
        let mut legacy = first[..first.len() - 12].to_vec();
        legacy[4..8].copy_from_slice(&2u32.to_le_bytes());
        let mut legacy_mirror = SceneManager::default();
        legacy_mirror.apply_wire_publication(&legacy, &[]).unwrap();
        assert!(legacy_mirror.render_publication.font_owners.is_empty());
        assert!(worker.remove_font3d(font));
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert!(mirror.render_publication.font_owners.is_empty());
        #[cfg(any(target_arch = "wasm32", feature = "native-render"))]
        assert!(mirror.font3d_glow_fonts().is_empty());
        assert!(
            !mirror.publication_font_owners().is_empty(),
            "startup registry remains stale"
        );
        let again = worker.builtin_font3d("pixel").unwrap();
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert!(mirror.render_publication.font_owners.contains(&again));
        worker.clear_scene();
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert!(mirror.render_publication.font_owners.is_empty());
    }

    fn tetra(scene: &mut SceneManager) -> (u32, u32) {
        let positions = [0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
        let indices = [0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3];
        let mut first = vec![0.13; 12];
        let mut second = vec![0.21; 12];
        // Bright facets must remain identical; only the three dark facets vary.
        let glow = [0.77, 0.45, 0.30];
        first[..3].copy_from_slice(&glow);
        second[..3].copy_from_slice(&glow);
        let colors = [first, second].concat();
        let handle = scene
            .register_mesh_variants(&positions, &indices, &colors, true)
            .unwrap();
        let asset = &scene.mesh_variants[&handle];
        assert_eq!(&asset.varying_triangles[..], &[1, 2, 3]);
        assert_eq!(asset.colors[0], asset.colors[4]);
        let id = scene.add_mesh_variant(handle, 1).unwrap();
        (handle, id)
    }
    fn assert_mesh(a: &ImportedMesh, b: &ImportedMesh) {
        assert_eq!(a.bounds_min, b.bounds_min);
        assert_eq!(a.bounds_max, b.bounds_max);
        assert_eq!(a.indices, b.indices);
        assert_eq!(a.vertices.len(), b.vertices.len());
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.color, b.color);
        }
    }
    #[test]
    fn binary_cut_material_tags_survive_pool_reuse_and_clear() {
        let mut worker = SceneManager::default();
        let unrelated = worker.add_cube(Vec3::ONE);
        let mut publisher = WirePublisher::default();
        let mut mirror = SceneManager::default();
        let mut rings = BTreeSet::new();
        for _ in 0..4 {
            let pieces: [u32; 2] = std::array::from_fn(|i| {
                let mesh =
                    crate::cutting::CutMesh::from_render_mesh(&crate::info_panel::numeric_glyph(3));
                let id = worker.add_triangle_mesh(mesh).unwrap();
                worker.set_object_position(id, Vec3::new(i as f32 * 0.2, 1., -1.));
                id
            });
            let ids = worker.play_cut_feedback(&pieces).unwrap();
            rings.insert(ids[0]);
            mirror
                .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
                .unwrap();
            assert!(mirror.is_cut_feedback_ring(ids[0]));
            assert!(!mirror.is_cut_feedback_ring(ids[1]));
            assert!(!mirror.is_cut_feedback_ring(unrelated));
            assert_eq!(mirror.cut_feedback_ring_ids(), [None; 2]);
        }
        assert_eq!(rings.len(), 2);
        worker.clear_cut_feedback();
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        // Clear parks the reusable meshes; their material remains textured.
        assert!(rings.iter().all(|id| mirror.is_cut_feedback_ring(*id)
            && mirror.objects[id].transform.position.y < -1000.));
        for id in &rings {
            worker.remove_object(*id);
        }
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert!(rings.iter().all(|id| !mirror.is_cut_feedback_ring(*id)));
    }
    #[test]
    fn binary_roundtrip_preserves_exact_render_material_variants_and_bounds() {
        let mut worker = SceneManager::default();
        let (handle, id) = tetra(&mut worker);
        worker.set_object_transform(
            id,
            Vec3::new(1., 2., 3.),
            Vec3::new(0.2, 0.3, 0.4),
            Vec3::new(0.5, 0.7, 0.9),
        );
        worker.set_emission(id, 0.83);
        worker.set_opacity(id, 0.06, 0.8, 1., 0.25);
        worker.set_color_cutout(id, 0.14);
        worker.add_light(Vec3::new(2., 3., 4.), Vec3::new(0.1, 0.2, 0.3));
        let mut publisher = WirePublisher::default();
        let bytes = publisher.capture(&mut worker).unwrap();
        assert_eq!(&bytes[..4], MAGIC);
        assert_eq!(publisher.sequence(), 1);
        let mut mirror = SceneManager::default();
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        let source = &worker.objects[&id];
        let target = &mirror.objects[&id];
        assert_eq!(source.transform.position, target.transform.position);
        assert_eq!(source.transform.rotation, target.transform.rotation);
        assert_eq!(source.transform.scale, target.transform.scale);
        assert_eq!(source.render_options, target.render_options);
        assert_eq!(source.emission, target.emission);
        assert_eq!(source.opacity, target.opacity);
        assert_eq!(source.color_cutout, target.color_cutout);
        assert!(source.render_data.instance == target.render_data.instance);
        assert_eq!(source.render_data.center, target.render_data.center);
        assert_eq!(
            source.render_data.half_extent,
            target.render_data.half_extent
        );
        assert_eq!(mirror.variant_instances[&id], (handle, 1));
        assert_eq!(
            &*mirror.mesh_variants[&handle].colors,
            &*worker.mesh_variants[&handle].colors
        );
        assert_mesh(
            mirror.prepared_variant_mesh(handle).unwrap(),
            &worker.mesh_variants[&handle].gpu_mesh(),
        );
        assert_eq!(
            mirror.prepared_variant_colors(handle).unwrap(),
            worker.mesh_variants[&handle].gpu_colors()
        );
        assert!(mirror.triangle_meshes[&id].mesh.ready_cpu().is_none());
        assert!(mirror.triangle_meshes[&id].mesh.cpu().is_err());
        assert!(mirror.triangle_meshes[&id].mesh.contact_shape().is_none());
        let time = mirror.time;
        mirror.update(0.25);
        mirror.sync_numeric_font();
        mirror.sync_font3d();
        assert_eq!(mirror.time, time);
    }
    #[test]
    fn binary_incremental_assets_survive_and_stable_object_nodes_are_reused() {
        let mut worker = SceneManager::default();
        let (handle, id) = tetra(&mut worker);
        let mut publisher = WirePublisher::default();
        let mut mirror = SceneManager::default();
        let first = publisher.capture(&mut worker).unwrap();
        mirror.apply_wire_publication(&first, &[]).unwrap();
        let source = mirror.triangle_meshes[&id].mesh.clone();
        let object_address = mirror.objects.get(&id).unwrap() as *const SceneObject;
        worker.set_object_position(id, Vec3::splat(3.));
        let second = publisher.capture(&mut worker).unwrap();
        let decoded = decode(&second).unwrap();
        assert!(decoded.new_triangles.is_empty());
        assert!(decoded.new_variants.is_empty());
        assert!(second.len() < first.len());
        mirror.apply_wire_publication(&second, &[]).unwrap();
        assert!(source.same_source(&mirror.triangle_meshes[&id].mesh));
        assert_eq!(
            object_address,
            mirror.objects.get(&id).unwrap() as *const SceneObject
        );
        worker.remove_object(id);
        worker.remove_mesh_variants(handle);
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert!(mirror.objects.is_empty());
        assert!(mirror.triangle_meshes.is_empty());
        assert!(mirror.mesh_variants.is_empty());
        assert!(mirror.wire_state.variants.is_empty());
    }
    #[test]
    fn binary_numeric_tags_panels_and_protected_ids_remain_exact() {
        let mut worker = SceneManager::default();
        let panel = worker.add_info_panel("diagnostics", 1., 0.5).unwrap();
        let sword = worker.add_cube(Vec3::ZERO);
        let numeric = worker.add_info_panel("score", 1., 0.5).unwrap();
        let mut mirror = SceneManager::default();
        assert_eq!(
            panel,
            mirror.add_info_panel("diagnostics", 1., 0.5).unwrap()
        );
        assert_eq!(sword, mirror.add_cube(Vec3::ZERO));
        mirror.set_object_position(sword, Vec3::splat(9.));
        mirror.set_emission(sword, 3.);
        mirror.update_info_panel(panel, &[("FPS", "120".into())]);
        let local_mesh = mirror.info_panels[&panel].mesh.clone();
        let mut publisher = WirePublisher::default();
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[panel, sword])
            .unwrap();
        assert_eq!(mirror.objects[&sword].transform.position, Vec3::splat(9.));
        assert_eq!(mirror.objects[&sword].emission, 3.);
        assert!(std::sync::Arc::ptr_eq(
            &local_mesh,
            &mirror.info_panels[&panel].mesh
        ));
        assert!(mirror.is_numeric_panel(numeric));
        assert_eq!(mirror.mesh_variants.len(), 10);
        assert_eq!(mirror.numeric_font.children, worker.numeric_font.children);
        let numeric_mesh = mirror.info_panels[&numeric].mesh.clone();
        mirror.info_panels.get_mut(&numeric).unwrap().dirty = false;
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[panel, sword])
            .unwrap();
        assert!(!mirror.info_panels[&numeric].dirty);
        assert!(std::sync::Arc::ptr_eq(
            &numeric_mesh,
            &mirror.info_panels[&numeric].mesh
        ));
        worker.remove_object(panel);
        worker.remove_object(sword);
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[panel, sword])
            .unwrap();
        assert!(mirror.objects.contains_key(&panel));
        assert!(mirror.objects.contains_key(&sword));
    }
    #[test]
    fn binary_corruption_and_sequence_gaps_are_transactional() {
        let mut worker = SceneManager::default();
        worker.add_cube(Vec3::ONE);
        let mut publisher = WirePublisher::default();
        let first = publisher.capture(&mut worker).unwrap();
        let mut mirror = SceneManager::default();
        let local = mirror.add_cube(Vec3::splat(7.));
        let revision = mirror.render_revision();
        let mut legacy = first.clone();
        legacy[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            mirror.apply_wire_publication(&legacy, &[]).is_err(),
            "v1 peers must rebuild together rather than silently lose material tags"
        );
        let mut corrupt = first.clone();
        corrupt[0] ^= 1;
        assert!(mirror.apply_wire_publication(&corrupt, &[]).is_err());
        for end in [0, 3, 8, 20, first.len() - 1] {
            assert!(mirror.apply_wire_publication(&first[..end], &[]).is_err());
        }
        let second = publisher.capture(&mut worker).unwrap();
        assert!(mirror.apply_wire_publication(&second, &[]).is_err());
        assert_eq!(mirror.wire_publication_sequence(), 0);
        assert_eq!(mirror.render_revision(), revision);
        assert_eq!(mirror.objects[&local].transform.position, Vec3::splat(7.));
        mirror.apply_wire_publication(&first, &[]).unwrap();
        assert!(mirror.apply_wire_publication(&first, &[]).is_err());
        mirror.apply_wire_publication(&second, &[]).unwrap();
        assert_eq!(mirror.wire_publication_sequence(), 2);
    }
    #[test]
    fn binary_standalone_geometry_is_supplied_and_panel_revisions_are_incremental() {
        let mut worker = SceneManager::default();
        let id = worker
            .add_triangle_mesh(crate::cutting::CutMesh::from_render_mesh(
                &crate::info_panel::numeric_glyph(2),
            ))
            .unwrap();
        let panel = worker.add_info_panel("diagnostics", 1., 0.5).unwrap();
        let mut publisher = WirePublisher::default();
        let mut mirror = SceneManager::default();
        mirror
            .apply_wire_publication(&publisher.capture(&mut worker).unwrap(), &[])
            .unwrap();
        assert_mesh(
            worker.triangle_meshes[&id].mesh.prepared_render().unwrap(),
            mirror.triangle_meshes[&id].mesh.prepared_render().unwrap(),
        );
        assert!(mirror.triangle_meshes[&id].mesh.ready_cpu().is_none());
        mirror.info_panels.get_mut(&panel).unwrap().dirty = false;
        worker.update_info_panel(panel, &[("FPS", "90".into())]);
        let bytes = publisher.capture(&mut worker).unwrap();
        let d = decode(&bytes).unwrap();
        assert!(d.new_triangles.is_empty());
        assert_eq!(d.new_panels.len(), 1);
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        assert!(mirror.info_panels[&panel].dirty);
        assert_mesh(
            &worker.info_panels[&panel].mesh,
            &mirror.info_panels[&panel].mesh,
        );
    }
}
