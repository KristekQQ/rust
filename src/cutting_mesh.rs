//! CPU originals and lazy reference recipes for GPU-resident pieces.
//! Geometry is materialized only for explicit CPU fallback/diagnostics.
use crate::{cutting::CutMesh, scene::Transform};
use glam::Vec3;
use std::{cell::OnceCell, rc::Rc};

#[derive(Clone, Copy)]
pub struct MeshStats {
    pub min: Vec3,
    pub max: Vec3,
    pub triangles: usize,
    pub vertices: usize,
    pub volume: f64,
}
#[derive(Clone)]
pub struct SupportCurve {
    pub rotation: Vec3,
    pub angular: Vec3,
    pub scale: Vec3,
    pub samples: [f32; 64],
    pub second_bound: f32,
}
impl SupportCurve {
    /// A lower bound on min_i y_i(t). min of smooth trajectories can have
    /// downward derivative jumps, so endpoint interpolation minus M*dt²/8 is
    /// conservative. This prevents penetration without reading GPU vertices.
    pub fn minimum(&self, t: f32) -> f32 {
        let sample = t.clamp(0., 1.) * 63.;
        let index = (sample as usize).min(62);
        let f = sample - index as f32;
        self.samples[index] * (1. - f) + self.samples[index + 1] * f
            - self.second_bound * f * (1. - f) / (2. * 63. * 63.)
    }
    pub fn at_transform(&self, transform: Transform) -> Option<f32> {
        if transform.scale != self.scale {
            return None;
        }
        let delta = transform.rotation - self.rotation;
        let axis = self.angular.abs().max_position();
        let t = if self.angular[axis].abs() > 1e-6 {
            delta[axis] / self.angular[axis]
        } else {
            0.
        };
        if !(-1e-5..=1.00001).contains(&t) || (delta - self.angular * t).abs().max_element() > 1e-5
        {
            return None;
        }
        Some(transform.position.y + self.minimum(t))
    }
}
#[derive(Clone)]
pub struct MeshData(Rc<MeshNode>);
struct MeshNode {
    stats: MeshStats,
    source: MeshSource,
    support: Option<SupportCurve>,
    contact: Option<Rc<crate::contact_shape::ContactShape>>,
}
enum MeshSource {
    Cpu(CutMesh),
    Colored {
        parent: MeshData,
        colors: Vec<Vec3>,
        mesh: OnceCell<CutMesh>,
    },
    Resident {
        recipe: Rc<SplitRecipe>,
        side: usize,
    },
}
pub struct SplitRecipe {
    parent: MeshData,
    a: Vec3,
    b: Vec3,
    motion: Vec3,
    color: Vec3,
    result: OnceCell<Result<Vec<CutMesh>, String>>,
}
impl SplitRecipe {
    pub fn new(parent: MeshData, a: Vec3, b: Vec3, motion: Vec3, color: Vec3) -> Rc<Self> {
        Rc::new(Self {
            parent,
            a,
            b,
            motion,
            color,
            result: OnceCell::new(),
        })
    }
}
impl From<CutMesh> for MeshData {
    fn from(mesh: CutMesh) -> Self {
        Self::from_cpu(mesh, true)
    }
}
impl MeshData {
    /// Noninteractive death fragments retain their exact render geometry, without
    /// baking a convex collision proxy that will never be queried.
    pub(crate) fn visual_only(mesh: CutMesh) -> Self {
        Self::from_cpu(mesh, false)
    }
    fn from_cpu(mesh: CutMesh, contact: bool) -> Self {
        let (min, max) = mesh.bounds();
        let stats = MeshStats {
            min,
            max,
            triangles: mesh.triangles.len(),
            vertices: mesh.positions.len(),
            volume: mesh.volume(),
        };
        Self(Rc::new(MeshNode {
            contact: contact.then(|| {
                Rc::new(crate::contact_shape::ContactShape::new(
                    &mesh.positions,
                    (min, max),
                    mesh.centroid(),
                ))
            }),
            stats,
            source: MeshSource::Cpu(mesh),
            support: None,
        }))
    }
}
impl MeshData {
    pub(crate) fn colored(&self, colors: Vec<Vec3>) -> Self {
        Self(Rc::new(MeshNode {
            contact: self.0.contact.clone(),
            stats: self.0.stats,
            support: None,
            source: MeshSource::Colored {
                parent: self.clone(),
                colors,
                mesh: OnceCell::new(),
            },
        }))
    }
    pub fn resident(
        stats: MeshStats,
        recipe: Rc<SplitRecipe>,
        side: usize,
        support: SupportCurve,
    ) -> Self {
        Self(Rc::new(MeshNode {
            contact: None,
            stats,
            source: MeshSource::Resident { recipe, side },
            support: Some(support),
        }))
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn same_source(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn stats(&self) -> MeshStats {
        self.0.stats
    }
    pub(crate) fn contact_shape(&self) -> Option<&crate::contact_shape::ContactShape> {
        self.0.contact.as_deref()
    }
    pub(crate) fn contact_center(&self) -> Vec3 {
        self.contact_shape()
            .map_or((self.0.stats.min + self.0.stats.max) * 0.5, |s| s.center)
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        (self.0.stats.min, self.0.stats.max)
    }
    pub fn volume(&self) -> f64 {
        self.0.stats.volume
    }
    pub fn triangle_count(&self) -> usize {
        self.0.stats.triangles
    }
    pub fn vertex_count(&self) -> usize {
        self.0.stats.vertices
    }
    pub fn is_resident(&self) -> bool {
        matches!(self.0.source, MeshSource::Resident { .. })
    }
    pub fn support(&self) -> Option<&SupportCurve> {
        self.0.support.as_ref()
    }
    pub fn ready_cpu(&self) -> Option<&CutMesh> {
        match &self.0.source {
            MeshSource::Cpu(mesh) => Some(mesh),
            MeshSource::Colored { mesh, .. } => mesh.get(),
            MeshSource::Resident { recipe, side } => recipe.result.get()?.as_ref().ok()?.get(*side),
        }
    }
    pub fn cpu(&self) -> Result<&CutMesh, String> {
        match &self.0.source {
            MeshSource::Cpu(mesh) => Ok(mesh),
            MeshSource::Colored {
                parent,
                colors,
                mesh,
            } => {
                if mesh.get().is_none() {
                    let _ = mesh.set(parent.cpu()?.recolored(colors));
                }
                Ok(mesh.get().expect("initialized color recipe"))
            }
            MeshSource::Resident { recipe, side } => {
                let result = recipe.result.get_or_init(|| {
                    let result = recipe
                        .parent
                        .cpu()?
                        .slice(recipe.a, recipe.b, recipe.motion, recipe.color)?
                        .ok_or("Resident reference replay did not split")?;
                    if result.pieces.len() != 2 {
                        return Err("Resident reference replay has different components".into());
                    }
                    Ok(result.pieces)
                });
                result
                    .as_ref()
                    .map_err(Clone::clone)?
                    .get(*side)
                    .ok_or_else(|| "Invalid resident side".into())
            }
        }
    }
    pub fn bottom(&self, transform: Transform) -> Result<f32, String> {
        if let MeshSource::Colored { parent, .. } = &self.0.source {
            return parent.bottom(transform);
        }
        if let Some(value) = self.support().and_then(|s| s.at_transform(transform)) {
            return Ok(value);
        }
        let matrix = transform.model();
        if self.is_resident() {
            // Arbitrary user transforms are outside the sampled animation path.
            // Return a conservative bounds support, never replay cuts for a HUD query.
            let (min, max) = self.bounds();
            let center = matrix.transform_point3((min + max) * 0.5);
            let half = (max - min) * 0.5;
            let radius = matrix.x_axis.y.abs() * half.x
                + matrix.y_axis.y.abs() * half.y
                + matrix.z_axis.y.abs() * half.z;
            return Ok(center.y - radius);
        }
        Ok(self
            .cpu()?
            .positions
            .iter()
            .map(|&p| matrix.transform_point3(p).y)
            .fold(f32::INFINITY, f32::min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tetrahedron() -> CutMesh {
        CutMesh::new(
            vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z],
            &[0, 2, 1, 0, 1, 3, 0, 3, 2, 1, 2, 3],
            Vec3::ONE,
        )
        .unwrap()
    }
    #[test]
    fn visual_fragment_keeps_exact_geometry_and_metadata_without_contact_bake() {
        let physical = MeshData::from(tetrahedron());
        let visual = MeshData::visual_only(tetrahedron());
        assert!(physical.contact_shape().is_some());
        assert!(visual.contact_shape().is_none());
        assert_eq!(physical.bounds(), visual.bounds());
        assert_eq!(physical.volume(), visual.volume());
        assert_eq!(physical.triangle_count(), visual.triangle_count());
        let a = physical.ready_cpu().unwrap().render_mesh();
        let b = visual.ready_cpu().unwrap().render_mesh();
        assert_eq!(a.indices, b.indices);
        for (a, b) in a.vertices.iter().zip(&b.vertices) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.color, b.color);
        }
    }

    #[test]
    fn contact_shape_is_eager_and_shared_without_materializing_variant_colors() {
        let base = MeshData::from(tetrahedron());
        let colored = base.colored(vec![Vec3::X; base.triangle_count()]);
        assert!(Rc::ptr_eq(
            base.0.contact.as_ref().unwrap(),
            colored.0.contact.as_ref().unwrap()
        ));
        assert!(colored.ready_cpu().is_none());
        assert_eq!(colored.contact_shape().unwrap().points.len(), 4);
        assert!(colored.ready_cpu().is_none());
    }

    #[test]
    fn resident_metadata_queries_never_replay_geometry_and_explicit_fallback_does() {
        let parent = MeshData::from(tetrahedron());
        let a = Vec3::new(0.2, 2., -1.);
        let b = Vec3::new(0.2, 2., 2.);
        let motion = Vec3::new(0., -4., 0.);
        let sliced = parent
            .cpu()
            .unwrap()
            .slice(a, b, motion, Vec3::ONE)
            .unwrap()
            .unwrap();
        let reference = MeshData::from(sliced.pieces.into_iter().next().unwrap());
        let recipe = SplitRecipe::new(parent, a, b, motion, Vec3::ONE);
        let support = SupportCurve {
            rotation: Vec3::ZERO,
            angular: Vec3::ZERO,
            scale: Vec3::ONE,
            samples: [0.; 64],
            second_bound: 0.,
        };
        let resident = MeshData::resident(reference.stats(), recipe, 0, support);
        let transform = Transform::new(Vec3::Y, Vec3::new(0.2, 0.3, 0.4), Vec3::new(-1., 2., 0.7));
        let bottom = resident.bottom(transform).unwrap();
        assert!(resident.ready_cpu().is_none());
        assert!(bottom <= reference.bottom(transform).unwrap() + 1e-6);
        assert!((resident.cpu().unwrap().volume() - resident.volume()).abs() < 1e-7);
        assert!(resident.ready_cpu().is_some());
    }
    #[test]
    fn resident_commit_inherits_latest_render_options() {
        let mut scene = crate::scene::SceneManager::default();
        let source = scene.add_triangle_mesh(tetrahedron()).unwrap();
        scene.set_cuttable(source, true, Vec3::ONE).unwrap();
        let cutter = scene.add_cube(Vec3::ZERO);
        scene
            .set_cutter(
                cutter,
                Some([Vec3::new(0.2, 2., -1.), Vec3::new(0.2, 2., 2.)]),
            )
            .unwrap();
        let motion = Vec3::new(0., -4., 0.);
        let candidate = scene
            .resident_candidates(cutter, motion, 0., None)
            .unwrap()
            .remove(0);
        let slice = candidate
            .mesh
            .cpu()
            .unwrap()
            .slice(candidate.a, candidate.b, candidate.motion, candidate.color)
            .unwrap()
            .unwrap();
        let mut pieces = slice.pieces.into_iter();
        let data = [
            MeshData::from(pieces.next().unwrap()),
            MeshData::from(pieces.next().unwrap()),
        ];
        let mut options = scene.objects()[&source].render_options;
        options.casts_shadow = false;
        scene.set_render_options(source, options);
        scene.set_opacity(source, 0.58, 0.25, 0.7, 0.72);
        let report = scene
            .commit_resident(cutter, motion, 0., None, vec![(candidate, data)], 1)
            .unwrap();
        assert!(report
            .pieces
            .iter()
            .all(|id| scene.objects()[id].opacity == 0.58
                && scene.objects()[id].opaque_above == 0.25
                && scene.objects()[id].opacity_variation == 0.7
                && scene.objects()[id].opacity_coverage == 0.72));
        assert!(report
            .pieces
            .iter()
            .all(|id| scene.objects()[id].render_options == options));
    }
    #[test]
    fn support_curve_is_conservative_between_samples_and_exact_at_endpoints() {
        let rotation = Vec3::new(0.1, 0.2, 0.3);
        let angular = Vec3::new(0.9, 0.12, -0.4);
        let scale = Vec3::new(1.1, 0.8, 0.9);
        let points = [
            Vec3::new(-1., -0.2, 0.5),
            Vec3::new(0.2, 1., -0.9),
            Vec3::new(0.8, -0.9, 0.3),
        ];
        let minimum = |t: f32| {
            let m = Transform::new(Vec3::ZERO, rotation + angular * t, scale).model();
            points
                .iter()
                .map(|&p| m.transform_point3(p).y)
                .fold(f32::INFINITY, f32::min)
        };
        let mut samples = [0.; 64];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = minimum(i as f32 / 63.);
        }
        let bound = points
            .iter()
            .map(|p| (*p * scale).length())
            .fold(0., f32::max)
            * angular.abs().element_sum().powi(2);
        let curve = SupportCurve {
            rotation,
            angular,
            scale,
            samples,
            second_bound: bound,
        };
        for i in 0..=500 {
            let t = i as f32 / 500.;
            let difference = minimum(t) - curve.minimum(t);
            assert!(
                (-2e-6..0.02).contains(&difference),
                "t={t}, gap={difference}"
            );
        }
        assert!((curve.minimum(0.) - minimum(0.)).abs() < 1e-6);
        assert!((curve.minimum(1.) - minimum(1.)).abs() < 1e-6);
    }
}
