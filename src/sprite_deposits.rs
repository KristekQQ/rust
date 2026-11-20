//! Optional CPU-owned floor deposits. Ordinary shared meshes publish to observers.
use super::{RenderOptions, SceneManager};
use glam::Vec3;

const CAPACITY: usize = 15;
const RETIRE_SECONDS: f32 = 0.4;
#[derive(Clone, Copy)]
struct Capture {
    position: Vec3,
    floor: f32,
    // Snapshot the target at release so a sidestep can dodge the drop.
    aim: Option<Vec3>,
}
impl Capture {
    fn fall_time(self, height: f32) -> f32 {
        let up = if self.aim.is_some() { 1.8 } else { 0. };
        (up + (up * up + 19.62 * (self.position.y - height))
            .max(0.)
            .sqrt())
            / 9.81
    }
    fn at(self, t: f32) -> Vec3 {
        let mut p = self.position;
        if let Some(aim) = self.aim {
            p += (aim - p).with_y(0.) * (t / self.fall_time(aim.y).max(0.01));
            p.y += 1.8 * t;
        }
        p.y = (p.y - 4.905 * t * t).max(self.floor);
        p
    }
}
struct Drop {
    id: u32,
    capture: Capture,
    age: f32,
    retirement: Option<f32>,
}
pub(super) struct Deposits {
    pub(super) target_head: Option<Vec3>,
    mesh: Option<u32>,
    drops: Vec<Drop>,
    pending: Option<Capture>,
    wait: f32,
    sequence: u32,
}
impl Default for Deposits {
    fn default() -> Self {
        Self {
            target_head: None,
            mesh: None,
            drops: Vec::new(),
            pending: None,
            wait: 4.,
            sequence: 0,
        }
    }
}
impl Deposits {
    pub(super) fn remove(&mut self, id: u32) {
        self.drops.retain(|drop| drop.id != id);
    }
}

// Three rounded hexagonal lobes, 72 triangles total, 12cm across / 4cm high at rest.
fn geometry() -> (Vec<f32>, Vec<u32>, Vec<f32>) {
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    let mut colors = Vec::new();
    for (x, z, radius, height, color) in [
        (-0.018, 0., 0.042, 0.032, [0.40, 0.25, 0.12]),
        (0.028, 0.012, 0.032, 0.024, [0.49, 0.32, 0.17]),
        (-0.006, -0.005, 0.025, 0.043, [0.90, 0.85, 0.67]),
    ] {
        let base = (positions.len() / 3) as u32;
        positions.extend([x, height, z]);
        positions.extend([x, 0., z]);
        for (ring_radius, ring_y) in [(radius, height * 0.22), (radius * 0.7, height * 0.75)] {
            for i in 0..6 {
                let angle = i as f32 * std::f32::consts::TAU / 6.;
                positions.extend([
                    x + ring_radius * angle.cos(),
                    ring_y,
                    z + ring_radius * angle.sin(),
                ]);
            }
        }
        for i in 0..6 {
            let a = base + 2 + i;
            let b = base + 2 + (i + 1) % 6;
            let upper_a = a + 6;
            let upper_b = b + 6;
            indices.extend([
                base,
                upper_b,
                upper_a,
                upper_a,
                upper_b,
                b,
                upper_a,
                b,
                a,
                base + 1,
                a,
                b,
            ]);
            for _ in 0..4 {
                colors.extend(color);
            }
        }
    }
    (positions, indices, colors)
}
impl SceneManager {
    pub fn visor_splat_remaining(&self) -> f32 {
        self.visor_splat_remaining
    }

    fn spawn_deposit(&mut self, pool: &mut Deposits, capture: Capture, reuse: Option<u32>) {
        let id = if let Some(id) = reuse {
            id
        } else {
            let handle = if let Some(handle) = pool.mesh {
                handle
            } else {
                let (positions, indices, colors) = geometry();
                let Ok(handle) = self.register_mesh_variants(&positions, &indices, &colors, true)
                else {
                    return;
                };
                pool.mesh = Some(handle);
                handle
            };
            let Ok(id) = self.add_mesh_variant(handle, 0) else {
                return;
            };
            self.set_render_options(
                id,
                RenderOptions {
                    casts_shadow: false,
                    receives_shadow: false,
                    reflectivity: 0.,
                    opaque_reorderable: true,
                    ..Default::default()
                },
            );
            id
        };
        self.set_object_position(id, capture.position);
        self.set_object_scale(id, Vec3::new(0.65, 1.5, 0.65));
        pool.drops.push(Drop {
            id,
            capture,
            age: 0.,
            retirement: None,
        });
    }
    fn enqueue_deposit(&mut self, pool: &mut Deposits, capture: Capture) {
        if pool.pending.is_some() {
            return;
        }
        if pool.drops.len() < CAPACITY {
            self.spawn_deposit(pool, capture, None);
        } else {
            pool.pending = Some(capture);
            pool.drops[0].retirement = Some(0.);
        }
    }
    pub(super) fn update_sprite_deposits(&mut self, dt: f32) {
        if self.is_render_mirror() || !dt.is_finite() || dt <= 0. {
            return;
        }
        self.visor_splat_remaining = (self.visor_splat_remaining - dt).max(0.);
        let mut pool = std::mem::take(&mut self.sprite_deposits);
        pool.drops
            .retain(|drop| self.objects.contains_key(&drop.id));
        let mut retired = None;
        let mut hit = None;
        for drop in &mut pool.drops {
            let previous_age = drop.age;
            drop.age = (drop.age + dt).min(1e10);
            let landing = drop.capture.fall_time(drop.capture.floor).max(0.001);
            let landed_age = (drop.age - landing).max(0.);
            let falling = drop.age < landing;
            if drop.capture.aim.is_some() && previous_age < landing {
                if let Some(head) = pool.target_head {
                    let crown = head + Vec3::Y * 0.065;
                    let crossing = drop.capture.fall_time(crown.y);
                    if previous_age < crossing
                        && crossing <= drop.age
                        && drop.capture.at(crossing).distance_squared(crown) <= 0.18 * 0.18
                    {
                        hit = Some(drop.id);
                        self.visor_splat_remaining = 7.;
                        continue;
                    }
                }
            }
            let scale = if falling {
                Vec3::new(0.65, 1.5, 0.65)
            } else {
                // Impact squash recovers in 220ms; the footprint remains a real 3D mesh.
                let impact = (1. - landed_age / 0.22).max(0.);
                Vec3::new(1. + impact * 0.3, 1. - impact * 0.6, 1. + impact * 0.3)
            };
            let shrink = if let Some(age) = drop.retirement.as_mut() {
                *age += dt;
                if *age >= RETIRE_SECONDS {
                    retired = Some(drop.id);
                }
                (1. - *age / RETIRE_SECONDS).clamp(0., 1.)
            } else {
                1.
            };
            if falling || previous_age < landing + 0.22 || drop.retirement.is_some() {
                self.set_object_position(drop.id, drop.capture.at(drop.age.min(landing)));
                self.set_object_scale(drop.id, scale * shrink.max(0.0001));
            }
        }
        if let Some(id) = hit {
            pool.remove(id);
            self.remove_object(id);
        }
        if let Some(id) = retired {
            pool.remove(id);
            if let Some(capture) = pool.pending.take() {
                self.spawn_deposit(&mut pool, capture, Some(id));
            }
        }
        // Explicit object removal may remove the retiring slot before its timer ends.
        if pool.drops.len() < CAPACITY && pool.drops.iter().all(|drop| drop.retirement.is_none()) {
            if let Some(capture) = pool.pending.take() {
                self.spawn_deposit(&mut pool, capture, None);
            }
        }
        pool.wait = (pool.wait - dt).max(0.);
        if pool.wait == 0. && pool.pending.is_none() {
            if let Some(candidate) = self.ground_drop_candidate(
                pool.sequence,
                pool.target_head,
                self.visor_splat_remaining == 0.,
            ) {
                self.enqueue_deposit(
                    &mut pool,
                    Capture {
                        position: candidate.position,
                        floor: candidate.floor,
                        aim: candidate.aim.map(|head| head + Vec3::Y * 0.065),
                    },
                );
                self.consume_ground_drop(candidate.emitter);
                pool.sequence = pool.sequence.wrapping_add(1);
                // Deterministic scene-wide jitter: interval6 produces 4–8 seconds.
                let jitter = (pool.sequence.wrapping_mul(2654435761) % 1001) as f32 / 1000.;
                pool.wait = candidate.interval * (2. / 3. + jitter * 2. / 3.);
            }
        }
        self.sprite_deposits = pool;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::wire::WirePublisher;
    fn capture(x: f32) -> Capture {
        Capture {
            position: Vec3::new(x, 2., -1.),
            floor: 0.012,
            aim: None,
        }
    }
    fn add(scene: &mut SceneManager, x: f32) {
        let mut pool = std::mem::take(&mut scene.sprite_deposits);
        scene.enqueue_deposit(&mut pool, capture(x));
        scene.sprite_deposits = pool;
    }
    #[test]
    fn visor_splat_is_dodgeable_and_expires_after_seven_seconds() {
        for dodge in [false, true] {
            let mut scene = SceneManager::default();
            let head = Vec3::new(0., 1.65, 0.);
            let capture = Capture {
                position: Vec3::new(0.18, 2., -0.14),
                floor: 0.012,
                aim: Some(head + Vec3::Y * 0.065),
            };
            let mut pool = Deposits::default();
            scene.enqueue_deposit(&mut pool, capture);
            scene.sprite_deposits = pool;
            scene.set_sprite_target_player(
                Some(head + if dodge { Vec3::X } else { Vec3::ZERO }),
                true,
            );
            scene.update_sprite_deposits(0.8);
            assert_eq!(scene.visor_splat_remaining(), if dodge { 0. } else { 7. });
            assert_eq!(scene.sprite_deposits.drops.len(), usize::from(dodge));
            scene.update_sprite_deposits(6.5);
            if !dodge {
                assert_eq!(scene.visor_splat_remaining(), 0.5);
            }
            scene.update_sprite_deposits(0.5);
            assert_eq!(scene.visor_splat_remaining(), 0.);
        }
    }
    #[test]
    fn deposits_strict_capacity_retires_oldest_before_reuse() {
        let mut scene = SceneManager::default();
        for i in 0..CAPACITY {
            add(&mut scene, i as f32);
        }
        let first = scene.sprite_deposits.drops[0].id;
        add(&mut scene, 20.);
        add(&mut scene, 30.); // Pending is bounded to one capture.
        assert_eq!(scene.sprite_deposits.drops.len(), CAPACITY);
        assert_eq!(scene.objects.len(), CAPACITY);
        scene.update_sprite_deposits(0.2);
        assert_eq!(scene.sprite_deposits.drops[0].id, first);
        assert!(scene.objects[&first].transform.scale.max_element() < 1.);
        scene.update_sprite_deposits(0.21);
        assert_eq!(scene.sprite_deposits.drops.last().unwrap().id, first);
        assert_eq!(scene.objects[&first].transform.position.x, 20.);
        assert_eq!(scene.objects.len(), CAPACITY);
        assert_eq!(scene.variant_instances.len(), CAPACITY);
    }
    #[test]
    fn deposits_land_finitely_publish_shared_geometry_and_clear() {
        let mut scene = SceneManager::default();
        add(&mut scene, 0.);
        add(&mut scene, 1.);
        let id = scene.sprite_deposits.drops[0].id;
        scene.update_sprite_deposits(0.1);
        assert!(scene.objects[&id].transform.position.y < 2.);
        scene.update_sprite_deposits(2.);
        let object = &scene.objects[&id];
        assert_eq!(object.transform.position.y, 0.012);
        assert_eq!(object.transform.scale, Vec3::ONE);
        assert_eq!(object.emission, 0.);
        assert!(!object.render_options.casts_shadow && !object.render_options.receives_shadow);
        assert_eq!(scene.mesh_variants.len(), 1);
        let bytes = WirePublisher::default().capture(&mut scene).unwrap();
        let mut mirror = SceneManager::default();
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        assert_eq!(mirror.objects.len(), 2);
        assert_eq!(
            mirror.objects[&id].transform.position,
            scene.objects[&id].transform.position
        );
        assert_eq!(mirror.triangle_meshes[&id].mesh.triangle_count(), 72);
        let before = mirror.objects[&id].transform;
        mirror.update_sprite_deposits(100.);
        assert_eq!(mirror.objects[&id].transform.position, before.position);
        scene.remove_object(id);
        assert_eq!(scene.sprite_deposits.drops.len(), 1);
        scene.clear_scene();
        assert!(scene.sprite_deposits.drops.is_empty());
        assert!(scene.sprite_deposits.mesh.is_none());
        assert!(scene.mesh_variants.is_empty());
    }
}
