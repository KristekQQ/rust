//! Exact instance upload address ranges. No visibility or record mapping changes.
//! Modes are diagnostic policies: union=0, coalesced=1, full active span=2.
const MAX_RANGES: usize = 4096; // One scratch list, at most64KiB on64-bit hosts.
#[derive(Default)]
pub(crate) struct DirtyRanges {
    pub mode: u32,
    pub gap: usize,
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub diagnostic: bool,
    union: Option<(usize, usize)>,
    spans: Vec<(usize, usize)>,
    single: [(usize, usize); 1],
    pub changed_records_bytes: usize,
    pub overflow: bool,
}
impl DirtyRanges {
    pub fn clear(&mut self) {
        self.union = None;
        self.spans.clear();
        self.changed_records_bytes = 0;
        self.overflow = false;
    }
    // Call in monotonically increasing, disjoint record order for one buffer.
    pub fn mark(&mut self, start: usize, end: usize) {
        debug_assert!(start < end && start.is_multiple_of(4) && end.is_multiple_of(4));
        debug_assert!(self.union.is_none_or(|(_, previous)| start >= previous));
        self.changed_records_bytes += end - start;
        self.union = Some(self.union.map_or((start, end), |(first, _)| (first, end)));
        if self.mode != 1 || self.overflow {
            return;
        }
        if let Some(last) = self.spans.last_mut() {
            if start - last.1 <= self.gap {
                last.1 = end;
                return;
            }
        }
        if self.spans.len() == MAX_RANGES {
            self.overflow = true;
            self.spans.clear(); // Keep union valid, never discard changed records.
        } else {
            self.spans.push((start, end));
        }
    }
    pub fn writes(&mut self, active_bytes: usize) -> &[(usize, usize)] {
        let Some((start, end)) = self.union else {
            return &[];
        };
        assert!(end <= active_bytes);
        if self.mode == 1 && !self.overflow {
            return &self.spans;
        }
        self.single[0] = if self.mode == 2 {
            (0, active_bytes)
        } else {
            (start, end)
        };
        &self.single
    }
    pub fn retained_bytes(&self) -> usize {
        self.spans.capacity() * std::mem::size_of::<(usize, usize)>()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_publication_across_gaps_layouts_and_shrink() {
        for stride in [16, 112] {
            for mode in 0..3 {
                for gap in [0, 256] {
                    let mut ranges = DirtyRanges {
                        mode,
                        gap,
                        ..Default::default()
                    };
                    let mut gpu = vec![17u8; stride * 20];
                    let mut cpu = gpu.clone();
                    for records in [vec![0, 3, 19], vec![2, 3, 4], vec![], vec![0]] {
                        ranges.clear();
                        for i in records {
                            cpu[i * stride..(i + 1) * stride].fill(91);
                            ranges.mark(i * stride, (i + 1) * stride);
                        }
                        for &(a, b) in ranges.writes(cpu.len()) {
                            gpu[a..b].copy_from_slice(&cpu[a..b]);
                        }
                        assert_eq!(gpu, cpu);
                    }
                    ranges.clear();
                    assert!(ranges.writes(0).is_empty());
                }
            }
        }
    }
    #[test]
    fn coalesces_only_allowed_gaps_and_bounds_scratch() {
        let mut r = DirtyRanges {
            mode: 1,
            gap: 16,
            ..Default::default()
        };
        r.mark(0, 16);
        r.mark(32, 48);
        r.mark(80, 96);
        assert_eq!(r.changed_records_bytes, 48);
        assert_eq!(r.writes(128), &[(0, 48), (80, 96)]);
        r.clear();
        r.gap = 0;
        for i in 0..MAX_RANGES + 10 {
            r.mark(i * 32, i * 32 + 16);
        }
        assert!(r.overflow);
        assert_eq!(
            r.writes((MAX_RANGES + 10) * 32),
            &[(0, (MAX_RANGES + 9) * 32 + 16)]
        );
        assert!(r.retained_bytes() <= MAX_RANGES * 16);
    }
}
