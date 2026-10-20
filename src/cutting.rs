//! CPU mesh slicing shared by WebGPU/WebGL. Simple closed oriented triangle solids;
//! caps may be concave, but nested contours (holes) are rejected transactionally.
use crate::spatial::{Bounds, Bvh};
use glam::{Mat4, Vec2, Vec3};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const MAX_TRIANGLES: usize = 20_000;
const EPS: f32 = 1e-5;

#[derive(Clone, Debug)]
pub struct Triangle {
    pub indices: [u32; 3],
    pub color: Vec3,
    /// Corner normals. Caps use one explicit plane normal; None derives a face normal.
    pub normals: Option<[Vec3; 3]>,
}
pub struct CutMesh {
    pub positions: Vec<Vec3>,
    pub triangles: Vec<Triangle>,
    bvh: OnceCell<Bvh>,
    local_bounds: (Vec3, Vec3),
}
#[derive(Default, Clone, Debug)]
pub struct CutStats {
    pub nodes: usize,
    pub triangle_tests: usize,
}
pub struct Slice {
    pub pieces: Vec<CutMesh>,
    pub stats: CutStats,
}

/// Immutable input for a batched WebGPU geometric-preparation dispatch.
pub struct CutGeometryInput {
    pub id: u32,
    pub positions: Vec<[f32; 4]>,
    pub edges: Vec<[u32; 2]>,
    pub plane: [f32; 4],
}
pub struct CutGeometry {
    distance: Vec<f32>,
    intersections: BTreeMap<(u32, u32), Vec3>,
}
impl CutGeometryInput {
    /// Correct near-plane distances/edges with the CPU reference arithmetic;
    /// grossly inconsistent rows request a complete CPU fallback.
    pub fn decode(&self, rows: &[[f32; 4]]) -> Result<Option<CutGeometry>, String> {
        if rows.len() != self.positions.len() + self.edges.len() {
            return Err("Invalid compute geometry size".into());
        }
        let mut distance: Vec<_> = rows[..self.positions.len()].iter().map(|r| r[0]).collect();
        if distance.iter().any(|d| !d.is_finite()) {
            return Err("Nonfinite compute geometry".into());
        }
        let mut corrected = vec![false; distance.len()];
        let normal = Vec3::from_slice(&self.plane);
        for (i, d) in distance.iter_mut().enumerate() {
            if d.abs() < EPS * 8. {
                let cpu = normal.dot(Vec3::from_slice(&self.positions[i])) + self.plane[3];
                if (cpu - *d).abs() > EPS * 8. {
                    return Ok(None);
                }
                *d = if cpu.abs() < EPS { 0. } else { cpu };
                corrected[i] = true;
            }
        }
        let mut intersections = BTreeMap::new();
        for ([i, j], row) in self
            .edges
            .iter()
            .copied()
            .zip(&rows[self.positions.len()..])
        {
            if distance[i as usize] * distance[j as usize] < 0. {
                if corrected[i as usize] || corrected[j as usize] {
                    let t = distance[i as usize] / (distance[i as usize] - distance[j as usize]);
                    intersections.insert(
                        (i, j),
                        Vec3::from_slice(&self.positions[i as usize])
                            .lerp(Vec3::from_slice(&self.positions[j as usize]), t),
                    );
                    continue;
                }
                if row.iter().any(|v| !v.is_finite()) || !(0.0..=1.0).contains(&row[3]) {
                    return Err("Invalid compute intersection".into());
                }
                intersections.insert((i, j), Vec3::from_slice(row));
            }
        }
        Ok(Some(CutGeometry {
            distance,
            intersections,
        }))
    }
}

fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

impl CutMesh {
    /// Resident cutting currently handles one connected genus-zero solid.
    /// Inputs are already validated closed oriented manifolds at construction.
    pub fn resident_eligible(&self) -> bool {
        let used: std::collections::HashSet<u32> =
            self.triangles.iter().flat_map(|t| t.indices).collect();
        used.len() + self.triangles.len() == self.triangles.len() * 3 / 2 + 2
            && components(self.triangles.clone()).len() == 1
    }
    pub fn new(positions: Vec<Vec3>, indices: &[u32], color: Vec3) -> Result<Self, String> {
        if positions.len() < 4
            || positions.len() > MAX_TRIANGLES * 3
            || indices.is_empty()
            || !indices.len().is_multiple_of(3)
            || indices.len() / 3 > MAX_TRIANGLES
            || !color.is_finite()
            || color.min_element() < 0.0
            || color.max_element() > 1.0
            || positions
                .iter()
                .any(|p| !p.is_finite() || p.abs().max_element() > 10_000.0)
            || indices.iter().any(|&i| i as usize >= positions.len())
        {
            return Err("Invalid mesh data or triangle budget exceeded".into());
        }
        let mesh = Self::from_triangles(
            positions,
            indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|i| Triangle {
                    indices: [i[0], i[1], i[2]],
                    color,
                    normals: None,
                })
                .collect(),
        )?;
        mesh.validate()?;
        mesh.spatial_index();
        Ok(mesh)
    }
    fn from_triangles(positions: Vec<Vec3>, triangles: Vec<Triangle>) -> Result<Self, String> {
        if triangles.len() > MAX_TRIANGLES {
            return Err("Cut triangle budget exceeded".into());
        }
        let local_bounds = triangles
            .iter()
            .flat_map(|t| t.indices)
            .map(|i| positions[i as usize])
            .fold(
                (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)),
                |(min, max), p| (min.min(p), max.max(p)),
            );
        Ok(Self {
            positions,
            triangles,
            bvh: OnceCell::new(),
            local_bounds,
        })
    }
    /// Internal render-only geometry, which may contain touching disconnected solids.
    /// Never expose this constructor through the cuttable mesh validation API.
    pub(crate) fn from_render_mesh(mesh: &crate::gltf_loader::ImportedMesh) -> Self {
        let positions = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .collect();
        let triangles = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|&indices| Triangle {
                indices,
                color: Vec3::from_array(mesh.vertices[indices[0] as usize].color),
                normals: Some(
                    indices.map(|index| Vec3::from_array(mesh.vertices[index as usize].normal)),
                ),
            })
            .collect();
        Self::from_triangles(positions, triangles).expect("internal render mesh budget")
    }
    /// A newly cut piece is immediately renderable. Build its triangle index
    /// only if it is actually cut again (not for every untouched output half).
    fn spatial_index(&self) -> &Bvh {
        self.bvh.get_or_init(|| {
            let entries: Vec<_> = self
                .triangles
                .iter()
                .enumerate()
                .map(|(id, t)| {
                    let [a, b, c] = t.indices.map(|i| self.positions[i as usize]);
                    let min = a.min(b).min(c);
                    let max = a.max(b).max(c);
                    (
                        id as u32,
                        Bounds {
                            center: (min + max) * 0.5,
                            half_extent: (max - min) * 0.5,
                        },
                    )
                })
                .collect();
            let mut bvh = Bvh::default();
            bvh.update(&entries, true);
            bvh
        })
    }
    pub fn bounds(&self) -> (Vec3, Vec3) {
        self.local_bounds
    }
    /// Opt into area-weighted normals at shared vertex indices on initial creation.
    /// Call before slicing: cuts preserve these corner values and keep caps flat.
    pub fn smooth_normals(&mut self) {
        let mut normals = vec![Vec3::ZERO; self.positions.len()];
        for t in &self.triangles {
            let [a, b, c] = t.indices.map(|i| self.positions[i as usize]);
            let normal = (b - a).cross(c - a);
            for i in t.indices {
                normals[i as usize] += normal;
            }
        }
        for t in &mut self.triangles {
            let [a, b, c] = t.indices.map(|i| self.positions[i as usize]);
            let flat = (b - a).cross(c - a).normalize();
            t.normals = Some(
                t.indices
                    .map(|i| normals[i as usize].try_normalize().unwrap_or(flat)),
            );
        }
    }
    /// Uniform-density closed-solid centroid, integrated using signed tetrahedra.
    pub(crate) fn centroid(&self) -> Vec3 {
        let origin = self.bounds().0.to_array().map(f64::from);
        let mut weighted = [0.; 3];
        let mut volume6 = 0.;
        for triangle in &self.triangles {
            let [a, b, c] = triangle.indices.map(|i| {
                let p = self.positions[i as usize].to_array().map(f64::from);
                std::array::from_fn::<_, 3, _>(|axis| p[axis] - origin[axis])
            });
            let weight = a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
            for axis in 0..3 {
                weighted[axis] += (a[axis] + b[axis] + c[axis]) * weight;
            }
            volume6 += weight;
        }
        Vec3::from_array(std::array::from_fn(|axis| {
            (origin[axis] + weighted[axis] / (4. * volume6)) as f32
        }))
    }
    /// Preserve corner normals, caps and indices; only local coordinates change.
    pub(crate) fn recenter(&mut self) -> Vec3 {
        let center = self.centroid();
        for position in &mut self.positions {
            *position -= center;
        }
        self.local_bounds.0 -= center;
        self.local_bounds.1 -= center;
        self.bvh = OnceCell::new();
        center
    }
    pub fn volume(&self) -> f64 {
        let origin = self.bounds().0;
        self.triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.indices.map(|i| {
                    (self.positions[i as usize] - origin)
                        .to_array()
                        .map(f64::from)
                });
                (a[0] * (b[1] * c[2] - b[2] * c[1])
                    + a[1] * (b[2] * c[0] - b[0] * c[2])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0
            })
            .sum()
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut edges = HashMap::<_, (usize, i32)>::with_capacity(self.triangles.len() * 2);
        for t in &self.triangles {
            let [a, b, c] = t.indices.map(|i| self.positions[i as usize]);
            // A valid near-pole incision can produce a very small, skinny face.
            // Keep it to preserve shared edge topology; reject zero/underflow area.
            if (b - a).cross(c - a).length_squared() < 1e-28 {
                return Err(format!(
                    "Degenerate triangle {:?}: {:?} {:?} {:?}",
                    t.indices, a, b, c
                ));
            }
            for k in 0..3 {
                let a = t.indices[k];
                let b = t.indices[(k + 1) % 3];
                let entry = edges.entry(edge(a, b)).or_default();
                entry.0 += 1;
                entry.1 += if a < b { 1 } else { -1 };
            }
        }
        if edges
            .values()
            .any(|&(count, orientation)| count != 2 || orientation != 0)
            || self.volume() <= 1e-10
        {
            return Err("Mesh must be closed, manifold and outward oriented".into());
        }
        Ok(())
    }
    pub fn transformed(&self, transform: Mat4) -> Result<Self, String> {
        let positions: Vec<_> = self
            .positions
            .iter()
            .map(|&p| transform.transform_point3(p))
            .collect();
        if positions
            .iter()
            .any(|p| !p.is_finite() || p.abs().max_element() > 10_000.0)
        {
            return Err("Cut transform outside supported range".into());
        }
        let mut triangles = self.triangles.clone();
        let normal_transform = transform.inverse().transpose();
        for t in &mut triangles {
            if let Some(normals) = &mut t.normals {
                for n in normals {
                    *n = normal_transform.transform_vector3(*n);
                    if !n.is_finite() {
                        return Err("Cut transform has no finite normal inverse".into());
                    }
                }
            }
        }
        if transform.determinant() < 0.0 {
            for t in &mut triangles {
                t.indices.swap(1, 2);
                if let Some(normals) = &mut t.normals {
                    normals.swap(1, 2);
                }
            }
        }
        Self::from_triangles(positions, triangles)
    }
    /// Shared topology is validated once; a color variant builds its own BVH lazily.
    pub(crate) fn recolored(&self, colors: &[Vec3]) -> Self {
        let mut triangles = self.triangles.clone();
        for (triangle, &color) in triangles.iter_mut().zip(colors) {
            triangle.color = color;
        }
        Self {
            positions: self.positions.clone(),
            triangles,
            bvh: OnceCell::new(),
            local_bounds: self.local_bounds,
        }
    }
    pub fn render_mesh(&self) -> crate::gltf_loader::ImportedMesh {
        let mut vertices = Vec::with_capacity(self.triangles.len() * 3);
        for t in &self.triangles {
            let [a, b, c] = t.indices.map(|i| self.positions[i as usize]);
            let flat = (b - a).cross(c - a).normalize();
            for (k, position) in [a, b, c].into_iter().enumerate() {
                // Keep the original linear normal field across subdivisions.
                // Renormalizing a new cut vertex would change the skin highlight.
                // The fragment shader normalizes after perspective interpolation.
                let normal = t.normals.map_or(flat, |n| n[k]);
                vertices.push(crate::gltf_loader::ImportedVertex {
                    position: position.to_array(),
                    normal: normal.to_array(),
                    color: t.color.to_array(),
                });
            }
        }
        let (min, max) = self.bounds();
        crate::gltf_loader::ImportedMesh {
            indices: (0..vertices.len() as u32).collect(),
            vertices,
            bounds_min: min.to_array(),
            bounds_max: max.to_array(),
            warnings: vec![],
        }
    }

    /// Plane is the finite parallelogram swept by blade [a,b] along motion.
    /// A partial incision is a miss: every cap vertex must lie in that sweep.
    pub fn slice(
        &self,
        a: Vec3,
        b: Vec3,
        motion: Vec3,
        cap_color: Vec3,
    ) -> Result<Option<Slice>, String> {
        self.slice_prepared(a, b, motion, cap_color, None)
    }
    pub fn geometry_input(
        &self,
        id: u32,
        a: Vec3,
        b: Vec3,
        motion: Vec3,
    ) -> Result<CutGeometryInput, String> {
        let normal = (b - a).cross(motion).normalize_or_zero();
        if !a.is_finite() || !b.is_finite() || !motion.is_finite() || normal.length_squared() < 0.5
        {
            return Err("Invalid compute cutting plane".into());
        }
        let d = normal.dot(a);
        let mut candidates = Vec::new();
        self.spatial_index().query::<false>(
            &|bounds| {
                (normal.dot(bounds.center) - d).abs() <= normal.abs().dot(bounds.half_extent) + EPS
            },
            &|_| true,
            &mut candidates,
            &mut 0,
            &mut 0,
        );
        let mut edges = BTreeSet::new();
        for id in candidates {
            let t = &self.triangles[id as usize];
            for k in 0..3 {
                edges.insert(edge(t.indices[k], t.indices[(k + 1) % 3]));
            }
        }
        Ok(CutGeometryInput {
            id,
            positions: self
                .positions
                .iter()
                .map(|p| p.extend(0.).to_array())
                .collect(),
            edges: edges.into_iter().map(|(a, b)| [a, b]).collect(),
            plane: normal.extend(-d).to_array(),
        })
    }
    pub fn slice_prepared(
        &self,
        a: Vec3,
        b: Vec3,
        motion: Vec3,
        cap_color: Vec3,
        prepared: Option<&CutGeometry>,
    ) -> Result<Option<Slice>, String> {
        self.slice_region(a, b, motion, cap_color, prepared, None, 1., &mut 0.)
    }

    /// A sampled planar blade ribbon. Each consecutive endpoint pair bounds
    /// two triangles; gaps and the unswept region near the grip remain misses.
    pub(crate) fn slice_ribbon(
        &self,
        samples: &[[Vec3; 2]],
        cap_color: Vec3,
    ) -> Result<Option<Slice>, String> {
        self.slice_ribbon_progress(samples, cap_color, 1.)
            .map(|(_, slice)| slice)
    }

    pub(crate) fn slice_ribbon_progress(
        &self,
        samples: &[[Vec3; 2]],
        cap_color: Vec3,
        threshold: f32,
    ) -> Result<(f32, Option<Slice>), String> {
        let Some(first) = samples.first() else {
            return Ok((0., None));
        };
        let a = first[0];
        let b = first[1];
        let blade = b - a;
        let Some(motion) = samples
            .iter()
            .flat_map(|s| s.iter())
            .map(|p| *p - a)
            .find(|v| blade.cross(*v).length() > EPS)
        else {
            return Ok((0., None));
        };
        let normal = blade.cross(motion).normalize();
        // A single planar mesh split cannot represent a twisted/nonplanar stroke.
        if samples
            .iter()
            .flatten()
            .any(|p| !p.is_finite() || normal.dot(*p - a).abs() > 1e-4)
        {
            return Ok((0., None));
        }
        let mut progress = 0.;
        let slice = self.slice_region(
            a,
            b,
            motion,
            cap_color,
            None,
            Some(samples),
            threshold,
            &mut progress,
        )?;
        Ok((progress, slice))
    }

    #[allow(clippy::too_many_arguments)]
    fn slice_region(
        &self,
        a: Vec3,
        b: Vec3,
        motion: Vec3,
        cap_color: Vec3,
        prepared: Option<&CutGeometry>,
        ribbon: Option<&[[Vec3; 2]]>,
        threshold: f32,
        progress: &mut f32,
    ) -> Result<Option<Slice>, String> {
        let blade = b - a;
        let cross = blade.cross(motion);
        if !a.is_finite() || !b.is_finite() || !motion.is_finite() || cross.length() < EPS {
            return Err("Blade and motion must define a finite nonzero cutting plane".into());
        }
        let normal = cross.normalize();
        let d = normal.dot(a);
        let cpu_distance;
        let distance = if let Some(geometry) = prepared {
            if geometry.distance.len() != self.positions.len() {
                return Err("Stale compute geometry".into());
            }
            &geometry.distance
        } else {
            cpu_distance = self
                .positions
                .iter()
                .map(|p| {
                    let v = normal.dot(*p) - d;
                    if v.abs() < EPS {
                        0.0
                    } else {
                        v
                    }
                })
                .collect::<Vec<_>>();
            &cpu_distance
        };
        if !distance.iter().any(|v| *v > EPS) || !distance.iter().any(|v| *v < -EPS) {
            return Ok(None);
        }
        let mut stats = CutStats::default();
        let mut candidates = vec![];
        self.spatial_index().query::<true>(
            &|bounds| {
                (normal.dot(bounds.center) - d).abs() <= normal.abs().dot(bounds.half_extent) + EPS
            },
            &|_| true,
            &mut candidates,
            &mut stats.nodes,
            &mut stats.triangle_tests,
        );
        let mut crossing = vec![false; self.triangles.len()];
        for id in candidates {
            crossing[id as usize] = true;
        }
        let mut positions = self.positions.clone();
        let mut intersections = BTreeMap::new();
        let mut halves = [vec![], vec![]];
        for (id, t) in self.triangles.iter().enumerate() {
            if !crossing[id] {
                halves[usize::from(distance[t.indices[0] as usize] < 0.0)].push(t.clone());
                continue;
            }
            // A wholly coplanar face already closes one half; its winding determines ownership.
            if t.indices.iter().all(|&i| distance[i as usize] == 0.0) {
                let [p, q, r] = t.indices.map(|i| positions[i as usize]);
                halves[usize::from((q - p).cross(r - p).dot(normal) > 0.0)].push(t.clone());
                continue;
            }
            for (side, half) in halves.iter_mut().enumerate() {
                let sign = if side == 0 { 1.0 } else { -1.0 };
                let mut polygon = vec![];
                for k in 0..3 {
                    let i = t.indices[k];
                    let j = t.indices[(k + 1) % 3];
                    let di = distance[i as usize];
                    let dj = distance[j as usize];
                    if di * sign >= 0.0 {
                        polygon.push((i, t.normals.map(|n| n[k])));
                    }
                    if di * dj < 0.0 {
                        let id = *intersections.entry(edge(i, j)).or_insert_with(|| {
                            // Canonical edge order makes repeated hits bit-identical.
                            let (i, j) = edge(i, j);
                            let di = distance[i as usize];
                            let dj = distance[j as usize];
                            let p = prepared
                                .and_then(|g| g.intersections.get(&(i, j)))
                                .copied()
                                .unwrap_or_else(|| {
                                    positions[i as usize]
                                        .lerp(positions[j as usize], di / (di - dj))
                                });
                            positions.push(p);
                            (positions.len() - 1) as u32
                        });
                        // Normals belong to corners, not the shared intersection ID:
                        // a skin and a cap may meet here with different normals.
                        let normal = t.normals.map(|n| {
                            let next = (k + 1) % 3;
                            if i < j {
                                n[k].lerp(n[next], di / (di - dj))
                            } else {
                                n[next].lerp(n[k], dj / (dj - di))
                            }
                        });
                        polygon.push((id, normal));
                    }
                }
                for k in 1..polygon.len().saturating_sub(1) {
                    half.push(Triangle {
                        indices: [polygon[0].0, polygon[k].0, polygon[k + 1].0],
                        color: t.color,
                        normals: t.normals.map(|_| {
                            [
                                polygon[0].1.unwrap(),
                                polygon[k].1.unwrap(),
                                polygon[k + 1].1.unwrap(),
                            ]
                        }),
                    });
                }
            }
        }
        let mut pieces = vec![];
        // Only plane edges can become an exposed boundary of a previously
        // closed validated solid. New intersection IDs lie on that plane by
        // construction. Avoid hashing every retained skin edge twice.
        let mut on_plane: Vec<_> = distance.iter().map(|&d| d == 0.).collect();
        on_plane.resize(positions.len(), true);
        let boundary_capacity = on_plane.iter().filter(|&&on| on).count() * 2;
        for (side, mut triangles) in halves.into_iter().enumerate() {
            let contours = boundary_loops(&triangles, &on_plane, boundary_capacity)?;
            if contours.is_empty() {
                return Ok(None);
            }
            if side == 0 && threshold < 1. {
                if let Some(samples) = ribbon {
                    let mut cap_positions = positions.clone();
                    let mut caps = Vec::new();
                    for contour in &contours {
                        let mut contour: Vec<_> = contour.iter().copied().rev().collect();
                        caps.extend(triangulate_cap(
                            &mut contour,
                            &mut cap_positions,
                            -normal,
                            cap_color,
                        )?);
                    }
                    *progress = ribbon_area_coverage(&caps, &cap_positions, samples, normal)?;
                    // Threshold tolerance must never promote a finite miss into a cut.
                    if *progress <= 0. || *progress + 2e-5 < threshold {
                        return Ok(None);
                    }
                }
            }
            for contour in &contours {
                let covered = if let Some(samples) = ribbon {
                    contour
                        .iter()
                        .zip(contour.iter().cycle().skip(1))
                        .all(|(&i, &j)| {
                            ribbon_covers_segment(
                                positions[i as usize],
                                positions[j as usize],
                                samples,
                                normal,
                            )
                        })
                } else {
                    contour
                        .iter()
                        .all(|&i| inside_sweep(positions[i as usize] - a, blade, motion))
                };
                if threshold >= 1. && !covered {
                    return Ok(None);
                }
            }
            // Disjoint outlines work; nested outlines need a hole-aware triangulator.
            let axis = normal.any_orthonormal_vector();
            let other = normal.cross(axis);
            for (i, outer) in contours.iter().enumerate() {
                for (j, inner) in contours.iter().enumerate() {
                    if i != j
                        && contains(outer, positions[inner[0] as usize], &positions, axis, other)
                    {
                        return Err("Cut caps with holes are not yet supported".into());
                    }
                }
            }
            for contour in contours {
                // Boundary traversal is reversed for cap winding.
                let mut contour: Vec<_> = contour.into_iter().rev().collect();
                let outward = if side == 0 { -normal } else { normal };
                triangles.extend(triangulate_cap(
                    &mut contour,
                    &mut positions,
                    outward,
                    cap_color,
                )?);
            }
            for component in components(triangles) {
                let piece = Self::compact(&positions, component)?;
                piece.validate()?;
                pieces.push(piece);
            }
        }
        if pieces.len() > 64 {
            return Err("Too many cut components".into());
        }
        let before = self.volume();
        let after: f64 = pieces.iter().map(Self::volume).sum();
        if (after - before).abs() > before.abs() * 0.001 + 1e-7 {
            return Err("Cut failed volume conservation".into());
        }
        Ok(Some(Slice { pieces, stats }))
    }
    fn compact(positions: &[Vec3], mut triangles: Vec<Triangle>) -> Result<Self, String> {
        let mut remap = vec![u32::MAX; positions.len()];
        let mut compact = vec![];
        for t in &mut triangles {
            for i in &mut t.indices {
                let mapped = &mut remap[*i as usize];
                if *mapped == u32::MAX {
                    compact.push(positions[*i as usize]);
                    *mapped = (compact.len() - 1) as u32;
                }
                *i = *mapped;
            }
        }
        Self::from_triangles(compact, triangles)
    }
}

// Clip a cap edge against each actual sampled ribbon triangle, then require
// the union of covered parameter intervals to cover the entire edge.
fn ribbon_covers_segment(a: Vec3, b: Vec3, samples: &[[Vec3; 2]], normal: Vec3) -> bool {
    let mut intervals = Vec::new();
    for pair in samples.windows(2) {
        for tri in [
            [pair[0][0], pair[0][1], pair[1][1]],
            [pair[0][0], pair[1][1], pair[1][0]],
        ] {
            let area = normal.dot((tri[1] - tri[0]).cross(tri[2] - tri[0]));
            if area.abs() < 1e-10 {
                continue;
            }
            let sign = area.signum();
            let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
            for i in 0..3 {
                let edge = tri[(i + 1) % 3] - tri[i];
                let start = sign * normal.dot(edge.cross(a - tri[i]));
                let delta = sign * normal.dot(edge.cross(b - a));
                let tolerance = EPS * edge.length();
                if delta.abs() < 1e-12 {
                    if start < -tolerance {
                        hi = -1.;
                        break;
                    }
                } else {
                    let t = (-tolerance - start) / delta;
                    if delta > 0. {
                        lo = lo.max(t);
                    } else {
                        hi = hi.min(t);
                    }
                }
            }
            if lo <= hi {
                intervals.push((lo, hi));
            }
        }
    }
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut covered = 0.;
    for (lo, hi) in intervals {
        if lo > covered + 1e-5 {
            return false;
        }
        covered = f32::max(covered, hi);
        if covered >= 1. - 1e-5 {
            return true;
        }
    }
    false
}

fn inside_sweep(p: Vec3, u: Vec3, v: Vec3) -> bool {
    let uu = u.dot(u);
    let vv = v.dot(v);
    let uv = u.dot(v);
    let determinant = uu * vv - uv * uv;
    let s = (p.dot(u) * vv - p.dot(v) * uv) / determinant;
    let t = (p.dot(v) * uu - p.dot(u) * uv) / determinant;
    (-EPS..=1.0 + EPS).contains(&s) && (-EPS..=1.0 + EPS).contains(&t)
}

fn boundary_loops(
    triangles: &[Triangle],
    on_plane: &[bool],
    capacity: usize,
) -> Result<Vec<Vec<u32>>, String> {
    let mut edges = HashMap::<_, (usize, u32, u32)>::with_capacity(capacity);
    for t in triangles {
        for k in 0..3 {
            let a = t.indices[k];
            let b = t.indices[(k + 1) % 3];
            if !on_plane[a as usize] || !on_plane[b as usize] {
                continue;
            }
            let occurrence = edges.entry(edge(a, b)).or_insert((0, a, b));
            occurrence.0 += 1;
        }
    }
    let mut next = BTreeMap::new();
    for &(count, a, b) in edges.values() {
        if count == 1 {
            if next.insert(a, b).is_some() {
                return Err("Ambiguous cut boundary".into());
            }
        } else if count != 2 {
            return Err("Nonmanifold cut boundary".into());
        }
    }
    let mut loops = vec![];
    while let Some((&start, _)) = next.first_key_value() {
        let mut ring = vec![];
        let mut current = start;
        loop {
            ring.push(current);
            current = next.remove(&current).ok_or("Open cut boundary")?;
            if current == start {
                break;
            }
        }
        if ring.len() < 3 {
            return Err("Degenerate cut boundary".into());
        }
        loops.push(ring);
    }
    Ok(loops)
}

fn contains(ring: &[u32], p: Vec3, positions: &[Vec3], u: Vec3, v: Vec3) -> bool {
    let (x, y) = (p.dot(u), p.dot(v));
    let mut inside = false;
    for k in 0..ring.len() {
        let a = positions[ring[k] as usize];
        let b = positions[ring[(k + 1) % ring.len()] as usize];
        let (ax, ay, bx, by) = (a.dot(u), a.dot(v), b.dot(u), b.dot(v));
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    inside
}

fn triangulate_cap(
    ring: &mut Vec<u32>,
    positions: &mut Vec<Vec3>,
    normal: Vec3,
    color: Vec3,
) -> Result<Vec<Triangle>, String> {
    let mut result = vec![];
    if ring.len() > 4096 {
        return Err("Cap contour budget exceeded".into());
    }
    let center = ring.iter().map(|&i| positions[i as usize]).sum::<Vec3>() / ring.len() as f32;
    // A kernel point gives a robust fan and retains collinear boundary vertices,
    // including those introduced when a previous cap is cut again.
    if (0..ring.len()).all(|k| {
        let a = positions[ring[k] as usize];
        let b = positions[ring[(k + 1) % ring.len()] as usize];
        (b - a).cross(center - a).dot(normal) > 1e-8
    }) {
        let center_id = positions.len() as u32;
        positions.push(center);
        return Ok((0..ring.len())
            .map(|k| Triangle {
                indices: [center_id, ring[k], ring[(k + 1) % ring.len()]],
                color,
                normals: Some([normal; 3]),
            })
            .collect());
    }
    while ring.len() > 3 {
        let mut ear = None;
        for k in 0..ring.len() {
            let ids = [
                ring[(k + ring.len() - 1) % ring.len()],
                ring[k],
                ring[(k + 1) % ring.len()],
            ];
            let [a, b, c] = ids.map(|i| positions[i as usize]);
            if (b - a).cross(c - a).dot(normal) <= 1e-10 {
                continue;
            }
            let occupied = ring.iter().filter(|i| !ids.contains(i)).any(|&i| {
                let p = positions[i as usize];
                (b - a).cross(p - a).dot(normal) >= -1e-9
                    && (c - b).cross(p - b).dot(normal) >= -1e-9
                    && (a - c).cross(p - c).dot(normal) >= -1e-9
            });
            if !occupied {
                ear = Some((k, ids));
                break;
            }
        }
        let (k, indices) = ear.ok_or("Cannot triangulate cut cap")?;
        result.push(Triangle {
            indices,
            color,
            normals: Some([normal; 3]),
        });
        ring.remove(k);
    }
    result.push(Triangle {
        indices: [ring[0], ring[1], ring[2]],
        color,
        normals: Some([normal; 3]),
    });
    Ok(result)
}

fn components(triangles: Vec<Triangle>) -> Vec<Vec<Triangle>> {
    let mut parent: Vec<_> = (0..triangles.len()).collect();
    let mut owners = HashMap::with_capacity(triangles.len() * 2);
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for (i, t) in triangles.iter().enumerate() {
        for k in 0..3 {
            if let Some(&j) = owners.get(&edge(t.indices[k], t.indices[(k + 1) % 3])) {
                let a = root(&mut parent, i);
                let b = root(&mut parent, j);
                parent[a] = b;
            } else {
                owners.insert(edge(t.indices[k], t.indices[(k + 1) % 3]), i);
            }
        }
    }
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for (i, t) in triangles.into_iter().enumerate() {
        groups.entry(root(&mut parent, i)).or_default().push(t);
    }
    groups.into_values().collect()
}

/// Subtract each swept triangle from disjoint residual cap polygons. This computes
/// union area without counting repeated/overlapping poses twice. A hard budget
/// bounds adversarial fragmentation; failure never rounds an unknown area upward.
fn ribbon_area_coverage(
    caps: &[Triangle],
    positions: &[Vec3],
    samples: &[[Vec3; 2]],
    normal: Vec3,
) -> Result<f32, String> {
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let project = |p: Vec3| Vec2::new(p.dot(u), p.dot(v));
    let area = |p: &[Vec2]| -> f64 {
        p.iter()
            .zip(p.iter().cycle().skip(1))
            .map(|(a, b)| a.perp_dot(*b) as f64)
            .sum::<f64>()
            .abs()
            * 0.5
    };
    let mut total = 0.;
    let mut uncovered = 0.;
    let mut operations = 0usize;
    for cap in caps {
        let polygon: Vec<_> = cap
            .indices
            .iter()
            .map(|&i| project(positions[i as usize]))
            .collect();
        total += area(&polygon);
        let mut residual = vec![polygon];
        for pair in samples.windows(2) {
            let [a, b] = pair[0].map(project);
            let [c, d] = pair[1].map(project);
            for mut triangle in [[a, b, d], [a, d, c]] {
                let cross = (triangle[1] - triangle[0]).perp_dot(triangle[2] - triangle[0]);
                if cross.abs() < 1e-16 {
                    continue;
                }
                if cross < 0. {
                    triangle.swap(1, 2);
                }
                let mut next = Vec::new();
                for polygon in residual {
                    let mut inside = polygon;
                    for k in 0..3 {
                        if inside.len() < 3 {
                            break;
                        }
                        operations += inside.len();
                        if operations > 2_000_000 || next.len() > 8192 {
                            return Err("Blade coverage complexity budget exceeded".into());
                        }
                        let a = triangle[k];
                        let edge = triangle[(k + 1) % 3] - a;
                        let mut keep = Vec::new();
                        let mut outside = Vec::new();
                        for (&p, &q) in inside.iter().zip(inside.iter().cycle().skip(1)) {
                            let dp = edge.perp_dot(p - a);
                            let dq = edge.perp_dot(q - a);
                            if dp >= 0. {
                                keep.push(p);
                            }
                            if dp <= 0. {
                                outside.push(p);
                            }
                            if (dp > 0. && dq < 0.) || (dp < 0. && dq > 0.) {
                                let hit = p.lerp(q, dp / (dp - dq));
                                keep.push(hit);
                                outside.push(hit);
                            }
                        }
                        if outside.len() >= 3 && area(&outside) > 1e-16 {
                            next.push(outside);
                        }
                        inside = keep;
                    }
                }
                residual = next;
                if residual.is_empty() {
                    break;
                }
            }
            if residual.is_empty() {
                break;
            }
        }
        uncovered += residual.iter().map(|p| area(p)).sum::<f64>();
    }
    Ok(if total > 1e-14 {
        (1. - uncovered / total).clamp(0., 1.) as f32
    } else {
        0.
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cutting_recenter_preserves_closed_volume_normals_and_world_vertices() {
        let mut mesh = cube();
        mesh.smooth_normals();
        let shift = Vec3::new(0.3, 1.7, -0.2);
        // Use the constructor to ensure cached bounds and the BVH match the offset geometry.
        let positions = mesh.positions.iter().map(|p| *p + shift).collect();
        let mut mesh = CutMesh::from_triangles(positions, mesh.triangles).unwrap();
        mesh.spatial_index();
        let before = mesh.positions.clone();
        let normals: Vec<_> = mesh.triangles.iter().map(|t| t.normals).collect();
        let volume = mesh.volume();
        let center = mesh.recenter();
        assert!(center.distance(shift) < 1e-6);
        assert!(mesh.centroid().length() < 1e-6);
        assert!(mesh.bvh.get().is_none());
        mesh.validate().unwrap();
        mesh.spatial_index();
        assert!((mesh.volume() - volume).abs() < 1e-5);
        for (a, b) in before.iter().zip(&mesh.positions) {
            assert!(a.distance(*b + center) < 1e-6);
        }
        assert_eq!(
            normals,
            mesh.triangles.iter().map(|t| t.normals).collect::<Vec<_>>()
        );
    }

    #[test]
    fn cutting_output_bvh_is_lazy_and_reused_for_subsequent_queries() {
        let mesh = cube();
        assert!(mesh.bvh.get().is_some());
        let result = slice(&mesh, 0.1).unwrap();
        assert!(result.pieces.iter().all(|m| m.bvh.get().is_none()));
        let piece = &result.pieces[0];
        let a = Vec3::new(-3., 0.2, -3.);
        let b = Vec3::new(3., 0.2, -3.);
        let motion = Vec3::new(0., 0., 6.);
        piece.geometry_input(7, a, b, motion).unwrap();
        let index = piece.bvh.get().unwrap() as *const Bvh;
        piece.geometry_input(7, a, b, motion).unwrap();
        assert_eq!(piece.bvh.get().unwrap() as *const Bvh, index);
        let again = piece.slice(a, b, motion, Vec3::X).unwrap().unwrap();
        assert_eq!(again.pieces.len(), 2);
        for p in &again.pieces {
            p.validate().unwrap();
        }
    }
    #[test]
    fn cutting_snapshot_ignores_unrelated_animation_but_tracks_real_inputs() {
        let mut scene = crate::scene::SceneManager::default();
        let target = scene.add_triangle_mesh(cube()).unwrap();
        scene.set_cuttable(target, true, Vec3::X).unwrap();
        let sword = scene.add_cube(Vec3::Y * 3.);
        scene
            .set_cutter(sword, Some([-Vec3::Z * 3., Vec3::Z * 3.]))
            .unwrap();
        let decor = scene.add_cube(Vec3::X * 8.);
        scene.set_object_spin(decor, Vec3::Y);
        let before = scene.cut_snapshot(sword).unwrap();
        let revision = scene.render_revision();
        scene.update(0.2);
        assert_ne!(revision, scene.render_revision());
        assert!(before == scene.cut_snapshot(sword).unwrap());
        scene.set_object_position(target, Vec3::X);
        assert!(before != scene.cut_snapshot(sword).unwrap());
        let moved = scene.cut_snapshot(sword).unwrap();
        scene
            .set_cutter(sword, Some([-Vec3::Z * 2., Vec3::Z * 2.]))
            .unwrap();
        assert!(moved != scene.cut_snapshot(sword).unwrap());
        scene.clear();
        assert!(scene.cut_snapshot(sword).is_err());
    }
    #[test]
    fn cutting_prepared_geometry_preserves_reference_and_rejects_ambiguity() {
        let mut mesh = cube();
        mesh.smooth_normals();
        let a = Vec3::new(0.137, 3., -3.);
        let b = a + Vec3::Z * 6.;
        let motion = -Vec3::Y * 6.;
        let input = mesh.geometry_input(9, a, b, motion).unwrap();
        let plane = Vec3::from_slice(&input.plane);
        let mut rows: Vec<[f32; 4]> = input
            .positions
            .iter()
            .map(|p| [plane.dot(Vec3::from_slice(p)) + input.plane[3], 0., 0., 0.])
            .collect();
        for &[i, j] in &input.edges {
            let di = rows[i as usize][0];
            let dj = rows[j as usize][0];
            rows.push(if di * dj < 0. {
                let t = di / (di - dj);
                Vec3::from_slice(&input.positions[i as usize])
                    .lerp(Vec3::from_slice(&input.positions[j as usize]), t)
                    .extend(t)
                    .to_array()
            } else {
                [0., 0., 0., -1.]
            });
        }
        let prepared = input.decode(&rows).unwrap().unwrap();
        let reference = mesh.slice(a, b, motion, Vec3::X).unwrap().unwrap();
        let computed = mesh
            .slice_prepared(a, b, motion, Vec3::X, Some(&prepared))
            .unwrap()
            .unwrap();
        assert_eq!(reference.pieces.len(), computed.pieces.len());
        for (expected, actual) in reference.pieces.iter().zip(&computed.pieces) {
            assert_eq!(expected.positions, actual.positions);
            assert_eq!(
                expected
                    .render_mesh()
                    .vertices
                    .iter()
                    .map(|v| v.normal)
                    .collect::<Vec<_>>(),
                actual
                    .render_mesh()
                    .vertices
                    .iter()
                    .map(|v| v.normal)
                    .collect::<Vec<_>>()
            );
            actual.validate().unwrap();
        }
        assert!(input.decode(&[]).is_err());
        rows[0][0] = f32::NAN;
        assert!(input.decode(&rows).is_err());
        rows[0][0] = 0.;
        assert!(input.decode(&rows).unwrap().is_none());
    }
    pub fn cube() -> CutMesh {
        let positions = vec![
            Vec3::new(-1., -1., -1.),
            Vec3::new(1., -1., -1.),
            Vec3::new(1., 1., -1.),
            Vec3::new(-1., 1., -1.),
            Vec3::new(-1., -1., 1.),
            Vec3::new(1., -1., 1.),
            Vec3::new(1., 1., 1.),
            Vec3::new(-1., 1., 1.),
        ];
        CutMesh::new(
            positions,
            &[
                0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0,
                7, 3, 1, 2, 6, 1, 6, 5,
            ],
            Vec3::ONE,
        )
        .unwrap()
    }
    fn slice(mesh: &CutMesh, x: f32) -> Option<Slice> {
        mesh.slice(
            Vec3::new(x, -3., -3.),
            Vec3::new(x, 3., -3.),
            Vec3::new(0., 0., 6.),
            Vec3::X,
        )
        .unwrap()
    }
    fn assert_near(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 2e-5, "{a:?} != {b:?}");
    }

    #[test]
    fn cutting_physical_exact_area_thresholds_and_union() {
        let mesh = cube();
        for percentage in [0.25, 0.50, 0.60, 0.74, 0.75, 0.90] {
            let bottom = [Vec3::new(0., -1., -2.), Vec3::new(0., -1., 2.)];
            let top = bottom.map(|p| p + Vec3::Y * (2. * percentage));
            let samples = [bottom, top, bottom, top];
            let (progress, split) = mesh.slice_ribbon_progress(&samples, Vec3::X, 0.75).unwrap();
            assert!(
                (progress - percentage).abs() < 2e-5,
                "expected={percentage}, actual={progress}"
            );
            assert_eq!(split.is_some(), percentage >= 0.75);
            assert!(mesh.slice_ribbon(&samples, Vec3::X).unwrap().is_none());
            if let Some(split) = split {
                assert!(
                    (split.pieces.iter().map(CutMesh::volume).sum::<f64>() - mesh.volume()).abs()
                        < 1e-5
                );
                for piece in split.pieces {
                    piece.validate().unwrap();
                }
            }
        }
        let off_tip = [
            [Vec3::new(0., -2., 1.01), Vec3::new(0., -2., 2.)],
            [Vec3::new(0., 2., 1.01), Vec3::new(0., 2., 2.)],
        ];
        assert_eq!(
            mesh.slice_ribbon_progress(&off_tip, Vec3::X, 0.75)
                .unwrap()
                .0,
            0.
        );
        for threshold in [f32::MIN_POSITIVE, 1e-6, 2e-5] {
            let (progress, split) = mesh
                .slice_ribbon_progress(&off_tip, Vec3::X, threshold)
                .unwrap();
            assert_eq!(progress, 0.);
            assert!(split.is_none(), "zero coverage at threshold={threshold}");
        }
        let grazing = [
            [Vec3::new(0., -1., 0.9), Vec3::new(0., -1., 1.1)],
            [Vec3::new(0., 1., 0.9), Vec3::new(0., 1., 1.1)],
        ];
        let (progress, split) = mesh.slice_ribbon_progress(&grazing, Vec3::X, 0.75).unwrap();
        assert!((progress - 0.05).abs() < 2e-5);
        assert!(split.is_none());
    }

    #[test]
    fn cutting_smoothing_is_optional_area_weighted_and_shared() {
        let mut mesh = cube()
            .transformed(Mat4::from_scale(Vec3::new(2., 1., 0.5)))
            .unwrap();
        assert!(mesh.triangles.iter().all(|t| t.normals.is_none()));
        for vertices in mesh.render_mesh().vertices.as_chunks::<3>().0 {
            assert_eq!(vertices[0].normal, vertices[1].normal);
            assert_eq!(vertices[0].normal, vertices[2].normal);
        }
        mesh.smooth_normals();
        let mut shared = BTreeMap::new();
        for t in &mesh.triangles {
            for (i, n) in t.indices.into_iter().zip(t.normals.unwrap()) {
                if let Some(previous) = shared.insert(i, n) {
                    assert_eq!(previous, n);
                }
            }
        }
        assert_near(shared[&0], Vec3::new(-4., -8., -16.).normalize());
        assert_near(shared[&1], Vec3::new(4., -4., -8.).normalize());
        for (t, rendered) in mesh
            .triangles
            .iter()
            .zip(mesh.render_mesh().vertices.as_chunks::<3>().0)
        {
            for (n, vertex) in t.normals.unwrap().into_iter().zip(rendered) {
                assert_near(Vec3::from_array(vertex.normal), n);
            }
        }
    }

    // Check the original barycentric field, not a recomputed average on cut topology.
    fn assert_cut_normals(original: &CutMesh, piece: &CutMesh) {
        piece.validate().unwrap();
        let rendered = piece.render_mesh();
        let mut skin = BTreeMap::new();
        let mut cap_corners = vec![];
        for (t, vertices) in piece
            .triangles
            .iter()
            .zip(rendered.vertices.as_chunks::<3>().0)
        {
            let [a, b, c] = t.indices.map(|i| piece.positions[i as usize]);
            if t.color == Vec3::ONE {
                let normals = t.normals.unwrap();
                assert_eq!(t.color, Vec3::ONE);
                for (n, vertex) in normals.into_iter().zip(vertices) {
                    assert_near(Vec3::from_array(vertex.normal), n);
                }
                for (i, actual) in t.indices.into_iter().zip(normals) {
                    if let Some(previous) = skin.insert(i, actual) {
                        assert_near(previous, actual);
                    }
                    let p = piece.positions[i as usize];
                    let mut found = false;
                    for source in &original.triangles {
                        let [a, b, c] = source.indices.map(|j| original.positions[j as usize]);
                        let u = b - a;
                        let v = c - a;
                        let cross = u.cross(v);
                        if (p - a).dot(cross).abs() > 1e-5 {
                            continue;
                        }
                        let s = (p - a).cross(v).dot(cross) / cross.length_squared();
                        let r = u.cross(p - a).dot(cross) / cross.length_squared();
                        if s >= -EPS && r >= -EPS && s + r <= 1. + EPS {
                            let n = source.normals.unwrap();
                            assert_near(actual, n[0] * (1. - s - r) + n[1] * s + n[2] * r);
                            found = true;
                            break;
                        }
                    }
                    assert!(found, "skin corner left original surface: {p:?}");
                }
            } else {
                assert!(t.color == Vec3::X || t.color == Vec3::Y);
                let flat = (b - a).cross(c - a).normalize();
                for vertex in vertices {
                    assert_near(Vec3::from_array(vertex.normal), flat);
                }
                cap_corners.extend(t.indices.map(|i| (i, flat)));
            }
        }
        assert!(!skin.is_empty() && !cap_corners.is_empty());
        assert!(cap_corners
            .iter()
            .any(|(i, flat)| { skin.get(i).is_some_and(|n| n.normalize().dot(*flat) < 0.99) }));
    }

    #[test]
    fn cutting_skin_interpolation_survives_repeated_cuts_and_flat_caps() {
        let mut mesh = cube();
        mesh.smooth_normals();
        for piece in slice(&mesh, 0.25).unwrap().pieces {
            assert_cut_normals(&mesh, &piece);
            let repeated = piece
                .slice(
                    Vec3::new(-3., 0.17, -3.),
                    Vec3::new(3., 0.17, -3.),
                    Vec3::new(0., 0., 6.),
                    Vec3::Y,
                )
                .unwrap()
                .unwrap();
            for child in repeated.pieces {
                assert_cut_normals(&mesh, &child);
                assert!(child.triangles.iter().any(|t| t.color == Vec3::X));
                assert!(child.triangles.iter().any(|t| t.color == Vec3::Y));
            }
        }
    }

    #[test]
    fn cutting_smooth_normals_transform_inverse_transpose_and_mirrored_corners() {
        let mut mesh = cube();
        mesh.smooth_normals();
        for x in [2., -2.] {
            let scale = Vec3::new(x, 0.7, 1.3);
            let rotation = glam::Quat::from_rotation_y(0.4);
            let transform = Mat4::from_scale_rotation_translation(scale, rotation, Vec3::Y);
            let moved = mesh.transformed(transform).unwrap();
            moved.validate().unwrap();
            let order = if x < 0. { [0, 2, 1] } else { [0, 1, 2] };
            for (before, after) in mesh.triangles.iter().zip(&moved.triangles) {
                for (k, source) in order.into_iter().enumerate() {
                    assert_eq!(after.indices[k], before.indices[source]);
                    let expected = rotation * (before.normals.unwrap()[source] / scale);
                    assert_near(after.normals.unwrap()[k], expected);
                }
            }
            for piece in slice(&moved, 0.13).unwrap().pieces {
                assert_cut_normals(&moved, &piece);
            }
        }
    }

    #[test]
    fn cutting_closes_caps_and_conserves_volume() {
        let cube = cube();
        let result = slice(&cube, 0.25).unwrap();
        assert_eq!(result.pieces.len(), 2);
        for p in &result.pieces {
            p.validate().unwrap();
            assert!(p.triangles.iter().any(|t| t.color == Vec3::X));
        }
        assert!((result.pieces.iter().map(CutMesh::volume).sum::<f64>() - 8.).abs() < 1e-5);
    }
    #[test]
    fn cutting_repeated_oblique_mirrored_and_scaled() {
        let mesh = cube()
            .transformed(Mat4::from_scale_rotation_translation(
                Vec3::new(-1.2, 0.8, 1.),
                glam::Quat::from_rotation_y(0.3),
                Vec3::new(0.2, 0., 0.),
            ))
            .unwrap();
        let first = slice(&mesh, 0.).unwrap();
        let mut count = 0;
        let mut volume = 0.;
        for p in first.pieces {
            let r = p
                .slice(
                    Vec3::new(-4., 0., -4.),
                    Vec3::new(4., 0., -4.),
                    Vec3::new(0., 0., 8.),
                    Vec3::Y,
                )
                .unwrap()
                .unwrap();
            count += r.pieces.len();
            volume += r.pieces.iter().map(CutMesh::volume).sum::<f64>();
        }
        assert_eq!(count, 4);
        assert!((volume - mesh.volume()).abs() < 1e-4);
    }
    #[test]
    fn cutting_misses_tangents_and_short_blades_do_not_split() {
        let mesh = cube();
        assert!(slice(&mesh, 2.).is_none());
        assert!(slice(&mesh, 1.).is_none());
        assert!(mesh
            .slice(
                Vec3::new(0., -0.1, -2.),
                Vec3::new(0., 0.1, -2.),
                Vec3::new(0., 0., 4.),
                Vec3::X
            )
            .unwrap()
            .is_none());
        assert!(mesh.slice(Vec3::ZERO, Vec3::Y, Vec3::Y, Vec3::X).is_err());
    }
    #[test]
    fn cutting_through_existing_vertices() {
        let mesh = cube();
        let r = mesh
            .slice(
                Vec3::new(-3., -3., -3.),
                Vec3::new(3., 3., -3.),
                Vec3::new(0., 0., 6.),
                Vec3::X,
            )
            .unwrap()
            .unwrap();
        assert_eq!(r.pieces.len(), 2);
        for p in r.pieces {
            p.validate().unwrap();
        }
    }
    #[test]
    fn cutting_rejects_open_invalid_and_inward_meshes() {
        assert!(CutMesh::new(vec![Vec3::ZERO; 4], &[0, 1, 99], Vec3::ONE).is_err());
        let mesh = cube();
        let mut indices: Vec<_> = mesh.triangles.iter().flat_map(|t| t.indices).collect();
        indices.truncate(indices.len() - 3);
        assert!(CutMesh::new(mesh.positions.clone(), &indices, Vec3::ONE).is_err());
        let indices: Vec<_> = mesh
            .triangles
            .iter()
            .flat_map(|t| [t.indices[0], t.indices[2], t.indices[1]])
            .collect();
        assert!(CutMesh::new(mesh.positions, &indices, Vec3::ONE).is_err());
    }
    #[test]
    fn cutting_bvh_prunes_triangle_tests() {
        let mesh = cube();
        let result = slice(&mesh, 0.).unwrap();
        assert!(result.stats.triangle_tests < mesh.triangles.len());
        assert!(result.stats.nodes > 0);
    }
    #[test]
    fn cutting_near_rounded_pole_preserves_small_valid_faces() {
        // Reproduce the small offset made by projecting a real cursor, rather
        // than only testing an exactly axis-aligned plane through a vertex.
        let segments = 32;
        let rings = 18;
        let mut positions = vec![Vec3::new(0., 0.2, 0.), Vec3::new(0., 2.12, 0.)];
        let mut indices = vec![];
        for j in 1..rings {
            let theta = j as f32 / rings as f32 * std::f32::consts::PI;
            for i in 0..segments {
                let phi = i as f32 / segments as f32 * std::f32::consts::TAU;
                let radius = theta.sin()
                    * 1.1
                    * (1.0 - 0.1 * theta.cos())
                    * (1.0 + 0.045 * (phi * 5.0).cos());
                let y = 0.2 + 1.1 * (1.0 - theta.cos())
                    - 0.28 * (-((std::f32::consts::PI - theta) / 0.38).powi(2)).exp();
                positions.push(Vec3::new(phi.cos() * radius, y, phi.sin() * radius));
            }
        }
        for i in 0..segments {
            let next = (i + 1) % segments;
            indices.extend([0, 2 + i, 2 + next]);
            let top = 2 + (rings - 2) * segments;
            indices.extend([1, top + next, top + i]);
            for j in 0..rings - 2 {
                let b = 2 + j * segments + i;
                let bn = 2 + j * segments + next;
                indices.extend([b, b + segments, bn + segments, b, bn + segments, bn]);
            }
        }
        let apple = CutMesh::new(positions, &indices, Vec3::X).unwrap();
        for x in [0.00010728836, 0.00007, 0.0002] {
            let result = apple
                .slice(
                    Vec3::new(x, 3.5, -2.65),
                    Vec3::new(x, 3.5, 2.65),
                    Vec3::new(0., -3.305, 0.),
                    Vec3::ONE,
                )
                .unwrap()
                .unwrap();
            assert_eq!(result.pieces.len(), 2);
            for piece in result.pieces {
                piece.validate().unwrap();
            }
        }
    }

    fn concave_u() -> CutMesh {
        let outline = [
            [-2., -2.],
            [2., -2.],
            [2., 2.],
            [1., 2.],
            [1., -1.],
            [-1., -1.],
            [-1., 2.],
            [-2., 2.],
        ];
        let mut positions = vec![];
        for y in [-1., 1.] {
            for [x, z] in outline {
                positions.push(Vec3::new(x, y, z));
            }
        }
        let face = [0, 1, 4, 0, 4, 5, 0, 5, 7, 5, 6, 7, 1, 2, 3, 1, 3, 4];
        let mut indices = face.to_vec();
        for t in face.as_chunks::<3>().0 {
            indices.extend([t[0] + 8, t[2] + 8, t[1] + 8]);
        }
        for i in 0..8 {
            let j = (i + 1) % 8;
            indices.extend([i, i + 8, j + 8, i, j + 8, j]);
        }
        CutMesh::new(positions, &indices, Vec3::ONE).unwrap()
    }
    #[test]
    fn cutting_concave_non_star_cap_and_multiple_components() {
        let mesh = concave_u();
        let horizontal = mesh
            .slice(
                Vec3::new(-4., 0., -4.),
                Vec3::new(4., 0., -4.),
                Vec3::new(0., 0., 8.),
                Vec3::X,
            )
            .unwrap()
            .unwrap();
        assert_eq!(horizontal.pieces.len(), 2);
        for p in horizontal.pieces {
            p.validate().unwrap();
        }
        // Across the two arms: one connected base and two separate tips.
        let arms = mesh
            .slice(
                Vec3::new(-4., -3., 0.),
                Vec3::new(4., -3., 0.),
                Vec3::new(0., 6., 0.),
                Vec3::X,
            )
            .unwrap()
            .unwrap();
        assert_eq!(arms.pieces.len(), 3);
        assert!(
            (arms.pieces.iter().map(CutMesh::volume).sum::<f64>() - mesh.volume()).abs() < 1e-5
        );
    }

    #[test]
    fn opacity_and_color_cutout_validate_extract_and_survive_closed_mesh_cuts() {
        use crate::{
            scene::{RenderOptions, SceneManager},
            visibility::{RenderPassKind, RenderQueue},
        };
        let mut scene = SceneManager::default();
        let target = scene.add_triangle_mesh(cube()).unwrap();
        assert_eq!(scene.objects()[&target].color_cutout, 0.);
        assert_eq!(scene.objects()[&target].opacity, 1.);
        assert_eq!(scene.objects()[&target].opaque_above, 8.);
        assert_eq!(scene.objects()[&target].opacity_variation, 0.);
        assert_eq!(scene.objects()[&target].opacity_coverage, -1.);
        let mut default_queue = RenderQueue::default();
        default_queue.extract_pass(&scene, glam::Mat4::IDENTITY, false, RenderPassKind::Main);
        assert_eq!(
            default_queue.imported[&target][0].surface_params,
            [0., -1., 0., 0.]
        );
        scene.set_render_options(
            target,
            RenderOptions {
                is_static: true,
                ..Default::default()
            },
        );
        let render = scene.render_revision();
        let static_revision = scene.static_revision();
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(!scene.set_color_cutout(target, invalid));
        }
        assert!(!scene.set_color_cutout(u32::MAX, 0.5));
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(!scene.set_opacity(target, invalid, 0.25, 0.7, 0.72));
        }
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 8.1] {
            assert!(!scene.set_opacity(target, 0.6, invalid, 0.7, 0.72));
        }
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert!(!scene.set_opacity(target, 0.6, 0.25, invalid, 0.72));
        }
        for invalid in [f32::NAN, f32::INFINITY, -0.1, -1.01, -2.0, 1.1] {
            assert!(!scene.set_opacity(target, 0.6, 0.25, 0.7, invalid));
        }
        assert!(!scene.set_opacity(u32::MAX, 0.6, 0.25, 0.7, 0.72));
        assert_eq!(scene.render_revision(), render);
        assert!(scene.set_color_cutout(target, 1.));
        assert!(scene.set_color_cutout(target, 0.));
        assert!(scene.set_color_cutout(target, 0.25));
        assert!(scene.render_revision() > render);
        assert!(scene.static_revision() > static_revision);
        assert!(scene.set_emission(target, 2.));
        for coverage in [-1.0, 0.0, 1.0, 0.72] {
            assert!(scene.set_opacity(target, 0.6, 0.25, 0.7, coverage));
        }
        let mut queue = RenderQueue::default();
        queue.extract_pass(&scene, glam::Mat4::IDENTITY, false, RenderPassKind::Main);
        assert_eq!(queue.imported[&target][0].emission, [2., 0.25, 0.6, 0.25]);
        assert_eq!(
            queue.imported[&target][0].surface_params,
            [0.7, 0.72, 0., 0.]
        );
        let volume = scene.triangle_meshes()[&target].mesh.volume();
        scene.set_cuttable(target, true, Vec3::ONE).unwrap();
        let sword = scene.add_cube(Vec3::new(0., -3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 6., 0.)]))
            .unwrap();
        let report = scene.cut_with(sword, Vec3::new(0., 0., 6.), 0.).unwrap();
        assert_eq!(report.removed, [target]);
        assert_eq!(report.pieces.len(), 2);
        let mut piece_volume = 0.;
        for id in report.pieces {
            assert_eq!(scene.objects()[&id].color_cutout, 0.25);
            assert_eq!(scene.objects()[&id].emission, 2.);
            assert_eq!(scene.objects()[&id].opacity, 0.6);
            assert_eq!(scene.objects()[&id].opaque_above, 0.25);
            assert_eq!(scene.objects()[&id].opacity_variation, 0.7);
            assert_eq!(scene.objects()[&id].opacity_coverage, 0.72);
            let mesh = scene.triangle_meshes()[&id].mesh.cpu().unwrap();
            mesh.validate().unwrap();
            piece_volume += mesh.volume();
        }
        assert!((piece_volume - volume).abs() < 1e-5);
    }

    #[test]
    fn cutting_scene_opt_in_finite_sweep_repeat_and_cleanup() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let rock = scene.add_triangle_mesh(cube()).unwrap();
        let protected = scene.add_triangle_mesh(cube()).unwrap();
        let sword = scene.add_cube(Vec3::new(0., -3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 6., 0.)]))
            .unwrap();
        assert!(scene
            .cut_with(sword, Vec3::new(0., 0., 6.), 0.)
            .unwrap()
            .pieces
            .is_empty());
        scene.set_cuttable(rock, true, Vec3::X).unwrap();
        scene.set_object_position(sword, Vec3::new(0., -3., -3.));
        let first = scene.cut_with(sword, Vec3::new(0., 0., 6.), 0.).unwrap();
        assert_eq!(first.removed, [rock]);
        assert_eq!(first.pieces.len(), 2);
        assert!(scene.object_exists(protected));
        assert!(!scene.object_exists(rock));
        assert!(first
            .pieces
            .iter()
            .all(|id| scene.triangle_meshes()[id].cuttable));
        scene.set_object_position(sword, Vec3::new(-3., 0., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(6., 0., 0.)]))
            .unwrap();
        let second = scene.cut_with(sword, Vec3::new(0., 0., 6.), 0.2).unwrap();
        assert_eq!(second.pieces.len(), 4);
        let volume: f64 = second
            .pieces
            .iter()
            .map(|id| scene.triangle_meshes()[id].mesh.volume())
            .sum();
        assert!((volume - 8.).abs() < 1e-5);
        assert!(scene.set_cuttable(sword, true, Vec3::X).is_err());
        scene.remove_object(sword);
        assert!(scene.cut_with(sword, Vec3::Z, 0.).is_err());
        scene.clear();
        assert!(scene.triangle_meshes().is_empty());
    }

    #[test]
    fn cutting_scene_rejection_is_atomic_across_targets() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let first = scene.add_triangle_mesh(cube()).unwrap();
        scene.set_cuttable(first, true, Vec3::X).unwrap();
        let mut hollow = cube();
        let inner = cube();
        let base = hollow.positions.len() as u32;
        hollow
            .positions
            .extend(inner.positions.iter().map(|p| *p * 0.5));
        hollow
            .triangles
            .extend(inner.triangles.iter().map(|t| Triangle {
                indices: [
                    t.indices[0] + base,
                    t.indices[2] + base,
                    t.indices[1] + base,
                ],
                color: Vec3::ONE,
                normals: None,
            }));
        let hollow = CutMesh::from_triangles(hollow.positions, hollow.triangles).unwrap();
        hollow.validate().unwrap();
        let second = scene.add_triangle_mesh(hollow).unwrap();
        scene.set_cuttable(second, true, Vec3::X).unwrap();
        let sword = scene.add_cube(Vec3::new(0., -3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 6., 0.)]))
            .unwrap();
        let count = scene.objects().len();
        let error = scene
            .cut_with(sword, Vec3::new(0., 0., 6.), 0.2)
            .err()
            .unwrap();
        assert!(error.contains("holes"));
        assert_eq!(scene.objects().len(), count);
        assert!(scene.object_exists(first) && scene.object_exists(second));
        assert_eq!(
            scene.objects()[&sword].transform.position,
            Vec3::new(0., -3., -3.)
        );
    }

    #[test]
    fn cutting_scene_separation_uses_real_halfspace_not_box_center() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let positions = vec![
            Vec3::new(-2., 1., 1.),
            Vec3::new(1., -2., 1.),
            Vec3::new(1., 1., -2.),
            Vec3::new(0., 0., 0.3),
            Vec3::new(0., 0., -0.3),
        ];
        let mut indices = vec![0, 1, 3, 1, 2, 3, 2, 0, 3, 1, 0, 4, 2, 1, 4, 0, 2, 4];
        if CutMesh::new(positions.clone(), &indices, Vec3::ONE).is_err() {
            for t in indices.as_chunks_mut::<3>().0 {
                t.swap(1, 2);
            }
        }
        let rock = scene
            .add_triangle_mesh(CutMesh::new(positions, &indices, Vec3::ONE).unwrap())
            .unwrap();
        scene.set_cuttable(rock, true, Vec3::X).unwrap();
        let u = Vec3::new(1., -1., 0.).normalize() * 8.;
        let v = Vec3::new(1., 1., -2.).normalize() * 8.;
        let normal = u.cross(v).normalize();
        let sword = scene.add_cube(-(u + v) * 0.5);
        scene.set_cutter(sword, Some([Vec3::ZERO, u])).unwrap();
        let result = scene.cut_with(sword, v, 0.4).unwrap();
        assert_eq!(result.pieces.len(), 2);
        let mut sides: Vec<_> = result
            .pieces
            .iter()
            .map(|id| scene.objects()[id].transform.position.dot(normal))
            .collect();
        sides.sort_by(f32::total_cmp);
        assert!((sides[0] + 0.2).abs() < 1e-5 && (sides[1] - 0.2).abs() < 1e-5);
    }

    #[test]
    fn cutting_scatter_opens_halves_and_lands_without_losing_volume() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let rock = scene.add_triangle_mesh(cube()).unwrap();
        scene.set_cuttable(rock, true, Vec3::X).unwrap();
        let sword = scene.add_cube(Vec3::new(0., 3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 0., 6.)]))
            .unwrap();
        let report = scene
            .cut_with_scatter(sword, Vec3::new(0., -6., 0.), 0.04, 0.2, 0.7, 0.5)
            .unwrap();
        assert_eq!(report.pieces.len(), 2);
        scene.paused = true;
        scene.update(0.4);
        assert!(report
            .pieces
            .iter()
            .all(|id| scene.objects()[id].transform.rotation == Vec3::ZERO));
        scene.paused = false;
        scene.update(0.3);
        assert!(report
            .pieces
            .iter()
            .all(|id| scene.objects()[id].transform.rotation.length() > 0.1));
        scene.update(1.0);
        let mut directions = vec![];
        for id in &report.pieces {
            let object = &scene.objects()[id];
            let mesh = scene.triangle_meshes()[id].mesh.cpu().unwrap();
            mesh.validate().unwrap();
            let bottom = mesh
                .positions
                .iter()
                .map(|&p| object.transform.model().transform_point3(p).y)
                .fold(f32::INFINITY, f32::min);
            assert!((bottom - 0.2).abs() < 1e-5);
            directions.push(object.transform.position.x);
        }
        assert!(directions[0] * directions[1] < 0.0);
        assert!(
            (report
                .pieces
                .iter()
                .map(|id| scene.triangle_meshes()[id].mesh.volume())
                .sum::<f64>()
                - 8.0)
                .abs()
                < 1e-5
        );
        let id = report.pieces[0];
        let before = scene.objects()[&id].transform.position;
        scene.update(1.0);
        assert_eq!(scene.objects()[&id].transform.position, before);
    }
    #[test]
    fn cutting_scatter_invalid_options_are_atomic_and_teleport_cancels_motion() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        let rock = scene.add_triangle_mesh(cube()).unwrap();
        scene.set_cuttable(rock, true, Vec3::X).unwrap();
        let sword = scene.add_cube(Vec3::new(0., 3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 0., 6.)]))
            .unwrap();
        assert!(scene
            .cut_with_scatter(sword, Vec3::new(0., -6., 0.), 0., f32::NAN, 0.7, 0.5)
            .is_err());
        assert!(scene.object_exists(rock));
        let report = scene
            .cut_with_scatter(sword, Vec3::new(0., -6., 0.), 0., 0.2, 0.7, 0.5)
            .unwrap();
        let id = report.pieces[0];
        scene.set_object_position(id, Vec3::new(5., 2., 3.));
        scene.update(1.0);
        assert_eq!(
            scene.objects()[&id].transform.position,
            Vec3::new(5., 2., 3.)
        );
        scene.clear();
        scene.update(1.0);
        assert!(scene.objects().is_empty());
    }
    #[test]
    fn cutting_scene_bvh_rejects_distant_objects_and_tracks_transform() {
        use crate::scene::SceneManager;
        let mut scene = SceneManager::default();
        for i in 0..128 {
            let id = scene.add_triangle_mesh(cube()).unwrap();
            scene.set_cuttable(id, true, Vec3::X).unwrap();
            scene.set_object_position(id, Vec3::new(i as f32 * 5., 0., 0.));
        }
        let sword = scene.add_cube(Vec3::new(0., -3., -3.));
        scene
            .set_cutter(sword, Some([Vec3::ZERO, Vec3::new(0., 6., 0.)]))
            .unwrap();
        let result = scene.cut_with(sword, Vec3::new(0., 0., 6.), 0.).unwrap();
        assert_eq!(result.object_candidates, 1);
        assert_eq!(result.pieces.len(), 2);
        let id = result.pieces[0];
        scene.set_object_position(id, Vec3::new(1000., 0., 0.));
        scene.set_object_position(sword, Vec3::new(0., -3., -3.));
        let result = scene.cut_with(sword, Vec3::new(0., 0., 6.), 0.).unwrap();
        assert_eq!(result.object_candidates, 1);
        assert!(result.pieces.is_empty());
    }
}
