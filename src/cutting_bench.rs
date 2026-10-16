//! Opt-in native CPU profile; the parent registers this module under `cfg(test)`.
//! Run optimized and serially: cargo test --release cutting_bench::profile_cpu_cutting
//!     -- --ignored --exact --nocapture --test-threads=1
//! `create_bvh` includes constructor validation, bounds and BVH construction;
//! public CutMesh APIs do not expose isolated BVH construction. `slice` includes
//! its internal validation; output BVHs are lazy and build only on another cut.
//! Render/validate columns cover both pieces
//! separately and must not be added to slice as if they were disjoint stages.

#![cfg(all(test, not(target_arch = "wasm32")))]

use crate::cutting::{CutMesh, Slice};
use glam::Vec3;
use std::hint::black_box;
use std::time::{Duration, Instant};

const SAMPLES: usize = 9;

fn geometry(segments: u32, rings: u32, apple: bool) -> (Vec<Vec3>, Vec<u32>) {
    // Shared poles and wrapped longitude indices keep every edge paired.
    let mut positions = vec![Vec3::new(0., -1., 0.), Vec3::new(0., 1., 0.)];
    if apple {
        positions[0].y = -1.1;
        positions[1].y = 0.82;
    }
    for j in 1..rings {
        let theta = j as f32 / rings as f32 * std::f32::consts::PI;
        for i in 0..segments {
            let phi = i as f32 / segments as f32 * std::f32::consts::TAU;
            let (radius, y) = if apple {
                (
                    theta.sin() * 1.1 * (1. - 0.1 * theta.cos()) * (1. + 0.045 * (phi * 5.).cos()),
                    -1.1 * theta.cos()
                        - 0.28 * (-((std::f32::consts::PI - theta) / 0.38).powi(2)).exp(),
                )
            } else {
                (theta.sin(), -theta.cos())
            };
            positions.push(Vec3::new(phi.cos() * radius, y, phi.sin() * radius));
        }
    }
    let mut indices = Vec::with_capacity((6 * segments * (rings - 1)) as usize);
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
    (positions, indices)
}

fn cut(mesh: &CutMesh) -> Slice {
    // Large finite sweep covers both solids. Oblique, offset plane avoids
    // axis/pole symmetry; every sample uses exactly the same plane.
    let normal = Vec3::new(0.73, 0.31, -0.47).normalize();
    let u = normal.any_orthonormal_vector();
    let v = normal.cross(u);
    let a = normal * 0.137 - 4. * u - 4. * v;
    mesh.slice(a, a + 8. * u, 8. * v, Vec3::ONE)
        .expect("benchmark slice failed")
        .expect("benchmark plane must split the sample")
}

fn median_ms(samples: &mut [Duration]) -> f64 {
    samples.sort_unstable();
    samples[samples.len() / 2].as_secs_f64() * 1_000.
}

#[test]
#[ignore = "manual native CPU timing; run --release --ignored --nocapture serially"]
fn profile_cpu_cutting() {
    println!("cpu_cutting samples={SAMPLES} medians_ms; render/validate=all output pieces; create_bvh=new incl validation; generation/cloning/drop excluded");
    for apple in [false, true] {
        for (segments, rings) in [(32, 16), (64, 64), (100, 96)] {
            let (positions, indices) = geometry(segments, rings, apple);
            let mesh = CutMesh::new(positions.clone(), &indices, Vec3::X)
                .expect("closed outward benchmark input");
            // Untimed preflight also warms each operation. All correctness
            // assertions live outside the measured sampling loop.
            mesh.validate().unwrap();
            assert_eq!(mesh.triangles.len(), (2 * segments * (rings - 1)) as usize);
            let result = cut(&mesh);
            assert_eq!(result.pieces.len(), 2);
            let before = mesh.volume();
            let after: f64 = result.pieces.iter().map(CutMesh::volume).sum();
            assert!((after - before).abs() <= before * 0.001 + 1e-7);
            for piece in &result.pieces {
                piece.validate().unwrap(); // closed, manifold, outward, nondegenerate
                let rendered = piece.render_mesh();
                assert_eq!(rendered.vertices.len(), piece.triangles.len() * 3);
                assert_eq!(rendered.indices.len(), rendered.vertices.len());
            }
            let out_vertices: usize = result.pieces.iter().map(|p| p.positions.len()).sum();
            let out_triangles: usize = result.pieces.iter().map(|p| p.triangles.len()).sum();
            let mut creation = Vec::with_capacity(SAMPLES);
            let mut slicing = Vec::with_capacity(SAMPLES);
            let mut rendering = Vec::with_capacity(SAMPLES);
            let mut validation = Vec::with_capacity(SAMPLES);
            for _ in 0..SAMPLES {
                // Fresh independent input each time; never recursively slice
                // previous output. Mesh generation and input copies are untimed.
                let input = positions.clone();
                let start = Instant::now();
                let fresh = black_box(CutMesh::new(black_box(input), &indices, Vec3::X));
                creation.push(start.elapsed());
                let fresh = fresh.expect("benchmark construction failed");

                let start = Instant::now();
                let sliced = black_box(cut(black_box(&fresh)));
                slicing.push(start.elapsed());

                let start = Instant::now();
                let rendered: Vec<_> = sliced
                    .pieces
                    .iter()
                    .map(|p| black_box(p.render_mesh()))
                    .collect();
                rendering.push(start.elapsed());
                black_box(&rendered);

                let start = Instant::now();
                let validated: Vec<_> = sliced
                    .pieces
                    .iter()
                    .map(|p| black_box(p.validate()))
                    .collect();
                validation.push(start.elapsed());
                for check in validated {
                    check.expect("benchmark output validation failed");
                }
            }
            println!(
                "{} in_v={} in_t={} pieces={} out_v={} out_t={} bvh_nodes={} triangle_tests={} create_bvh={:.3} slice={:.3} render_mesh={:.3} validate={:.3}",
                if apple { "apple" } else { "sphere" },
                positions.len(), indices.len() / 3, result.pieces.len(),
                out_vertices, out_triangles, result.stats.nodes, result.stats.triangle_tests,
                median_ms(&mut creation), median_ms(&mut slicing),
                median_ms(&mut rendering), median_ms(&mut validation),
            );
        }
    }
}
