//! Static AABB collisions. The scene owner rebuilds the BVH on collider revisions.
//!
//! Character motion resolves Y, then X, then Z. This deliberately provides no
//! stepping, triangle collision, or recovery from an initially overlapping box.

use glam::Vec3;

const LEAF_SIZE: usize = 4;
const CONTACT_EPSILON: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// Rejects non-finite coordinates and zero or negative extents on any axis.
    pub fn new(min: Vec3, max: Vec3) -> Option<Self> {
        (min.is_finite() && max.is_finite() && max.cmpgt(min).all()).then_some(Self { min, max })
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    // Inclusive broadphase overlap retains touching faces for support probing.
    fn intersects(self, other: Self) -> bool {
        self.min.cmple(other.max).all() && self.max.cmpge(other.min).all()
    }

    fn translated(self, offset: Vec3) -> Self {
        Self {
            min: self.min + offset,
            max: self.max + offset,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Collider {
    pub id: u32,
    pub bounds: Aabb,
}

#[derive(Debug)]
enum NodeKind {
    Leaf { start: usize, end: usize },
    Branch { left: usize, right: usize },
}

#[derive(Debug)]
struct Node {
    bounds: Aabb,
    kind: NodeKind,
}

#[derive(Default)]
pub struct CollisionWorld {
    colliders: Vec<Collider>,
    nodes: Vec<Node>,
}

#[derive(Clone, Copy, Debug)]
pub struct MotionResult {
    pub position: Vec3,
    pub grounded: bool,
    pub hit_ceiling: bool,
    /// Exact broadphase query hits, after filtering out `ignore_id`.
    pub candidates: usize,
}

impl CollisionWorld {
    pub fn rebuild(&mut self, colliders: Vec<Collider>) {
        // Public AABB fields allow malformed boxes; do not let them poison BVH bounds.
        self.colliders = colliders
            .into_iter()
            .filter(|c| Aabb::new(c.bounds.min, c.bounds.max).is_some())
            .collect();
        self.nodes.clear();
        if !self.colliders.is_empty() {
            self.build_node(0, self.colliders.len());
        }
    }

    fn build_node(&mut self, start: usize, end: usize) -> usize {
        let bounds = self.colliders[start + 1..end]
            .iter()
            .fold(self.colliders[start].bounds, |b, c| b.union(c.bounds));
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            kind: NodeKind::Leaf { start, end },
        });
        if end - start > LEAF_SIZE {
            let extent = bounds.max - bounds.min;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let middle = start + (end - start) / 2;
            self.colliders[start..end].select_nth_unstable_by(middle - start, |a, b| {
                let center_a = a.bounds.min[axis] * 0.5 + a.bounds.max[axis] * 0.5;
                let center_b = b.bounds.min[axis] * 0.5 + b.bounds.max[axis] * 0.5;
                center_a.total_cmp(&center_b).then(a.id.cmp(&b.id))
            });
            let left = self.build_node(start, middle);
            let right = self.build_node(middle, end);
            self.nodes[index].kind = NodeKind::Branch { left, right };
        }
        index
    }

    /// Returns exact inclusive AABB overlaps; result order is unspecified.
    pub fn query(&self, bounds: Aabb, ignore_id: Option<u32>) -> Vec<Collider> {
        let mut hits = Vec::new();
        self.visit_query(bounds, ignore_id, |c| hits.push(c));
        hits
    }

    // Returns leaf collider checks so native tests can verify actual BVH pruning.
    fn visit_query(
        &self,
        bounds: Aabb,
        ignore_id: Option<u32>,
        mut hit: impl FnMut(Collider),
    ) -> usize {
        if self.nodes.is_empty() {
            return 0;
        }
        let mut stack = vec![0];
        let mut checks = 0;
        while let Some(index) = stack.pop() {
            let node = &self.nodes[index];
            if !node.bounds.intersects(bounds) {
                continue;
            }
            match node.kind {
                NodeKind::Leaf { start, end } => {
                    for collider in &self.colliders[start..end] {
                        checks += 1;
                        if Some(collider.id) != ignore_id && collider.bounds.intersects(bounds) {
                            hit(*collider);
                        }
                    }
                }
                NodeKind::Branch { left, right } => {
                    stack.push(right);
                    stack.push(left);
                }
            }
        }
        checks
    }

    /// Sweeps an upright box with its bottom at `feet.y`, preserving wall sliding.
    /// Invalid motion inputs return the original feet without collision flags.
    pub fn move_character(
        &self,
        feet: Vec3,
        half_extents: Vec3,
        delta: Vec3,
        ignore_id: Option<u32>,
    ) -> MotionResult {
        let mut result = MotionResult {
            position: feet,
            grounded: false,
            hit_ceiling: false,
            candidates: 0,
        };
        if !feet.is_finite()
            || !half_extents.is_finite()
            || !half_extents.cmpgt(Vec3::ZERO).all()
            || !delta.is_finite()
            || !(feet + delta).is_finite()
        {
            return result;
        }
        let horizontal = Vec3::new(half_extents.x, 0.0, half_extents.z);
        let Some(mut body) = Aabb::new(
            feet - horizontal,
            feet + horizontal + Vec3::Y * (half_extents.y * 2.0),
        ) else {
            return result;
        };
        let destination = body.translated(delta);
        if !destination.min.is_finite() || !destination.max.is_finite() {
            return result;
        }
        let swept = body.union(destination);
        let candidates = self.query(
            Aabb {
                min: swept.min - Vec3::splat(CONTACT_EPSILON),
                max: swept.max + Vec3::splat(CONTACT_EPSILON),
            },
            ignore_id,
        );
        result.candidates = candidates.len();

        for axis in [1, 0, 2] {
            let requested = delta[axis];
            let mut travel = requested;
            let mut contact_face = None;
            for collider in &candidates {
                let obstacle = collider.bounds;
                if !overlaps_other_axes(body, obstacle, axis) {
                    continue;
                }
                if requested > 0.0 {
                    let gap = obstacle.min[axis] - body.max[axis];
                    if gap >= -CONTACT_EPSILON && gap <= travel {
                        travel = gap.max(0.0);
                        contact_face = Some(obstacle.min[axis]);
                    }
                } else if requested < 0.0 {
                    let gap = obstacle.max[axis] - body.min[axis];
                    if gap <= CONTACT_EPSILON && gap >= travel {
                        travel = gap.min(0.0);
                        contact_face = Some(obstacle.max[axis]);
                    }
                }
            }
            let mut offset = Vec3::ZERO;
            offset[axis] = travel;
            body = body.translated(offset);
            result.position[axis] += travel;
            if let Some(face) = contact_face {
                // Snap to the face to prevent subtraction/addition roundoff from
                // making the next sweep start just inside a thin obstacle.
                let size = half_extents[axis] * 2.0;
                if requested > 0.0 {
                    body.max[axis] = face;
                    body.min[axis] = face - size;
                } else {
                    body.min[axis] = face;
                    body.max[axis] = face + size;
                }
                result.position[axis] = if axis == 1 {
                    body.min[axis]
                } else if requested > 0.0 {
                    face - half_extents[axis]
                } else {
                    face + half_extents[axis]
                };
                if axis == 1 && requested > 0.0 {
                    result.hit_ceiling = true;
                }
            }
        }

        // Probe final support, after horizontal movement: landing then walking off
        // a platform in the same move must not leave a stale grounded flag.
        result.grounded = delta.y <= 0.0
            && candidates.iter().any(|c| {
                let gap = body.min.y - c.bounds.max.y;
                (-CONTACT_EPSILON..=CONTACT_EPSILON).contains(&gap)
                    && overlaps_other_axes(body, c.bounds, 1)
            });
        result
    }
}

fn overlaps_other_axes(a: Aabb, b: Aabb, movement_axis: usize) -> bool {
    (0..3).filter(|axis| *axis != movement_axis).all(|axis| {
        // Strict overlap lets the character slide along an already touching face.
        a.min[axis] < b.max[axis] && a.max[axis] > b.min[axis]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HALF: Vec3 = Vec3::new(0.5, 1.0, 0.5);

    fn collider(id: u32, min: [f32; 3], max: [f32; 3]) -> Collider {
        Collider {
            id,
            bounds: Aabb::new(Vec3::from_array(min), Vec3::from_array(max)).unwrap(),
        }
    }

    fn world(colliders: Vec<Collider>) -> CollisionWorld {
        let mut world = CollisionWorld::default();
        world.rebuild(colliders);
        world
    }

    fn assert_position(actual: Vec3, expected: Vec3) {
        assert!(
            actual.abs_diff_eq(expected, 1.0e-4),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn aabb_requires_finite_positive_extents() {
        assert!(Aabb::new(Vec3::ZERO, Vec3::ONE).is_some());
        for max in [Vec3::ZERO, Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_ONE] {
            assert!(Aabb::new(Vec3::ZERO, max).is_none());
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(Aabb::new(Vec3::new(value, 0.0, 0.0), Vec3::ONE).is_none());
            assert!(Aabb::new(Vec3::ZERO, Vec3::new(1.0, value, 1.0)).is_none());
        }
    }

    #[test]
    fn wall_sliding_preserves_tangent_motion_and_face_contact() {
        let world = world(vec![collider(1, [2.0, -5.0, -20.0], [2.1, 5.0, 20.0])]);
        let first = world.move_character(Vec3::ZERO, HALF, Vec3::new(8.0, 0.0, 3.0), None);
        assert_position(first.position, Vec3::new(1.5, 0.0, 3.0));
        let second = world.move_character(first.position, HALF, Vec3::new(4.0, 0.0, 2.0), None);
        assert_position(second.position, Vec3::new(1.5, 0.0, 5.0));
        assert!(!second.grounded && !second.hit_ceiling);
    }

    #[test]
    fn perpendicular_corner_blocks_both_horizontal_axes() {
        let world = world(vec![
            collider(1, [2.0, -2.0, -10.0], [2.1, 5.0, 10.0]),
            collider(2, [-10.0, -2.0, 2.0], [10.0, 5.0, 2.1]),
        ]);
        let result = world.move_character(Vec3::ZERO, HALF, Vec3::new(50.0, 0.0, 50.0), None);
        assert_position(result.position, Vec3::new(1.5, 0.0, 1.5));
        assert_eq!(result.candidates, 2);
    }

    #[test]
    fn high_speed_cannot_cross_thin_walls_in_either_direction() {
        let world = world(vec![collider(1, [2.0, -5.0, -5.0], [2.001, 5.0, 5.0])]);
        let positive = world.move_character(Vec3::ZERO, HALF, Vec3::X * 10000.0, None);
        assert_position(positive.position, Vec3::new(1.5, 0.0, 0.0));
        let negative = world.move_character(Vec3::X * 10.0, HALF, -Vec3::X * 10000.0, None);
        assert_position(negative.position, Vec3::new(2.501, 0.0, 0.0));
    }

    #[test]
    fn repeated_fractional_face_contacts_do_not_tunnel() {
        let world = world(vec![collider(
            1,
            [2.1337, -5.0, -1000.0],
            [2.1338, 5.0, 1000.0],
        )]);
        let half = Vec3::new(0.3, 0.8, 0.3);
        let mut feet = Vec3::new(0.123, 0.0, 0.0);
        for _ in 0..100 {
            feet = world
                .move_character(feet, half, Vec3::new(1000.0, 0.0, 0.125), None)
                .position;
            assert!((feet.x - (2.1337 - half.x)).abs() <= CONTACT_EPSILON);
        }
        assert_position(feet, Vec3::new(2.1337 - half.x, 0.0, 12.5));
    }

    #[test]
    fn z_sweep_blocks_a_thin_wall() {
        let world = world(vec![collider(1, [-5.0, -5.0, 2.0], [5.0, 5.0, 2.001])]);
        let result = world.move_character(Vec3::ZERO, HALF, Vec3::Z * 10000.0, None);
        assert_position(result.position, Vec3::new(0.0, 0.0, 1.5));
        let reverse = world.move_character(Vec3::Z * 10.0, HALF, -Vec3::Z * 10000.0, None);
        assert_position(reverse.position, Vec3::new(0.0, 0.0, 2.501));
    }

    #[test]
    fn floor_landing_and_zero_vertical_delta_keep_support() {
        let world = world(vec![collider(1, [-20.0, -0.01, -20.0], [20.0, 0.0, 20.0])]);
        let landing = world.move_character(Vec3::Y * 50.0, HALF, -Vec3::Y * 1000.0, None);
        assert_position(landing.position, Vec3::ZERO);
        assert!(landing.grounded && !landing.hit_ceiling);
        let standing = world.move_character(landing.position, HALF, Vec3::ZERO, None);
        assert!(standing.grounded);
        let walking = world.move_character(standing.position, HALF, Vec3::X * 4.0, None);
        assert_position(walking.position, Vec3::X * 4.0);
        assert!(walking.grounded);
    }

    #[test]
    fn highest_platform_catches_fall_before_floor() {
        let world = world(vec![
            collider(1, [-20.0, -1.0, -20.0], [20.0, 0.0, 20.0]),
            collider(2, [-2.0, 2.9, -2.0], [2.0, 3.0, 2.0]),
        ]);
        let result = world.move_character(Vec3::Y * 10.0, HALF, -Vec3::Y * 100.0, None);
        assert_position(result.position, Vec3::Y * 3.0);
        assert!(result.grounded);
        assert_eq!(result.candidates, 2);
    }

    #[test]
    fn ceiling_stops_head_at_high_speed_and_exact_contact() {
        let world = world(vec![collider(1, [-5.0, 4.0, -5.0], [5.0, 4.01, 5.0])]);
        for rise in [2.0, 1000.0] {
            let result = world.move_character(Vec3::ZERO, HALF, Vec3::Y * rise, None);
            assert_position(result.position, Vec3::Y * 2.0);
            assert!(result.hit_ceiling && !result.grounded);
        }
    }

    #[test]
    fn walking_off_support_clears_grounded_even_after_landing() {
        let world = world(vec![collider(1, [-1.0, -0.1, -1.0], [1.0, 0.0, 1.0])]);
        let edge = world.move_character(Vec3::ZERO, HALF, Vec3::X * 1.5, None);
        assert!(!edge.grounded); // Mere edge contact has no supporting footprint.
        let off = world.move_character(Vec3::Y * 2.0, HALF, Vec3::new(4.0, -5.0, 0.0), None);
        assert_position(off.position, Vec3::X * 4.0);
        assert!(!off.grounded);
        let fall = world.move_character(off.position, HALF, -Vec3::Y, None);
        assert_position(fall.position, Vec3::new(4.0, -1.0, 0.0));
        assert!(!fall.grounded);
    }

    #[test]
    fn support_probe_is_tiny_and_jumping_clears_support() {
        let world = world(vec![collider(1, [-5.0, -1.0, -5.0], [5.0, 0.0, 5.0])]);
        let near = world.move_character(Vec3::Y * (CONTACT_EPSILON * 0.5), HALF, Vec3::ZERO, None);
        assert!(near.grounded);
        let far = world.move_character(Vec3::Y * (CONTACT_EPSILON * 2.0), HALF, Vec3::ZERO, None);
        assert!(!far.grounded);
        let jump = world.move_character(Vec3::ZERO, HALF, Vec3::Y * (CONTACT_EPSILON * 0.5), None);
        assert!(!jump.grounded);
    }

    #[test]
    fn ignores_own_id_in_queries_motion_and_support() {
        let bounds = collider(7, [2.0, -1.0, -5.0], [2.1, 5.0, 5.0]);
        let world = world(vec![bounds]);
        assert_eq!(world.query(bounds.bounds, None).len(), 1);
        assert!(world.query(bounds.bounds, Some(7)).is_empty());
        let result = world.move_character(Vec3::ZERO, HALF, Vec3::X * 10.0, Some(7));
        assert_position(result.position, Vec3::X * 10.0);
        assert_eq!(result.candidates, 0);
        let floor = world.move_character(Vec3::new(2.0, 5.0, 0.0), HALF, Vec3::ZERO, Some(7));
        assert!(!floor.grounded);
    }

    #[test]
    fn no_auto_step_on_low_obstacle() {
        let world = world(vec![collider(1, [2.0, 0.0, -5.0], [3.0, 0.2, 5.0])]);
        let result = world.move_character(Vec3::ZERO, HALF, Vec3::X * 10.0, None);
        assert_position(result.position, Vec3::X * 1.5);
    }

    #[test]
    fn bvh_exactly_matches_brute_force_queries() {
        let mut seed = 0x1234_5678_u32;
        let mut random = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / 16_777_216.0
        };
        let mut colliders = Vec::new();
        for id in 0..257 {
            let min = Vec3::new(random(), random(), random()) * 100.0 - Vec3::splat(50.0);
            let extent = Vec3::new(random(), random(), random()) * 8.0 + Vec3::splat(0.01);
            colliders.push(Collider {
                id,
                bounds: Aabb::new(min, min + extent).unwrap(),
            });
        }
        let world = world(colliders.clone());
        for i in 0..200 {
            let min = Vec3::new(random(), random(), random()) * 100.0 - Vec3::splat(50.0);
            let bounds = Aabb::new(min, min + Vec3::splat(20.0)).unwrap();
            let ignore = (i % 2 == 0).then_some(i);
            let mut expected: Vec<_> = colliders
                .iter()
                .filter(|c| Some(c.id) != ignore && c.bounds.intersects(bounds))
                .map(|c| c.id)
                .collect();
            let mut actual: Vec<_> = world.query(bounds, ignore).iter().map(|c| c.id).collect();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected, "query {i}");
        }
        // Every collider's own bounds, including exact face/edge overlaps.
        for c in &colliders {
            let mut expected: Vec<_> = colliders
                .iter()
                .filter(|other| other.bounds.intersects(c.bounds))
                .map(|other| other.id)
                .collect();
            let mut actual: Vec<_> = world
                .query(c.bounds, None)
                .iter()
                .map(|other| other.id)
                .collect();
            actual.sort_unstable();
            expected.sort_unstable();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn bvh_prunes_separated_colliders_and_rebuild_clears_old_nodes() {
        let mut world = world(
            (0..1024)
                .map(|id| {
                    let x = id as f32 * 10.0;
                    collider(id, [x, 0.0, 0.0], [x + 1.0, 1.0, 1.0])
                })
                .collect(),
        );
        let bounds = Aabb::new(Vec3::ZERO, Vec3::ONE).unwrap();
        let mut hits = Vec::new();
        let checks = world.visit_query(bounds, None, |c| hits.push(c.id));
        assert_eq!(hits, vec![0]);
        assert!(checks <= LEAF_SIZE, "tested {checks} of 1024 colliders");
        let motion = world.move_character(Vec3::new(-2.0, 0.0, 0.5), HALF, Vec3::X * 4.0, None);
        assert_eq!(motion.candidates, 1);
        world.rebuild(Vec::new());
        assert!(world.query(bounds, None).is_empty());
        assert!(world.nodes.is_empty());
        world.rebuild(vec![collider(9, [0.0; 3], [1.0; 3])]);
        assert_eq!(world.query(bounds, None)[0].id, 9);
    }

    #[test]
    fn empty_world_and_invalid_motion_are_stable() {
        let world = CollisionWorld::default();
        let result = world.move_character(Vec3::ZERO, HALF, Vec3::new(3.0, -2.0, 4.0), None);
        assert_position(result.position, Vec3::new(3.0, -2.0, 4.0));
        assert_eq!(result.candidates, 0);
        assert!(!result.grounded && !result.hit_ceiling);
        for half in [Vec3::ZERO, Vec3::NEG_ONE, Vec3::splat(f32::NAN)] {
            let result = world.move_character(Vec3::ONE, half, Vec3::X, None);
            assert_position(result.position, Vec3::ONE);
        }
    }
}
