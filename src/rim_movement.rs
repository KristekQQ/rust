//! Aurelia's static convex world and real-time input / scaled-time locomotion.
use glam::Vec3;
use serde::Deserialize;

const EPS: f32 = 1e-6;
const SUPPORT: f32 = 1e-4;
const RADIUS: f32 = 0.32;
const GRAVITY: f32 = 9.8;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
}
impl Bounds {
    fn min(self) -> Vec3 {
        Vec3::new(self.min_x, self.min_y, self.min_z)
    }
    fn max(self) -> Vec3 {
        Vec3::new(self.max_x, self.max_y, self.max_z)
    }
    fn points(a: Vec3, b: Vec3, pad: Vec3) -> Self {
        let lo = a.min(b) - pad;
        let hi = a.max(b) + pad;
        Self {
            min_x: lo.x,
            max_x: hi.x,
            min_y: lo.y,
            max_y: hi.y,
            min_z: lo.z,
            max_z: hi.z,
        }
    }
    fn overlaps(self, b: Self) -> bool {
        self.min().cmple(b.max()).all() && self.max().cmpge(b.min()).all()
    }
    fn union(self, b: Self) -> Self {
        Self::points(self.min().min(b.min()), self.max().max(b.max()), Vec3::ZERO)
    }
}
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Plane {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub d: f32,
}
impl Plane {
    fn normal(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}
#[derive(Clone, Debug, Deserialize)]
pub struct Shape {
    pub bounds: Bounds,
    pub planes: Vec<Plane>,
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub fraction: f32,
    pub point: Vec3,
    pub normal: Vec3,
}
#[derive(Clone, Copy, Debug)]
pub struct MoveResult {
    pub position: Vec3,
    pub grounded: bool,
    pub hit_ceiling: bool,
}
struct Node {
    bounds: Bounds,
    start: usize,
    end: usize,
    children: Option<(usize, usize)>,
}
/// Cached median-split BVH. Queries visit leaves directly, with no heap allocation.
pub struct World {
    shapes: Vec<Shape>,
    indices: Vec<usize>,
    nodes: Vec<Node>,
}
impl World {
    pub fn new(mut shapes: Vec<Shape>) -> Result<Self, String> {
        for (i, s) in shapes.iter_mut().enumerate() {
            if !s.bounds.min().is_finite()
                || !s.bounds.max().is_finite()
                || !s.bounds.min().cmple(s.bounds.max()).all()
                || s.planes.len() < 4
            {
                return Err(format!("Invalid convex shape {i}"));
            }
            for p in &mut s.planes {
                let length = p.normal().length();
                if !length.is_finite() || length < EPS || !p.d.is_finite() {
                    return Err(format!("Invalid plane in shape {i}"));
                }
                p.x /= length;
                p.y /= length;
                p.z /= length;
                p.d /= length;
                // A supplied AABB must intersect each halfspace; otherwise it
                // cannot conservatively represent this convex object.
                let n = p.normal();
                let low = Vec3::new(
                    if n.x >= 0. {
                        s.bounds.min_x
                    } else {
                        s.bounds.max_x
                    },
                    if n.y >= 0. {
                        s.bounds.min_y
                    } else {
                        s.bounds.max_y
                    },
                    if n.z >= 0. {
                        s.bounds.min_z
                    } else {
                        s.bounds.max_z
                    },
                );
                if n.dot(low) > p.d + 1e-3 {
                    return Err(format!("Inconsistent bounds in shape {i}"));
                }
            }
        }
        let mut world = Self {
            indices: (0..shapes.len()).collect(),
            nodes: Vec::new(),
            shapes,
        };
        if !world.shapes.is_empty() {
            world.build(0, world.shapes.len());
        }
        Ok(world)
    }
    fn build(&mut self, start: usize, end: usize) -> usize {
        let bounds = self.indices[start + 1..end]
            .iter()
            .fold(self.shapes[self.indices[start]].bounds, |b, &i| {
                b.union(self.shapes[i].bounds)
            });
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            start,
            end,
            children: None,
        });
        if end - start > 4 {
            let extent = bounds.max() - bounds.min();
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let shapes = &self.shapes;
            self.indices[start..end].sort_unstable_by(|&a, &b| {
                let aa = shapes[a].bounds;
                let bb = shapes[b].bounds;
                (aa.min()[axis] + aa.max()[axis]).total_cmp(&(bb.min()[axis] + bb.max()[axis]))
            });
            let mid = (start + end) / 2;
            let left = self.build(start, mid);
            let right = self.build(mid, end);
            self.nodes[id].children = Some((left, right));
        }
        id
    }
    fn visit(&self, id: usize, bounds: Bounds, f: &mut impl FnMut(&Shape)) {
        let node = &self.nodes[id];
        if !node.bounds.overlaps(bounds) {
            return;
        }
        if let Some((a, b)) = node.children {
            self.visit(a, bounds, f);
            self.visit(b, bounds, f);
        } else {
            for &i in &self.indices[node.start..node.end] {
                let shape = &self.shapes[i];
                if shape.bounds.overlaps(bounds) {
                    f(shape);
                }
            }
        }
    }
    fn query(&self, bounds: Bounds, mut f: impl FnMut(&Shape)) {
        if !self.nodes.is_empty() {
            self.visit(0, bounds, &mut f);
        }
    }
    fn sweep(
        &self,
        center: Vec3,
        delta: Vec3,
        height: f32,
        radius: f32,
        body: bool,
    ) -> Option<Hit> {
        let bounds = Bounds::points(
            center,
            center + delta,
            Vec3::new(radius + EPS, height / 2. + EPS, radius + EPS),
        );
        let mut nearest: Option<Hit> = None;
        self.query(bounds, |shape| {
            if let Some(hit) = clip(center, delta, shape, height, radius, body) {
                if nearest.is_none_or(|n| hit.fraction < n.fraction) {
                    nearest = Some(hit);
                }
            }
        });
        nearest
    }
    pub fn cast(&self, from: Vec3, to: Vec3) -> Option<Hit> {
        if !from.is_finite() || !to.is_finite() {
            return None;
        }
        self.sweep(from, to - from, 0., 0., false)
    }
    pub fn clear(&self, feet: Vec3, height: f32) -> bool {
        if !feet.is_finite()
            || !height.is_finite()
            || height <= 0.
            || feet.x <= -78.
            || feet.x >= 78.
            || feet.z <= -82.
            || feet.z >= 60.
        {
            return false;
        }
        let center = feet + Vec3::Y * height / 2.;
        let bounds = Bounds::points(center, center, Vec3::new(RADIUS, height / 2., RADIUS));
        let mut clear = true;
        self.query(bounds, |s| {
            if s.planes.iter().all(|p| {
                p.normal().dot(center)
                    < p.d + RADIUS * (p.x.abs() + p.z.abs()) + height / 2. * p.y.abs() - EPS
            }) {
                clear = false;
            }
        });
        clear
    }
    pub fn move_body(&self, feet: Vec3, delta: Vec3, height: f32) -> MoveResult {
        if !feet.is_finite() || !delta.is_finite() || !height.is_finite() || height <= 0. {
            return MoveResult {
                position: feet,
                grounded: false,
                hit_ceiling: false,
            };
        }
        let mut center = feet + Vec3::Y * height / 2.;
        let mut remaining = delta;
        let mut ceiling = false;
        for _ in 0..4 {
            if remaining.length() < EPS {
                break;
            }
            let Some(hit) = self.sweep(center, remaining, height, RADIUS, true) else {
                center += remaining;
                break;
            };
            center += remaining * hit.fraction;
            ceiling |= hit.normal.y < -0.5;
            remaining *= 1. - hit.fraction;
            if hit.normal.y > 0.5
                && remaining.y <= 0.
                && remaining.x.abs() + remaining.z.abs() < EPS
            {
                break;
            }
            let inward = remaining.dot(hit.normal);
            if inward < 0. {
                remaining -= hit.normal * inward;
            } else {
                break;
            }
        }
        let support = self
            .sweep(center, -Vec3::Y * SUPPORT, height, RADIUS, true)
            .filter(|h| h.normal.y > 0.5);
        if let Some(hit) = support {
            if delta.y <= 0. {
                center.y -= SUPPORT * hit.fraction;
            }
        }
        MoveResult {
            position: center - Vec3::Y * height / 2.,
            grounded: support.is_some(),
            hit_ceiling: ceiling,
        }
    }
}
fn clip(
    start: Vec3,
    delta: Vec3,
    shape: &Shape,
    height: f32,
    radius: f32,
    body: bool,
) -> Option<Hit> {
    let mut enter: f32 = 0.;
    let mut exit: f32 = 1.;
    let mut normal = None;
    let mut inside = true;
    let mut closest = f32::NEG_INFINITY;
    let mut closest_normal = Vec3::Y;
    for p in &shape.planes {
        let n = p.normal();
        let distance =
            n.dot(start) - p.d - radius * (p.x.abs() + p.z.abs()) - height / 2. * p.y.abs();
        let speed = n.dot(delta);
        if distance >= -EPS {
            inside = false;
        }
        if distance > closest {
            closest = distance;
            closest_normal = n;
        }
        if speed.abs() < EPS {
            if distance > EPS || (body && distance >= -EPS) {
                return None;
            }
            continue;
        }
        // At f32 precision a resting contact may lie either side of its
        // plane by less than EPS. Treat that entire band as time zero,
        // avoiding a gap between the inside and entering cases.
        let t = if distance.abs() <= EPS {
            0.
        } else {
            -distance / speed
        };
        if speed < 0. {
            if t >= enter - EPS {
                enter = enter.max(t);
                normal = Some(n);
            }
        } else {
            exit = exit.min(t);
        }
        if enter > exit + EPS {
            return None;
        }
    }
    if inside {
        return Some(Hit {
            fraction: 0.,
            point: start,
            normal: closest_normal,
        });
    }
    let normal = normal?;
    if enter < -EPS || enter > 1. + EPS || exit < 0. || (body && exit <= enter.max(0.) + EPS) {
        return None;
    }
    let fraction = enter.clamp(0., 1.);
    Some(Hit {
        fraction,
        point: start + delta * fraction,
        normal,
    })
}

pub struct Player {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub stamina: f32,
    pub aiming: bool,
    pub target: Option<Vec3>,
    pub jump_count: u32,
    pub dash_count: u32,
    velocity: Vec3,
    airborne: bool,
    regen_delay: f32,
    spent: u32,
    dash_remaining: f32,
    dash_direction: Vec3,
    dash_distance: f32,
    body_height: f32,
}
impl Player {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            yaw: 0.,
            pitch: 0.,
            stamina: 4.,
            aiming: false,
            target: None,
            jump_count: 0,
            dash_count: 0,
            velocity: Vec3::ZERO,
            airborne: false,
            regen_delay: 0.,
            spent: 0,
            dash_remaining: 0.,
            dash_direction: Vec3::ZERO,
            dash_distance: 0.,
            body_height: 1.7,
        }
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
    pub fn head(&self, height: f32) -> Vec3 {
        self.position + Vec3::Y * height
    }
    fn spend(&mut self) -> bool {
        if self.stamina < 1. - EPS {
            return false;
        }
        self.stamina = (self.stamina - 1.).max(0.);
        self.regen_delay = 2.;
        self.spent += 1;
        true
    }
    pub fn reset_input(&mut self) {
        self.aiming = false;
        self.target = None;
        self.dash_remaining = 0.;
    }
    pub fn teleport(&mut self, world: &World, target: Vec3) -> bool {
        if !world.clear(target, self.body_height) {
            return false;
        }
        self.position = target;
        self.velocity = Vec3::ZERO;
        self.airborne = false;
        self.reset_input();
        true
    }
    fn aim_target(&self, world: &World, height: f32, ray: Option<(Vec3, Vec3)>) -> Option<Vec3> {
        let (origin, direction) = ray.unwrap_or((self.head(height), self.forward()));
        if !origin.is_finite()
            || !direction.is_finite()
            || direction.length_squared() < EPS
            || origin.y <= self.position.y + 0.15
        {
            return None;
        }
        if origin.distance_squared(self.head(height)) > EPS
            && world.cast(self.head(height), origin).is_some()
        {
            return None;
        }
        let velocity = direction.normalize() * 22.;
        let mut previous = origin;
        for i in 1..=720 {
            let time = i as f32 / 120.;
            let next = origin + velocity * time - Vec3::Y * (0.5 * GRAVITY * time * time);
            if let Some(hit) = world.cast(previous, next) {
                let impact_time = (i as f32 - 1. + hit.fraction) / 120.;
                if hit.normal.y > 0.55 && velocity.y - GRAVITY * impact_time < 0. {
                    let target = hit.point
                        + Vec3::Y
                            * (RADIUS * (hit.normal.x.abs() + hit.normal.z.abs()) / hit.normal.y
                                + 1e-4);
                    if Vec3::new(target.x - self.position.x, 0., target.z - self.position.z)
                        .length()
                        <= 35.
                        && world.clear(target, height)
                    {
                        return Some(target);
                    }
                }
                return None;
            }
            previous = next;
            if next.y < -0.5 {
                break;
            }
        }
        None
    }
    fn launch(&mut self, target: Vec3) {
        let height = self.position.y.max(0.);
        let target_height = target.y.max(0.);
        let distance =
            Vec3::new(target.x - self.position.x, 0., target.z - self.position.z).length();
        let rise = (distance * 0.16).clamp(0.8, 2.2) + (target_height - height).max(0.) * 0.8;
        let apex = height.max(target_height) + rise;
        let vy = (2. * GRAVITY * (apex - height)).sqrt();
        let duration = vy / GRAVITY + (2. * (apex - target_height) / GRAVITY).sqrt();
        self.velocity = Vec3::new(
            (target.x - self.position.x) / duration,
            vy,
            (target.z - self.position.z) / duration,
        );
        self.airborne = true;
        self.dash_remaining = 0.;
        self.jump_count += 1;
    }
    /// Input and stamina use real time; movement, gravity and dash use scaled time.
    // Explicit independent controls shared by desktop and tracked VR input.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        world: &World,
        dt: f32,
        axis: [f32; 2],
        sprint: bool,
        aim: bool,
        dash: bool,
        head_height: f32,
        aim_ray: Option<(Vec3, Vec3)>,
    ) -> f32 {
        if !dt.is_finite() || dt <= 0. {
            return 0.;
        }
        let height = if head_height.is_finite() {
            head_height.clamp(0.6, 2.5)
        } else {
            1.7
        };
        let before = self.position;
        self.body_height = height;
        let was_aiming = self.aiming;
        self.aiming = aim && self.stamina >= 1. - EPS;
        if self.aiming || was_aiming {
            self.target = self.aim_target(world, height, aim_ray);
        } else {
            self.target = None;
        }
        if was_aiming && !aim {
            if let Some(target) = self.target {
                if self.spend() {
                    self.launch(target);
                }
            }
            self.target = None;
        }
        let forward = Vec3::new(-self.yaw.sin(), 0., -self.yaw.cos());
        let right = Vec3::new(-forward.z, 0., forward.x);
        let axis = if axis[0].is_finite() && axis[1].is_finite() {
            axis
        } else {
            [0., 0.]
        };
        let wish = right * axis[0] + forward * axis[1];
        if dash && self.dash_remaining <= 0. && self.spend() {
            self.dash_direction = if wish.length() > 0.2 {
                wish.normalize()
            } else {
                forward
            };
            self.dash_remaining = 0.2;
            self.dash_distance = 0.;
            self.dash_count += 1;
            self.aiming = false;
            self.target = None;
            self.velocity.x = 0.;
            self.velocity.z = 0.;
        }
        let simdt = dt.min(0.05) * if self.aiming { 0.12 } else { 1. };
        if !self.airborne && self.dash_remaining <= 0. && wish.length_squared() > EPS {
            self.position = world
                .move_body(
                    self.position,
                    wish / wish.length().max(1.) * (if sprint { 6. } else { 2.5 }) * simdt,
                    height,
                )
                .position;
        }
        if self.dash_remaining > 0. {
            let time = simdt.min(self.dash_remaining);
            let distance = time * 25.;
            let before = self.position;
            self.position = world
                .move_body(self.position, self.dash_direction * distance, height)
                .position;
            self.dash_remaining = (self.dash_remaining - time).max(0.);
            self.dash_distance += distance;
            if self.position.distance(before) < distance * 0.1 {
                self.dash_remaining = 0.;
            }
        }
        let mut remaining = simdt;
        while remaining > EPS {
            let h = remaining.min(1. / 120.);
            remaining -= h;
            let delta = Vec3::new(
                if self.airborne {
                    self.velocity.x * h
                } else {
                    0.
                },
                self.velocity.y * h - 0.5 * GRAVITY * h * h,
                if self.airborne {
                    self.velocity.z * h
                } else {
                    0.
                },
            );
            let hit = world.move_body(self.position, delta, height);
            self.position = hit.position;
            self.velocity.y -= GRAVITY * h;
            if hit.hit_ceiling && self.velocity.y > 0. {
                self.velocity.y = 0.;
            }
            if hit.grounded && self.velocity.y <= 0. {
                self.airborne = false;
                self.velocity = Vec3::ZERO;
            } else {
                self.airborne = true;
            }
        }
        if self.position.x <= -78.
            || self.position.x >= 78.
            || self.position.z <= -82.
            || self.position.z >= 60.
        {
            self.position.x = before.x;
            self.position.z = before.z;
        }
        let recovery = (dt - self.regen_delay).max(0.);
        self.regen_delay = (self.regen_delay - dt).max(0.);
        self.stamina = (self.stamina + recovery).min(4.);
        simdt
    }
    pub fn describe(&self) -> serde_json::Value {
        serde_json::json!({"position":{"x":self.position.x,"y":self.position.y,"z":self.position.z},"yaw":self.yaw,"pitch":self.pitch,"aiming":self.aiming,"target":self.target.map(|p|serde_json::json!({"x":p.x,"y":p.y,"z":p.z})),"jump":{"active":self.airborne,"count":self.jump_count,"velocity":{"x":self.velocity.x,"y":self.velocity.y,"z":self.velocity.z}},"dash":{"active":self.dash_remaining>0.,"remaining":self.dash_remaining,"distance":self.dash_distance,"count":self.dash_count},"stamina":{"value":self.stamina,"max":4,"available":(self.stamina+EPS).floor(),"regenDelay":self.regen_delay,"spent":self.spent}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn box_shape(min: Vec3, max: Vec3) -> Shape {
        Shape {
            bounds: Bounds::points(min, max, Vec3::ZERO),
            planes: vec![
                Plane {
                    x: 1.,
                    y: 0.,
                    z: 0.,
                    d: max.x,
                },
                Plane {
                    x: -1.,
                    y: 0.,
                    z: 0.,
                    d: -min.x,
                },
                Plane {
                    x: 0.,
                    y: 1.,
                    z: 0.,
                    d: max.y,
                },
                Plane {
                    x: 0.,
                    y: -1.,
                    z: 0.,
                    d: -min.y,
                },
                Plane {
                    x: 0.,
                    y: 0.,
                    z: 1.,
                    d: max.z,
                },
                Plane {
                    x: 0.,
                    y: 0.,
                    z: -1.,
                    d: -min.z,
                },
            ],
        }
    }
    fn ground() -> Shape {
        box_shape(Vec3::new(-100., -1., -100.), Vec3::new(100., 0., 100.))
    }
    #[test]
    fn wall_slide_and_thin_wall_dash() {
        let world = World::new(vec![
            ground(),
            box_shape(Vec3::new(2., 0., -10.), Vec3::new(2.05, 5., 10.)),
        ])
        .unwrap();
        let hit = world.move_body(Vec3::ZERO, Vec3::new(10., 0., 3.), 1.7);
        assert!((hit.position.x - 1.68).abs() < 1e-4);
        assert!((hit.position.z - 3.).abs() < 1e-4);
        let mut p = Player::new(Vec3::ZERO);
        p.step(&world, 0.05, [1., 0.], false, false, true, 1.7, None);
        for _ in 0..8 {
            p.step(&world, 0.05, [0., 0.], false, false, false, 1.7, None);
        }
        assert!(p.position.x <= 1.681 && p.position.x > 1.67);
        assert_eq!(p.dash_count, 1);
    }
    #[test]
    fn roof_support_and_open_space_below() {
        let roof = Shape {
            bounds: Bounds::points(Vec3::new(-2., 3., -2.), Vec3::new(2., 5., 2.), Vec3::ZERO),
            planes: vec![
                Plane {
                    x: 1.,
                    y: 1.,
                    z: 0.,
                    d: 5.,
                },
                Plane {
                    x: -1.,
                    y: 1.,
                    z: 0.,
                    d: 5.,
                },
                Plane {
                    x: 0.,
                    y: -1.,
                    z: 0.,
                    d: -3.,
                },
                Plane {
                    x: 0.,
                    y: 0.,
                    z: 1.,
                    d: 2.,
                },
                Plane {
                    x: 0.,
                    y: 0.,
                    z: -1.,
                    d: 2.,
                },
            ],
        };
        let world = World::new(vec![ground(), roof]).unwrap();
        assert!(world.clear(Vec3::ZERO, 1.7));
        let hit = world.move_body(Vec3::new(0.8, 8., 0.), Vec3::new(0., -5., 0.), 1.7);
        assert!(hit.grounded);
        assert!((hit.position.y - 4.52).abs() < 1e-3);
        let mut position = hit.position;
        for _ in 0..120 {
            let hit = world.move_body(position, Vec3::new(0., -0.01, 0.), 1.7);
            assert!(hit.grounded, "roof at {position:?} -> {hit:?}");
            assert!((hit.position.x - 0.8).abs() < 1e-4);
            position = hit.position;
        }
    }
    #[test]
    fn aim_release_ballistic_landing_and_stamina_clock() {
        let world = World::new(vec![ground()]).unwrap();
        let mut p = Player::new(Vec3::ZERO);
        assert!(
            (p.step(&world, 0.05, [0., 0.], false, true, false, 1.7, None) - 0.006).abs() < EPS
        );
        let target = p.target.expect("forward arc lands on ground");
        assert!(target.z < -10. && target.z > -35.);
        p.step(&world, 0.05, [0., 0.], false, false, false, 1.7, None);
        assert_eq!(p.jump_count, 1);
        assert_eq!(p.stamina, 3.);
        for _ in 0..100 {
            p.step(&world, 0.02, [0., 0.], false, false, false, 1.7, None);
        }
        assert!(
            (p.position.z - target.z).abs() < 0.1,
            "land {:?}, target {:?}, velocity {:?}",
            p.position,
            target,
            p.velocity
        );
        assert!(p.position.y.abs() < 0.001);
        p.step(&world, 0.5, [0., 0.], false, true, false, 1.7, None);
        assert!((p.stamina - 3.55).abs() < 0.02);
    }
    #[test]
    fn dash_five_metres_and_ceiling() {
        let world = World::new(vec![ground()]).unwrap();
        let mut p = Player::new(Vec3::ZERO);
        for i in 0..4 {
            p.step(&world, 0.05, [0., 0.], false, false, i == 0, 1.7, None);
        }
        assert!((p.position.z + 5.).abs() < 1e-4);
        let ceiling = World::new(vec![box_shape(
            Vec3::new(-5., 2., -5.),
            Vec3::new(5., 3., 5.),
        )])
        .unwrap();
        let hit = ceiling.move_body(Vec3::ZERO, Vec3::Y * 10., 1.7);
        assert!(hit.hit_ceiling);
        assert!((hit.position.y - 0.3).abs() < 1e-4);
    }
    #[test]
    fn arch_passage_stays_open_and_bvh_finds_crown() {
        let mut shapes = vec![ground()];
        for i in 0..16 {
            let a = (i as f32 + 0.5) * std::f32::consts::PI / 16.;
            let center = Vec3::new(3. * a.cos(), 3. * a.sin() + 1., 0.);
            let axes = [
                Vec3::new(a.cos(), a.sin(), 0.),
                Vec3::new(-a.sin(), a.cos(), 0.),
                Vec3::Z,
            ];
            let half = std::f32::consts::PI / 32.;
            let radial = 0.2 + 3. * (1. - half.cos());
            let sizes = [
                radial,
                ((3. + radial) * half.tan()).max(3. * half.sin() + 0.2) + 1e-5,
                0.2,
            ];
            let mut lo = Vec3::splat(f32::INFINITY);
            let mut hi = Vec3::splat(f32::NEG_INFINITY);
            for x in [-1., 1.] {
                for y in [-1., 1.] {
                    for z in [-1., 1.] {
                        let p = center
                            + axes[0] * sizes[0] * x
                            + axes[1] * sizes[1] * y
                            + axes[2] * sizes[2] * z;
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                }
            }
            let mut planes = Vec::new();
            for j in 0..3 {
                for sign in [-1., 1.] {
                    let n = axes[j] * sign;
                    planes.push(Plane {
                        x: n.x,
                        y: n.y,
                        z: n.z,
                        d: n.dot(center) + sizes[j],
                    });
                }
            }
            shapes.push(Shape {
                bounds: Bounds::points(lo, hi, Vec3::ZERO),
                planes,
            });
        }
        let world = World::new(shapes).unwrap();
        assert!(world
            .cast(Vec3::new(0., 1.5, 2.), Vec3::new(0., 1.5, -2.))
            .is_none());
        let crossing = world.move_body(Vec3::new(0., 0., 2.), Vec3::new(0., 0., -4.), 1.7);
        assert!((crossing.position.z + 2.).abs() < 1e-4);
        let crown = world
            .cast(Vec3::new(0., 8., 0.), Vec3::new(0., 0., 0.))
            .unwrap();
        assert!(crown.point.y > 4. && crown.normal.y > 0.9);
    }
    #[test]
    fn stamina_recovery_uses_real_time_through_slow_aim() {
        let world = World::new(vec![ground()]).unwrap();
        let mut p = Player::new(Vec3::ZERO);
        p.step(&world, 0.05, [0., 0.], false, false, true, 1.7, None);
        for _ in 0..40 {
            assert!(
                (p.step(&world, 0.05, [0., 0.], false, true, false, 1.7, None) - 0.006).abs() < EPS
            );
        }
        assert!((p.stamina - 3.05).abs() < 0.001);
        for _ in 0..10 {
            p.step(&world, 0.05, [0., 0.], false, true, false, 1.7, None);
        }
        assert!((p.stamina - 3.55).abs() < 0.001);
    }
    #[test]
    fn original_city_spawn_walks_forward_and_keeps_support() {
        let collision: serde_json::Value =
            serde_json::from_str(include_str!("../assets/models/rim/collisions.json")).unwrap();
        let shapes = serde_json::from_value(collision["shapes"].clone()).unwrap();
        let world = World::new(shapes).unwrap();
        let mut player = Player::new(Vec3::new(0., 0., 42.));
        for frame in 0..30 {
            player.step(&world, 1. / 60., [0., 1.], false, false, false, 1.7, None);
            assert!(!player.airborne, "frame {frame}: {}", player.describe());
            assert!(
                world.move_body(player.position, Vec3::ZERO, 1.7).grounded,
                "frame {frame}: {}",
                player.describe()
            );
        }
        assert!(player.position.z < 41.5, "{}", player.describe());
        assert!(player.position.x.abs() < 0.001 && player.position.y.abs() < 0.001);
    }
    #[test]
    fn invalid_shapes_are_rejected() {
        let mut shape = ground();
        shape.planes[0].x = f32::NAN;
        assert!(World::new(vec![shape]).is_err());
        let mut shape = ground();
        shape.bounds.max_x = f32::INFINITY;
        assert!(World::new(vec![shape]).is_err());
        let mut shape = ground();
        shape.planes.clear();
        assert!(World::new(vec![shape]).is_err());
    }
}
