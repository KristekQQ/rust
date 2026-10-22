//! Immutable convex contact proxy, baked with CPU geometry, shared by color variants.
//! Fixed directional vertex reduction bounds solver work independently of render detail.
use glam::Vec3;

pub(crate) struct ContactShape {
    pub points: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub edges: Vec<Vec3>,
    pub center: Vec3,
    pub half: Vec3,
}

fn add_axis(axes: &mut Vec<Vec3>, axis: Vec3) {
    if axis.length_squared() < 1e-12 {
        return;
    }
    let axis = axis.normalize();
    if !axes.iter().any(|n| n.dot(axis).abs() > 0.99999) {
        axes.push(axis);
    }
}

impl ContactShape {
    pub fn new(positions: &[Vec3], (min, max): (Vec3, Vec3), center: Vec3) -> Self {
        let mut points = Vec::new();
        // Keep actual extreme vertices, never an enclosing sphere or AABB corner.
        // This is a convex approximation: concavities need compound colliders.
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let direction = Vec3::new(x as f32, y as f32, z as f32);
                    if direction == Vec3::ZERO {
                        continue;
                    }
                    if let Some(&point) = positions
                        .iter()
                        .max_by(|a, b| a.dot(direction).total_cmp(&b.dot(direction)))
                    {
                        let point = point - center;
                        if !points
                            .iter()
                            .any(|p: &Vec3| p.distance_squared(point) < 1e-12)
                        {
                            points.push(point);
                        }
                    }
                }
            }
        }
        let mut normals = Vec::new();
        let mut edges = Vec::new();
        let epsilon = (max - min).length() * 1e-5;
        // At most 26 points: find supporting hull planes once, during creation.
        for i in 0..points.len() {
            for j in i + 1..points.len() {
                for k in j + 1..points.len() {
                    let normal = (points[j] - points[i]).cross(points[k] - points[i]);
                    if normal.length_squared() < 1e-12 {
                        continue;
                    }
                    let normal = normal.normalize();
                    let mut positive = false;
                    let mut negative = false;
                    for p in &points {
                        let distance = normal.dot(*p - points[i]);
                        positive |= distance > epsilon;
                        negative |= distance < -epsilon;
                    }
                    if positive && negative {
                        continue;
                    }
                    add_axis(&mut normals, normal);
                    for edge in [
                        points[j] - points[i],
                        points[k] - points[j],
                        points[i] - points[k],
                    ] {
                        add_axis(&mut edges, edge);
                    }
                }
            }
        }
        Self {
            points,
            normals,
            edges,
            center,
            half: (max - center).max(center - min),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_shape_reduction_is_bounded_and_keeps_real_extrema() {
        let points: Vec<_> = (0..1000)
            .map(|i| {
                let a = i as f32 * 2.399963;
                let y = 1. - 2. * i as f32 / 999.;
                let r = (1. - y * y).sqrt();
                Vec3::new(a.cos() * r, y, a.sin() * r)
            })
            .collect();
        let shape = ContactShape::new(&points, (-Vec3::ONE, Vec3::ONE), Vec3::ZERO);
        assert!(shape.points.len() <= 26 && shape.points.len() > 10);
        assert!(!shape.normals.is_empty() && !shape.edges.is_empty());
        assert!(shape.points.iter().all(|p| points.contains(p)));
        assert!(shape.points.contains(&Vec3::Y));
    }
}
