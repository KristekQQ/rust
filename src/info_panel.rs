//! Opt-in world-space bitmap text, expressed as ordinary opaque triangles.
use crate::gltf_loader::{ImportedMesh, ImportedVertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Scene,
    Round,
    Restart,
    Diagnostics,
    Text,
}
impl PanelKind {
    pub fn parse(kind: &str) -> Result<Self, String> {
        match kind {
            "scene" => Ok(Self::Scene),
            "round" => Ok(Self::Round),
            "restart" => Ok(Self::Restart),
            "diagnostics" => Ok(Self::Diagnostics),
            _ => Err("Info panel kind must be scene, round, restart, or diagnostics".into()),
        }
    }
}
pub struct InfoPanel {
    pub kind: PanelKind,
    pub width: f32,
    pub height: f32,
    pub mesh: ImportedMesh,
    pub dirty: bool,
    rows: Vec<(&'static str, String)>,
}
impl InfoPanel {
    pub fn new(kind: PanelKind, width: f32, height: f32) -> Result<Self, String> {
        if !width.is_finite()
            || !height.is_finite()
            || !(0.01..=100.).contains(&width)
            || !(0.01..=100.).contains(&height)
        {
            return Err("Panel dimensions must be finite meters in 0.01..100".into());
        }
        let rows = match kind {
            PanelKind::Text => return Err("Use the text panel constructor".into()),
            PanelKind::Diagnostics => vec![
                ("FPS", "0.0".into()),
                ("FRAME MS", "0.00".into()),
                ("CPU MS", "0.00".into()),
                ("CULL MS", "0.00".into()),
                ("VISIBLE", "0".into()),
                ("CULLED", "0".into()),
                ("OBJECTS", "0".into()),
                ("DRAWS", "0".into()),
            ],
            PanelKind::Scene => vec![
                ("FPS", "0.0".into()),
                ("FRAME MS", "0.00".into()),
                ("CPU MS", "0.00".into()),
                ("OBJEKTY", "0".into()),
                ("DRAWS", "0".into()),
            ],
            PanelKind::Round => vec![("ROZKROJENO", "0".into())],
            PanelKind::Restart => vec![("MECEM", "RESTART".into())],
        };
        Ok(Self {
            kind,
            width,
            height,
            mesh: geometry(kind, width, height, &rows),
            dirty: true,
            rows,
        })
    }
    pub fn text(title: &str, lines: &[String], width: f32, height: f32) -> Result<Self, String> {
        let mut panel = Self::new(PanelKind::Restart, width, height)?;
        if title.chars().count() > 48
            || lines.len() > 12
            || lines.iter().any(|line| line.chars().count() > 64)
            || title
                .chars()
                .chain(lines.iter().flat_map(|line| line.chars()))
                .any(char::is_control)
        {
            return Err("Text panels allow a 48 character title and 12 lines of 64 characters without control characters".into());
        }
        panel.kind = PanelKind::Text;
        panel.mesh = text_geometry(title, lines, width, height);
        Ok(panel)
    }
    pub fn update(&mut self, rows: Vec<(&'static str, String)>) -> bool {
        if self.kind == PanelKind::Text || rows == self.rows {
            return false;
        }
        self.mesh = geometry(self.kind, self.width, self.height, &rows);
        self.rows = rows;
        self.dirty = true;
        true
    }
}
const GOLD: [f32; 3] = [1., 0.72, 0.28];
const CYAN: [f32; 3] = [0.50, 0.94, 1.];
fn empty_mesh(width: f32, height: f32) -> ImportedMesh {
    ImportedMesh {
        vertices: vec![],
        indices: vec![],
        bounds_min: [-width / 2., -height / 2., 0.],
        bounds_max: [width / 2., height / 2., 0.004],
        warnings: vec![],
    }
}
fn quad(mesh: &mut ImportedMesh, x: f32, y: f32, w: f32, h: f32, z: f32, color: [f32; 3]) {
    let base = mesh.vertices.len() as u32;
    for position in [[x, y - h, z], [x + w, y - h, z], [x + w, y, z], [x, y, z]] {
        mesh.vertices.push(ImportedVertex {
            position,
            normal: [0., 0., 1.],
            color,
        });
    }
    mesh.indices
        .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
fn text_width(text: &str, unit: f32) -> f32 {
    text.chars().count().saturating_mul(6).saturating_sub(1) as f32 * unit
}
/// Merge contiguous bitmap pixels on each scanline into one rectangle.
fn text(mesh: &mut ImportedMesh, value: &str, x: f32, y: f32, unit: f32, color: [f32; 3]) {
    for (column, ch) in value.chars().enumerate() {
        for (gy, bits) in crate::debug_overlay::glyph(ch.to_ascii_uppercase())
            .iter()
            .enumerate()
        {
            let mut gx = 0;
            while gx < 5 {
                if bits & (1 << (4 - gx)) == 0 {
                    gx += 1;
                    continue;
                }
                let start = gx;
                while gx < 5 && bits & (1 << (4 - gx)) != 0 {
                    gx += 1;
                }
                quad(
                    mesh,
                    x + (column * 6 + start) as f32 * unit,
                    y - gy as f32 * unit,
                    (gx - start) as f32 * unit,
                    unit,
                    0.003,
                    color,
                );
            }
        }
    }
}
fn fit(text: &str, width: f32, height: f32) -> f32 {
    (width / (text.chars().count().max(1) * 6) as f32).min(height / 7.)
}
fn text_geometry(title: &str, lines: &[String], width: f32, height: f32) -> ImportedMesh {
    let mut mesh = empty_mesh(width, height);
    let available = width * 0.86;
    let top = height * 0.42;
    let title_unit = fit(title, available, height * 0.25);
    text(
        &mut mesh,
        title,
        -text_width(title, title_unit) / 2.,
        top,
        title_unit,
        GOLD,
    );
    let row_height = height * 0.52 / lines.len().max(1) as f32;
    let line_unit = lines
        .iter()
        .map(|line| fit(line, available, row_height * 0.65))
        .fold(row_height * 0.65 / 7., f32::min);
    for (row, line) in lines.iter().enumerate() {
        text(
            &mut mesh,
            line,
            -text_width(line, line_unit) / 2.,
            top - height * 0.37 - row as f32 * row_height,
            line_unit,
            CYAN,
        );
    }
    mesh
}
/// Centered local XY plane, with padded aligned columns and larger score type.
pub fn geometry(kind: PanelKind, width: f32, height: f32, rows: &[(&str, String)]) -> ImportedMesh {
    let mut mesh = empty_mesh(width, height);
    let left = -width * 0.43;
    let top = height * 0.42;
    quad(
        &mut mesh,
        -width / 2.,
        height / 2.,
        width,
        height,
        0.,
        [0.015, 0.025, 0.04],
    );
    let title = match kind {
        PanelKind::Scene => "SCENA",
        PanelKind::Round => "SKORE",
        PanelKind::Restart => "DALSI KOLO",
        PanelKind::Diagnostics => "VYKON",
        PanelKind::Text => "",
    };
    let title_unit = fit(title, width * 0.86, height * 0.15);
    text(&mut mesh, title, left, top, title_unit, GOLD);
    quad(
        &mut mesh,
        left,
        top - height * 0.21,
        width * 0.86,
        height * 0.006,
        0.001,
        GOLD,
    );
    let large_score = kind == PanelKind::Round && !rows.is_empty();
    if large_score {
        let value = &rows[0].1;
        let unit = fit(value, width * 0.86, height * 0.28);
        text(
            &mut mesh,
            value,
            width * 0.43 - text_width(value, unit),
            top - height * 0.27,
            unit,
            CYAN,
        );
    }
    let start = usize::from(large_score);
    let remaining = &rows[start..];
    let area = if large_score { 0.30 } else { 0.62 };
    let row_height = height * area / remaining.len().max(1) as f32;
    let max_columns = remaining
        .iter()
        .map(|(label, value)| label.len() + value.chars().count() + 3)
        .max()
        .unwrap_or(1);
    let unit = (width * 0.86 / (max_columns * 6) as f32).min(row_height * 0.70 / 7.);
    for (row, (label, value)) in remaining.iter().enumerate() {
        let y = top - height * (if large_score { 0.62 } else { 0.28 }) - row as f32 * row_height;
        text(&mut mesh, label, left, y, unit, GOLD);
        text(
            &mut mesh,
            value,
            width * 0.43 - text_width(value, unit),
            y,
            unit,
            CYAN,
        );
    }
    mesh
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn info_panel_geometry_is_finite_readable_and_bounded() {
        let panel = InfoPanel::new(PanelKind::Scene, 1.2, 0.72).unwrap();
        let mesh = &panel.mesh;
        assert!(mesh.vertices.len() > 100);
        assert_eq!(mesh.indices.len() % 6, 0);
        for v in &mesh.vertices {
            assert!(v.position.iter().all(|n| n.is_finite()));
            for (i, p) in v.position.iter().enumerate() {
                assert!(*p >= mesh.bounds_min[i] - 1e-6 && *p <= mesh.bounds_max[i] + 1e-6);
            }
            assert_eq!(v.normal, [0., 0., 1.]);
        }
        assert!(mesh.vertices.iter().any(|v| v.color == CYAN));
        for ch in "SCENAFPS0123456789".chars() {
            assert!(crate::debug_overlay::glyph(ch).iter().any(|b| *b != 0));
        }
        for tri in mesh.indices.as_chunks::<3>().0 {
            let a = glam::Vec3::from_array(mesh.vertices[tri[0] as usize].position);
            let b = glam::Vec3::from_array(mesh.vertices[tri[1] as usize].position);
            let c = glam::Vec3::from_array(mesh.vertices[tri[2] as usize].position);
            assert!((b - a).cross(c - a).z > 0.);
        }
    }
    #[test]
    fn info_panel_bitmap_runs_preserve_pixels_with_fewer_triangles() {
        let mut mesh = empty_mesh(1., 1.);
        text(&mut mesh, "E", 0., 0., 0.01, GOLD);
        let pixels = crate::debug_overlay::glyph('E')
            .iter()
            .map(|bits| bits.count_ones())
            .sum::<u32>();
        let area: f32 = mesh
            .vertices
            .as_chunks::<4>()
            .0
            .iter()
            .map(|v| (v[1].position[0] - v[0].position[0]) * (v[3].position[1] - v[0].position[1]))
            .sum();
        assert!((area - pixels as f32 * 0.0001).abs() < 1e-6);
        assert!(mesh.indices.len() < pixels as usize * 6);
    }
    #[test]
    fn info_panel_text_is_static_bounded_and_has_no_background() {
        for (width, height) in [(1.6, 0.7), (0.01, 100.), (100., 0.01)] {
            let mut panel = InfoPanel::text(
                "CODEX",
                &["POSTAVENO SPOLECNE".into(), "S HONZOU".into()],
                width,
                height,
            )
            .unwrap();
            assert_eq!(panel.kind, PanelKind::Text);
            assert!(panel.mesh.vertices.iter().all(|v| v.position[2] == 0.003));
            for v in &panel.mesh.vertices {
                for axis in 0..3 {
                    assert!(v.position[axis] >= panel.mesh.bounds_min[axis] - 1e-6);
                    assert!(v.position[axis] <= panel.mesh.bounds_max[axis] + 1e-6);
                }
            }
            panel.dirty = false;
            assert!(!panel.update(vec![("FPS", "90".into())]));
            assert!(!panel.dirty);
        }
        assert!(InfoPanel::text(&"A".repeat(49), &[], 1., 1.).is_err());
        assert!(InfoPanel::text("OK", &vec!["X".into(); 13], 1., 1.).is_err());
        assert!(InfoPanel::text("OK", &["A".repeat(65)], 1., 1.).is_err());
        assert!(InfoPanel::text("BAD\nTITLE", &[], 1., 1.).is_err());
        assert!(InfoPanel::text("OK", &[], f32::INFINITY, 1.).is_err());
        assert!(InfoPanel::text("OK", &[], 1., 0.).is_err());
        assert!(PanelKind::parse("text").is_err());
        let diagnostics = InfoPanel::new(PanelKind::Diagnostics, 1.2, 0.72).unwrap();
        assert_eq!(
            diagnostics.rows.iter().map(|r| r.0).collect::<Vec<_>>(),
            ["FPS", "FRAME MS", "CPU MS", "CULL MS", "VISIBLE", "CULLED", "OBJECTS", "DRAWS"]
        );
    }
    #[test]
    fn info_panel_updates_only_changed_content() {
        let mut panel = InfoPanel::new(PanelKind::Round, 1., 0.5).unwrap();
        let before = panel.mesh.indices.len();
        panel.dirty = false;
        assert!(!panel.update(vec![("ROZKROJENO", "0".into())]));
        assert!(!panel.dirty);
        assert!(panel.update(vec![("ROZKROJENO", "88888".into())]));
        assert!(panel.dirty);
        assert_ne!(before, panel.mesh.indices.len());
        assert!(InfoPanel::new(PanelKind::Scene, f32::NAN, 1.).is_err());
        assert!(PanelKind::parse("hud").is_err());
    }
}
