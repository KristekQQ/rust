//! Opt-in XR locomotion with optional shared scene collision surfaces. Platform poses stay in local-floor space;
//! one Rust-owned rigid origin transforms both eyes, head and equipment together.
use glam::{Mat4, Vec2, Vec3};

#[derive(Clone, Debug)]
pub struct LocomotionConfig {
    /// Walkable head-centre bounds; callers inset these from authored walls.
    pub bounds: [f32; 4],
    /// Solid floor footprints, inflated here by the player's clearance radius.
    pub obstacles: Vec<[f32; 4]>,
    pub scene_collisions: bool,
    pub teleport_speed: f32,
    pub speed: f32,
    pub teleport_range: f32,
    pub slow_scale: f32,
    pub marker: u32,
}
impl LocomotionConfig {
    pub fn validate(&self) -> Result<(), String> {
        fn rect(r: &[f32; 4]) -> bool {
            r.iter().all(|v| v.is_finite() && v.abs() <= 90.) && r[0] < r[1] && r[2] < r[3]
        }
        if !rect(&self.bounds)
            || self.obstacles.len() > 128
            || !self.obstacles.iter().all(rect)
            || !self.teleport_speed.is_finite()
            || !(1. ..=30.).contains(&self.teleport_speed)
            || !self.speed.is_finite()
            || !(0.1..=6.).contains(&self.speed)
            || !self.teleport_range.is_finite()
            || !(1. ..=30.).contains(&self.teleport_range)
            || !self.slow_scale.is_finite()
            || !(0.01..=1.).contains(&self.slow_scale)
        {
            return Err("Invalid locomotion bounds, obstacles, speed, range or time scale".into());
        }
        if !self.walkable(Vec2::ZERO) {
            return Err("Locomotion spawn must be walkable".into());
        }
        Ok(())
    }
    fn walkable(&self, p: Vec2) -> bool {
        let b = self.bounds;
        p.x >= b[0]
            && p.x <= b[1]
            && p.y >= b[2]
            && p.y <= b[3]
            && (self.scene_collisions || !self.obstacles.iter().any(|r| inside(p, expanded(*r))))
    }
    fn clear_segment(&self, a: Vec2, b: Vec2) -> bool {
        self.walkable(b)
            && !self
                .obstacles
                .iter()
                .any(|r| segment_hits(a, b, expanded(*r)))
    }
}
fn expanded(r: [f32; 4]) -> [f32; 4] {
    [r[0] - 0.4, r[1] + 0.4, r[2] - 0.4, r[3] + 0.4]
}
fn inside(p: Vec2, r: [f32; 4]) -> bool {
    p.x >= r[0] && p.x <= r[1] && p.y >= r[2] && p.y <= r[3]
}
fn flat(p: Vec3) -> Vec2 {
    Vec2::new(p.x, p.z)
}
fn segment_hits(a: Vec2, b: Vec2, r: [f32; 4]) -> bool {
    let mut lo: f32 = 0.;
    let mut hi: f32 = 1.;
    for (start, delta, min, max) in [(a.x, b.x - a.x, r[0], r[1]), (a.y, b.y - a.y, r[2], r[3])] {
        if delta.abs() < 1e-6 {
            if start < min || start > max {
                return false;
            }
        } else {
            let x = (min - start) / delta;
            let y = (max - start) / delta;
            lo = lo.max(x.min(y));
            hi = hi.min(x.max(y));
            if lo > hi {
                return false;
            }
        }
    }
    true
}
#[derive(Default)]
pub struct XrLocomotion {
    pub config: Option<LocomotionConfig>,
    offset: Vec3,
    yaw: f32,
    pub aiming: bool,
    pub target: Option<Vec3>,
    held: bool,
    armed: bool,
    turn_latched: bool,
    vertical_speed: f32,
    previous_head: Option<Vec3>,
}
impl XrLocomotion {
    pub fn configure(&mut self, config: Option<LocomotionConfig>) -> Result<(), String> {
        if let Some(config) = &config {
            config.validate()?;
        }
        *self = Self {
            config,
            ..Default::default()
        };
        Ok(())
    }
    pub fn origin(&self) -> Mat4 {
        Mat4::from_translation(self.offset) * Mat4::from_rotation_y(self.yaw)
    }
    pub fn world(&self, pose: Mat4) -> Mat4 {
        self.origin() * pose
    }
    pub fn time_scale(&self) -> f32 {
        if self.aiming {
            self.config.as_ref().map_or(1., |c| c.slow_scale)
        } else {
            1.
        }
    }
    /// Cancellation requires a fresh release before A can arm again.
    pub fn cancel(&mut self) {
        self.aiming = false;
        self.target = None;
        self.held = false;
        self.armed = false;
        self.turn_latched = false;
        self.previous_head = None;
    }
    pub fn reset(&mut self) {
        self.cancel();
        self.offset = Vec3::ZERO;
        self.yaw = 0.;
        self.vertical_speed = 0.;
    }
    /// Returns true for a discontinuity, so equipment cannot sweep across a teleport/snap.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        head: Option<Mat4>,
        ray: Option<Mat4>,
        axes: [f32; 3],
        held: bool,
        cancel: bool,
        dt: f32,
    ) -> bool {
        self.step_with_world(head, ray, axes, held, cancel, dt, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn step_with_world(
        &mut self,
        head: Option<Mat4>,
        ray: Option<Mat4>,
        axes: [f32; 3],
        held: bool,
        cancel: bool,
        dt: f32,
        world: Option<&crate::collision::CollisionWorld>,
    ) -> bool {
        let Some(config) = self.config.clone() else {
            return false;
        };
        let Some(head) = head.filter(|m| crate::xr_input::valid_pose(*m)) else {
            self.cancel();
            return false;
        };
        if !dt.is_finite() || !(0. ..=0.25).contains(&dt) || !axes.iter().all(|a| a.is_finite()) {
            self.cancel();
            return false;
        }
        let ray = ray.filter(|m| crate::xr_input::valid_pose(*m));
        let scene_world = world.filter(|_| config.scene_collisions);
        if (ray.is_none() || cancel) && scene_world.is_none() {
            self.cancel();
            return false;
        }
        let held = held && ray.is_some() && !cancel;
        if ray.is_none() || cancel {
            self.cancel();
        }
        if !held {
            self.armed = true;
        }
        let mut jumped = false;
        let mut world_head = self.world(head).w_axis.truncate();
        let height = head.w_axis.y.clamp(0.6, 2.5);
        let half = Vec3::new(0.4, height * 0.5, 0.4);
        if let Some(world) = scene_world {
            if let Some(previous) = self.previous_head {
                let previous = self.origin().transform_point3(previous);
                let feet = Vec3::new(previous.x, self.offset.y, previous.z);
                let delta = Vec3::new(world_head.x - previous.x, 0., world_head.z - previous.z);
                let moved = world.move_character(feet, half, delta, None);
                let resolved = if config.walkable(flat(moved.position)) {
                    moved.position
                } else {
                    feet
                };
                self.offset += resolved - feet - delta;
                world_head = self.world(head).w_axis.truncate();
            }
        }
        self.previous_head = Some(head.w_axis.truncate());
        if axes[2].abs() < 0.25 {
            self.turn_latched = false;
        }
        if !held && axes[2].abs() > 0.7 && !self.turn_latched {
            self.yaw -= axes[2].signum() * std::f32::consts::FRAC_PI_6;
            // Turn about the physical head, not the reference-space origin.
            self.offset += world_head - self.world(head).w_axis.truncate();
            self.turn_latched = true;
            jumped = true;
        }
        if !held {
            let stick = Vec2::new(axes[0], axes[1]).clamp_length_max(1.);
            if stick.length() > 0.18 {
                let forward = self.world(head).transform_vector3(Vec3::NEG_Z);
                let forward = Vec3::new(forward.x, 0., forward.z)
                    .try_normalize()
                    .unwrap_or(Vec3::NEG_Z);
                let right = forward.cross(Vec3::Y);
                let delta = (right * stick.x - forward * stick.y) * config.speed * dt.min(0.05);
                if let Some(world) = scene_world {
                    let feet = Vec3::new(world_head.x, self.offset.y, world_head.z);
                    let moved = world.move_character(feet, half, delta, None);
                    if config.walkable(flat(moved.position)) {
                        self.offset += moved.position - feet;
                        world_head = self.world(head).w_axis.truncate();
                    }
                } else {
                    // Axis sliding plus swept footprint tests prevent tunnelling at low FPS.
                    for step in [Vec3::new(delta.x, 0., 0.), Vec3::new(0., 0., delta.z)] {
                        if config.clear_segment(flat(world_head), flat(world_head + step)) {
                            self.offset += step;
                            world_head += step;
                        }
                    }
                }
            }
        }
        if let Some(world) = scene_world {
            // Use real elapsed time (not the aiming slow-motion scale), in bounded substeps.
            let steps = (dt / (1. / 72.)).ceil().max(1.) as usize;
            let h = dt / steps as f32;
            for _ in 0..steps {
                self.vertical_speed = (self.vertical_speed - 9.8 * h).max(-30.);
                let feet = Vec3::new(world_head.x, self.offset.y, world_head.z);
                let moved =
                    world.move_character(feet, half, Vec3::Y * self.vertical_speed * h, None);
                self.offset.y = moved.position.y;
                if moved.grounded || moved.hit_ceiling {
                    self.vertical_speed = 0.;
                }
            }
            world_head = self.world(head).w_axis.truncate();
        }
        self.aiming = held && self.armed;
        if self.aiming {
            self.target = if let Some(world) = scene_world {
                self.surface_landing(self.world(ray.unwrap()), world_head, half, &config, world)
            } else {
                self.landing(self.world(ray.unwrap()), world_head, &config)
            };
        } else {
            if self.held && !held {
                if let Some(target) = self.target.filter(|t| config.walkable(flat(*t))) {
                    if scene_world.is_none_or(|w| {
                        w.character_clear(target, half)
                            && w.cast_segment(target + Vec3::Y * 0.005, target - Vec3::Y * 0.005)
                                .is_some_and(|hit| {
                                    hit.normal.y > 0.9 && hit.position.distance(target) < 0.006
                                })
                    }) {
                        self.offset +=
                            Vec3::new(target.x - world_head.x, 0., target.z - world_head.z);
                        if scene_world.is_some() {
                            self.offset.y = target.y;
                            self.vertical_speed = 0.;
                        }
                        jumped = true;
                    }
                }
            }
            self.target = None;
        }
        self.held = self.aiming;
        jumped
    }
    fn surface_landing(
        &self,
        ray: Mat4,
        head: Vec3,
        half: Vec3,
        config: &LocomotionConfig,
        world: &crate::collision::CollisionWorld,
    ) -> Option<Vec3> {
        let start = ray.w_axis.truncate();
        // A controller behind a wall cannot author a path through it.
        if world.cast_segment(head, start).is_some() {
            return None;
        }
        let velocity = ray.transform_vector3(Vec3::NEG_Z) * config.teleport_speed;
        let mut previous = start;
        for n in 1..=360 {
            let t = n as f32 / 60.;
            let point = start + velocity * t - Vec3::Y * (4.9 * t * t);
            if let Some(hit) = world.cast_segment(previous, point) {
                let target = hit.position;
                return (hit.normal.y > 0.9
                    && velocity.y - 9.8 * t < 0.
                    && flat(target).distance(flat(head)) <= config.teleport_range
                    && config.walkable(flat(target))
                    && world.character_clear(target, half))
                .then_some(target);
            }
            previous = point;
            if point.y < -100. {
                break;
            }
        }
        None
    }
    fn landing(&self, ray: Mat4, head: Vec3, config: &LocomotionConfig) -> Option<Vec3> {
        let start = ray.w_axis.truncate();
        if start.y <= 0. || start.y > 6.5 {
            return None;
        }
        let velocity = ray.transform_vector3(Vec3::NEG_Z) * 9.;
        // Solve y(t)=0 analytically, sample only the collision path, not the endpoint.
        let time = (velocity.y + (velocity.y * velocity.y + 19.6 * start.y).sqrt()) / 9.8;
        let target = start + velocity * time - Vec3::Y * (4.9 * time * time);
        if flat(target).distance(flat(head)) > config.teleport_range
            || !config.walkable(flat(target))
        {
            return None;
        }
        let mut previous = flat(head);
        for n in 0..=48 {
            let t = time * n as f32 / 48.;
            let point = start + velocity * t - Vec3::Y * (4.9 * t * t);
            if point.y > 6.5 || !config.clear_segment(previous, flat(point)) {
                return None;
            }
            previous = flat(point);
        }
        Some(Vec3::new(target.x, 0., target.z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> XrLocomotion {
        let mut l = XrLocomotion::default();
        l.configure(Some(LocomotionConfig {
            bounds: [-11., 11., -23., 7.],
            obstacles: vec![],
            scene_collisions: false,
            teleport_speed: 9.,
            speed: 2.5,
            teleport_range: 10.,
            slow_scale: 0.15,
            marker: 1,
        }))
        .unwrap();
        l
    }
    fn head() -> Option<Mat4> {
        Some(Mat4::from_translation(Vec3::new(0., 1.6, 0.)))
    }
    fn ray() -> Option<Mat4> {
        Some(Mat4::from_translation(Vec3::new(0.3, 1.1, -0.2)))
    }
    fn tick(l: &mut XrLocomotion, axes: [f32; 3], held: bool) -> bool {
        l.step(head(), ray(), axes, held, false, 1. / 72.)
    }
    fn surface_world() -> crate::collision::CollisionWorld {
        use crate::collision::{Aabb, Collider, CollisionWorld};
        let mut world = CollisionWorld::default();
        world.rebuild(vec![
            Collider {
                id: 1,
                bounds: Aabb::new(Vec3::new(-20., -1., -30.), Vec3::new(20., 0., 10.)).unwrap(),
            },
            Collider {
                id: 2,
                bounds: Aabb::new(Vec3::new(-3., 1.7, -9.), Vec3::new(3., 2., -3.)).unwrap(),
            },
        ]);
        world
    }
    fn aimed_ray(target: Vec3) -> Mat4 {
        let start = Vec3::new(0., 1.1, 0.);
        let d = -target.z;
        let y = target.y - start.y;
        let v: f32 = 9.;
        let pitch = ((v * v + (v.powi(4) - 9.8 * (9.8 * d * d + 2. * y * v * v)).sqrt())
            / (9.8 * d))
            .atan();
        Mat4::from_translation(start) * Mat4::from_rotation_x(pitch)
    }
    #[test]
    fn xr_locomotion_surface_teleport_stand_walk_off_and_land() {
        let mut l = setup();
        l.config.as_mut().unwrap().scene_collisions = true;
        let world = surface_world();
        let ray = Some(aimed_ray(Vec3::new(0., 2., -6.)));
        l.step_with_world(head(), ray, [0.; 3], false, false, 1. / 72., Some(&world));
        l.step_with_world(head(), ray, [0.; 3], true, false, 1. / 72., Some(&world));
        assert!(
            (l.target.unwrap() - Vec3::new(0., 2., -6.)).length() < 0.05,
            "{:?}",
            l.target
        );
        assert!(l.step_with_world(head(), ray, [0.; 3], false, false, 1. / 72., Some(&world)));
        assert!((l.offset.y - 2.).abs() < 1e-5);
        for _ in 0..120 {
            l.step_with_world(head(), ray, [0.; 3], false, false, 1. / 72., Some(&world));
        }
        assert!((l.world(head().unwrap()).w_axis.y - 3.6).abs() < 1e-5);
        for _ in 0..120 {
            l.step_with_world(
                head(),
                ray,
                [1., 0., 0.],
                false,
                false,
                1. / 72.,
                Some(&world),
            );
        }
        assert!(l.offset.x > 3.4 && l.offset.y < 1.9, "{:?}", l.offset);
        for _ in 0..120 {
            l.step_with_world(head(), None, [0.; 3], false, false, 1. / 72., Some(&world));
        }
        assert!(
            l.offset.y.abs() < 1e-5,
            "must land on street even without controller tracking"
        );
    }
    #[test]
    fn xr_locomotion_surface_room_scale_edge_and_under_bridge() {
        let mut l = setup();
        l.config.as_mut().unwrap().scene_collisions = true;
        let world = surface_world();
        l.offset = Vec3::new(0., 2., -6.);
        l.step_with_world(head(), ray(), [0.; 3], false, false, 1. / 72., Some(&world));
        let moved = Some(Mat4::from_translation(Vec3::new(4., 1.6, 0.)));
        for _ in 0..120 {
            l.step_with_world(moved, ray(), [0.; 3], false, false, 1. / 72., Some(&world));
        }
        assert!(
            l.offset.y.abs() < 1e-5,
            "physical walking beyond edge falls"
        );
        l.reset();
        l.offset.z = -6.;
        l.step_with_world(
            head(),
            ray(),
            [1., 0., 0.],
            false,
            false,
            1. / 72.,
            Some(&world),
        );
        assert!(
            l.offset.x > 0. && l.offset.y.abs() < 1e-5,
            "bridge must leave space below it"
        );
    }
    #[test]
    fn xr_locomotion_surface_rejects_insufficient_headroom_and_sides() {
        use crate::collision::{Aabb, Collider};
        let mut l = setup();
        l.config.as_mut().unwrap().scene_collisions = true;
        let mut world = surface_world();
        let mut boxes = world.query(
            Aabb::new(Vec3::splat(-50.), Vec3::splat(50.)).unwrap(),
            None,
        );
        boxes.push(Collider {
            id: 3,
            bounds: Aabb::new(Vec3::new(-3., 3., -9.), Vec3::new(3., 3.2, -3.)).unwrap(),
        });
        world.rebuild(boxes);
        let half = Vec3::new(0.4, 0.8, 0.4);
        assert!(!world.character_clear(Vec3::new(0., 2., -6.), half));
        let side = Mat4::from_translation(Vec3::new(0., 1.8, 0.)) * Mat4::from_rotation_x(0.2);
        assert!(l
            .surface_landing(
                side,
                Vec3::new(0., 1.6, 0.),
                half,
                l.config.as_ref().unwrap(),
                &world
            )
            .is_none());
    }
    #[test]
    fn xr_locomotion_walk_deadzone_and_bounds() {
        let mut l = setup();
        tick(&mut l, [0.1, 0., 0.], false);
        assert_eq!(l.offset, Vec3::ZERO);
        for _ in 0..1500 {
            tick(&mut l, [0., -1., 0.], false);
        }
        assert!(l.offset.z >= -23. && l.offset.z < -22.9);
    }
    #[test]
    fn xr_locomotion_teleport_slow_release_and_real_head_offset() {
        let mut l = setup();
        let head_transform = Mat4::from_translation(Vec3::new(1., 1.7, 0.5));
        let h = Some(head_transform);
        l.step(h, ray(), [0.; 3], false, false, 0.01);
        l.step(h, ray(), [0.; 3], true, false, 0.01);
        assert_eq!(l.time_scale(), 0.15);
        let target = l.target.unwrap();
        assert!(l.step(h, ray(), [0.; 3], false, false, 0.01));
        let new_head = l.world(head_transform).w_axis.truncate();
        assert!(flat(new_head).distance(flat(target)) < 1e-5);
        assert!((new_head.y - 1.7).abs() < 1e-5);
        assert_eq!(l.time_scale(), 1.);
    }
    #[test]
    fn xr_locomotion_tracking_loss_and_cancel_never_teleport() {
        let mut l = setup();
        tick(&mut l, [0.; 3], false);
        tick(&mut l, [0.; 3], true);
        l.step(None, None, [0.; 3], true, false, 0.01);
        tick(&mut l, [0.; 3], true);
        tick(&mut l, [0.; 3], false);
        assert_eq!(l.offset, Vec3::ZERO);
        assert_eq!(l.time_scale(), 1.);
        tick(&mut l, [0.; 3], true);
        l.step(head(), ray(), [0.; 3], true, true, 0.01);
        tick(&mut l, [0.; 3], false);
        assert_eq!(l.offset, Vec3::ZERO);
    }
    #[test]
    fn xr_locomotion_snap_pivots_at_head_and_latches() {
        let mut l = setup();
        let head_transform = Mat4::from_translation(Vec3::new(1., 1.6, 0.5));
        let h = Some(head_transform);
        assert!(l.step(h, ray(), [0., 0., 1.], false, false, 0.01));
        let origin = l.origin();
        assert!(
            l.world(head_transform)
                .w_axis
                .truncate()
                .distance(head_transform.w_axis.truncate())
                < 1e-5
        );
        l.step(h, ray(), [0., 0., 1.], false, false, 0.01);
        assert_eq!(origin, l.origin());
    }
    #[test]
    fn xr_locomotion_blocked_arc_and_swept_walk() {
        let mut l = setup();
        l.config
            .as_mut()
            .unwrap()
            .obstacles
            .push([-2., 2., -2., -1.5]);
        tick(&mut l, [0.; 3], false);
        tick(&mut l, [0.; 3], true);
        assert!(l.target.is_none());
        tick(&mut l, [0.; 3], false);
        for _ in 0..200 {
            tick(&mut l, [0., -1., 0.], false);
        }
        assert!(l.offset.z > -1.1);
        assert!(segment_hits(
            Vec2::new(0., 0.),
            Vec2::new(0., -10.),
            [-1., 1., -5., -4.9]
        ));
    }
    #[test]
    fn xr_locomotion_invalid_input_and_range() {
        let mut l = setup();
        l.config.as_mut().unwrap().teleport_range = 1.;
        tick(&mut l, [0.; 3], false);
        tick(&mut l, [0.; 3], true);
        assert!(l.target.is_none());
        assert!(!l.step(head(), ray(), [f32::NAN, 0., 0.], false, false, 0.01));
        assert!(l.origin().is_finite());
        l.reset();
        assert_eq!(l.origin(), Mat4::IDENTITY);
        assert_eq!(l.time_scale(), 1.);
    }
}
