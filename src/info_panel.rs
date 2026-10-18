//! Opt-in world-space bitmap text, expressed as ordinary opaque triangles.
use crate::gltf_loader::{ImportedMesh, ImportedVertex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Scene,
    Round,
    Restart,
}
impl PanelKind {
    pub fn parse(kind: &str) -> Result<Self, String> {
        match kind {
            "scene" => Ok(Self::Scene),
            "round" => Ok(Self::Round),
            "restart" => Ok(Self::Restart),
            _ => Err("Info panel kind must be scene, round, or restart".into()),
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
    pub fn update(&mut self, rows: Vec<(&'static str, String)>) -> bool {
        if rows == self.rows {
            return false;
        }
        self.mesh = geometry(self.kind, self.width, self.height, &rows);
        self.rows = rows;
        self.dirty = true;
        true
    }
}
/// Centered local XY plane. Layer separation avoids coplanar depth artifacts.
pub fn geometry(kind: PanelKind, width: f32, height: f32, rows: &[(&str, String)]) -> ImportedMesh {
    let mut mesh = ImportedMesh {
        vertices: vec![],
        indices: vec![],
        bounds_min: [-width / 2., -height / 2., 0.],
        bounds_max: [width / 2., height / 2., 0.002],
        warnings: vec![],
    };
    let labels = rows.iter().map(|r| r.0.len()).max().unwrap_or(1);
    let values = rows.iter().map(|r| r.1.len()).max().unwrap_or(1);
    let columns = (labels + values + 3).max(10);
    let unit = (width / (columns * 6 + 8) as f32).min(height / ((rows.len() + 1) * 12 + 8) as f32);
    let left = -width / 2.;
    let top = height / 2.;
    let mut quad = |x: f32, y: f32, w: f32, h: f32, z: f32, color: [f32; 3]| {
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
    };
    quad(left, top, width, height, 0., [0.015, 0.025, 0.04]);
    let border = unit * 0.5;
    for (x, y, w, h) in [
        (left, top, width, border),
        (left, top - height + border, width, border),
        (left, top, border, height),
        (left + width - border, top, border, height),
    ] {
        quad(x, y, w, h, 0.001, [0.25, 0.55, 0.65]);
    }
    for row in 0..=rows.len() {
        let title = match kind {
            PanelKind::Scene => "SCENA",
            PanelKind::Round => "SKORE",
            PanelKind::Restart => "DALSI KOLO",
        };
        let texts = if row == 0 {
            [(title, 4.), ("", 0.)]
        } else {
            [
                (rows[row - 1].0, 4.),
                (rows[row - 1].1.as_str(), (labels * 6 + 14) as f32),
            ]
        };
        for (text, start) in texts {
            for (column, ch) in text.chars().enumerate() {
                for (gy, bits) in crate::debug_overlay::glyph(ch).iter().enumerate() {
                    for gx in 0..5 {
                        if bits & (1 << (4 - gx)) != 0 {
                            quad(
                                left + (start + (column * 6 + gx) as f32) * unit,
                                top - (5 + (row * 12) + gy) as f32 * unit,
                                unit,
                                unit,
                                0.002,
                                [0.85, 0.95, 1.],
                            );
                        }
                    }
                }
            }
        }
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
        assert!(mesh.vertices.len() > 500);
        assert_eq!(mesh.indices.len() % 6, 0);
        for v in &mesh.vertices {
            assert!(v.position.iter().all(|n| n.is_finite()));
            for (i, p) in v.position.iter().enumerate() {
                assert!(*p >= mesh.bounds_min[i] - 1e-6 && *p <= mesh.bounds_max[i] + 1e-6);
            }
            assert_eq!(v.normal, [0., 0., 1.]);
        }
        assert!(mesh.vertices.iter().any(|v| v.color == [0.85, 0.95, 1.]));
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
