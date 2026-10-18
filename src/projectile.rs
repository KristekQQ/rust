//! CPU sphere proxies. Scratch storage is retained between frames.
use glam::Vec3;

#[derive(Clone, Copy)]
pub(crate) struct Body {
    pub id: u32,
    /// Nonzero for pieces from the same solid; overlapping sphere proxies are false contacts.
    pub family: u64,
    pub position: Vec3,
    pub velocity: Vec3,
    pub gravity: f32,
    pub bounce: f32,
    pub radius: f32,
}

fn collide(a: &mut Body, b: &mut Body) -> (u32, bool) {
    if a.family != 0 && a.family == b.family {
        return (0, false);
    }
    let delta = b.position - a.position;
    let radii = a.radius + b.radius;
    let squared = delta.length_squared();
    if squared >= radii * radii {
        return (0, false);
    }
    let distance = squared.sqrt();
    let normal = if distance > 1e-6 {
        delta / distance
    } else {
        Vec3::X
    };
    let correction = normal * ((radii - distance) * 0.5 + 1e-5);
    a.position -= correction;
    b.position += correction;
    let closing_speed = (b.velocity - a.velocity).dot(normal);
    if closing_speed >= 0.0 {
        return (0, true);
    }
    let impulse = normal * (-(1.0 + 0.75) * closing_speed * 0.5);
    a.velocity -= impulse;
    b.velocity += impulse;
    (1, true)
}

pub(crate) fn simulate(
    bodies: &mut [Body],
    order: &mut Vec<usize>,
    pairs: &mut Vec<(usize, usize)>,
    dt: f32,
) -> u32 {
    if bodies.is_empty() {
        return 0;
    }
    let duration = dt.min(0.25);
    let steps = (duration / (1.0 / 240.0)).ceil().max(1.0) as u32;
    let step = duration / steps as f32;
    let friction = (-0.9 * step).exp();
    let max_radius = bodies.iter().map(|b| b.radius).fold(0.0, f32::max);
    let mut contacts: u32 = 0;
    order.clear();
    order.extend(0..bodies.len());
    for _ in 0..steps {
        for body in bodies.iter_mut() {
            body.velocity.y -= body.gravity * step;
            body.position += body.velocity * step;
            if body.position.y < body.radius {
                body.position.y = body.radius;
                if body.velocity.y < 0.0 {
                    body.velocity.y = -body.velocity.y * body.bounce;
                }
                body.velocity.x *= friction;
                body.velocity.z *= friction;
            }
        }
        // Corrections can create secondary contacts; rebuild the broad phase.
        for _ in 0..4 {
            let mut corrected = false;
            if bodies.len() <= 16 {
                // Tiny groups are cheaper without sorting or a spatial index.
                for i in 0..bodies.len() {
                    let (before, after) = bodies.split_at_mut(i + 1);
                    for b in after {
                        let (hits, moved) = collide(&mut before[i], b);
                        contacts = contacts.saturating_add(hits);
                        corrected |= moved;
                    }
                }
            } else {
                // Sweep sorted X bounds; build candidates before any contact correction.
                order.sort_unstable_by(|&a, &b| {
                    bodies[a].position.x.total_cmp(&bodies[b].position.x)
                });
                pairs.clear();
                for (offset, &a) in order.iter().enumerate() {
                    for &b in &order[offset + 1..] {
                        let delta = bodies[b].position - bodies[a].position;
                        if delta.x > bodies[a].radius + max_radius {
                            break;
                        }
                        let reach = bodies[a].radius + bodies[b].radius;
                        if delta.x > reach || delta.y.abs() > reach || delta.z.abs() > reach {
                            continue;
                        }
                        pairs.push((a.min(b), a.max(b)));
                    }
                }
                // Stable ID order keeps collision results independent of sorting ties.
                pairs.sort_unstable();
                for &(a, b) in pairs.iter() {
                    let (before, after) = bodies.split_at_mut(b);
                    let (hits, moved) = collide(&mut before[a], &mut after[0]);
                    contacts = contacts.saturating_add(hits);
                    corrected |= moved;
                }
            }
            if !corrected {
                break;
            }
        }
    }
    contacts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projectile_sibling_proxies_never_inject_a_false_impulse() {
        let body = Body {
            id: 1,
            family: 42,
            position: Vec3::Y * 3.,
            velocity: Vec3::X * 0.1,
            gravity: 0.,
            bounce: 0.,
            radius: 0.2,
        };
        let mut bodies = [
            body,
            Body {
                id: 2,
                velocity: -Vec3::X * 0.1,
                ..body
            },
        ];
        let contacts = simulate(&mut bodies, &mut Vec::new(), &mut Vec::new(), 1. / 90.);
        assert_eq!(contacts, 0);
        assert_eq!(bodies[0].velocity, body.velocity);
        assert!(bodies[0].position.distance(bodies[1].position) < 0.003);
        bodies[1].family = 43;
        bodies[0].position.x = -0.02;
        bodies[1].position.x = 0.02;
        let contacts = simulate(&mut bodies, &mut Vec::new(), &mut Vec::new(), 1. / 90.);
        assert!(contacts > 0);
        assert!(bodies[0].position.distance(bodies[1].position) >= 0.4);
    }

    #[test]
    fn sweep_rebuilds_after_correction_creates_secondary_contact() {
        let mut bodies: Vec<_> = (0..17)
            .map(|id| Body {
                id,
                family: 0,
                position: Vec3::new(
                    match id {
                        0 => 0.0,
                        1 => 1.0,
                        2 => 3.1,
                        _ => id as f32 * 10.0,
                    },
                    4.0,
                    0.0,
                ),
                velocity: Vec3::ZERO,
                gravity: 0.0,
                bounce: 0.0,
                radius: 1.0,
            })
            .collect();
        simulate(&mut bodies, &mut Vec::new(), &mut Vec::new(), 1.0 / 240.0);
        assert!((bodies[2].position - bodies[1].position).length() >= 2.0 - 1e-4);
    }
    #[test]
    fn sweep_keeps_contacts_across_mixed_radii_and_equal_x() {
        let mut bodies: Vec<_> = (0..40)
            .map(|i| Body {
                id: i,
                family: 0,
                position: Vec3::new((i / 2) as f32 * 4.0, 4.0 + (i % 2) as f32 * 0.4, 0.0),
                velocity: if i % 2 == 0 { Vec3::Y } else { -Vec3::Y },
                radius: if i % 2 == 0 { 0.2 } else { 0.5 },
                gravity: 0.0,
                bounce: 0.0,
            })
            .collect();
        let contacts = simulate(&mut bodies, &mut Vec::new(), &mut Vec::new(), 1.0 / 240.0);
        assert_eq!(contacts, 20);
        for pair in bodies.as_chunks::<2>().0 {
            assert!((pair[1].position - pair[0].position).length() >= 0.7 - 1e-5);
        }
    }
}
