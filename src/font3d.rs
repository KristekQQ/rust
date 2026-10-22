//! General scene-owned mesh fonts. Geometry and style are shared by glyph instances.
use super::*;
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Default)]
pub(super) struct Font3D {
    fonts: BTreeMap<u32, Font>,
    builtin: Option<u32>,
    bindings: BTreeMap<u32, PanelBinding>,
    layout_rebuilds: u64,
    sync_updates: u64,
    pub(super) texts: BTreeMap<u32, Text>,
    pub(super) children: BTreeSet<u32>,
}
struct PanelBinding {
    font: u32,
    options: Options,
    revision: u64,
}
struct Glyph {
    advance: f32,
    mesh: Option<crate::cutting_mesh::MeshData>,
    front: Vec<bool>,
}
struct Font {
    name: String,
    license: String,
    depth: f32,
    registered: bool,
    glyphs: BTreeMap<char, Glyph>,
    cache: BTreeMap<(char, [u32; 4]), u32>,
}
#[derive(Clone, PartialEq)]
struct Options {
    size: f32,
    max_width: Option<f32>,
    align: String,
    spacing: f32,
    color: [f32; 3],
    side: f32,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            size: 1.,
            max_width: None,
            align: "center".into(),
            spacing: 0.,
            color: [1.; 3],
            side: 0.32,
        }
    }
}
impl Options {
    fn merge(&self, input: &str) -> Result<Self, String> {
        if input.len() > 4096 {
            return Err("Text options exceed budget".into());
        }
        let v: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        let map = v.as_object().ok_or("Text options must be an object")?;
        let mut o = self.clone();
        for (key, value) in map {
            match key.as_str() {
                "size" => o.size = number(value)?,
                "maxWidth" => {
                    o.max_width = if value.is_null() {
                        None
                    } else {
                        Some(number(value)?)
                    }
                }
                "letterSpacing" => o.spacing = number(value)?,
                "sideIntensity" => o.side = number(value)?,
                "align" => o.align = value.as_str().ok_or("Invalid align")?.into(),
                "color" => {
                    let a = value
                        .as_array()
                        .filter(|a| a.len() == 3)
                        .ok_or("Invalid color")?;
                    for (c, v) in o.color.iter_mut().zip(a) {
                        *c = number(v)?;
                    }
                }
                _ => return Err(format!("Unknown text option: {key}")),
            }
        }
        if o.size <= 0.
            || o.size > 10000.
            || o.max_width.is_some_and(|w| w <= 0. || w > 10000.)
            || o.spacing.abs() > 100.
            || !(0.0..=1.).contains(&o.side)
            || o.color.iter().any(|c| !(0.0..=1.).contains(c))
            || !["left", "center", "right"].contains(&o.align.as_str())
        {
            return Err("Invalid text options".into());
        }
        Ok(o)
    }
    fn style(&self) -> [u32; 4] {
        [
            self.color[0].to_bits(),
            self.color[1].to_bits(),
            self.color[2].to_bits(),
            self.side.to_bits(),
        ]
    }
}
fn number(v: &Value) -> Result<f32, String> {
    let n = v.as_f64().ok_or("Expected number")? as f32;
    if !n.is_finite() {
        return Err("Expected finite number".into());
    }
    Ok(n)
}
#[derive(PartialEq)]
struct Key {
    matrix: [f32; 16],
    options: RenderOptions,
    material: [f32; 7],
}
pub(super) struct Text {
    font: u32,
    text: String,
    options: Options,
    width: f32,
    height: f32,
    depth: f32,
    unit: f32,
    offset_y: f32,
    layout: Vec<(char, f32)>,
    children: Vec<u32>,
    key: Option<Key>,
}
fn layout(font: &Font, text: &str, o: Options, font_id: u32) -> Result<Text, String> {
    if text.len() > 8192 || text.chars().count() > 1024 {
        return Err("Text exceeds budget".into());
    }
    let mut x = 0.;
    let mut items = Vec::new();
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let count = text.chars().count();
    for (index, ch) in text.chars().enumerate() {
        let g = font
            .glyphs
            .get(&ch)
            .ok_or_else(|| format!("Unsupported font character: {ch}"))?;
        if let Some(mesh) = &g.mesh {
            items.push((ch, x));
            min_y = min_y.min(mesh.bounds().0.y);
            max_y = max_y.max(mesh.bounds().1.y);
        }
        x += g.advance;
        if index + 1 < count {
            x += o.spacing;
        }
    }
    if !x.is_finite() || x < 0. {
        return Err("Invalid text layout width".into());
    }
    let unit = o
        .max_width
        .filter(|_| x > 0.)
        .map_or(o.size, |w| o.size.min(w / x));
    let width = x * unit;
    let offset = match o.align.as_str() {
        "left" => 0.,
        "right" => -x,
        _ => -x * 0.5,
    };
    for (_, x) in &mut items {
        *x += offset;
    }
    Ok(Text {
        font: font_id,
        text: text.into(),
        options: o,
        width,
        height: if items.is_empty() {
            0.
        } else {
            (max_y - min_y) * unit
        },
        depth: if items.is_empty() {
            0.
        } else {
            font.depth * unit
        },
        unit,
        offset_y: 0.,
        layout: items,
        children: vec![],
        key: None,
    })
}
// Fit actual ink to the original centered panel; digit metrics keep changing
// timer values on one stable baseline. Parent scaling is applied only at sync.
fn panel_layout(
    font: &Font,
    panel: &crate::info_panel::InfoPanel,
    options: &Options,
    id: u32,
) -> Result<Text, String> {
    let value = panel.font3d_value()?;
    let mut raw = options.clone();
    raw.size = 1.;
    raw.max_width = None;
    raw.align = "left".into();
    let mut t = layout(font, value, raw, id)?;
    t.options = options.clone();
    if t.layout.is_empty() {
        return Ok(t);
    }
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &(ch, x) in &t.layout {
        let (a, b) = font.glyphs[&ch].mesh.as_ref().unwrap().bounds();
        min = min.min(a + Vec3::new(x, 0., 0.));
        max = max.max(b + Vec3::new(x, 0., 0.));
    }
    let mut bottom = min.y;
    let mut top = max.y;
    if matches!(
        panel.kind,
        crate::info_panel::PanelKind::Countdown
            | crate::info_panel::PanelKind::Score
            | crate::info_panel::PanelKind::NumericText
    ) {
        for ch in '0'..='9' {
            if let Some(mesh) = font.glyphs.get(&ch).and_then(|g| g.mesh.as_ref()) {
                let (a, b) = mesh.bounds();
                bottom = bottom.min(a.y);
                top = top.max(b.y);
            }
        }
    }
    let width = (max.x - min.x).max(1e-6);
    let available = options
        .max_width
        .unwrap_or(panel.width * 0.94)
        .min(panel.width * 0.94);
    t.unit = (panel.height * 0.90 / (top - bottom).max(1e-6) * options.size.min(1.))
        .min(available / width);
    let shift = match options.align.as_str() {
        "left" => -min.x - available / (2. * t.unit),
        "right" => -max.x + available / (2. * t.unit),
        _ => -(min.x + max.x) * 0.5,
    };
    for (_, x) in &mut t.layout {
        *x += shift;
    }
    t.offset_y = -(bottom + top) * 0.5 * t.unit;
    t.width = width * t.unit;
    t.height = (max.y - min.y) * t.unit;
    t.depth = font.depth * t.unit;
    Ok(t)
}
impl SceneManager {
    pub fn bind_info_panel_font3d(
        &mut self,
        id: u32,
        font: u32,
        options: &str,
    ) -> Result<(), String> {
        let panel = self
            .info_panels
            .get(&id)
            .ok_or("Invalid info panel handle")?;
        panel.font3d_value()?;
        if font == INVALID_ID {
            self.remove_font3d_text(id);
            let panel = self.info_panels.get_mut(&id).unwrap();
            panel.set_font3d_bound(false);
            self.sync_numeric_font();
            self.render_revision = self.render_revision.wrapping_add(1);
            self.static_revision = self.static_revision.wrapping_add(1);
            return Ok(());
        }
        let f = self
            .font3d
            .fonts
            .get(&font)
            .filter(|f| f.registered)
            .ok_or("Invalid font handle")?;
        let defaults = Options {
            color: [0.02, 0.8, 1.],
            ..Options::default()
        };
        let options = defaults.merge(options)?;
        let text = panel_layout(f, panel, &options, font)?;
        self.reserve_font3d_ids(text.layout.len())?;
        let old_count = self.font3d.texts.get(&id).map_or(0, |t| t.children.len());
        if self.font3d.children.len() - old_count + text.layout.len() > 32768 {
            return Err("Scene glyph budget exceeded".into());
        }
        let revision = panel.revision;
        self.remove_font3d_children(id);
        self.remove_numeric_panel(id);
        self.font3d.bindings.insert(
            id,
            PanelBinding {
                font,
                options,
                revision,
            },
        );
        self.font3d.texts.insert(id, text);
        self.font3d.layout_rebuilds += 1;
        self.info_panels
            .get_mut(&id)
            .unwrap()
            .set_font3d_bound(true);
        self.sync_font3d();
        self.collect_font3d();
        Ok(())
    }
    fn sync_font3d_panel_bindings(&mut self) {
        let mut cursor = None;
        loop {
            let next = self
                .font3d
                .bindings
                .iter()
                .find(|(id, b)| {
                    cursor.is_none_or(|previous| **id > previous)
                        && self
                            .info_panels
                            .get(*id)
                            .is_some_and(|p| p.revision != b.revision)
                })
                .map(|(&id, _)| id);
            let Some(id) = next else {
                break;
            };
            cursor = Some(id);
            let binding = &self.font3d.bindings[&id];
            let panel = &self.info_panels[&id];
            let result = panel_layout(
                &self.font3d.fonts[&binding.font],
                panel,
                &binding.options,
                binding.font,
            );
            self.font3d.bindings.get_mut(&id).unwrap().revision = panel.revision;
            let Ok(text) = result else {
                continue;
            }; // Retain last valid value on unsupported dynamic content.
            if self.reserve_font3d_ids(text.layout.len()).is_err()
                || self.font3d.children.len() - self.font3d.texts[&id].children.len()
                    + text.layout.len()
                    > 32768
            {
                continue;
            }
            self.remove_font3d_children(id);
            self.font3d.texts.insert(id, text);
            self.font3d.layout_rebuilds += 1;
            self.collect_font3d();
        }
    }
    pub fn font3d_stats(&self) -> Value {
        json!({"fonts":self.font3d.fonts.len(), "texts":self.font3d.texts.len(), "cachedGlyphs":self.font3d.fonts.values().map(|f| f.cache.len()).sum::<usize>(), "liveGlyphs":self.font3d.children.len(), "layoutRebuilds":self.font3d.layout_rebuilds, "syncUpdates":self.font3d.sync_updates})
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn font3d_assets(&self) -> impl Iterator<Item = &u32> {
        self.font3d.fonts.values().flat_map(|f| f.cache.values())
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn font3d_children(&self) -> impl Iterator<Item = &u32> {
        self.font3d.children.iter()
    }

    pub fn register_font3d(&mut self, input: &str) -> Result<u32, String> {
        if input.len() > 16 * 1024 * 1024 || self.font3d.fonts.len() >= 64 {
            return Err("Font budget exceeded".into());
        }
        let v: Value = serde_json::from_str(input).map_err(|e| e.to_string())?;
        if v.get("format")
            .is_some_and(|f| f.as_str() != Some("mesh-font"))
            || v.get("version").is_some_and(|v| v.as_u64() != Some(1))
        {
            return Err("Unsupported mesh font format/version".into());
        }
        let name = v["font"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 256)
            .ok_or("Invalid font name")?
            .to_string();
        let depth = number(&v["depth"])?;
        if depth <= 0. || depth > 100. {
            return Err("Invalid font depth".into());
        }
        let license = v
            .get("license")
            .map_or(Ok(""), |v| v.as_str().ok_or("Invalid license"))?
            .to_string();
        if license.len() > 4096 {
            return Err("License exceeds budget".into());
        }
        let entries = v["glyphs"]
            .as_object()
            .filter(|g| !g.is_empty() && g.len() <= 2048)
            .ok_or("Invalid glyph map")?;
        let mut glyphs = BTreeMap::new();
        let mut triangles = 0;
        for (ch, g) in entries {
            let mut chars = ch.chars();
            let ch = chars
                .next()
                .filter(|c| !c.is_control())
                .ok_or("Invalid glyph key")?;
            if chars.next().is_some() {
                return Err("Glyph key must contain one character".into());
            }
            let advance = number(&g["advance"])?;
            if advance <= 0. || advance > 100. {
                return Err("Invalid glyph advance".into());
            }
            let p = g["positions"].as_array().ok_or("Invalid positions")?;
            let i = g["indices"].as_array().ok_or("Invalid indices")?;
            let f = g["front"].as_array().ok_or("Invalid front mask")?;
            if p.len() % 3 != 0
                || i.len() % 3 != 0
                || f.len() != i.len() / 3
                || p.len() > 300_000
                || i.len() > 300_000
            {
                return Err("Invalid glyph buffers".into());
            }
            triangles += i.len() / 3;
            if triangles > 1_000_000 {
                return Err("Font triangle budget exceeded".into());
            }
            let positions = p
                .as_chunks::<3>()
                .0
                .iter()
                .map(|p| Ok(Vec3::new(number(&p[0])?, number(&p[1])?, number(&p[2])?)))
                .collect::<Result<Vec<_>, String>>()?;
            let indices = i
                .iter()
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or("Invalid glyph index".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let front = f
                .iter()
                .map(|v| match v.as_u64() {
                    Some(0) => Ok(false),
                    Some(1) => Ok(true),
                    _ => Err("Invalid front mask".to_string()),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mesh = if indices.is_empty() {
                if !positions.is_empty() {
                    return Err("Empty glyph has positions".into());
                }
                None
            } else {
                Some(
                    crate::cutting::CutMesh::new(positions, &indices, Vec3::ONE)
                        .map_err(|e| format!("Glyph {ch}: {e}"))?
                        .into(),
                )
            };
            glyphs.insert(
                ch,
                Glyph {
                    advance,
                    mesh,
                    front,
                },
            );
        }
        let id = self.allocate_id().ok_or("Font IDs exhausted")?;
        self.font3d.fonts.insert(
            id,
            Font {
                name,
                license,
                depth,
                registered: true,
                glyphs,
                cache: BTreeMap::new(),
            },
        );
        Ok(id)
    }
    pub fn builtin_font3d(&mut self, name: &str) -> Result<u32, String> {
        if name != "pixel" {
            return Err("Unknown builtin font".into());
        }
        if let Some(id) = self
            .font3d
            .builtin
            .filter(|id| self.font3d.fonts.get(id).is_some_and(|f| f.registered))
        {
            return Ok(id);
        }
        let mut glyphs = serde_json::Map::new();
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ./+-".chars() {
            let mut positions = Vec::<f32>::new();
            let mut indices = Vec::<u32>::new();
            let mut front = Vec::<u8>::new();
            for (y, row) in crate::debug_overlay::glyph(ch).iter().enumerate() {
                if ch == ' ' {
                    break;
                }
                for x in 0..5 {
                    if row & (1 << (4 - x)) == 0 {
                        continue;
                    }
                    let base = (positions.len() / 3) as u32;
                    let px = x as f32 / 7.;
                    let py = (6 - y) as f32 / 7.;
                    let s = 0.13;
                    for [a, b, c] in [
                        [0., 0., 0.],
                        [s, 0., 0.],
                        [s, s, 0.],
                        [0., s, 0.],
                        [0., 0., 0.13],
                        [s, 0., 0.13],
                        [s, s, 0.13],
                        [0., s, 0.13],
                    ] {
                        positions.extend([px + a, py + b, c]);
                    }
                    for tri in [
                        [0, 2, 1],
                        [0, 3, 2],
                        [4, 5, 6],
                        [4, 6, 7],
                        [0, 1, 5],
                        [0, 5, 4],
                        [1, 2, 6],
                        [1, 6, 5],
                        [2, 3, 7],
                        [2, 7, 6],
                        [3, 0, 4],
                        [3, 4, 7],
                    ] {
                        indices.extend(tri.map(|i| base + i));
                    }
                    front.extend([0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0]);
                }
            }
            glyphs.insert(
                ch.to_string(),
                json!({"advance":6./7.,"positions":positions,"indices":indices,"front":front}),
            );
        }
        let id = self.register_font3d(&json!({"font":"pixel","depth":0.13,"license":"Engine builtin bitmap glyphs","glyphs":glyphs}).to_string())?;
        self.font3d.builtin = Some(id);
        Ok(id)
    }
    pub fn add_font3d_text(&mut self, font: u32, text: &str, options: &str) -> Result<u32, String> {
        if self.font3d.texts.len() >= 1024 {
            return Err("Text count budget exceeded".into());
        }
        let f = self
            .font3d
            .fonts
            .get(&font)
            .filter(|f| f.registered)
            .ok_or("Invalid font handle")?;
        let t = layout(f, text, Options::default().merge(options)?, font)?;
        if self.font3d.children.len() + t.layout.len() > 32768 {
            return Err("Scene glyph budget exceeded".into());
        }
        self.reserve_font3d_ids(t.layout.len())?;
        let id = self.add_imported(Vec3::ZERO, Vec3::ONE);
        self.font3d.layout_rebuilds += 1;
        self.font3d.texts.insert(id, t);
        self.sync_font3d();
        Ok(id)
    }
    fn reserve_font3d_ids(&self, glyphs: usize) -> Result<(), String> {
        if self.next_id as u64 + glyphs as u64 * 2 + 1 >= i32::MAX as u64 {
            Err("Object IDs exhausted".into())
        } else {
            Ok(())
        }
    }
    pub fn update_font3d_text(&mut self, id: u32, text: &str, options: &str) -> Result<(), String> {
        let old = self.font3d.texts.get(&id).ok_or("Invalid text handle")?;
        let merged = old.options.merge(options)?;
        if old.text == text && old.options == merged {
            return Ok(());
        }
        let t = layout(&self.font3d.fonts[&old.font], text, merged, old.font)?;
        if self.font3d.children.len() - old.children.len() + t.layout.len() > 32768 {
            return Err("Scene glyph budget exceeded".into());
        }
        self.reserve_font3d_ids(t.layout.len())?;
        self.remove_font3d_children(id);
        self.font3d.layout_rebuilds += 1;
        self.font3d.texts.insert(id, t);
        self.sync_font3d();
        self.collect_font3d();
        Ok(())
    }
    pub fn font3d_info(&self, id: u32) -> Result<String, String> {
        let f = self.font3d.fonts.get(&id).ok_or("Invalid font handle")?;
        let texts: Vec<_> = self
            .font3d
            .texts
            .values()
            .filter(|t| t.font == id)
            .collect();
        Ok(json!({"name":f.name,"characters":f.glyphs.keys().collect::<String>(),"depth":f.depth,"license":f.license,"cachedGlyphs":f.cache.len(),"liveTexts":texts.len(),"liveGlyphs":texts.iter().map(|t| t.children.len()).sum::<usize>(),"registered":f.registered}).to_string())
    }
    pub fn font3d_text_info(&self, id: u32) -> Result<String, String> {
        let t = self.font3d.texts.get(&id).ok_or("Invalid text handle")?;
        Ok(json!({"text":t.text,"fontId":t.font,"width":t.width,"height":t.height,"depth":t.depth,"glyphCount":t.layout.len(),"size":t.options.size,"align":t.options.align,"letterSpacing":t.options.spacing,"color":t.options.color,"sideIntensity":t.options.side}).to_string())
    }
    pub fn remove_font3d(&mut self, id: u32) -> bool {
        let Some(f) = self.font3d.fonts.get_mut(&id).filter(|f| f.registered) else {
            return false;
        };
        f.registered = false;
        self.collect_font3d();
        true
    }
    fn remove_font3d_children(&mut self, id: u32) {
        let children = self
            .font3d
            .texts
            .get_mut(&id)
            .map(|t| std::mem::take(&mut t.children))
            .unwrap_or_default();
        for child in children {
            self.font3d.children.remove(&child);
            self.remove_object(child);
        }
    }
    pub(super) fn remove_font3d_text(&mut self, id: u32) {
        self.font3d.bindings.remove(&id);
        if !self.font3d.texts.contains_key(&id) {
            return;
        }
        self.remove_font3d_children(id);
        self.font3d.texts.remove(&id);
        self.collect_font3d();
    }
    fn collect_font3d(&mut self) {
        let used: BTreeSet<_> = self
            .font3d
            .texts
            .values()
            .flat_map(|t| {
                t.layout
                    .iter()
                    .map(move |(ch, _)| (t.font, *ch, t.options.style()))
            })
            .collect();
        let mut remove = Vec::new();
        for (&id, f) in &mut self.font3d.fonts {
            f.cache.retain(|&(ch, style), handle| {
                let keep = used.contains(&(id, ch, style));
                if !keep {
                    remove.push(*handle);
                }
                keep
            });
        }
        self.font3d
            .fonts
            .retain(|id, f| f.registered || self.font3d.texts.values().any(|t| t.font == *id));
        for handle in remove {
            self.remove_mesh_variants(handle);
        }
    }
    pub fn sync_font3d(&mut self) {
        self.sync_font3d_panel_bindings();
        let mut state = std::mem::take(&mut self.font3d);
        for (&id, text) in &mut state.texts {
            let Some(parent) = self.objects.get(&id) else {
                continue;
            };
            let transform = parent.transform;
            let key = Key {
                matrix: transform.model().to_cols_array(),
                options: parent.render_options,
                material: [
                    parent.emission,
                    parent.color_cutout,
                    parent.opacity,
                    parent.opaque_above,
                    parent.opacity_variation,
                    parent.opacity_coverage,
                    parent.cut_assist,
                ],
            };
            if text.key.as_ref() == Some(&key) {
                continue;
            }
            if text.children.is_empty() {
                let font = state.fonts.get_mut(&text.font).unwrap();
                for &(ch, _) in &text.layout {
                    let style = text.options.style();
                    let handle = *font.cache.entry((ch, style)).or_insert_with(|| {
                        let glyph = &font.glyphs[&ch];
                        let rgb: Vec<_> = glyph
                            .front
                            .iter()
                            .map(|front| {
                                Vec3::from_array(text.options.color)
                                    * if *front { 1. } else { text.options.side }
                            })
                            .collect();
                        let colors = rgb.iter().map(|c| [c.x, c.y, c.z, 1.]).collect();
                        // Shared renderer assets require a materialized CPU base;
                        // lazy color recipes are only used for later variants.
                        let mesh = crate::cutting_mesh::MeshData::from(
                            glyph
                                .mesh
                                .as_ref()
                                .unwrap()
                                .cpu()
                                .expect("validated font glyph")
                                .recolored(&rgb),
                        );
                        let handle = self.allocate_id().expect("preflight font asset ID");
                        self.mesh_variants.insert(
                            handle,
                            MeshVariants {
                                variants: vec![mesh],
                                colors,
                                registered: true,
                                varying_triangles: vec![],
                            },
                        );
                        handle
                    });
                    let child = self
                        .add_numeric_glyph_instance(handle)
                        .expect("preflight font instance ID");
                    state.children.insert(child);
                    text.children.push(child);
                }
            }
            for (&child, &(_, x)) in text.children.iter().zip(&text.layout) {
                let object = self.objects.get_mut(&child).unwrap();
                object.transform = Transform::new(
                    transform
                        .model()
                        .transform_point3(Vec3::new(x * text.unit, text.offset_y, 0.)),
                    transform.rotation,
                    transform.scale * text.unit,
                );
                object.render_options = key.options;
                [
                    object.emission,
                    object.color_cutout,
                    object.opacity,
                    object.opaque_above,
                    object.opacity_variation,
                    object.opacity_coverage,
                    object.cut_assist,
                ] = key.material;
                object.refresh_render_data();
            }
            self.render_revision = self.render_revision.wrapping_add(1);
            if key.options.is_static {
                self.static_revision = self.static_revision.wrapping_add(1);
            }
            text.key = Some(key);
            state.sync_updates += 1;
        }
        self.font3d = state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font3d_layout_sharing_atomic_updates_and_lifetime() {
        let mut s = SceneManager::default();
        let f = s.builtin_font3d("pixel").unwrap();
        assert_eq!(f, s.builtin_font3d("pixel").unwrap());
        let id = s
            .add_font3d_text(
                f,
                "AA A",
                r#"{"size":2,"maxWidth":2,"align":"right","letterSpacing":0.1}"#,
            )
            .unwrap();
        let t = &s.font3d.texts[&id];
        for handle in s.font3d.fonts[&f].cache.values() {
            let uploaded = s.mesh_variants[handle].gpu_mesh();
            assert!(!uploaded.vertices.is_empty() && !uploaded.indices.is_empty());
        }
        assert_eq!(t.width, 2.);
        assert_eq!(t.children.len(), 3);
        let child = t.children[0];
        assert_eq!(
            s.variant_instances[&child],
            s.variant_instances[&t.children[1]]
        );
        assert_eq!(s.mesh_variants.len(), 1);
        let info = s.font3d_text_info(id).unwrap();
        assert!(s.update_font3d_text(id, "a", "{}").is_err());
        assert!(s.update_font3d_text(id, "AB", r#"{"size":0}"#).is_err());
        assert_eq!(info, s.font3d_text_info(id).unwrap());
        let mut queue = crate::visibility::RenderQueue::default();
        queue.extract(&s, Mat4::IDENTITY, false);
        assert!(!queue.imported.contains_key(&id));
        assert_eq!(queue.imported.values().map(Vec::len).sum::<usize>(), 3);
        for side in [0.1, 0.2, 0.3, 0.4] {
            s.update_font3d_text(id, "AA", &json!({"sideIntensity":side}).to_string())
                .unwrap();
            assert_eq!(s.mesh_variants.len(), 1);
        }
        let other = s.add_font3d_text(f, "A", "{}").unwrap();
        assert_eq!(s.mesh_variants.len(), 2);
        s.remove_object(other);
        assert_eq!(s.mesh_variants.len(), 1);
        assert!(s.remove_font3d(f));
        assert!(s.add_font3d_text(f, "A", "{}").is_err());
        s.update_font3d_text(id, "B", "{}").unwrap();
        assert_eq!(s.mesh_variants.len(), 1);
        s.remove_object(id);
        assert!(s.mesh_variants.is_empty());
        assert!(s.font3d_info(f).is_err());
        let f = s.builtin_font3d("pixel").unwrap();
        s.add_font3d_text(f, "", "{}").unwrap();
        s.clear_scene();
        assert!(s.add_font3d_text(f, "A", "{}").is_err());
        assert!(s.font3d.fonts.is_empty());
    }
    #[test]
    fn font3d_panel_binding_updates_native_values_and_restores_tubes() {
        let mut s = SceneManager::default();
        let f = s
            .register_font3d(include_str!("../assets/fonts/cynatar/glyphs.json"))
            .unwrap();
        let timer = s.add_info_panel("countdown", 1.2, 0.3).unwrap();
        let score = s.add_info_panel("score", 1.2, 0.3).unwrap();
        s.sync_numeric_font();
        assert_eq!(s.numeric_font_stats(), (10, 3, 2));
        s.bind_info_panel_font3d(timer, f, "{}").unwrap();
        s.bind_info_panel_font3d(score, f, "{}").unwrap();
        assert_eq!(s.numeric_font_stats(), (0, 0, 0));
        let stats = s.font3d_stats();
        s.sync_numeric_font();
        s.sync_font3d();
        assert_eq!(stats, s.font3d_stats());
        let hidden_geometry = s.info_panels[&timer].mesh.indices.clone();
        s.info_panels
            .get_mut(&timer)
            .unwrap()
            .update(vec![("", "128".into())]);
        s.info_panels
            .get_mut(&score)
            .unwrap()
            .update(vec![("", "9999".into())]);
        assert_eq!(s.info_panels[&timer].mesh.indices, hidden_geometry);
        s.sync_font3d();
        assert_eq!(s.font3d.texts[&timer].text, "128");
        assert_eq!(s.font3d.texts[&score].text, "9999");
        assert!(s.font3d.texts[&timer].width <= 1.2 * 0.94 + 1e-6);
        assert!(s.font3d.texts[&timer].height <= 0.3 * 0.9 + 1e-6);
        s.bind_info_panel_font3d(timer, INVALID_ID, "{}").unwrap();
        s.bind_info_panel_font3d(score, INVALID_ID, "{}").unwrap();
        assert_eq!(s.numeric_font_stats(), (10, 7, 2));
        assert!(s.font3d.children.is_empty());
        assert!(s.font3d.fonts[&f].cache.is_empty());
        assert!(s.object_exists(timer));
        assert!(s.object_exists(score));
    }
    #[test]
    fn font3d_panel_empty_bonus_pose_atomic_failure_and_cleanup() {
        let mut s = SceneManager::default();
        let f = s.builtin_font3d("pixel").unwrap();
        let id = s.add_info_panel("combo-award", 1., 0.25).unwrap();
        s.bind_info_panel_font3d(id, f, "{}").unwrap();
        assert_eq!(s.font3d.texts[&id].children.len(), 0);
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "+25".into())]);
        s.sync_font3d();
        assert_eq!(s.font3d.texts[&id].children.len(), 3);
        s.bind_info_panel_font3d(id, f, r#"{"size":2}"#).unwrap();
        assert!(s.font3d.texts[&id].height <= 0.25 * 0.90 + 1e-6);
        let info = s.font3d_text_info(id).unwrap();
        assert!(s.bind_info_panel_font3d(id, f, r#"{"size":-1}"#).is_err());
        assert!(s.bind_info_panel_font3d(id, INVALID_ID - 1, "{}").is_err());
        assert_eq!(s.font3d_text_info(id).unwrap(), info);
        let pose = Transform::new(
            Vec3::new(2., 3., -4.),
            Vec3::new(0.3, -0.4, 0.2),
            Vec3::new(2., 0.5, -1.),
        );
        s.objects.get_mut(&id).unwrap().transform = pose;
        s.sync_font3d();
        let t = &s.font3d.texts[&id];
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for (&child, &(ch, x)) in t.children.iter().zip(&t.layout) {
            let expected = pose.model()
                * Mat4::from_translation(Vec3::new(x * t.unit, t.offset_y, 0.))
                * Mat4::from_scale(Vec3::splat(t.unit));
            assert!(s.objects[&child]
                .transform
                .model()
                .abs_diff_eq(expected, 1e-5));
            let (a, b) = s.font3d.fonts[&f].glyphs[&ch]
                .mesh
                .as_ref()
                .unwrap()
                .bounds();
            min = min.min(a * t.unit + Vec3::new(x * t.unit, t.offset_y, 0.));
            max = max.max(b * t.unit + Vec3::new(x * t.unit, t.offset_y, 0.));
        }
        assert!((min.x + max.x).abs() < 1e-6);
        assert!((min.y + max.y).abs() < 1e-6);
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "unsupported".into())]);
        s.sync_font3d(); // Bad dynamic data preserves the last valid presentation.
        assert_eq!(s.font3d.texts[&id].text, "+25");
        assert!(s.remove_font3d(f));
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "+75".into())]);
        s.sync_font3d();
        assert_eq!(s.font3d.texts[&id].text, "+75");
        s.remove_object(id);
        assert!(s.font3d.bindings.is_empty());
        assert!(s.font3d.children.is_empty());
        assert!(s.mesh_variants.is_empty());
        assert!(s.font3d_info(f).is_err());
    }
    #[test]
    fn font3d_panel_rejects_unsupported_source_and_restores_bitmap() {
        let mut s = SceneManager::default();
        let f = s.builtin_font3d("pixel").unwrap();
        let id = s.add_info_panel("debug-text", 1., 0.3).unwrap();
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "lowercase".into())]);
        assert!(s.bind_info_panel_font3d(id, f, "{}").is_err());
        assert!(!s.info_panels[&id].font3d_bound);
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "ABC".into())]);
        s.bind_info_panel_font3d(id, f, "{}").unwrap();
        s.info_panels
            .get_mut(&id)
            .unwrap()
            .update(vec![("", "ABCDE".into())]);
        s.sync_font3d();
        s.bind_info_panel_font3d(id, INVALID_ID, "{}").unwrap();
        assert!(!s.info_panels[&id].font3d_bound);
        assert!(s.info_panels[&id].dirty);
        assert!(!s.info_panels[&id].mesh.indices.is_empty());
        assert!(s.font3d.texts.is_empty());
        let other = s.add_info_panel("scene", 1., 0.3).unwrap();
        assert!(s.bind_info_panel_font3d(other, f, "{}").is_err());
        s.bind_info_panel_font3d(id, f, "{}").unwrap();
        s.clear_scene();
        assert!(s.font3d.bindings.is_empty());
        assert!(s.mesh_variants.is_empty());
    }

    #[test]
    fn font3d_identical_updates_preserve_layout_instances_and_assets() {
        let mut s = SceneManager::default();
        let font = s.builtin_font3d("pixel").unwrap();
        let id = s.add_font3d_text(font, "AAAA", r#"{"size":2}"#).unwrap();
        let stats = s.font3d_stats();
        let children = s.font3d.texts[&id].children.clone();
        let assets = s.font3d.fonts[&font].cache.clone();
        let next_id = s.next_id;
        for options in ["{}", r#"{"size":2,"align":"center"}"#] {
            s.update_font3d_text(id, "AAAA", options).unwrap();
            assert_eq!(s.font3d_stats(), stats);
            assert_eq!(s.font3d.texts[&id].children, children);
            assert_eq!(s.font3d.fonts[&font].cache, assets);
            assert_eq!(s.next_id, next_id);
        }
    }

    #[test]
    fn font3d_rejects_standalone_child_removal_and_releases_parent() {
        let mut s = SceneManager::default();
        let font = s.builtin_font3d("pixel").unwrap();
        let id = s.add_font3d_text(font, "AA", "{}").unwrap();
        let children = s.font3d.texts[&id].children.clone();
        assert!(!s.remove_object(children[0]));
        assert!(s.objects.contains_key(&children[0]));
        let parent = s.objects.get_mut(&id).unwrap();
        parent.transform.position = Vec3::new(1., 2., 3.);
        parent.opacity = 0.5;
        s.sync_font3d();
        assert_eq!(s.font3d.texts[&id].children, children);
        assert_eq!(s.objects[&children[0]].opacity, 0.5);
        assert!(s.remove_object(id));
        assert!(children.iter().all(|child| !s.objects.contains_key(child)));
        assert!(s.font3d.children.is_empty());
        assert!(s.mesh_variants.is_empty());
    }

    #[test]
    fn font3d_parent_pose_material_and_cached_sync() {
        let mut s = SceneManager::default();
        let f = s.builtin_font3d("pixel").unwrap();
        let id = s
            .add_font3d_text(f, "AB", r#"{"align":"left","size":0.4}"#)
            .unwrap();
        let transform = Transform::new(
            Vec3::new(2., 3., -4.),
            Vec3::new(0.3, -0.7, 0.2),
            Vec3::new(-2., 3., 0.5),
        );
        let p = s.objects.get_mut(&id).unwrap();
        p.transform = transform;
        p.opacity = 0.3;
        p.emission = 2.;
        p.render_options.is_static = true;
        s.sync_font3d();
        let t = &s.font3d.texts[&id];
        for (&child, &(_, x)) in t.children.iter().zip(&t.layout) {
            let c = &s.objects[&child];
            let expected = transform.model()
                * Mat4::from_translation(Vec3::new(x * t.unit, 0., 0.))
                * Mat4::from_scale(Vec3::splat(t.unit));
            assert!(c.transform.model().abs_diff_eq(expected, 1e-5));
            assert_eq!(c.opacity, 0.3);
            assert_eq!(c.emission, 2.);
            assert!(c.render_options.is_static);
        }
        let revision = s.render_revision();
        let stats = s.font3d_stats();
        s.sync_font3d();
        assert_eq!(revision, s.render_revision());
        assert_eq!(stats, s.font3d_stats());
    }
    #[test]
    fn font3d_import_cynatar_and_reject_corrupt_geometry_atomically() {
        let mut s = SceneManager::default();
        let source = include_str!("../assets/fonts/cynatar/glyphs.json");
        let f = s.register_font3d(source).unwrap();
        let info: Value = serde_json::from_str(&s.font3d_info(f).unwrap()).unwrap();
        assert_eq!(info["characters"].as_str().unwrap().chars().count(), 185);
        s.add_font3d_text(f, "CYNATAR 123", "{}").unwrap();
        let mut data: Value = serde_json::from_str(source).unwrap();
        data["glyphs"]["!"]["indices"][0] = json!(u32::MAX);
        let next = s.next_id;
        assert!(s.register_font3d(&data.to_string()).is_err());
        assert_eq!(s.next_id, next);
        assert_eq!(s.font3d.fonts.len(), 1);
        data["glyphs"]["!"]["indices"][0] = json!(0);
        data["glyphs"]["!"]["front"] = json!([]);
        assert!(s.register_font3d(&data.to_string()).is_err());
    }
}
