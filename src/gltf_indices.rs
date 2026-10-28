//! Lossless immutable index upload. CPU/cutting source indices are never changed.
use crate::gltf_model::PrimitiveMode;

pub(crate) enum IndexData {
    U16(Vec<u16>),
    U32(Vec<u32>),
}
impl IndexData {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::U16(v) => bytemuck::cast_slice(v),
            Self::U32(v) => bytemuck::cast_slice(v),
        }
    }
    pub(crate) fn len(&self) -> usize {
        match self {
            Self::U16(v) => v.len(),
            Self::U32(v) => v.len(),
        }
    }
    pub(crate) fn is_u16(&self) -> bool {
        matches!(self, Self::U16(_))
    }
}
/// Normalize unsupported topologies before selecting storage by actual values.
/// WebGL2 fixed-index restart is always enabled, including non-strip draws:
/// 0xffff must remain an ordinary Uint32 index on the portable path.
pub(crate) fn prepare(
    input: &[u32],
    vertices: usize,
    mode: PrimitiveMode,
    compact: bool,
) -> (IndexData, PrimitiveMode, Option<u32>) {
    let input = if input.is_empty() {
        (0..u32::try_from(vertices).expect("validated vertex count")).collect()
    } else {
        input.to_vec()
    };
    let (values, mode) = match mode {
        PrimitiveMode::LineLoop => {
            let mut out = Vec::with_capacity(input.len() * 2);
            for i in 0..input.len() {
                out.extend_from_slice(&[input[i], input[(i + 1) % input.len()]]);
            }
            (out, PrimitiveMode::Lines)
        }
        PrimitiveMode::TriangleFan => {
            let mut out = Vec::with_capacity(input.len().saturating_sub(2) * 3);
            for i in 1..input.len().saturating_sub(1) {
                out.extend_from_slice(&[input[0], input[i], input[i + 1]]);
            }
            (out, PrimitiveMode::Triangles)
        }
        _ => (input, mode),
    };
    let max = values.iter().copied().max();
    let data = if compact && max.is_none_or(|v| v < u16::MAX as u32) {
        IndexData::U16(
            values
                .into_iter()
                .map(|v| u16::try_from(v).expect("proven portable index range"))
                .collect(),
        )
    } else {
        IndexData::U32(values)
    };
    (data, mode, max)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn decoded(data: &IndexData) -> Vec<u32> {
        match data {
            IndexData::U16(v) => v.iter().map(|&v| u32::from(v)).collect(),
            IndexData::U32(v) => v.clone(),
        }
    }
    #[test]
    fn portable_restart_boundaries_for_every_topology() {
        for mode in [
            PrimitiveMode::Points,
            PrimitiveMode::Lines,
            PrimitiveMode::LineStrip,
            PrimitiveMode::LineLoop,
            PrimitiveMode::Triangles,
            PrimitiveMode::TriangleStrip,
            PrimitiveMode::TriangleFan,
        ] {
            for high in [65534, 65535, 65536] {
                let original = [0, 1, high];
                let (packed, _, max) = prepare(&original, 65537, mode, true);
                let (reference, _, _) = prepare(&original, 65537, mode, false);
                assert_eq!(packed.is_u16(), high == 65534);
                assert_eq!(decoded(&packed), decoded(&reference));
                assert_eq!(max, Some(high));
                assert_eq!(original, [0, 1, high]);
            }
        }
    }
    #[test]
    fn actual_values_not_number_of_items_or_vertices() {
        let many = vec![2; 90003];
        let (a, _, _) = prepare(&many, 90000, PrimitiveMode::Triangles, true);
        assert!(a.is_u16());
        assert_eq!(a.len(), 90003);
        assert_eq!(decoded(&a), many);
        let (b, _, _) = prepare(&[0, 1, 70000], 70001, PrimitiveMode::Triangles, true);
        assert!(!b.is_u16());
        assert_eq!(decoded(&b), [0, 1, 70000]);
    }
    #[test]
    fn conversions_and_unindexed_preserve_exact_order() {
        let cases = [
            (
                PrimitiveMode::LineLoop,
                vec![3, 1, 2],
                vec![3, 1, 1, 2, 2, 3],
            ),
            (
                PrimitiveMode::TriangleFan,
                vec![3, 1, 2, 0],
                vec![3, 1, 2, 3, 2, 0],
            ),
            (
                PrimitiveMode::TriangleStrip,
                vec![3, 1, 2, 0],
                vec![3, 1, 2, 0],
            ),
        ];
        for (mode, input, want) in cases {
            let (data, _, _) = prepare(&input, 4, mode, true);
            assert_eq!(decoded(&data), want);
        }
        let (data, _, _) = prepare(&[], 3, PrimitiveMode::Triangles, true);
        assert_eq!(decoded(&data), [0, 1, 2]);
        let (large, _, _) = prepare(&[], 65536, PrimitiveMode::Points, true);
        assert!(!large.is_u16());
    }
    #[test]
    fn odd_payload_has_no_fictitious_indices() {
        let (data, _, _) = prepare(&[0, 1, 2], 3, PrimitiveMode::Triangles, true);
        assert_eq!(data.len(), 3);
        assert_eq!(data.bytes().len(), 6);
        let aligned = data.bytes().len().next_multiple_of(4);
        assert_eq!(aligned, 8);
        assert_eq!(decoded(&data), [0, 1, 2]);
    }
}
