//! Geometry-only median BVH. Bounds may be planar or even points.
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub center: Vec3,
    pub half_extent: Vec3,
}
impl Bounds {
    fn union(self, other: Self) -> Self {
        let min = (self.center - self.half_extent).min(other.center - other.half_extent);
        let max = (self.center + self.half_extent).max(other.center + other.half_extent);
        let center = (min + max) * 0.5;
        let half_extent = (max - min) * 0.5;
        // Parent bounds must not shrink due to floating-point reconstruction.
        Self {
            center,
            half_extent: half_extent + (center.abs() + half_extent) * (2.0 * f32::EPSILON),
        }
    }
}

struct Node {
    bounds: Bounds,
    kind: NodeKind,
}
enum NodeKind {
    Leaf(u32),
    Branch(usize, usize),
}

#[derive(Default)]
pub(crate) struct Bvh {
    nodes: Vec<Node>,
    ids: Vec<u32>,
}
impl Bvh {
    /// IDs in input must be unique and ascending. Returns true for a topology rebuild;
    /// unchanged membership only refits bounds, including all internal nodes.
    pub fn update(&mut self, entries: &[(u32, Bounds)], force_rebuild: bool) -> bool {
        let same_ids = self.ids.len() == entries.len()
            && self
                .ids
                .iter()
                .zip(entries)
                .all(|(id, entry)| *id == entry.0);
        if !force_rebuild && same_ids {
            for index in (0..self.nodes.len()).rev() {
                self.nodes[index].bounds = match self.nodes[index].kind {
                    NodeKind::Leaf(id) => {
                        entries[entries.binary_search_by_key(&id, |e| e.0).unwrap()].1
                    }
                    NodeKind::Branch(left, right) => {
                        self.nodes[left].bounds.union(self.nodes[right].bounds)
                    }
                };
            }
            return false;
        }
        self.ids = entries.iter().map(|e| e.0).collect();
        self.nodes.clear();
        if !entries.is_empty() {
            self.build(&mut entries.to_vec());
        }
        true
    }
    fn build(&mut self, entries: &mut [(u32, Bounds)]) -> usize {
        let bounds = entries
            .iter()
            .skip(1)
            .fold(entries[0].1, |b, e| b.union(e.1));
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            kind: NodeKind::Leaf(entries[0].0),
        });
        if entries.len() > 1 {
            let axis = if bounds.half_extent.x >= bounds.half_extent.y
                && bounds.half_extent.x >= bounds.half_extent.z
            {
                0
            } else if bounds.half_extent.y >= bounds.half_extent.z {
                1
            } else {
                2
            };
            let middle = entries.len() / 2;
            entries.select_nth_unstable_by(middle, |a, b| {
                a.1.center[axis]
                    .total_cmp(&b.1.center[axis])
                    .then(a.0.cmp(&b.0))
            });
            let (left, right) = entries.split_at_mut(middle);
            let left = self.build(left);
            let right = self.build(right);
            self.nodes[index].kind = NodeKind::Branch(left, right);
        }
        index
    }
    /// Internal bounds and eligible object bounds are counted separately.
    pub fn query<const TRACK: bool>(
        &self,
        intersects: &impl Fn(Bounds) -> bool,
        eligible: &impl Fn(u32) -> bool,
        output: &mut Vec<u32>,
        nodes_visited: &mut usize,
        object_tests: &mut usize,
    ) {
        if !self.nodes.is_empty() {
            self.query_node::<TRACK>(0, intersects, eligible, output, nodes_visited, object_tests);
        }
    }
    fn query_node<const TRACK: bool>(
        &self,
        index: usize,
        intersects: &impl Fn(Bounds) -> bool,
        eligible: &impl Fn(u32) -> bool,
        output: &mut Vec<u32>,
        nodes_visited: &mut usize,
        object_tests: &mut usize,
    ) {
        let node = &self.nodes[index];
        if TRACK {
            *nodes_visited += 1;
        }
        match node.kind {
            NodeKind::Leaf(id) => {
                if eligible(id) {
                    if TRACK {
                        *object_tests += 1;
                    }
                    if intersects(node.bounds) {
                        output.push(id);
                    }
                }
            }
            NodeKind::Branch(left, right) => {
                if intersects(node.bounds) {
                    self.query_node::<TRACK>(
                        left,
                        intersects,
                        eligible,
                        output,
                        nodes_visited,
                        object_tests,
                    );
                    self.query_node::<TRACK>(
                        right,
                        intersects,
                        eligible,
                        output,
                        nodes_visited,
                        object_tests,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn planar_and_point_bounds_survive_refit_and_membership_changes() {
        let mut tree = Bvh::default();
        let bounds = |x| Bounds {
            center: Vec3::new(x, 0.0, 0.0),
            half_extent: Vec3::ZERO,
        };
        let mut entries = [
            (2, bounds(0.0)),
            (
                7,
                Bounds {
                    center: Vec3::new(10.0, 0.0, 0.0),
                    half_extent: Vec3::new(1.0, 0.0, 1.0),
                },
            ),
        ];
        assert!(tree.update(&entries, false));
        let query = |tree: &Bvh| {
            let mut output = Vec::new();
            let (mut nodes, mut objects) = (0, 0);
            tree.query::<true>(
                &|b| b.center.x - b.half_extent.x <= 1.0 && b.center.x + b.half_extent.x >= -1.0,
                &|_| true,
                &mut output,
                &mut nodes,
                &mut objects,
            );
            output.sort_unstable();
            output
        };
        assert_eq!(query(&tree), vec![2]);
        entries[0].1 = bounds(20.0);
        entries[1].1.center.x = 0.0;
        assert!(!tree.update(&entries, false));
        assert_eq!(query(&tree), vec![7]);
        assert!(tree.update(&entries[1..], false));
        assert_eq!(query(&tree), vec![7]);
        assert!(tree.update(&[], false));
        assert!(query(&tree).is_empty());
    }
}
