//! Stable draw-ordinal transpose; does not choose visibility or batch boundaries.
const ORDINAL_LIMIT: usize = 65536;
const ITEM_BYTES_LIMIT: usize = 4 * 1024 * 1024;

pub(crate) fn merge<T: Copy>(
    items: &[T],
    ordinals: usize,
    ordinal: impl Fn(T) -> usize,
    offsets: &mut Vec<usize>,
    ordered: &mut Vec<T>,
) -> bool {
    if ordinals > ORDINAL_LIMIT
        || items.len().saturating_mul(std::mem::size_of::<T>()) > ITEM_BYTES_LIMIT
    {
        return false;
    }
    offsets.clear();
    offsets.reserve_exact(ordinals.saturating_sub(offsets.len()));
    offsets.resize(ordinals, 0);
    ordered.clear();
    for &item in items {
        let index = ordinal(item);
        if index >= ordinals {
            return false;
        }
        offsets[index] += 1;
    }
    let mut next = 0;
    for count in offsets.iter_mut() {
        let n = *count;
        *count = next;
        next += n;
    }
    if let Some(&first) = items.first() {
        ordered.reserve_exact(items.len().saturating_sub(ordered.len()));
        ordered.resize(items.len(), first);
        // Input is model-major. Stable scatter preserves the original model
        // order within each ordinal, including absent/culled primitives.
        for &item in items {
            let offset = &mut offsets[ordinal(item)];
            ordered[*offset] = item;
            *offset += 1;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_as_cursor_merge_with_gaps_and_empty_models() {
        let mut counts = Vec::new();
        let mut ordered = Vec::new();
        let mut random = 1337u32;
        for roots in [0, 1, 7, 67, 1025] {
            for width in [0, 1, 3, 67] {
                let mut lists = vec![Vec::new(); roots];
                for (model, list) in lists.iter_mut().enumerate() {
                    for draw in 0..width {
                        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
                        if !random.is_multiple_of(3) {
                            list.push((model, draw));
                        }
                    }
                }
                let input: Vec<_> = lists.iter().flatten().copied().collect();
                let mut reference = Vec::new();
                let mut cursors = vec![0; roots];
                let mut ordinal = 0;
                loop {
                    let mut next = usize::MAX;
                    for (model, list) in lists.iter().enumerate() {
                        if list.get(cursors[model]).is_some_and(|i| i.1 == ordinal) {
                            reference.push(list[cursors[model]]);
                            cursors[model] += 1;
                        }
                        if let Some(item) = list.get(cursors[model]) {
                            next = next.min(item.1);
                        }
                    }
                    if next == usize::MAX {
                        break;
                    }
                    ordinal = next;
                }
                assert!(merge(&input, width, |i| i.1, &mut counts, &mut ordered));
                assert_eq!(ordered, reference);
            }
        }
    }
    #[test]
    fn bounded_fallback_and_reuse() {
        let mut counts = Vec::new();
        let mut ordered = Vec::new();
        assert!(!merge(
            &[0usize],
            ORDINAL_LIMIT + 1,
            |v| v,
            &mut counts,
            &mut ordered
        ));
        assert!(!merge(&[2usize], 2, |v| v, &mut counts, &mut ordered));
        assert!(merge(&[1usize, 0, 1], 2, |v| v, &mut counts, &mut ordered));
        assert_eq!(ordered, [0, 1, 1]);
        assert!(merge(&[], 0, |v| v, &mut counts, &mut ordered));
        assert!(ordered.is_empty());
    }
}
