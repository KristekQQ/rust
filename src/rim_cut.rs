//! Binary, worker-safe clipping of the actual posed GLB triangles. No scene/GPU access.
use glam::Vec3;
#[derive(Clone, Copy)]
struct Vertex {
    p: Vec3,
    n: Vec3,
    c: Vec3,
}
impl Vertex {
    fn lerp(self, b: Self, t: f32) -> Self {
        Self {
            p: self.p.lerp(b.p, t),
            n: self.n.lerp(b.n, t).normalize_or_zero(),
            c: self.c.lerp(b.c, t),
        }
    }
}
fn push(out: &mut Vec<f32>, v: Vertex) {
    out.extend_from_slice(&[
        v.p.x, v.p.y, v.p.z, v.n.x, v.n.y, v.n.z, v.c.x, v.c.y, v.c.z,
    ]);
}
/// Input words: token, triangleCount, normal.xyz, point.xyz, then 27 floats per triangle.
/// Output words: token, positiveVertexCount, negativeVertexCount, then both flat streams.
pub fn split(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() < 32 || bytes.len() > 32 + 100_000 * 108 || !bytes.len().is_multiple_of(4) {
        return Err("Invalid cut job".into());
    }
    let words: Vec<f32> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let token = words[0].to_bits();
    let count = words[1].to_bits() as usize;
    if count > 100_000 || words.len() != 8 + count * 27 || words[2..].iter().any(|v| !v.is_finite())
    {
        return Err("Invalid cut geometry".into());
    }
    let normal = Vec3::from_slice(&words[2..5]).normalize_or_zero();
    let point = Vec3::from_slice(&words[5..8]);
    if normal.length_squared() < 0.5 {
        return Err("Invalid cut plane".into());
    }
    let mut halves = [Vec::new(), Vec::new()];
    for triangle in words[8..].as_chunks::<27>().0.iter() {
        let vertices = std::array::from_fn::<_, 3, _>(|i| {
            let v = &triangle[i * 9..];
            Vertex {
                p: Vec3::from_slice(v),
                n: Vec3::from_slice(&v[3..]),
                c: Vec3::from_slice(&v[6..]),
            }
        });
        for (side, out) in halves.iter_mut().enumerate() {
            let sign = if side == 0 { 1. } else { -1. };
            let mut polygon = Vec::with_capacity(4);
            for i in 0..3 {
                let a = vertices[i];
                let b = vertices[(i + 1) % 3];
                let da = normal.dot(a.p - point) * sign;
                let db = normal.dot(b.p - point) * sign;
                if da >= 0. {
                    polygon.push(a);
                }
                if (da >= 0.) != (db >= 0.) {
                    polygon.push(a.lerp(b, da / (da - db)));
                }
            }
            for i in 1..polygon.len().saturating_sub(1) {
                let tri = [polygon[0], polygon[i], polygon[i + 1]];
                if (tri[1].p - tri[0].p)
                    .cross(tri[2].p - tri[0].p)
                    .length_squared()
                    > 1e-16
                {
                    for v in tri {
                        push(out, v);
                    }
                }
            }
        }
    }
    let mut output = Vec::with_capacity(12 + (halves[0].len() + halves[1].len()) * 4);
    for value in [
        token,
        (halves[0].len() / 9) as u32,
        (halves[1].len() / 9) as u32,
    ] {
        output.extend_from_slice(&value.to_le_bytes());
    }
    for side in halves {
        for value in side {
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_actual_crossing_triangle_and_preserves_normals() {
        let mut b = Vec::new();
        for x in [7u32, 1] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        for x in [1f32, 0., 0., 0., 0., 0.] {
            b.extend_from_slice(&x.to_le_bytes());
        }
        for p in [[-1f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]] {
            for x in p.into_iter().chain([0., 0., 1., 0.2, 0.4, 0.6]) {
                b.extend_from_slice(&x.to_le_bytes());
            }
        }
        let r = split(&b).unwrap();
        assert_eq!(u32::from_le_bytes(r[0..4].try_into().unwrap()), 7);
        assert_eq!(u32::from_le_bytes(r[4..8].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(r[8..12].try_into().unwrap()), 3);
        for (i, c) in r[12..].as_chunks::<36>().0.iter().enumerate() {
            let x = f32::from_le_bytes(c[0..4].try_into().unwrap());
            assert!(if i < 3 { x >= 0. } else { x <= 0. });
        }
    }
    #[test]
    fn rejects_truncated_and_nonfinite_jobs() {
        assert!(split(&[]).is_err());
        let mut b = vec![0u8; 32];
        b[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(split(&b).is_err());
    }
}
