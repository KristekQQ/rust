use glam::{Mat4, Quat, Vec3};

// Matches examples/neon-arena/xp-drink.js: base-origin mesh, rolled rim and opening.
const RIM_HEIGHT: f32 = 0.219;
const OPENING: Vec3 = Vec3::new(0., 0.216, 0.015);
const CENTER_HEIGHT: f32 = 0.10;
const SIP_SECONDS: f32 = 0.20;
const SIP_RADIUS: f32 = 0.16;
// The mesh grows along +Y; the equipped Neon katana points along grip -Z.
const CAN_TO_SWORD: f32 = -std::f32::consts::FRAC_PI_2;

/// The can mesh is authored upright, with the origin at its base (metres).
#[derive(Clone, Copy)]
pub struct DrinkCarrier {
    pub visit: (u32, u32),
    pub lip: Vec3,
    pub rotation: Quat,
    pub available: bool,
}

#[derive(Default)]
pub struct XpDrink {
    state: DrinkState,
    visit: Option<(u32, u32)>,
}
#[derive(Default)]
enum DrinkState {
    #[default]
    Hidden,
    Carried {
        lip: Vec3,
        rotation: Quat,
        available: bool,
        age: f32,
    },
    Offered {
        base: Vec3,
        age: f32,
    },
    Held {
        hand: usize,
        sip: f32,
    },
}

impl XpDrink {
    /// Lifecycle resets hide the drink without reissuing the same cat visit.
    pub fn reset(&mut self) {
        self.state = DrinkState::Hidden;
    }

    pub fn follow_carrier(&mut self, carrier: Option<DrinkCarrier>, enabled: bool) {
        let Some(carrier) = carrier else {
            if matches!(self.state, DrinkState::Carried { .. }) {
                self.reset();
            }
            if !enabled {
                self.reset();
            }
            return;
        };
        let fresh = self.visit != Some(carrier.visit);
        if fresh && !matches!(self.state, DrinkState::Held { .. }) {
            self.visit = Some(carrier.visit);
            self.state = if enabled {
                DrinkState::Carried {
                    lip: carrier.lip,
                    rotation: carrier.rotation,
                    available: carrier.available,
                    age: 0.,
                }
            } else {
                DrinkState::Hidden
            };
        }
        if !enabled {
            self.reset();
            return;
        }
        if let DrinkState::Carried {
            lip,
            rotation,
            available,
            ..
        } = &mut self.state
        {
            *lip = carrier.lip;
            *rotation = carrier.rotation;
            *available = carrier.available;
        }
    }

    pub fn offer(&mut self, origin: Vec3) {
        if !self.active() && origin.is_finite() {
            self.state = DrinkState::Offered {
                base: origin + Vec3::Y * 0.52,
                age: 0.,
            };
        }
    }

    pub fn active(&self) -> bool {
        !matches!(self.state, DrinkState::Hidden)
    }

    pub fn carried_visit(&self) -> Option<(u32, u32)> {
        matches!(self.state, DrinkState::Carried { .. })
            .then_some(self.visit)
            .flatten()
    }

    pub fn held_hand(&self) -> Option<usize> {
        match &self.state {
            DrinkState::Held { hand, .. } => Some(*hand),
            _ => None,
        }
    }

    pub fn update(
        &mut self,
        dt: f32,
        head: Option<Mat4>,
        hands: [Option<Mat4>; 2],
    ) -> (Option<Mat4>, bool) {
        let dt = if dt.is_finite() { dt.max(0.) } else { 0. };
        let hands = hands.map(|h| h.filter(|p| p.is_finite()));
        if let DrinkState::Carried {
            lip,
            rotation,
            available,
            age,
        } = &mut self.state
        {
            if *available {
                *age += dt;
            }
            if *age >= 12. {
                self.reset();
                return (None, false);
            }
            let base = *lip - *rotation * Vec3::Y * RIM_HEIGHT;
            let center = base + *rotation * Vec3::Y * 0.1;
            let pickup = available
                .then(|| {
                    hands
                        .iter()
                        .enumerate()
                        .filter_map(|(hand, pose)| {
                            let pose = (*pose)?;
                            let distance = pose.w_axis.truncate().distance_squared(center);
                            (distance <= 0.2 * 0.2).then_some((hand, distance))
                        })
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                })
                .flatten();
            if let Some((hand, _)) = pickup {
                self.state = DrinkState::Held { hand, sip: 0. };
            } else {
                return (
                    Some(Mat4::from_rotation_translation(*rotation, base)),
                    false,
                );
            }
        }
        if let DrinkState::Offered { base, age } = &mut self.state {
            *age += dt;
            if *age >= 12. {
                self.state = DrinkState::Hidden;
                return (None, false);
            }
            let position = *base + Vec3::Y * (0.012 * (*age * 3.).sin());
            let center = position + Vec3::Y * 0.1;
            let nearest = hands
                .iter()
                .enumerate()
                .filter_map(|(i, pose)| {
                    let p = pose.as_ref()?;
                    let distance = p.w_axis.truncate().distance_squared(center);
                    (distance <= 0.2 * 0.2).then_some((i, distance))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((hand, _)) = nearest {
                self.state = DrinkState::Held { hand, sip: 0. };
            } else {
                return (Some(Mat4::from_translation(position)), false);
            }
        }
        let DrinkState::Held { hand, sip } = &mut self.state else {
            return (None, false);
        };
        let Some(grip) = hands[*hand] else {
            *sip = 0.;
            return (None, false);
        };
        let rotation = Quat::from_mat4(&grip).normalize() * Quat::from_rotation_x(CAN_TO_SWORD);
        let center = grip.w_axis.truncate();
        let base = center - rotation * Vec3::Y * CENTER_HEIGHT;
        let lip = base + rotation * OPENING;
        let at_mouth = head.filter(|p| p.is_finite()).is_some_and(|p| {
            let mouth = p.transform_point3(Vec3::new(0., -0.12, -0.16));
            center
                .distance_squared(mouth)
                .min(lip.distance_squared(mouth))
                <= SIP_RADIUS * SIP_RADIUS
        });
        // A tracking stall must not count as a completed sip.
        if at_mouth {
            *sip += dt.min(0.08);
        } else {
            *sip = 0.;
        }
        if *sip >= SIP_SECONDS {
            self.state = DrinkState::Hidden;
            return (None, true);
        }
        (Some(Mat4::from_rotation_translation(rotation, base)), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn carrier(visit: u32, available: bool) -> DrinkCarrier {
        DrinkCarrier {
            visit: (4, visit),
            lip: Vec3::new(0.2, 0.6, -0.7),
            rotation: Quat::IDENTITY,
            available,
        }
    }
    fn sip_grip(head: Mat4, tilt: f32) -> Mat4 {
        let rotation = Quat::from_rotation_x(tilt - CAN_TO_SWORD);
        let can = rotation * Quat::from_rotation_x(CAN_TO_SWORD);
        let mouth = head.transform_point3(Vec3::new(0., -0.12, -0.16));
        Mat4::from_rotation_translation(rotation, mouth - can * (OPENING - Vec3::Y * CENTER_HEIGHT))
    }
    #[test]
    fn xp_drink_centers_in_either_grip_with_lid_along_blade() {
        for hand in 0..2 {
            let mut drink = XpDrink::default();
            let mut carry = carrier(1, false);
            carry.rotation = Quat::from_rotation_z(1.2);
            drink.follow_carrier(Some(carry), true);
            let original = drink.update(0.1, None, [None; 2]).0.unwrap();
            assert!((original.transform_point3(Vec3::Y * RIM_HEIGHT) - carry.lip).length() < 1e-6);
            let grip = Mat4::from_rotation_translation(
                Quat::from_rotation_y(0.4),
                original.transform_point3(Vec3::Y * CENTER_HEIGHT),
            );
            let mut hands = [None; 2];
            hands[hand] = Some(grip);
            drink.update(0.1, None, hands);
            assert_eq!(
                drink.held_hand(),
                None,
                "approach is carried until available"
            );
            carry.available = true;
            drink.follow_carrier(Some(carry), true);
            let pickup = drink.update(0.01, None, hands).0.unwrap();
            assert_eq!(drink.held_hand(), Some(hand));
            let local = grip.inverse() * pickup;
            assert!((local.transform_point3(Vec3::Y * CENTER_HEIGHT)).length() < 1e-6);
            let axis = local.transform_vector3(Vec3::Y);
            assert!(axis.distance(Vec3::NEG_Z) < 1e-6, "lid follows sword tip");
            drink.follow_carrier(None, true);
            hands[hand] = Some(Mat4::from_translation(Vec3::Z * 0.1) * grip);
            let moved = drink.update(0.01, None, hands).0.unwrap();
            assert!(
                (moved.w_axis.truncate() - pickup.w_axis.truncate() - Vec3::Z * 0.1).length()
                    < 1e-6
            );
        }
    }
    #[test]
    fn xp_drink_consumes_near_face_with_either_hand_at_any_tilt() {
        for hand in 0..2 {
            let mut drink = XpDrink::default();
            let carry = carrier(1, true);
            drink.follow_carrier(Some(carry), true);
            let mut hands = [None; 2];
            hands[hand] = Some(Mat4::from_translation(
                carry.lip - Vec3::Y * (RIM_HEIGHT - CENTER_HEIGHT),
            ));
            drink.update(0., None, hands);
            let head = Mat4::from_translation(Vec3::new(0., 1.6, 0.));
            let mut away = sip_grip(head, std::f32::consts::FRAC_PI_2);
            away.w_axis.x += 0.25;
            hands[hand] = Some(away);
            for _ in 0..75 {
                assert!(!drink.update(1. / 90., Some(head), hands).1);
            }
            hands[hand] = Some(sip_grip(
                head,
                if hand == 0 {
                    0.
                } else {
                    -std::f32::consts::FRAC_PI_2
                },
            ));
            let (rendered, consumed) = drink.update(0.1, Some(head), hands);
            assert!(!consumed, "one long frame cannot finish drinking");
            let opening = rendered.unwrap().transform_point3(OPENING);
            assert!(
                opening.distance(head.transform_point3(Vec3::new(0., -0.12, -0.16))) < 1e-6,
                "visible opening anchors sip motion"
            );
            // Tracking loss clears accumulated sipping.
            hands[hand] = None;
            assert!(!drink.update(0.1, Some(head), hands).1);
            hands[hand] = Some(sip_grip(
                head,
                if hand == 0 {
                    0.
                } else {
                    -std::f32::consts::FRAC_PI_2
                },
            ));
            let mut consumed = 0;
            for _ in 0..75 {
                consumed += usize::from(drink.update(1. / 90., Some(head), hands).1);
            }
            assert_eq!(consumed, 1);
            assert!(!drink.active());
        }
    }
    #[test]
    fn xp_drink_expiry_reset_and_consumption_do_not_repeat_the_visit() {
        let mut drink = XpDrink::default();
        let carry = carrier(1, true);
        drink.follow_carrier(Some(carry), true);
        drink.update(12., None, [None; 2]);
        drink.follow_carrier(Some(carry), true);
        assert!(!drink.active());
        drink.reset();
        drink.follow_carrier(Some(carry), true);
        assert!(!drink.active());
        let carry = carrier(2, true);
        drink.follow_carrier(Some(carry), true);
        let center = carry.lip - Vec3::Y * (RIM_HEIGHT - CENTER_HEIGHT);
        drink.update(0.01, None, [Some(Mat4::from_translation(center)), None]);
        let head = Mat4::from_translation(Vec3::new(0., 1.6, 0.));
        let pose = sip_grip(head, std::f32::consts::FRAC_PI_2);
        let mut consumed = 0;
        for _ in 0..75 {
            consumed += usize::from(drink.update(1. / 90., Some(head), [Some(pose), None]).1);
        }
        assert_eq!(consumed, 1);
        drink.follow_carrier(Some(carry), true);
        assert!(!drink.active());
        drink.follow_carrier(Some(carrier(3, true)), false);
        drink.follow_carrier(Some(carrier(3, true)), true);
        assert!(
            !drink.active(),
            "round activation does not reissue an earlier visit"
        );
    }
}
