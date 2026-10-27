//! Append-only exact overlap queries for one ordered, disjoint batching run.
//! Tree topology follows insertion order; it never reorders scene objects.
use glam::Vec3;

type Bounds = (Vec3, Vec3);
const LEAF: u32 = u32::MAX;
const LINEAR_LIMIT: usize = 16;
const TREE_LIMIT: usize = 8192;
const QUERY_LIMIT: u64 = 64;
const EXHAUSTION_LIMIT: u8 = 8;

#[derive(Clone, Copy)]
struct Node {
    bounds: Bounds,
    left: u32,
    right: u32,
}
#[derive(Clone, Copy)]
struct Root {
    node: u32,
    height: u8,
}
#[derive(Default)]
pub(crate) struct OrderedBounds {
    leaves: Vec<Bounds>,
    nodes: Vec<Node>,
    roots: Vec<Root>,
    invalid: bool,
    budget_exhaustions: u8,
    linear_only: bool,
}
#[derive(Default)]
pub(crate) struct Probe {
    pub overlap: bool,
    pub leaf_tests: u64,
    pub node_tests: u64,
    pub fallback: bool,
}
fn valid((min, max): Bounds) -> bool {
    min.is_finite() && max.is_finite() && min.cmple(max).all()
}
fn disjoint(a: Bounds, b: Bounds) -> bool {
    (a.1.cmplt(b.0) | b.1.cmplt(a.0)).any()
}
/// Reject an unknown or malformed primitive before combining it with valid ones.
pub(crate) fn union_bounds(bounds: impl Iterator<Item = Option<Bounds>>) -> Option<Bounds> {
    let mut result: Option<Bounds> = None;
    for next in bounds {
        let next = next?;
        if !valid(next) {
            return None;
        }
        result = Some(match result {
            Some(prior) => (prior.0.min(next.0), prior.1.max(next.1)),
            None => next,
        });
    }
    result
}
impl OrderedBounds {
    pub fn clear(&mut self) {
        self.leaves.clear();
        self.nodes.clear();
        self.roots.clear();
        self.invalid = false;
        self.budget_exhaustions = 0;
        self.linear_only = false;
    }
    pub fn capacity(&self) -> usize {
        self.leaves.capacity() + self.nodes.capacity() + self.roots.capacity()
    }
    pub fn bytes(&self) -> usize {
        self.leaves.capacity() * std::mem::size_of::<Bounds>()
            + self.nodes.capacity() * std::mem::size_of::<Node>()
            + self.roots.capacity() * std::mem::size_of::<Root>()
    }
    pub fn push(&mut self, bounds: Bounds) {
        if !valid(bounds) || self.nodes.len() >= LEAF as usize - 64 {
            self.invalid = true;
            return;
        }
        self.leaves.push(bounds);
        if self.linear_only || self.leaves.len() <= LINEAR_LIMIT || self.leaves.len() > TREE_LIMIT {
            return;
        }
        if self.nodes.is_empty() {
            for index in 0..self.leaves.len() {
                self.insert_node(self.leaves[index]);
            }
        } else {
            self.insert_node(bounds);
        }
    }
    fn insert_node(&mut self, bounds: Bounds) {
        let mut root = Root {
            node: self.nodes.len() as u32,
            height: 0,
        };
        self.nodes.push(Node {
            bounds,
            left: LEAF,
            right: LEAF,
        });
        while self
            .roots
            .last()
            .is_some_and(|prior| prior.height == root.height)
        {
            let prior = self.roots.pop().unwrap();
            let a = self.nodes[prior.node as usize].bounds;
            let b = self.nodes[root.node as usize].bounds;
            let parent = self.nodes.len() as u32;
            self.nodes.push(Node {
                bounds: (a.0.min(b.0), a.1.max(b.1)),
                left: prior.node,
                right: root.node,
            });
            root = Root {
                node: parent,
                height: root.height + 1,
            };
        }
        self.roots.push(root);
    }
    pub fn probe(&mut self, bounds: Bounds) -> Probe {
        let mut result = Probe::default();
        if self.invalid || !valid(bounds) {
            result.overlap = true;
            return result;
        }
        if self.linear_only || self.leaves.len() <= LINEAR_LIMIT || self.leaves.len() > TREE_LIMIT {
            result.overlap = self.linear_probe(bounds, &mut result);
            return result;
        }
        for root in &self.roots {
            match self.visit(root.node, bounds, &mut result) {
                Some(true) => {
                    result.overlap = true;
                    break;
                }
                Some(false) => {}
                None => {
                    result.fallback = true;
                    result.overlap = self.linear_probe(bounds, &mut result);
                    // Poor spatial locality can make every query traverse the
                    // full budget. Stop paying for an unhelpful index in this
                    // run only; all leaves remain available to the exact scan.
                    self.budget_exhaustions += 1;
                    self.linear_only = self.budget_exhaustions >= EXHAUSTION_LIMIT;
                    break;
                }
            }
        }
        result
    }
    fn linear_probe(&self, bounds: Bounds, result: &mut Probe) -> bool {
        // Keep counter/index writes out of the hot predicate. any consumes the
        // first hit too, so the remaining length gives the exact examined count.
        let mut remaining = self.leaves.iter();
        let overlap = remaining.any(|&prior| !disjoint(prior, bounds));
        let tested = (self.leaves.len() - remaining.len()) as u64;
        result.leaf_tests += tested;
        result.node_tests += tested;
        overlap
    }
    fn visit(&self, index: u32, bounds: Bounds, result: &mut Probe) -> Option<bool> {
        if result.node_tests >= QUERY_LIMIT {
            return None;
        }
        let node = self.nodes[index as usize];
        result.node_tests += 1;
        if node.left == LEAF {
            result.leaf_tests += 1;
        }
        if disjoint(node.bounds, bounds) {
            return Some(false);
        }
        if node.left == LEAF {
            return Some(true);
        }
        if self.visit(node.left, bounds, result)? {
            return Some(true);
        }
        self.visit(node.right, bounds, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds(x: f32, y: f32, z: f32, extent: f32) -> Bounds {
        let p = Vec3::new(x, y, z);
        (p - Vec3::splat(extent), p + Vec3::splat(extent))
    }
    fn grouping(boxes: &[Bounds], accelerated: bool) -> Vec<std::ops::Range<usize>> {
        let mut result = Vec::new();
        let mut tree = OrderedBounds::default();
        let mut start = 0;
        while start < boxes.len() {
            tree.clear();
            tree.push(boxes[start]);
            let mut end = start + 1;
            while end < boxes.len() {
                let conflict = if accelerated {
                    tree.probe(boxes[end]).overlap
                } else {
                    boxes[start..end].iter().any(|&a| !disjoint(a, boxes[end]))
                };
                if conflict {
                    break;
                }
                tree.push(boxes[end]);
                end += 1;
            }
            result.push(start..end);
            start = end;
        }
        result
    }
    #[test]
    fn linear_fallback_counts_first_last_miss_and_prior_tree_work_exactly() {
        let mut tree = OrderedBounds::default();
        let check = |tree: &OrderedBounds, query, expected_hit, expected_tests: u64| {
            let mut probe = Probe {
                leaf_tests: 7,
                node_tests: 9,
                ..Probe::default()
            };
            assert_eq!(tree.linear_probe(query, &mut probe), expected_hit);
            assert_eq!(probe.leaf_tests, 7 + expected_tests);
            assert_eq!(probe.node_tests, 9 + expected_tests);
        };
        check(&tree, bounds(9., 0., 0., 0.1), false, 0);
        for x in [0., 2., 4.] {
            tree.push(bounds(x, 0., 0., 0.1));
        }
        check(&tree, bounds(0., 0., 0., 0.1), true, 1);
        check(&tree, bounds(4., 0., 0., 0.1), true, 3);
        check(&tree, bounds(9., 0., 0., 0.1), false, 3);
        tree.clear();
        let count = 4096usize;
        for i in 0..count {
            let logical = if i % 2 == 0 { i / 2 } else { count - 1 - i / 2 };
            tree.push(bounds(logical as f32 * 2., 0., 0., 0.1));
        }
        let probe = tree.probe(bounds(count as f32 - 0.5, 0., 0., 0.1));
        assert!(probe.fallback && !probe.overlap);
        assert!(probe.leaf_tests >= count as u64 && probe.leaf_tests <= count as u64 + QUERY_LIMIT);
        assert_eq!(probe.node_tests, count as u64 + QUERY_LIMIT);
    }
    #[test]
    fn repeated_budget_exhaustion_uses_exact_scan_until_clear_and_keeps_late_leaves() {
        let mut tree = OrderedBounds::default();
        let count = 4096usize;
        for i in 0..count {
            let logical = if i % 2 == 0 { i / 2 } else { count - 1 - i / 2 };
            tree.push(bounds(logical as f32 * 2., 0., 0., 0.1));
        }
        for _ in 0..EXHAUSTION_LIMIT {
            let probe = tree.probe(bounds(count as f32 - 0.5, 0., 0., 0.1));
            assert!(probe.fallback && !probe.overlap);
        }
        assert!(tree.linear_only);
        let nodes = tree.nodes.len();
        tree.push(bounds(10000., 0., 0., 0.1));
        assert_eq!(
            tree.nodes.len(),
            nodes,
            "disabled index stops constructing nodes"
        );
        let probe = tree.probe(bounds(10000., 0., 0., 0.1));
        assert!(probe.overlap && !probe.fallback);
        assert_eq!(probe.leaf_tests, count as u64 + 1);
        tree.clear();
        assert!(!tree.linear_only && tree.budget_exhaustions == 0);
        for i in 0..count {
            tree.push(bounds(i as f32 * 2., 0., 0., 0.1));
        }
        let probe = tree.probe(bounds(10000., 0., 0., 0.1));
        assert!(!probe.overlap && !probe.fallback);
        assert_eq!(probe.leaf_tests, 0);
        assert_eq!(probe.node_tests, 1);
    }
    #[test]
    fn ordered_bounds_preserves_every_greedy_group_against_independent_pairs() {
        let mut seed = 0x6a09e667u32;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed as f64 / u32::MAX as f64
        };
        for count in [1, 2, 3, 31, 128, 1025] {
            for mode in 0..12 {
                let boxes: Vec<_> = (0..count)
                    .map(|i| {
                        let x = if mode % 3 == 0 {
                            i as f32 * 0.4
                        } else {
                            (rand() * 12.) as f32
                        };
                        bounds(
                            x,
                            (rand() * 12.) as f32,
                            (rand() * 3.) as f32,
                            if mode % 4 == 0 { 1.2 } else { 0.1 },
                        )
                    })
                    .collect();
                assert_eq!(
                    grouping(&boxes, true),
                    grouping(&boxes, false),
                    "count{count} mode{mode}"
                );
            }
        }
    }
    #[test]
    fn ordered_bounds_keeps_touching_overlap_and_conservative_invalid_inputs() {
        let mut tree = OrderedBounds::default();
        tree.push(bounds(0., 0., 0., 1.));
        assert!(tree.probe(bounds(2., 0., 0., 1.)).overlap);
        assert!(
            !tree
                .probe(bounds(f32::from_bits(2f32.to_bits() + 1), 0., 0., 1.))
                .overlap
        );
        assert!(tree.probe((Vec3::splat(f32::NAN), Vec3::ONE)).overlap);
        assert!(tree.probe((Vec3::ONE, Vec3::ZERO)).overlap);
        tree.push((Vec3::ZERO, Vec3::splat(f32::INFINITY)));
        assert!(tree.probe(bounds(100., 0., 0., 0.1)).overlap);
        tree.clear();
        assert!(!tree.probe(bounds(100., 0., 0., 0.1)).overlap);
    }
    #[test]
    fn union_rejects_invalid_primitive_before_nan_can_be_hidden() {
        let good = bounds(0., 0., 0., 1.);
        for bad in [
            None,
            Some((Vec3::splat(f32::NAN), Vec3::ONE)),
            Some((Vec3::ONE, Vec3::ZERO)),
            Some((Vec3::ZERO, Vec3::splat(f32::INFINITY))),
        ] {
            assert!(union_bounds([Some(good), bad].into_iter()).is_none());
        }
        assert_eq!(
            union_bounds([Some(good), Some(bounds(4., 0., 0., 1.))].into_iter()),
            Some((Vec3::new(-1., -1., -1.), Vec3::new(5., 1., 1.)))
        );
        assert!(union_bounds(std::iter::empty()).is_none());
    }
    #[test]
    fn ordered_bounds_past_tree_limit_keeps_every_leaf_and_reuses_after_clear() {
        let mut tree = OrderedBounds::default();
        for i in 0..TREE_LIMIT + 2 {
            tree.push(bounds(i as f32 * 2., 0., 0., 0.1));
        }
        assert!(
            tree.probe(bounds((TREE_LIMIT + 1) as f32 * 2., 0., 0., 0.1))
                .overlap
        );
        assert!(!tree.probe(bounds(-10., 0., 0., 0.1)).overlap);
        tree.clear();
        tree.push(bounds(0., 0., 0., 0.1));
        assert!(tree.probe(bounds(0., 0., 0., 0.1)).overlap);
        assert!(!tree.probe(bounds(5., 0., 0., 0.1)).overlap);
    }
    #[test]
    fn ordered_bounds_grid_prunes_real_work_and_reuses_storage() {
        let mut tree = OrderedBounds::default();
        let mut nodes = 0;
        let mut leaves = 0;
        let boxes: Vec<_> = (0..4096)
            .map(|i| bounds((i % 64) as f32, (i / 64) as f32, 0., 0.1))
            .collect();
        for &b in &boxes {
            let p = tree.probe(b);
            assert!(!p.overlap);
            nodes += p.node_tests;
            leaves += p.leaf_tests;
            tree.push(b);
        }
        assert!(nodes < (4096 * 4095 / 2) / 20, "node tests{nodes}");
        assert!(leaves < 8192, "leaf tests{leaves}");
        let mut shuffled = boxes.clone();
        let mut seed = 0x9e3779b9u32;
        for i in (1..shuffled.len()).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let j = seed as usize % (i + 1);
            shuffled.swap(i, j);
        }
        assert_eq!(grouping(&shuffled, true), grouping(&shuffled, false));
        let alternating: Vec<_> = (0..boxes.len())
            .map(|i| {
                boxes[if i % 2 == 0 {
                    i / 2
                } else {
                    boxes.len() - 1 - i / 2
                }]
            })
            .collect();
        assert_eq!(grouping(&alternating, true), grouping(&alternating, false));
        let dense = vec![bounds(0., 0., 0., 0.2); 4096];
        assert_eq!(grouping(&dense, true), grouping(&dense, false));
        let bytes = tree.bytes();
        tree.clear();
        for b in boxes {
            tree.push(b);
        }
        assert_eq!(bytes, tree.bytes());
        assert!(tree.capacity() > 0);
        tree = OrderedBounds::default();
        assert_eq!(tree.bytes(), 0);
    }
}
