//! Opt-in world-space bitmap text, expressed as ordinary opaque triangles.
use crate::gltf_loader::{ImportedMesh, ImportedVertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Scene,
    Round,
    Restart,
    Diagnostics,
    Text,
    NumericText,
    DebugText,
    Countdown,
    Score,
    ComboAward,
}
impl PanelKind {
    pub fn neon(self) -> bool {
        matches!(
            self,
            Self::Countdown | Self::Score | Self::ComboAward | Self::NumericText
        )
    }
    pub fn parse(kind: &str) -> Result<Self, String> {
        match kind {
            "countdown" => Ok(Self::Countdown),
            "score" => Ok(Self::Score),
            "combo-award" => Ok(Self::ComboAward),
            "debug-text" => Ok(Self::DebugText),
            "scene" => Ok(Self::Scene),
            "round" => Ok(Self::Round),
            "restart" => Ok(Self::Restart),
            "diagnostics" => Ok(Self::Diagnostics),
            _ => Err("Info panel kind must be scene, round, restart, diagnostics, countdown, score, combo-award, or debug-text".into()),
        }
    }
}
pub struct InfoPanel {
    pub kind: PanelKind,
    pub width: f32,
    pub height: f32,
    pub mesh: ImportedMesh,
    pub dirty: bool,
    pub(crate) revision: u64,
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
            PanelKind::Countdown => vec![("", "30".into())],
            PanelKind::Score => vec![("", "0".into())],
            PanelKind::ComboAward => vec![("", "".into())],
            PanelKind::DebugText => vec![("", "".into())],
            PanelKind::Text | PanelKind::NumericText => {
                return Err("Use the static text constructor".into())
            }
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
            revision: 0,
            rows,
        })
    }
    pub fn numeric_text(value: &str, width: f32, height: f32) -> Result<Self, String> {
        if value.is_empty()
            || value.len() > 32
            || !value.bytes().all(|digit| digit.is_ascii_digit())
        {
            return Err("Numeric text requires 1..32 ASCII digits".into());
        }
        let mut panel = Self::new(PanelKind::Score, width, height)?;
        panel.kind = PanelKind::NumericText;
        panel.rows = vec![("", value.into())];
        panel.mesh = geometry(panel.kind, width, height, &panel.rows);
        Ok(panel)
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
        if matches!(self.kind, PanelKind::Text | PanelKind::NumericText) || rows == self.rows {
            return false;
        }
        self.mesh = geometry(self.kind, self.width, self.height, &rows);
        self.rows = rows;
        self.dirty = true;
        self.revision = self.revision.wrapping_add(1);
        true
    }
}
const GOLD: [f32; 3] = [1., 0.72, 0.28];
const CYAN: [f32; 3] = [0.50, 0.94, 1.];
const TIMER_BLUE: [f32; 3] = [0.015, 0.32, 1.];
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
    if matches!(
        kind,
        PanelKind::Countdown | PanelKind::Score | PanelKind::NumericText
    ) {
        return numeric_geometry(width, height, rows.first().map_or("", |row| row.1.as_str()));
    }
    let mut mesh = empty_mesh(width, height);
    if kind == PanelKind::DebugText {
        let value = rows.first().map_or("", |row| row.1.as_str());
        if !value.is_empty() {
            let unit = fit(value, width * 0.94, height * 0.90);
            text(
                &mut mesh,
                value,
                -text_width(value, unit) / 2.,
                3.5 * unit,
                unit,
                [0.55, 0.55, 0.55],
            );
        }
        return mesh;
    }
    if matches!(
        kind,
        PanelKind::Countdown | PanelKind::Score | PanelKind::ComboAward
    ) {
        let value = rows.first().map_or("", |row| row.1.as_str());
        let unit = fit(value, width * 0.94, height * 0.90);
        text(
            &mut mesh,
            value,
            -text_width(value, unit) / 2.,
            3.5 * unit,
            unit,
            if kind == PanelKind::ComboAward {
                [1., 0.35, 0.92]
            } else {
                TIMER_BLUE
            },
        );
        // Center actual lit pixels, including narrow digits and the leading plus.
        if !mesh.vertices.is_empty() {
            let min = mesh
                .vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::INFINITY, f32::min);
            let max = mesh
                .vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::NEG_INFINITY, f32::max);
            for vertex in &mut mesh.vertices {
                vertex.position[0] -= (min + max) * 0.5;
            }
        }
        return mesh;
    }
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
        PanelKind::Text
        | PanelKind::NumericText
        | PanelKind::Countdown
        | PanelKind::Score
        | PanelKind::ComboAward => "",
        PanelKind::DebugText => "",
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
    fn info_panel_empty_text_is_valid_and_can_become_visible_then_empty() {
        let static_empty = InfoPanel::text("", &[], 1., 0.2).unwrap();
        assert!(static_empty.mesh.vertices.is_empty() && static_empty.mesh.indices.is_empty());
        let mut award = InfoPanel::new(PanelKind::ComboAward, 1., 0.2).unwrap();
        assert!(award.mesh.vertices.is_empty() && award.mesh.indices.is_empty());
        assert!(award.update(vec![("", "+10".into())]));
        assert!(!award.mesh.indices.is_empty());
        assert!(award.update(vec![("", String::new())]));
        assert!(award.mesh.vertices.is_empty() && award.mesh.indices.is_empty());
        assert!(award
            .mesh
            .bounds_min
            .iter()
            .chain(&award.mesh.bounds_max)
            .all(|n| n.is_finite()));
    }
    #[test]
    fn debug_text_panel_is_empty_neutral_centered_and_cached_on_repeat() {
        let mut panel = InfoPanel::new(PanelKind::DebugText, 1.2, 0.3).unwrap();
        assert!(panel.mesh.vertices.is_empty());
        assert!(!panel.kind.neon());
        assert!(!panel.numeric());
        assert!(panel.update(vec![("", "DEBUG".into())]));
        assert!(!panel.mesh.vertices.is_empty());
        assert!(panel
            .mesh
            .vertices
            .iter()
            .all(|v| v.color == [0.55, 0.55, 0.55]));
        assert!(panel.mesh.vertices.iter().all(|v| v.position[2] == 0.003));
        let min_x = panel
            .mesh
            .vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = panel
            .mesh
            .vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!((min_x + max_x).abs() < 1e-6);
        assert!(panel
            .mesh
            .vertices
            .iter()
            .all(|v| v.position[0].abs() <= panel.width / 2.
                && v.position[1].abs() <= panel.height / 2.));
        let indices = panel.mesh.indices.clone();
        panel.dirty = false;
        assert!(!panel.update(vec![("", "DEBUG".into())]));
        assert_eq!(panel.mesh.indices, indices);
        assert!(!panel.dirty);
    }
    #[test]
    fn info_panel_neon_numbers_are_centered_background_free_and_content_cached() {
        assert!(crate::debug_overlay::glyph('+')
            .iter()
            .any(|bits| *bits != 0));
        for kind in [
            PanelKind::Countdown,
            PanelKind::Score,
            PanelKind::ComboAward,
        ] {
            let mut panel = InfoPanel::new(kind, 1.2, 0.3).unwrap();
            let value = if kind == PanelKind::ComboAward {
                "+100"
            } else {
                "100"
            };
            panel.update(vec![("", value.into())]);
            if kind == PanelKind::ComboAward {
                assert!(panel.mesh.vertices.iter().all(|v| v.position[2] == 0.003));
            } else {
                assert!(panel.mesh.bounds_max[2] - panel.mesh.bounds_min[2] > 0.01);
                assert!(panel
                    .mesh
                    .vertices
                    .iter()
                    .all(|vertex| vertex.color == TIMER_BLUE));
                assert!(panel
                    .mesh
                    .vertices
                    .iter()
                    .any(|v| v.normal[2] > 0.1 && v.normal[2] < 0.9));
            }
            let min = panel
                .mesh
                .vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::INFINITY, f32::min);
            let max = panel
                .mesh
                .vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::NEG_INFINITY, f32::max);
            assert!((min + max).abs() < 1e-6);
            panel.dirty = false;
            assert!(!panel.update(vec![("", value.into())]));
            assert!(!panel.dirty);
            assert!(panel.update(vec![("", "200".into())]));
            assert!(panel.dirty);
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

pub(crate) const TUBE_RADIUS: f32 = 0.58;
// Extended sci-fi proportions; spacing and fitting use these actual wide bounds.
const GLYPH_ADVANCE: f32 = 9.4;
const TUBE_SIDES: usize = 24;
/// Technical stencil numerals: straight runs, chamfered corners and tube breaks.
/// Continuous round tubes follow exact circular fillets joined tangent to straight runs.
struct NumericStroke {
    points: Vec<glam::Vec2>,
    tangents: Vec<glam::Vec2>,
}
fn digit_curves(digit: u8) -> Vec<Vec<glam::Vec2>> {
    digit_paths(digit)
        .into_iter()
        .map(|stroke| stroke.points)
        .collect()
}
fn digit_paths(digit: u8) -> Vec<NumericStroke> {
    use glam::Vec2;
    let strokes: Vec<Vec<[f32; 2]>> = match digit {
        0 => vec![
            vec![
                [2.1, 2.8],
                [1.3, 2.8],
                [0.6, 2.1],
                [0.6, -2.1],
                [1.3, -2.8],
                [2.1, -2.8],
            ],
            vec![
                [2.9, -2.8],
                [3.7, -2.8],
                [4.4, -2.1],
                [4.4, 2.1],
                [3.7, 2.8],
                [2.9, 2.8],
            ],
        ],
        1 => vec![
            vec![[1., 1.8], [1.6, 2.2]],
            vec![[2.5, 2.8], [2.5, -1.5]],
            vec![[1.1, -2.8], [3.9, -2.8]],
        ],
        2 => vec![
            vec![
                [0.6, 2.1],
                [1.3, 2.8],
                [3.7, 2.8],
                [4.4, 2.1],
                [4.4, 1.2],
                [2.9, 0.],
            ],
            vec![
                [1.9, -0.8],
                [0.6, -1.1],
                [0.6, -2.1],
                [1.3, -2.8],
                [4.4, -2.8],
            ],
        ],
        3 => vec![
            vec![[0.7, 2.8], [3.7, 2.8], [4.4, 2.1], [4.4, 0.9]],
            vec![[1.3, 0.], [3.7, 0.]],
            vec![[4.4, -0.9], [4.4, -2.1], [3.7, -2.8], [0.7, -2.8]],
        ],
        4 => vec![
            vec![[1.2, 2.8], [0.6, 1.3], [0.6, 0.7], [1.3, 0.], [2.6, 0.]],
            vec![[4., 2.8], [4., -2.8]],
        ],
        5 => vec![
            vec![[4.4, 2.8], [0.6, 2.8], [0.6, 0.9], [1.3, 0.], [2.1, 0.]],
            vec![
                [2.9, 0.],
                [3.7, 0.],
                [4.4, -0.7],
                [4.4, -2.1],
                [3.7, -2.8],
                [0.6, -2.8],
            ],
        ],
        6 => vec![vec![
            [4.3, 2.8],
            [1.3, 2.8],
            [0.6, 2.1],
            [0.6, -2.1],
            [1.3, -2.8],
            [3.7, -2.8],
            [4.4, -2.1],
            [4.4, -0.7],
            [3.7, 0.],
            [1.9, 0.],
        ]],
        7 => vec![
            vec![[0.6, 2.8], [3.7, 2.8], [4.4, 2.1], [4.4, 1.3], [3.3, 0.5]],
            vec![[2.5, -0.5], [1.3, -2.8]],
        ],
        8 => vec![
            vec![
                [1.6, 0.65],
                [0.6, 1.1],
                [0.6, 2.1],
                [1.3, 2.8],
                [3.7, 2.8],
                [4.4, 2.1],
                [4.4, 1.1],
                [3.4, 0.65],
            ],
            vec![
                [1.6, -0.65],
                [0.6, -1.1],
                [0.6, -2.1],
                [1.3, -2.8],
                [3.7, -2.8],
                [4.4, -2.1],
                [4.4, -1.1],
                [3.4, -0.65],
            ],
        ],
        _ => vec![vec![
            [0.7, -2.8],
            [3.7, -2.8],
            [4.4, -2.1],
            [4.4, 2.1],
            [3.7, 2.8],
            [1.3, 2.8],
            [0.6, 2.1],
            [0.6, 0.7],
            [1.3, 0.],
            [3.1, 0.],
        ]],
    };
    strokes
        .into_iter()
        .map(|stroke| {
            use std::f32::consts::PI;
            // Proportions and slant affect the axis; the swept cross section stays circular.
            let points: Vec<_> = stroke
                .into_iter()
                .map(|[x, y]| Vec2::new((x - 2.5) * 1.55 + 2.5 + 0.14 * y, y))
                .collect();
            let mut turns = vec![0.; points.len()];
            let mut tan_half = vec![0.; points.len()];
            for i in 1..points.len() - 1 {
                let incoming = (points[i] - points[i - 1]).normalize();
                let outgoing = (points[i + 1] - points[i]).normalize();
                turns[i] = incoming.perp_dot(outgoing).atan2(incoming.dot(outgoing));
                tan_half[i] = (turns[i].abs() * 0.5).tan();
            }
            // Adjacent fillets share each finite edge; leave a real straight span.
            let mut max_radius = f32::INFINITY;
            for i in 0..points.len() - 1 {
                let demand = tan_half[i] + tan_half[i + 1];
                if demand > 1e-6 {
                    max_radius = max_radius.min(points[i].distance(points[i + 1]) * 0.98 / demand);
                }
            }
            let radius = 0.68_f32.min(max_radius);
            assert!(
                radius > TUBE_RADIUS + 0.01,
                "digit {digit} needs longer stencil edges for safe circular bends: {radius}"
            );
            let mut result = vec![points[0]];
            let mut tangents = vec![(points[1] - points[0]).normalize()];
            let line = |result: &mut Vec<Vec2>, tangents: &mut Vec<Vec2>, end: Vec2| {
                let start = *result.last().unwrap();
                if start.distance(end) < 1e-7 {
                    return;
                }
                let direction = (end - start).normalize();
                let count = (start.distance(end) / 0.16).ceil().max(1.) as usize;
                for step in 1..=count {
                    result.push(start.lerp(end, step as f32 / count as f32));
                    tangents.push(direction);
                }
            };
            for i in 1..points.len() - 1 {
                let incoming = (points[i] - points[i - 1]).normalize();
                let outgoing = (points[i + 1] - points[i]).normalize();
                if turns[i].abs() < 1e-6 {
                    line(&mut result, &mut tangents, points[i]);
                    continue;
                }
                let trim = radius * tan_half[i];
                let start = points[i] - incoming * trim;
                let end = points[i] + outgoing * trim;
                let center = start + incoming.perp() * turns[i].signum() * radius;
                let initial_angle = (start - center).to_angle();
                line(&mut result, &mut tangents, start);
                let count = (turns[i].abs() / (PI / 48.)).ceil().max(4.) as usize;
                for step in 1..=count {
                    let angle = initial_angle + turns[i] * step as f32 / count as f32;
                    result.push(if step == count {
                        end
                    } else {
                        center + Vec2::from_angle(angle) * radius
                    });
                    tangents.push(if step == count {
                        outgoing
                    } else {
                        Vec2::from_angle(angle).perp() * turns[i].signum()
                    });
                }
            }
            line(&mut result, &mut tangents, *points.last().unwrap());
            NumericStroke {
                points: result,
                tangents,
            }
        })
        .collect()
}

fn bound_mesh(mesh: &mut ImportedMesh) {
    for axis in 0..3 {
        mesh.bounds_min[axis] = mesh
            .vertices
            .iter()
            .map(|v| v.position[axis])
            .fold(f32::INFINITY, f32::min);
        mesh.bounds_max[axis] = mesh
            .vertices
            .iter()
            .map(|v| v.position[axis])
            .fold(f32::NEG_INFINITY, f32::max);
    }
}
fn tube_triangle(mesh: &mut ImportedMesh, vertices: [&ImportedVertex; 3]) {
    use glam::Vec3;
    let [a, b, c] = vertices.map(|v| Vec3::from_array(v.position));
    if (b - a).cross(c - a).length_squared() < 1e-14 {
        return;
    }
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend(vertices.into_iter().cloned());
    mesh.indices.extend([base, base + 1, base + 2]);
}
fn build_numeric_glyph(digit: u8) -> ImportedMesh {
    use glam::{Vec2, Vec3};
    use std::f32::consts::{FRAC_PI_2, TAU};
    let mut mesh = empty_mesh(5., 7.);
    for stroke in digit_paths(digit) {
        let points = stroke.points;
        let closed = points.first().unwrap().distance(*points.last().unwrap()) < 1e-5;
        let last = points.len() - 1;
        let tangent = |i: usize| stroke.tangents[i];
        let mut rings = Vec::new();
        let mut ring = |center: Vec2, dir: Vec2, cap: f32| {
            let side = Vec3::new(-dir.y, dir.x, 0.);
            let dir = dir.extend(0.);
            let mut vertices = Vec::with_capacity(TUBE_SIDES + 1);
            for j in 0..=TUBE_SIDES {
                let theta = (j % TUBE_SIDES) as f32 / TUBE_SIDES as f32 * TAU;
                let radial = side * theta.cos() + Vec3::Z * theta.sin();
                // Exact shared seams/poles keep every capsule watertight.
                let (cap_sin, cap_cos) = if cap.abs() == FRAC_PI_2 {
                    (cap.signum(), 0.)
                } else {
                    cap.sin_cos()
                };
                let normal = radial * cap_cos + dir * cap_sin;
                let mut position = center.extend(0.) + normal * TUBE_RADIUS;
                position.z = position.z / (2. * TUBE_RADIUS) - 0.5;
                let normal = Vec3::new(normal.x, normal.y, normal.z * 2. * TUBE_RADIUS).normalize();
                vertices.push(ImportedVertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    // Internal numeric GPU payload: unit capsule axis packed as RGB.
                    color: (dir * 0.5 + Vec3::splat(0.5)).to_array(),
                });
            }
            rings.push(vertices);
        };
        if !closed {
            for i in 0..6 {
                ring(
                    points[0],
                    tangent(0),
                    -FRAC_PI_2 + (i as f32 / 6.) * FRAC_PI_2,
                );
            }
        }
        for (i, &point) in points.iter().enumerate() {
            ring(point, tangent(i), 0.);
        }
        if !closed {
            for i in 1..=6 {
                ring(points[last], tangent(last), (i as f32 / 6.) * FRAC_PI_2);
            }
        }
        for pair in rings.windows(2) {
            for j in 0..TUBE_SIDES {
                // Tangent x radial points outward for this winding.
                tube_triangle(&mut mesh, [&pair[0][j], &pair[1][j + 1], &pair[1][j]]);
                tube_triangle(&mut mesh, [&pair[0][j], &pair[0][j + 1], &pair[1][j + 1]]);
            }
        }
    }
    bound_mesh(&mut mesh);
    mesh
}
/// Canonical curved tube assets; cloned CPU diagnostics share exactly the same shape.
pub(crate) fn numeric_glyph(digit: u8) -> ImportedMesh {
    numeric_glyph_ref(digit).clone()
}
fn numeric_glyph_ref(digit: u8) -> &'static ImportedMesh {
    static GLYPHS: std::sync::OnceLock<[ImportedMesh; 10]> = std::sync::OnceLock::new();
    &GLYPHS.get_or_init(|| std::array::from_fn(|d| build_numeric_glyph(d as u8)))[digit as usize]
}
/// Narrow blue axis ribbons feed one soft halo through the existing cached pass.
#[cfg_attr(not(any(test, target_arch = "wasm32")), allow(dead_code))]
pub(crate) fn numeric_glyph_glow(digit: u8) -> ImportedMesh {
    let mut mesh = empty_mesh(5., 7.);
    for points in digit_curves(digit) {
        for pair in points.windows(2) {
            let side = (pair[1] - pair[0]).normalize().perp() * 0.0001;
            let base = mesh.vertices.len() as u32;
            for point in [
                pair[0] - side,
                pair[1] - side,
                pair[1] + side,
                pair[0] + side,
            ] {
                mesh.vertices.push(ImportedVertex {
                    position: [point.x, point.y, 0.001],
                    normal: [0., 0., 1.],
                    color: TIMER_BLUE,
                });
            }
            mesh.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    bound_mesh(&mut mesh);
    mesh
}
fn numeric_layout(value: &str, width: f32, height: f32) -> Vec<(u8, f32, f32, f32)> {
    if value.is_empty() || !value.bytes().all(|c| c.is_ascii_digit()) {
        return Vec::new();
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for (i, d) in value.bytes().enumerate() {
        let mesh = numeric_glyph_ref(d - b'0');
        min = min.min(i as f32 * GLYPH_ADVANCE + mesh.bounds_min[0]);
        max = max.max(i as f32 * GLYPH_ADVANCE + mesh.bounds_max[0]);
    }
    let unit = (width * 0.94 / (max - min)).min(height * 0.90 / 7.);
    let center = (min + max) * 0.5;
    value
        .bytes()
        .enumerate()
        .map(|(i, d)| {
            (
                d - b'0',
                (i as f32 * GLYPH_ADVANCE - center) * unit,
                unit,
                2. * TUBE_RADIUS * unit,
            )
        })
        .collect()
}
fn numeric_geometry(width: f32, height: f32, value: &str) -> ImportedMesh {
    let mut mesh = empty_mesh(width, height);
    for (digit, x, unit, depth) in numeric_layout(value, width, height) {
        let glyph = numeric_glyph(digit);
        let base = mesh.vertices.len() as u32;
        mesh.indices.extend(glyph.indices.iter().map(|i| i + base));
        for mut v in glyph.vertices {
            // Public CPU diagnostics keep the visible palette; the canonical
            // render asset uses RGB only to carry its per-capsule axis.
            v.color = TIMER_BLUE;
            v.position = [
                v.position[0] * unit + x,
                v.position[1] * unit,
                v.position[2] * depth + 0.003,
            ];
            v.normal = glam::Vec3::new(v.normal[0] / unit, v.normal[1] / unit, v.normal[2] / depth)
                .normalize()
                .to_array();
            mesh.vertices.push(v);
        }
    }
    if !mesh.vertices.is_empty() {
        bound_mesh(&mut mesh);
    }
    mesh
}
impl InfoPanel {
    pub(crate) fn numeric(&self) -> bool {
        matches!(
            self.kind,
            PanelKind::Countdown | PanelKind::Score | PanelKind::NumericText
        )
    }
    pub(crate) fn numeric_layout(&self) -> Vec<(u8, f32, f32, f32)> {
        numeric_layout(
            self.rows.first().map_or("", |row| row.1.as_str()),
            self.width,
            self.height,
        )
    }
}

#[cfg(test)]
mod numeric_tube_tests {
    use super::*;
    use glam::Vec3;
    #[test]
    fn info_panel_numeric_tubes_preserve_axis_payload_and_immutable_cache() {
        for digit in 0..10 {
            let canonical = numeric_glyph_ref(digit);
            assert!(std::ptr::eq(canonical, numeric_glyph_ref(digit)));
            let mut clone = numeric_glyph(digit);
            let mut varying_axis = false;
            for triangle in clone.indices.as_chunks::<3>().0 {
                let axes = triangle.map(|index| {
                    Vec3::from_array(clone.vertices[index as usize].color) * 2. - Vec3::ONE
                });
                assert!(axes
                    .iter()
                    .all(|axis| (axis.length() - 1.).abs() < 1e-5 && axis.z.abs() < 1e-6));
                varying_axis |= !axes[0].abs_diff_eq(axes[1], 1e-5);
            }
            assert!(
                digit == 1 || varying_axis,
                "digit {digit} lost its smooth bend axis gradient"
            );
            let diagnostics = numeric_geometry(4., 0.7, &char::from(b'0' + digit).to_string());
            assert!(diagnostics
                .vertices
                .iter()
                .all(|vertex| vertex.color == TIMER_BLUE));
            let original = canonical.vertices[0].color;
            clone.vertices[0].color = [0.; 3];
            assert_eq!(numeric_glyph_ref(digit).vertices[0].color, original);
            assert_eq!(clone.bounds_min, canonical.bounds_min);
            assert_eq!(clone.bounds_max, canonical.bounds_max);
            assert_eq!(clone.indices, canonical.indices);
        }
        assert!(digit_curves(0).iter().all(|stroke| stroke.len() > 24));
    }
    #[test]
    fn info_panel_separate_stencil_strokes_have_real_surface_gaps() {
        use glam::Vec2;
        let point_gap = |p: Vec2, a: Vec2, b: Vec2| {
            let axis = b - a;
            p.distance(a + axis * ((p - a).dot(axis) / axis.length_squared()).clamp(0., 1.))
        };
        for digit in 0..10 {
            let strokes = digit_curves(digit);
            for i in 0..strokes.len() {
                for j in i + 1..strokes.len() {
                    let mut gap = f32::INFINITY;
                    for a in strokes[i].windows(2) {
                        for b in strokes[j].windows(2) {
                            let cross = (a[1] - a[0]).perp_dot(b[1] - b[0]);
                            if cross.abs() > 1e-7 {
                                let t = (b[0] - a[0]).perp_dot(b[1] - b[0]) / cross;
                                let u = (b[0] - a[0]).perp_dot(a[1] - a[0]) / cross;
                                assert!(
                                    !((0. ..=1.).contains(&t) && (0. ..=1.).contains(&u)),
                                    "digit {digit} has crossing stencil strokes"
                                );
                            }
                            gap = gap
                                .min(point_gap(a[0], b[0], b[1]))
                                .min(point_gap(a[1], b[0], b[1]))
                                .min(point_gap(b[0], a[0], a[1]))
                                .min(point_gap(b[1], a[0], a[1]));
                        }
                    }
                    assert!(
                        gap > 2. * TUBE_RADIUS + 0.01,
                        "digit {digit}: capped strokes overlap at axis gap {gap}"
                    );
                }
            }
        }
    }
    #[test]
    fn info_panel_numeric_capsules_are_closed_with_middle_diameter_and_axis_halo() {
        for digit in 0..10 {
            let mesh = numeric_glyph_ref(digit);
            let capsule = &digit_curves(digit)[0];
            let side = (capsule[1] - capsule[0]).normalize().perp();
            let ends = [capsule[0] + side * 0.58, capsule[0] - side * 0.58];
            for point in ends {
                assert!(mesh
                    .vertices
                    .iter()
                    .any(|v| Vec3::from_array(v.position).distance(point.extend(-0.5)) < 1e-5));
            }
            assert!((ends[0].distance(ends[1]) - 1.16).abs() < 1e-5);
            let halo = numeric_glyph_glow(digit);
            for (points, source) in halo.vertices.as_chunks::<4>().0.iter().zip(
                digit_curves(digit)
                    .iter()
                    .flat_map(|stroke| stroke.windows(2)),
            ) {
                let axis = source[1] - source[0];
                for vertex in points {
                    let point = glam::Vec2::new(vertex.position[0], vertex.position[1]);
                    let t = (point - source[0]).dot(axis) / axis.length_squared();
                    assert!((-1e-5..=1. + 1e-5).contains(&t));
                    assert!((point.distance(source[0] + axis * t) - 0.0001).abs() < 1e-5);
                }
            }
            for triangle in halo.indices.as_chunks::<3>().0 {
                let [a, b, c] =
                    triangle.map(|index| Vec3::from_array(halo.vertices[index as usize].position));
                assert!((b - a).cross(c - a).z > 0.);
            }
            let mut edges = std::collections::BTreeMap::<[[u32; 3]; 2], usize>::new();
            for triangle in mesh.indices.as_chunks::<3>().0 {
                let points = triangle.map(|index| {
                    mesh.vertices[index as usize].position.map(|v| {
                        if v == 0. {
                            0
                        } else {
                            v.to_bits()
                        }
                    })
                });
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let mut edge = [points[a], points[b]];
                    edge.sort_unstable();
                    *edges.entry(edge).or_default() += 1;
                }
            }
            assert!(
                edges.values().all(|count| count % 2 == 0),
                "open capsule for {digit}"
            );
            let panel = numeric_geometry(4., 0.7, &char::from(b'0' + digit).to_string());
            assert!(panel.bounds_max[0] - panel.bounds_min[0] <= 4. * 0.94 + 1e-5);
            assert!(panel.bounds_max[1] - panel.bounds_min[1] <= 0.7 + 1e-5);
        }
    }
    #[test]
    fn info_panel_extended_numbers_fit_long_values_without_touching() {
        let zero = numeric_glyph_ref(0);
        assert!(zero.bounds_max[0] - zero.bounds_min[0] > 6.5);
        for value in ["30", "120", "12345678", "888888888888"] {
            let layout = numeric_layout(value, 1.55, 0.25);
            let mesh = numeric_geometry(1.55, 0.25, value);
            assert!(mesh.bounds_max[0] - mesh.bounds_min[0] <= 1.55 * 0.94 + 1e-5);
            for pair in layout.windows(2) {
                let a = numeric_glyph_ref(pair[0].0);
                let b = numeric_glyph_ref(pair[1].0);
                assert!(
                    pair[0].1 + a.bounds_max[0] * pair[0].2
                        < pair[1].1 + b.bounds_min[0] * pair[1].2
                );
            }
        }
    }
    #[test]
    fn info_panel_numeric_tubes_have_smooth_round_surfaces_and_blue_axis_halos() {
        for digit in 0..10 {
            let mesh = numeric_glyph(digit);
            assert!(mesh.vertices.len() > 1000);
            assert!((mesh.bounds_max[2] - mesh.bounds_min[2] - 1.).abs() < 1e-5);
            assert!(mesh.vertices.iter().all(|vertex| {
                let axis = Vec3::from_array(vertex.color) * 2. - Vec3::ONE;
                axis.is_finite() && (axis.length() - 1.).abs() < 1e-5
            }));
            for tri in mesh.indices.as_chunks::<3>().0 {
                let [a, b, c] = tri.map(|i| Vec3::from_array(mesh.vertices[i as usize].position));
                let n = tri
                    .map(|i| Vec3::from_array(mesh.vertices[i as usize].normal))
                    .into_iter()
                    .sum::<Vec3>();
                assert!(a.is_finite() && b.is_finite() && c.is_finite());
                assert!(
                    (b - a).cross(c - a).dot(n) > 0.,
                    "inward triangle for {digit}"
                );
            }
            for stroke in digit_curves(digit) {
                for points in stroke.windows(3) {
                    let a = points[0].distance(points[1]);
                    let b = points[1].distance(points[2]);
                    let c = points[0].distance(points[2]);
                    let twice_area = (points[1] - points[0])
                        .perp_dot(points[2] - points[0])
                        .abs();
                    if twice_area > 1e-7 {
                        let radius = a * b * c / (2. * twice_area);
                        assert!(
                            radius > TUBE_RADIUS,
                            "digit {digit} has unsafe bend radius {radius}"
                        );
                    }
                }
            }
            let halo = numeric_glyph_glow(digit);
            assert!(halo.vertices.iter().all(|v| v.color == TIMER_BLUE));
            // One font unit scales XY; normalized depth must restore the same diameter.
            let layout = numeric_layout("0", 1.2, 0.3);
            assert!((layout[0].3 / layout[0].2 - 1.16).abs() < 1e-6);
        }
    }
}
