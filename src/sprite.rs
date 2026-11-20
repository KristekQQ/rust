//! Authoritative decorative atlas playback; mirrors receive presentation only.
use crate::scene::SceneManager;
use glam::{Vec2, Vec3};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteConfig {
    pub key: u32,
    pub columns: u32,
    pub rows: u32,
    pub frames: u32,
    pub fps: f32,
    pub duration: f32,
    pub ambient_interval: f32,
    /// Optional first ambient wait; later visits use ambient_interval.
    #[serde(default)]
    pub ambient_delay: Option<f32>,
    #[serde(default)]
    pub flock_jitter: Option<f32>,
    pub cooldown: f32,
    pub travel: [f32; 3],
    /// Optional quadratic Bézier control point offset from the visit origin.
    #[serde(default)]
    pub travel_control: Option<[f32; 3]>,
    /// Optional bounded world-axis drift over normalized travel progress.
    #[serde(default)]
    pub travel_sway: Option<SpriteTravelSway>,
    /// Optional blade target; decorative sprites remain noninteractive.
    #[serde(default)]
    pub sword_target: Option<SpriteSwordTarget>,
    /// Optional authoritative pooled floor deposits; interval is scene-wide, in seconds.
    #[serde(default)]
    pub ground_drop: Option<SpriteGroundDrop>,
    #[serde(default)]
    pub cut_effect_key: Option<u32>,
    /// Traverse once at constant path progress, then hide; excludes visit/petting pauses.
    #[serde(default)]
    pub one_way: bool,
    /// Mirror a right-facing atlas with world-X travel.
    #[serde(default)]
    pub flip_with_travel: bool,
    /// Seconds held at the destination, included in duration.
    #[serde(default)]
    pub endpoint_pause: f32,
    /// Initial visible hold at the origin, included in duration.
    #[serde(default)]
    pub start_pause: f32,
    /// Reach the endpoint and show its calm artwork this many seconds earlier.
    /// The return start and total visit duration remain unchanged.
    #[serde(default)]
    pub arrival_pose_lead: f32,
    /// First atlas frame for the optional endpoint-pause animation.
    #[serde(default)]
    pub idle_start_frame: u32,
    /// Endpoint-pause frame count; zero preserves the held walking pose.
    #[serde(default)]
    pub idle_frames: u32,
    /// Optional endpoint-pause speed; absent uses the walking fps.
    #[serde(default)]
    pub idle_fps: Option<f32>,
    /// Allow tracked grip contact to hold the endpoint and play a petting pose.
    #[serde(default)]
    pub pettable: bool,
    #[serde(default)]
    pub combo: bool,
    /// Optional second atlas band, selected locally by viewer elevation.
    #[serde(default)]
    pub elevated_view: Option<SpriteElevatedView>,
    #[serde(default)]
    pub directional_view: Option<SpriteDirectionalView>,
    #[serde(default)]
    pub atlas_bands: Vec<SpriteAtlasBand>,
}
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteGroundDrop {
    pub interval: f32,
    pub floor_y: f32,
}
/// Radius is in model units; reach_height is relative to tracked head height.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteSwordTarget {
    pub points: u32,
    pub radius: f32,
    #[serde(default)]
    pub reach_height: Option<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SpriteCutHit {
    pub original: u32,
    pub points: u32,
}
fn segment_distance_squared(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0., 1.);
    p.distance_squared(a + ab * t)
}
/// Closest distance to the finite blade sweep, including degenerate stationary edges.
fn sweep_triangle_distance_squared(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> f32 {
    let n = (b - a).cross(c - a);
    let n2 = n.length_squared();
    if n2 > 1e-12 {
        let projection = p - n * ((p - a).dot(n) / n2);
        if [(a, b), (b, c), (c, a)]
            .iter()
            .all(|&(u, v)| (v - u).cross(projection - u).dot(n) >= -1e-9)
        {
            return p.distance_squared(projection);
        }
    }
    segment_distance_squared(p, a, b)
        .min(segment_distance_squared(p, b, c))
        .min(segment_distance_squared(p, c, a))
}
/// Deterministic path variation; phase is in radians and amplitude is in world units.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteTravelSway {
    pub amplitude: [f32; 3],
    pub cycles: f32,
    pub phase: f32,
}
impl SpriteTravelSway {
    fn valid(self) -> bool {
        let amplitude = Vec3::from_array(self.amplitude);
        amplitude.is_finite()
            && self.cycles.is_finite()
            && (0. ..=64.).contains(&self.cycles)
            && self.phase.is_finite()
            && (amplitude * (std::f32::consts::TAU * (2. * self.cycles + 1.))).is_finite()
    }
    fn sample(self, t: f32) -> (Vec3, Vec3) {
        // The zero-value/zero-slope envelope preserves both endpoint positions and headings.
        if t <= 0. || t >= 1. {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let frequency = std::f32::consts::TAU * self.cycles;
        let angle = frequency * t + self.phase.rem_euclid(std::f32::consts::TAU);
        let (wave, wave_cos) = angle.sin_cos();
        let (harmonic, harmonic_cos) = (2. * angle + 0.7).sin_cos();
        let wave = 0.8 * wave + 0.2 * harmonic;
        let wave_derivative = frequency * (0.8 * wave_cos + 0.4 * harmonic_cos);
        let (envelope_sin, envelope_cos) = (std::f32::consts::PI * t).sin_cos();
        let envelope = envelope_sin * envelope_sin;
        let envelope_derivative = std::f32::consts::TAU * envelope_sin * envelope_cos;
        let amplitude = Vec3::from_array(self.amplitude);
        (
            amplitude * (envelope * wave),
            amplitude * (envelope_derivative * wave + envelope * wave_derivative),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteElevatedView {
    pub start_frame: u32,
    pub elevation: f32,
    pub max_front_pitch: f32,
    #[serde(default = "default_elevated_pitch")]
    pub pitch: f32,
}
fn default_elevated_pitch() -> f32 {
    65.
}
/// Artwork top follows flight projected onto the selected underside plane.
fn underside_right(facing: Vec3, heading: Vec3) -> Vec3 {
    let up = (heading - facing * heading.dot(facing)).normalize();
    up.cross(facing).normalize()
}
impl SpriteElevatedView {
    pub(crate) fn valid(self, cells: u32, last_frame: u32) -> bool {
        self.start_frame > last_frame
            && self
                .start_frame
                .checked_add(last_frame)
                .is_some_and(|end| end < cells)
            && self.elevation.is_finite()
            && self.elevation.abs() > 0.
            && self.elevation.abs() < 90.
            && self.max_front_pitch.is_finite()
            && (0. ..=self.elevation.abs()).contains(&self.max_front_pitch)
            && self.pitch.is_finite()
            && (self.elevation.abs()..=90.).contains(&self.pitch.abs())
            && self.pitch.signum() == self.elevation.signum()
    }
    /// Signed viewer elevation: negative thresholds select the underside.
    fn selected(self, elevation: f32) -> bool {
        if self.elevation < 0. {
            elevation <= self.elevation.to_radians()
        } else {
            elevation >= self.elevation.to_radians()
        }
    }
    fn viewer_pitch(self, elevation: f32) -> f32 {
        if self.selected(elevation) {
            if self.elevation < 0. {
                elevation.min(self.pitch.to_radians())
            } else {
                self.pitch.to_radians()
            }
        } else {
            elevation.clamp(
                -self.max_front_pitch.to_radians(),
                self.max_front_pitch.to_radians(),
            )
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAtlasBand {
    pub first_frame: u32,
    pub frame_count: u32,
    pub columns: u32,
    pub origin: [f32; 2],
    pub tile_size: [f32; 2],
    #[serde(default)]
    pub first_tile: u32,
    #[serde(default)]
    pub repeat: Option<u32>,
    #[serde(default)]
    pub tile_map: Option<Vec<u32>>,
}
#[repr(C)]
#[derive(
    Clone, Copy, Debug, Default, PartialEq, serde::Serialize, bytemuck::Pod, bytemuck::Zeroable,
)]
pub struct SpriteAtlasBandUniform {
    pub frames: [u32; 4], // First semantic frame, count, repetition period, first tile.
    pub rect: [f32; 4],   // UV origin and one tile's UV size.
    pub tiles: [u32; 4],  // Columns, pattern length, packed tile indices, optional fragment clip.
}
impl SpriteAtlasBandUniform {
    pub(crate) fn valid(self, cells: u32) -> bool {
        let [first, count, repeat, tile] = self.frames;
        let [columns, pattern_len, pattern, padding] = self.tiles;
        if count == 0
            || repeat == 0
            || repeat > cells
            || columns == 0
            || columns > 256
            || first.checked_add(count).is_none_or(|end| end > cells)
            || pattern_len > 8
            || (pattern_len > 0 && pattern_len != repeat)
            || (padding != 0
                && padding != 0x0040_0000
                && (padding & 0xfe00_0000 != 0xba00_0000
                    || count != 1
                    || repeat != 1
                    || columns != 1
                    || pattern_len != 0
                    || tile != 0))
            || self.rect.iter().any(|v| !v.is_finite())
            || self.rect[0] < 0.
            || self.rect[1] < 0.
            || self.rect[2] <= 0.
            || self.rect[3] <= 0.
        {
            return false;
        }
        let max_offset = if pattern_len > 0 {
            (0..pattern_len)
                .map(|i| (pattern >> (i * 4)) & 15)
                .max()
                .unwrap()
        } else {
            repeat - 1
        };
        let Some(last) = tile.checked_add(max_offset) else {
            return false;
        };
        let max_column = if tile / columns == last / columns {
            last % columns
        } else {
            columns - 1
        };
        self.rect[0] + (max_column + 1) as f32 * self.rect[2] <= 1.000001
            && self.rect[1] + ((last / columns) as f32 + 1.) * self.rect[3] <= 1.000001
    }
    fn uv_rect(self, frame: u32) -> [f32; 4] {
        let index = (frame - self.frames[0]) % self.frames[2];
        let offset = if self.tiles[1] > 0 {
            (self.tiles[2] >> (index * 4)) & 15
        } else {
            index
        };
        let tile = self.frames[3] + offset;
        [
            self.rect[0] + (tile % self.tiles[0]) as f32 * self.rect[2],
            self.rect[1] + (tile / self.tiles[0]) as f32 * self.rect[3],
            self.rect[2],
            self.rect[3],
        ]
    }
}
// Wire-compatible fragment metadata in the formerly zero atlas padding word.
// 7-bit tag, side, 12-bit signed line offset and 12-bit normal angle. At a
// 192px frame this has <0.2px normal/offset quantization error, including diagonals.
fn fragment_clip(normal: Vec2, offset: f32, positive: bool) -> u32 {
    let angle = normal.y.atan2(normal.x).rem_euclid(std::f32::consts::TAU);
    let angle = ((angle / std::f32::consts::TAU * 4096.).round() as u32) & 4095;
    let offset = ((offset.clamp(-1., 1.) * 2047.).round() as i32 + 2048) as u32;
    0xba00_0000 | ((positive as u32) << 24) | (offset << 12) | angle
}
/// Project the actual swept blade trajectory into the artwork's world plane.
/// Use the blade axis when translation vanishes, then a finite horizontal cut.
fn sprite_cut_line(
    previous: [Vec3; 2],
    current: [Vec3; 2],
    center: Vec3,
    right: Vec3,
    up: Vec3,
    scale: Vec3,
) -> (Vec2, f32, Vec3) {
    let blade = ((previous[1] - previous[0]) + (current[1] - current[0])) * 0.5;
    let base = (previous[0] + current[0]) * 0.5;
    let fraction = if blade.length_squared() > 1e-10 {
        ((center - base).dot(blade) / blade.length_squared()).clamp(0., 1.)
    } else {
        0.5
    };
    let from = previous[0].lerp(previous[1], fraction);
    let to = current[0].lerp(current[1], fraction);
    let project = |p: Vec3| Vec2::new(p.dot(right), p.dot(up));
    let mut stroke = project(to - from);
    if stroke.length_squared() < 1e-10 {
        stroke = project(blade);
    }
    if stroke.length_squared() < 1e-10 {
        stroke = Vec2::X;
    }
    let stroke = stroke.normalize();
    let normal = Vec2::new(-stroke.y, stroke.x);
    let separation = right * normal.x + up * normal.y;
    let size = Vec2::new(scale.x.abs().max(1e-6), scale.y.abs().max(1e-6));
    // glTF UV Y increases downward, whereas the billboard's local Y points up.
    let uv_normal = Vec2::new(normal.x * size.x, -normal.y * size.y).normalize();
    let hit = project(from - center) / size * Vec2::new(1., -1.);
    (uv_normal, uv_normal.dot(hit), separation)
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct SpriteAtlasLayout {
    pub count: u32,
    pub bands: [SpriteAtlasBandUniform; 8],
}
impl SpriteAtlasLayout {
    fn pack(bands: &[SpriteAtlasBand], cells: u32) -> Option<Self> {
        if bands.is_empty() || bands.len() > 8 {
            return None;
        }
        let mut result = Self {
            count: bands.len() as u32,
            bands: [SpriteAtlasBandUniform::default(); 8],
        };
        for (source, target) in bands.iter().zip(result.bands.iter_mut()) {
            let map = source.tile_map.as_deref().unwrap_or(&[]);
            if source.tile_map.is_some()
                && (map.is_empty() || map.len() > 8 || map.iter().any(|&tile| tile > 15))
            {
                return None;
            }
            let repeat = source.repeat.unwrap_or(if map.is_empty() {
                source.frame_count
            } else {
                map.len() as u32
            });
            *target = SpriteAtlasBandUniform {
                frames: [
                    source.first_frame,
                    source.frame_count,
                    repeat,
                    source.first_tile,
                ],
                rect: [
                    source.origin[0],
                    source.origin[1],
                    source.tile_size[0],
                    source.tile_size[1],
                ],
                tiles: [
                    source.columns,
                    map.len() as u32,
                    map.iter()
                        .enumerate()
                        .fold(0, |bits, (i, &tile)| bits | (tile << (i * 4))),
                    0,
                ],
            };
        }
        result.valid(cells).then_some(result)
    }
    pub(crate) fn valid(self, cells: u32) -> bool {
        self.count > 0
            && self.count <= 8
            && self.bands[..self.count as usize]
                .iter()
                .all(|&band| band.valid(cells))
            && self.bands[..self.count as usize]
                .iter()
                .enumerate()
                .all(|(i, a)| {
                    self.bands[i + 1..self.count as usize].iter().all(|b| {
                        a.frames[0] + a.frames[1] <= b.frames[0]
                            || b.frames[0] + b.frames[1] <= a.frames[0]
                    })
                })
    }
}
/// Five right-facing artwork bands cover eight viewer-relative walking directions.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteDirectionalView {
    pub frames: u32,
    pub front_start_frame: u32,
    pub front_oblique_start_frame: u32,
    pub side_start_frame: u32,
    pub rear_oblique_start_frame: u32,
    pub rear_start_frame: u32,
}
impl SpriteDirectionalView {
    pub(crate) fn starts(self) -> [u32; 5] {
        [
            self.front_start_frame,
            self.front_oblique_start_frame,
            self.side_start_frame,
            self.rear_oblique_start_frame,
            self.rear_start_frame,
        ]
    }
    pub(crate) fn valid(self, cells: u32) -> bool {
        let starts = self.starts();
        self.frames > 0
            && starts.iter().all(|start| {
                start
                    .checked_add(self.frames)
                    .is_some_and(|end| end <= cells)
            })
            && starts.iter().enumerate().all(|(i, &a)| {
                starts[i + 1..]
                    .iter()
                    .all(|&b| a.abs_diff(b) >= self.frames)
            })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct SpriteVisual {
    pub key: u32,
    pub columns: u32,
    pub rows: u32,
    pub frame: u32,
    pub visible: bool,
    pub elevated_view: Option<SpriteElevatedView>,
    pub directional_view: Option<SpriteDirectionalView>,
    pub atlas_layout: Option<SpriteAtlasLayout>,
}
#[cfg(any(target_arch = "wasm32", feature = "native-render", test))]
impl SpriteVisual {
    pub(crate) fn atlas_uniform(self) -> [SpriteAtlasBandUniform; 8] {
        self.atlas_layout
            .map_or([SpriteAtlasBandUniform::default(); 8], |layout| {
                layout.bands
            })
    }
    pub(crate) fn direction_uniform(self) -> [[f32; 4]; 2] {
        self.directional_view.map_or([[0.; 4]; 2], |v| {
            [
                [
                    v.frames as f32,
                    v.front_start_frame as f32,
                    v.front_oblique_start_frame as f32,
                    v.side_start_frame as f32,
                ],
                [
                    v.rear_oblique_start_frame as f32,
                    v.rear_start_frame as f32,
                    0.,
                    0.,
                ],
            ]
        })
    }
    pub(crate) fn view_uniform(self) -> [f32; 4] {
        self.elevated_view.map_or([0.; 4], |v| {
            [
                v.start_frame as f32,
                v.elevation.to_radians(),
                v.max_front_pitch.to_radians(),
                v.pitch.to_radians(),
            ]
        })
    }
    pub(crate) fn uniform(self) -> [f32; 4] {
        [
            self.columns as f32,
            self.rows as f32,
            self.frame as f32,
            if self.visible { 1. } else { -1. },
        ]
    }
}
pub(crate) struct SpriteController {
    config: SpriteConfig,
    origin: Vec3,
    base_origin: Vec3,
    target_player: Option<Vec3>,
    target_scoring: bool,
    fragment: bool,
    origin_scale_x: f32,
    origin_rotation: Vec3,
    elapsed: Option<f32>,
    idle: f32,
    since_trigger: f32,
    waiting_for_flock: bool,
    flock_drop: Option<FlockDrop>,
    pet_hold_remaining: f32,
    pet_visit: Option<PetVisit>,
    visit_id: u32,
    gift_enabled: bool,
    gift_contact_seconds: f32,
    gift_contact_sample: bool,
    fetch_pending: bool,
    gift_visit: bool,
    gift_earned: bool,
    gift_viewer_y: Option<f32>,
}
enum FlockDrop {
    Floor(u32),
    Overhead(u32),
}

pub(super) struct GroundDropCandidate {
    pub(super) emitter: u32,
    pub(super) position: Vec3,
    pub(super) floor: f32,
    pub(super) interval: f32,
    pub(super) aim: Option<Vec3>,
}

impl SpriteController {
    fn begin_visit(&mut self) {
        self.visit_id = self.visit_id.wrapping_add(1);
        if self.config.flock_jitter.is_some() && self.config.ground_drop.is_some() {
            // Draw once for this bird's visit. Keep the same choice while it is in flight.
            let seed =
                self.config.key.wrapping_mul(0x9e37_79b9) ^ self.visit_id.wrapping_mul(0x85eb_ca6b);
            let hash = (seed ^ (seed >> 16)).wrapping_mul(0x7feb_352d);
            let rank = hash ^ (hash >> 15);
            self.flock_drop = match rank >> 29 {
                4 | 5 => Some(FlockDrop::Overhead(rank)),
                6 => Some(FlockDrop::Floor(rank)),
                _ => None,
            };
        }
    }
}

const DRINK_PET_SECONDS: f32 = 1.6;
struct PetVisit {
    contact_seconds: f32,
    contact_sample: bool,
    after_gift: bool,
    elapsed: f32,
    farewell: Option<f32>,
    initial_head_y: f32,
    current_head_y: Option<f32>,
    hands_tracked: bool,
    contact_hand: Option<usize>,
    without_contact: f32,
}
impl SpriteConfig {
    /// Position offset and full forward-path tangent at the existing eased progress.
    fn travel_sample(&self, t: f32) -> (Vec3, Vec3) {
        let travel = Vec3::from_array(self.travel);
        let (position, tangent) = if let Some(control) = self.travel_control {
            let control = Vec3::from_array(control);
            (
                control * (2. * (1. - t) * t) + travel * (t * t),
                2. * ((1. - t) * control + t * (travel - control)),
            )
        } else {
            (travel * t, travel)
        };
        if let Some(sway) = self.travel_sway {
            let (offset, derivative) = sway.sample(t);
            (position + offset, tangent + derivative)
        } else {
            (position, tangent)
        }
    }
    fn valid(&self) -> bool {
        self.key > 0
            && self.columns > 0
            && self.rows > 0
            && self.columns <= 256
            && self.rows <= 256
            && self.frames > 0
            && self.frames <= self.columns * self.rows
            && (self.idle_frames > 0 || self.idle_start_frame == 0)
            && self
                .idle_start_frame
                .checked_add(self.idle_frames)
                .is_some_and(|end| end <= self.columns * self.rows)
            && self.idle_fps.is_none_or(|fps| fps.is_finite() && fps > 0.)
            && self.elevated_view.is_none_or(|view| {
                view.valid(
                    self.columns * self.rows,
                    (self.frames - 1).max(
                        self.idle_start_frame
                            .saturating_add(self.idle_frames)
                            .saturating_sub(1),
                    ),
                )
            })
            && self.directional_view.is_none_or(|view| {
                view.valid(self.columns * self.rows)
                    && view.frames == self.frames
                    && (self.idle_frames == 0 || self.idle_start_frame >= view.frames)
            })
            && (self.atlas_bands.is_empty()
                || SpriteAtlasLayout::pack(&self.atlas_bands, self.columns * self.rows).is_some())
            && self.fps.is_finite()
            && self.fps > 0.
            && self.fps <= 240.
            && self.duration.is_finite()
            && self.duration > 0.
            && self.ambient_interval.is_finite()
            && self.ambient_interval >= 0.
            && self.flock_jitter.is_none_or(|j| {
                self.one_way && self.ambient_interval > 0. && j.is_finite() && j >= 0.
            })
            && self
                .ambient_delay
                .is_none_or(|delay| delay.is_finite() && delay >= 0.)
            && (!self.one_way
                || (self.start_pause == 0.
                    && self.endpoint_pause == 0.
                    && self.arrival_pose_lead == 0.
                    && !self.pettable
                    && self.idle_frames == 0))
            && self.ground_drop.is_none_or(|drop| {
                self.one_way
                    && drop.interval.is_finite()
                    && drop.interval >= 0.5
                    && drop.floor_y.is_finite()
            })
            && self.cooldown.is_finite()
            && self.cooldown >= 0.
            && Vec3::from_array(self.travel).is_finite()
            && self.travel_sway.is_none_or(SpriteTravelSway::valid)
            && self.sword_target.is_none_or(|target| {
                target.points > 0
                    && target.radius.is_finite()
                    && target.radius > 0.
                    && target.radius <= 2.
                    && target
                        .reach_height
                        .is_none_or(|h| h.is_finite() && (-1.5..=0.6).contains(&h))
                    && self.one_way
                    && !self.pettable
                    && self.key < 1000
            })
            && self
                .travel_control
                .is_none_or(|control| Vec3::from_array(control).is_finite())
            && self.endpoint_pause.is_finite()
            && self.endpoint_pause >= 0.
            && self.start_pause.is_finite()
            && self.start_pause >= 0.
            && self.start_pause + self.endpoint_pause < self.duration
            && self.arrival_pose_lead.is_finite()
            && self.arrival_pose_lead >= 0.
            && self.arrival_pose_lead
                < (self.duration - self.start_pause - self.endpoint_pause) * 0.5
    }
}
impl SceneManager {
    /// Snapshot player position for the NEXT visit, never chase the head mid-flight.
    pub fn set_sprite_target_player(&mut self, head: Option<Vec3>, scoring: bool) {
        if self.is_render_mirror() {
            return;
        }
        let head = head.filter(|h| h.is_finite());
        // Floor-relative flight reference survives crouches and temporary tracking loss.
        if let Some(head) = head {
            self.flock_standing_eye_y = self.flock_standing_eye_y.max(head.y.clamp(1.65, 2.2));
        }
        // Ambient flyovers can hit a tracked player even between rounds.
        self.sprite_deposits.target_head = head;
        if self.sprite_deposits.target_head.is_none() {
            self.visor_splat_remaining = 0.;
        }
        for sprite in self
            .sprites
            .values_mut()
            .filter(|s| s.config.sword_target.is_some() || s.config.ground_drop.is_some())
        {
            sprite.target_player = head;
            sprite.target_scoring = scoring && head.is_some();
        }
    }
    pub fn cut_sprite_sweep(
        &mut self,
        previous: [Vec3; 2],
        current: [Vec3; 2],
    ) -> Vec<SpriteCutHit> {
        if self.is_render_mirror() || previous.into_iter().chain(current).any(|p| !p.is_finite()) {
            return Vec::new();
        }
        let hits: Vec<_> = self
            .sprites
            .iter()
            .filter_map(|(&id, s)| {
                let target = s.config.sword_target?;
                if s.target_player.is_none() || s.elapsed.is_none() {
                    return None;
                }
                let o = self.objects.get(&id)?;
                if !o.sprite.is_some_and(|v| v.visible) {
                    return None;
                }
                let radius = target.radius * o.transform.scale.abs().max_element() + 0.025;
                let p = o.transform.position;
                let distance =
                    sweep_triangle_distance_squared(p, previous[0], previous[1], current[1]).min(
                        sweep_triangle_distance_squared(p, previous[0], current[1], current[0]),
                    );
                (distance <= radius * radius).then_some(SpriteCutHit {
                    original: id,
                    points: if s.target_scoring { target.points } else { 0 },
                })
            })
            .collect();
        for hit in &hits {
            self.split_sprite_target(hit.original, previous, current);
            let sprite = self.sprites.get_mut(&hit.original).unwrap();
            sprite.elapsed = None;
            sprite.waiting_for_flock = sprite.config.flock_jitter.is_some();
            sprite.idle = 0.;
            if let Some(o) = self.objects.get_mut(&hit.original) {
                o.sprite.as_mut().unwrap().visible = false;
            }
            self.changed(hit.original);
        }
        hits
    }
    /// Two tiny, short-lived cards reuse the original texture. No new image/mesh payload.
    fn split_sprite_target(&mut self, id: u32, previous: [Vec3; 2], current: [Vec3; 2]) {
        let Some(model) = self.models.get(&id) else {
            return;
        };
        let asset = model.asset.clone();
        let object = &self.objects[&id];
        let transform = object.transform;
        let visual = object.sprite.unwrap();
        let source = self.sprites[&id].config.clone();
        let viewer = self.sprites[&id]
            .target_player
            .unwrap_or(transform.position + Vec3::Z);
        let toward = (viewer - transform.position).with_y(0.).normalize_or_zero();
        let right = Vec3::Y.cross(toward).normalize_or_zero();
        let mut right = if right.length_squared() > 0.5 {
            right
        } else {
            Vec3::X
        };
        // Freeze the actual hit-facing artwork. The two fragments must not
        // independently swap directional/mirrored bands while separating.
        let heading = Vec3::new(transform.rotation.y.sin(), 0., transform.rotation.y.cos());
        let facing = (viewer - transform.position).normalize_or_zero();
        let below = source
            .elevated_view
            .filter(|v| v.selected(facing.y.clamp(-1., 1.).asin()));
        let (start, mirrored) = if let Some(view) = below {
            (view.start_frame, false)
        } else if let Some(v) = source.directional_view {
            let azimuth = toward
                .dot(Vec3::new(heading.z, 0., -heading.x))
                .atan2(toward.dot(heading));
            let sector = (azimuth.abs() / std::f32::consts::FRAC_PI_4 + 0.5).floor() as usize;
            (
                [
                    v.front_start_frame,
                    v.front_oblique_start_frame,
                    v.side_start_frame,
                    v.rear_oblique_start_frame,
                    v.rear_start_frame,
                ][sector.min(4)],
                azimuth > 0. && sector > 0 && sector < 4,
            )
        } else {
            (0, transform.scale.x < 0.)
        };
        let elevation = facing.y.clamp(-1., 1.).asin();
        let pitch = if let Some(view) = below {
            elevation.min(view.pitch.to_radians())
        } else if source.directional_view.is_some() {
            elevation.clamp(-65f32.to_radians(), 65f32.to_radians())
        } else if let Some(view) = source.elevated_view {
            view.viewer_pitch(elevation)
        } else {
            elevation
        };
        let basis_facing = right.cross(Vec3::Y) * pitch.cos() + Vec3::Y * pitch.sin();
        if below.is_some() {
            right = underside_right(basis_facing, heading);
        }
        let up = basis_facing.cross(right);
        let (clip_normal, clip_offset, separation) = sprite_cut_line(
            previous,
            current,
            transform.position,
            right,
            up,
            transform.scale,
        );
        let frame = start + visual.frame % source.frames;
        let rect = visual
            .atlas_layout
            .and_then(|l| {
                l.bands[..l.count as usize]
                    .iter()
                    .find(|b| frame >= b.frames[0] && frame < b.frames[0] + b.frames[1])
                    .map(|b| b.uv_rect(frame))
            })
            .unwrap_or([
                (frame % source.columns) as f32 / source.columns as f32,
                (frame / source.columns) as f32 / source.rows as f32,
                1. / source.columns as f32,
                1. / source.rows as f32,
            ]);
        if let Some(template_key) = source.cut_effect_key {
            let template = self
                .sprites
                .iter()
                .find(|(_, sprite)| sprite.config.key == template_key)
                .and_then(|(id, sprite)| {
                    Some((self.models.get(id)?.asset.clone(), sprite.config.clone()))
                });
            if let Some((art, mut effect)) = template {
                effect.key = source.key + 3000;
                let lifetime = effect.duration;
                if let Ok(id) = self.add_model(art) {
                    let scale = transform.scale.abs();
                    self.set_object_position(id, transform.position - Vec3::Y * (0.18 * scale.y));
                    self.set_object_scale(id, scale * Vec3::new(1.34, 1.6, 1.));
                    if self.configure_sprite(id, effect) {
                        self.trigger_sprite(id);
                        self.schedule_remove_object(id, lifetime + 0.05);
                    } else {
                        self.remove_object(id);
                    }
                }
            }
        }
        for half in 0..2u32 {
            let mut config = source.clone();
            config.key = source.key + (half + 1) * 1000;
            config.sword_target = None;
            config.ground_drop = None;
            config.cut_effect_key = None;
            config.flock_jitter = None;
            config.columns = 1;
            config.rows = 1;
            config.frames = 1;
            config.fps = 1.;
            config.duration = 0.55;
            config.ambient_interval = 0.;
            config.ambient_delay = None;
            config.cooldown = 0.;
            config.combo = false;
            config.travel_sway = None;
            config.flip_with_travel = false;
            config.one_way = true;
            config.start_pause = 0.;
            config.endpoint_pause = 0.;
            config.arrival_pose_lead = 0.;
            config.idle_frames = 0;
            let sign = if half == 0 { -1. } else { 1. };
            config.travel = (separation * (sign * 0.85) + Vec3::new(0., -0.8, 0.)).to_array();
            config.travel_control =
                Some((separation * (sign * 0.65) + Vec3::new(0., 0.15, 0.)).to_array());
            config.directional_view = None;
            config.elevated_view = None;
            config.atlas_bands = vec![SpriteAtlasBand {
                first_frame: 0,
                frame_count: 1,
                columns: 1,
                origin: [rect[0], rect[1]],
                tile_size: [rect[2], rect[3]],
                first_tile: 0,
                repeat: None,
                tile_map: None,
            }];
            let Ok(piece) = self.add_model(asset.clone()) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&piece) {
                o.transform = transform;
                let rotation =
                    glam::Quat::from_mat3(&glam::Mat3::from_cols(right, up, basis_facing));
                let (y, x, z) = rotation.to_euler(glam::EulerRot::YXZ);
                o.transform.rotation = Vec3::new(x, y, z);
                o.transform.scale = transform.scale.abs();
                o.transform.scale.x = o.transform.scale.x.abs() * if mirrored { -1. } else { 1. };
                o.refresh_render_data();
            }
            if self.configure_sprite(piece, config) {
                self.sprites.get_mut(&piece).unwrap().fragment = true;
                self.objects
                    .get_mut(&piece)
                    .unwrap()
                    .sprite
                    .as_mut()
                    .unwrap()
                    .atlas_layout
                    .as_mut()
                    .unwrap()
                    .bands[0]
                    .tiles[3] = fragment_clip(clip_normal, clip_offset, half == 1);
                self.trigger_sprite(piece);
                self.schedule_remove_object(piece, 0.7);
            } else {
                self.remove_object(piece);
            }
        }
    }
    pub fn set_billboard_camera(&mut self, camera: Option<Vec3>) {
        let camera = camera.filter(|p| p.is_finite());
        if self.billboard_camera == camera {
            return;
        }
        self.billboard_camera = camera;
        let ids: Vec<_> = self.sprite_ids.iter().copied().collect();
        for id in ids {
            Self::queue_model_gpu_update(&mut self.models, &mut self.model_gpu_changes, id);
        }
    }
    /// Bind a preloaded immutable atlas model by semantic key before mirror frames.
    pub fn bind_sprite(&mut self, id: u32, key: u32) -> bool {
        let Some(model) = self.model(id) else {
            return false;
        };
        if key == 0 {
            return false;
        }
        let asset = model.asset.clone();
        self.sprite_assets.insert(key, asset);
        // A preload is an asset binding only; never leave a full-atlas ghost root.
        self.remove_object(id);
        self.sync_sprite_models();
        true
    }
    pub fn configure_sprite(&mut self, id: u32, config: SpriteConfig) -> bool {
        if self.is_render_mirror()
            || !config.valid()
            || !self.models.contains_key(&id)
            || self
                .objects
                .iter()
                .any(|(&other, o)| other != id && o.sprite.is_some_and(|s| s.key == config.key))
        {
            return false;
        }
        // Native simulation and renderer load their own model instances. Bind
        // the assets needed by transient cut sprites before the first snapshot.
        if config.sword_target.is_some() {
            let art = self.models[&id].asset.clone();
            for offset in [1000, 2000] {
                self.sprite_assets.insert(config.key + offset, art.clone());
            }
            if let Some(key) = config.cut_effect_key {
                if let Some(effect) = self
                    .sprites
                    .iter()
                    .find(|(_, sprite)| sprite.config.key == key)
                    .and_then(|(id, _)| self.models.get(id))
                {
                    self.sprite_assets
                        .insert(config.key + 3000, effect.asset.clone());
                }
            }
        }
        let Some(object) = self.objects.get_mut(&id) else {
            return false;
        };
        object.sprite = Some(SpriteVisual {
            key: config.key,
            columns: config.columns,
            rows: config.rows,
            frame: 0,
            visible: false,
            elevated_view: config.elevated_view,
            directional_view: config.directional_view,
            atlas_layout: SpriteAtlasLayout::pack(
                &config.atlas_bands,
                config.columns * config.rows,
            ),
        });
        object.render_options.is_static = false;
        object.render_options.casts_shadow = false;
        object.render_options.receives_shadow = false;
        object.render_options.reflectivity = 0.;
        object.emission = 0.;
        if let Some((min, max)) = object.local_bounds {
            let radius = min.length().max(max.length());
            object.local_bounds = Some((-Vec3::splat(radius), Vec3::splat(radius)));
            object.refresh_render_data();
        }
        self.sprite_ids.insert(id);
        let visit_id = self.sprites.get(&id).map_or(0, |sprite| sprite.visit_id);
        let (gift_enabled, fetch_pending) =
            self.sprites.get(&id).map_or((false, false), |sprite| {
                (sprite.gift_enabled, sprite.fetch_pending)
            });
        self.sprites.insert(
            id,
            SpriteController {
                origin: object.transform.position,
                base_origin: object.transform.position,
                target_player: None,
                target_scoring: false,
                fragment: false,
                origin_scale_x: object.transform.scale.x,
                origin_rotation: object.transform.rotation,
                since_trigger: config.cooldown,
                waiting_for_flock: false,
                flock_drop: None,
                idle: config
                    .ambient_delay
                    .map_or(0., |delay| config.ambient_interval - delay),
                config,
                elapsed: None,
                pet_hold_remaining: 0.,
                pet_visit: None,
                visit_id,
                gift_enabled,
                fetch_pending,
                gift_contact_seconds: 0.,
                gift_contact_sample: false,
                gift_visit: false,
                gift_earned: false,
                gift_viewer_y: None,
            },
        );
        self.changed(id);
        true
    }
    pub fn trigger_sprite(&mut self, id: u32) -> bool {
        if self.is_render_mirror() {
            return false;
        }
        let Some(sprite) = self.sprites.get_mut(&id) else {
            return false;
        };
        if sprite.waiting_for_flock
            || sprite.elapsed.is_some()
            || sprite.since_trigger < sprite.config.cooldown
        {
            return false;
        }
        sprite.elapsed = Some(0.);
        sprite.begin_visit();
        sprite.gift_visit = sprite.fetch_pending && sprite.gift_enabled;
        if sprite.gift_visit {
            sprite.fetch_pending = false;
        }
        sprite.gift_earned = false;
        sprite.gift_contact_seconds = 0.;
        sprite.gift_contact_sample = false;
        sprite.pet_hold_remaining = 0.;
        sprite.pet_visit = None;
        sprite.since_trigger = 0.;
        sprite.idle = 0.;
        if let Some(object) = self.objects.get_mut(&id) {
            let visual = object.sprite.as_mut().unwrap();
            visual.visible = true;
            visual.frame = 0;
            let (_, tangent) = sprite.config.travel_sample(0.);
            if sprite.config.directional_view.is_some() && !sprite.fragment {
                object.transform.scale.x = sprite.origin_scale_x.abs();
                if tangent.x.hypot(tangent.z) > 0.00001 {
                    object.transform.rotation = Vec3::new(0., tangent.x.atan2(tangent.z), 0.);
                }
                object.refresh_render_data();
            } else if sprite.config.flip_with_travel && tangent.x != 0. {
                object.transform.scale.x = sprite.origin_scale_x * tangent.x.signum();
                object.refresh_render_data();
            }
        }
        self.changed(id);
        true
    }
    /// Cheap opt-in contact with the billboard head; no physics, haptics or audio.
    /// The viewer must be the shared head position, never an individual XR eye.
    pub fn pet_sprite(&mut self, id: u32, contact: Vec3, viewer: Vec3) -> bool {
        self.pet_sprite_contact(id, contact, viewer, None)
    }
    fn pet_sprite_contact(
        &mut self,
        id: u32,
        contact: Vec3,
        viewer: Vec3,
        hand: Option<usize>,
    ) -> bool {
        if self.is_render_mirror() || !contact.is_finite() || !viewer.is_finite() {
            return false;
        }
        let Some(sprite) = self.sprites.get(&id) else {
            return false;
        };
        let Some(elapsed) = sprite.elapsed else {
            return false;
        };
        let elapsed = elapsed - sprite.config.start_pause;
        let leg =
            (sprite.config.duration - sprite.config.start_pause - sprite.config.endpoint_pause)
                * 0.5;
        if !sprite.config.pettable
            || sprite
                .pet_visit
                .as_ref()
                .is_some_and(|visit| visit.farewell.is_some())
            || sprite.config.endpoint_pause <= 0.
            || elapsed < leg - sprite.config.arrival_pose_lead
            || elapsed > leg + sprite.config.endpoint_pause
        {
            return false;
        }
        let Some(object) = self.objects.get(&id) else {
            return false;
        };
        if !object.sprite.is_some_and(|visual| visual.visible) {
            return false;
        }
        // Match the shader basis, including elevated artwork and vertical-pole fallback.
        let delta = viewer - object.transform.position;
        let distance = delta.length();
        let facing = if distance > 0.00001 && distance.is_finite() {
            delta / distance
        } else {
            Vec3::Z
        };
        let horizontal = Vec3::new(facing.z, 0., -facing.x);
        let horizontal_length = horizontal.length();
        let mut right = if horizontal_length > 0.00001 {
            horizontal / horizontal_length
        } else {
            Vec3::X
        };
        // Idle top artwork uses the authored arrival heading, matching rendering.
        if object.sprite.is_some_and(|visual| {
            visual
                .directional_view
                .is_some_and(|directional| visual.frame >= directional.frames)
                && visual.elevated_view.is_some_and(|view| {
                    view.elevation > 0. && view.selected(facing.y.clamp(-1., 1.).asin())
                })
        }) {
            let yaw = object.transform.rotation.y;
            right = Vec3::new(yaw.cos(), 0., -yaw.sin());
        }
        let facing = if let Some(view) = object.sprite.and_then(|v| v.elevated_view) {
            let elevation = facing.y.clamp(-1., 1.).asin();
            let pitch = view.viewer_pitch(elevation);
            let forward = right.cross(Vec3::Y);
            forward * pitch.cos() + Vec3::Y * pitch.sin()
        } else {
            facing
        };
        if object
            .sprite
            .and_then(|v| v.elevated_view)
            .is_some_and(|view| {
                view.elevation < 0.
                    && view.selected((delta.y / distance.max(0.00001)).clamp(-1., 1.).asin())
            })
        {
            let yaw = object.transform.rotation.y;
            right = underside_right(facing, Vec3::new(yaw.sin(), 0., yaw.cos()));
        }
        let up = facing.cross(right);
        let head = object.transform.position + up * (0.55 * object.transform.scale.y.abs());
        let back = object.transform.position + up * (0.22 * object.transform.scale.y.abs());
        if segment_distance_squared(contact, back, head) > 0.25 * 0.25 {
            return false;
        }
        let sprite = self.sprites.get_mut(&id).unwrap();
        if sprite.gift_enabled {
            sprite.gift_contact_sample = true;
        }
        if sprite.pet_visit.is_none() {
            self.pet_gift_origin = Some(object.transform.position);
        }
        let visit = sprite.pet_visit.get_or_insert(PetVisit {
            elapsed: 0.,
            farewell: None,
            initial_head_y: viewer.y,
            contact_seconds: 0.,
            contact_sample: false,
            after_gift: false,
            current_head_y: Some(viewer.y),
            hands_tracked: hand.is_some(),
            contact_hand: hand,
            without_contact: 0.,
        });
        visit.contact_sample = true;
        visit.current_head_y = Some(viewer.y);
        // Unindexed legacy contacts cannot establish reliable withdrawal tracking.
        visit.contact_hand = hand;
        visit.hands_tracked = hand.is_some();
        visit.without_contact = 0.;
        sprite.pet_hold_remaining = 0.2;
        if sprite.config.idle_frames > 0 {
            let fps = sprite.config.idle_fps.unwrap_or(sprite.config.fps);
            let frame = (visit.elapsed * fps).floor() as u32 % sprite.config.idle_frames;
            let phase = 3 % sprite.config.idle_frames;
            self.objects
                .get_mut(&id)
                .unwrap()
                .sprite
                .as_mut()
                .unwrap()
                .frame =
                sprite.config.idle_start_frame + (phase + frame) % sprite.config.idle_frames;
            self.changed(id);
        }
        true
    }
    /// Read-only equipment intent. Uses the same arrived pettable sprite and billboard
    /// head as pet contact, with separate entry/exit thresholds to avoid flicker.
    pub fn pet_hand_intent(&self, head: glam::Mat4, grip: Vec3, held: bool) -> bool {
        let viewer = head.w_axis.truncate();
        let forward = -head.z_axis.truncate();
        if !viewer.is_finite() || !grip.is_finite() {
            return false;
        }
        self.sprites.iter().any(|(id, sprite)| {
            let Some(elapsed) = sprite.elapsed else {
                return false;
            };
            let elapsed = elapsed - sprite.config.start_pause;
            let leg =
                (sprite.config.duration - sprite.config.start_pause - sprite.config.endpoint_pause)
                    * 0.5;
            if !sprite.config.pettable
                || sprite.config.endpoint_pause <= 0.
                || elapsed < leg - sprite.config.arrival_pose_lead
                || elapsed > leg + sprite.config.endpoint_pause
                || sprite
                    .pet_visit
                    .as_ref()
                    .is_some_and(|visit| visit.farewell.is_some())
            {
                return false;
            }
            let Some(object) = self.objects.get(id) else {
                return false;
            };
            if !object.sprite.is_some_and(|visual| visual.visible) {
                return false;
            }
            let facing = (viewer - object.transform.position).normalize_or_zero();
            let mut right = Vec3::new(facing.z, 0., -facing.x).normalize_or_zero();
            if right.length_squared() < 0.5 {
                right = Vec3::X;
            }
            if object.sprite.is_some_and(|visual| {
                visual
                    .directional_view
                    .is_some_and(|directional| visual.frame >= directional.frames)
                    && visual.elevated_view.is_some_and(|view| {
                        view.elevation > 0. && view.selected(facing.y.clamp(-1., 1.).asin())
                    })
            }) {
                let yaw = object.transform.rotation.y;
                right = Vec3::new(yaw.cos(), 0., -yaw.sin());
            }
            let facing = if let Some(view) = object.sprite.and_then(|v| v.elevated_view) {
                let pitch = view.viewer_pitch(facing.y.clamp(-1., 1.).asin());
                right.cross(Vec3::Y) * pitch.cos() + Vec3::Y * pitch.sin()
            } else {
                facing
            };
            if object
                .sprite
                .and_then(|v| v.elevated_view)
                .is_some_and(|view| {
                    view.elevation < 0.
                        && view.selected(
                            ((viewer - object.transform.position).normalize_or_zero().y)
                                .clamp(-1., 1.)
                                .asin(),
                        )
                })
            {
                let yaw = object.transform.rotation.y;
                right = underside_right(facing, Vec3::new(yaw.sin(), 0., yaw.cos()));
            }
            let up = facing.cross(right);
            let target = object.transform.position + up * (0.55 * object.transform.scale.y.abs());
            let back = object.transform.position + up * (0.22 * object.transform.scale.y.abs());
            let gaze = ((target + back) * 0.5 - viewer)
                .normalize_or_zero()
                .dot(forward);
            // Reveal the palm while reaching, before the grip enters the smaller
            // physical pet-contact radius. A broad exit threshold prevents flicker.
            segment_distance_squared(grip, back, target)
                <= if held { 0.75 * 0.75 } else { 0.60 * 0.60 }
                && gaze >= if held { 0.65 } else { 0.8 }
        })
    }
    /// Test only explicitly pettable sprites against a tracked world-space grip.
    pub fn pet_sprites(&mut self, contact: Vec3, viewer: Vec3) -> usize {
        self.pet_sprites_contact(contact, viewer, None)
    }
    fn pet_sprites_contact(&mut self, contact: Vec3, viewer: Vec3, hand: Option<usize>) -> usize {
        if self.is_render_mirror() || !contact.is_finite() || !viewer.is_finite() {
            return 0;
        }
        // No allocation while all pettable sprites are hidden or travelling.
        let ids: Vec<_> = self
            .sprites
            .iter()
            .filter_map(|(&id, sprite)| {
                let elapsed = sprite.elapsed? - sprite.config.start_pause;
                let leg = (sprite.config.duration
                    - sprite.config.start_pause
                    - sprite.config.endpoint_pause)
                    * 0.5;
                (sprite.config.pettable
                    && sprite.config.endpoint_pause > 0.
                    && elapsed >= leg - sprite.config.arrival_pose_lead
                    && elapsed <= leg + sprite.config.endpoint_pause
                    && self
                        .objects
                        .get(&id)
                        .and_then(|o| o.sprite)
                        .is_some_and(|v| v.visible))
                .then_some(id)
            })
            .collect();
        ids.into_iter()
            .filter(|&id| self.pet_sprite_contact(id, contact, viewer, hand))
            .count()
    }
    /// Submit tracking once or more per frame; all visit timers advance only in update.
    /// Missing tracking cannot be interpreted as standing up or withdrawing hands.
    pub fn interact_sprites(&mut self, viewer: Option<Vec3>, hands: [Option<Vec3>; 2]) -> usize {
        if self.is_render_mirror() {
            return 0;
        }
        let viewer = viewer.filter(|v| v.is_finite());
        let hands = hands.map(|hand| hand.filter(|v| v.is_finite()));
        for sprite in self.sprites.values_mut() {
            sprite.gift_viewer_y = viewer.map(|head| head.y);
            if let Some(visit) = sprite.pet_visit.as_mut() {
                visit.current_head_y = viewer.map(|head| head.y);
                visit.hands_tracked = visit.contact_hand.is_some_and(|hand| hands[hand].is_some());
            }
        }
        let Some(viewer) = viewer else {
            return 0;
        };
        hands
            .into_iter()
            .enumerate()
            .filter_map(|(index, hand)| hand.map(|hand| (index, hand)))
            .map(|(index, hand)| self.pet_sprites_contact(hand, viewer, Some(index)))
            .sum()
    }
    /// One delivery per visit, shared by both tracked hands.
    /// Carry presentation uses an unused atlas flag, preserving the observer wire
    /// layout and ordinary semantic frames. Only atlases with the carry band opt in.
    pub fn set_pet_drink_visual(&mut self, visit: Option<(u32, u32)>) {
        if self.is_render_mirror() {
            return;
        }
        let ids: Vec<_> = self
            .sprites
            .iter()
            .filter(|(_, sprite)| sprite.config.pettable)
            .map(|(&id, sprite)| (id, sprite.visit_id))
            .collect();
        for (id, visit_id) in ids {
            let Some(visual) = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.sprite.as_mut())
            else {
                continue;
            };
            let Some(layout) = visual.atlas_layout.as_mut() else {
                continue;
            };
            let supported = layout
                .bands
                .iter()
                .any(|band| band.frames[0] == 64 && band.frames[1] >= 8);
            let flag = if supported && visit == Some((id, visit_id)) {
                0x0040_0000
            } else {
                0
            };
            let padding = &mut layout.bands[0].tiles[3];
            if *padding != flag {
                *padding = flag;
                self.changed(id);
            }
        }
    }
    /// Match the authoritative player's rendered billboard mouth. Looking around
    /// without translating the viewer does not change the attachment or rotate can.
    pub fn pet_drink_carrier(&self, viewer: Option<Vec3>) -> Option<crate::xp_drink::DrinkCarrier> {
        if self.is_render_mirror() {
            return None;
        }
        self.sprites.iter().find_map(|(&id, sprite)| {
            let elapsed = sprite.elapsed?;
            let config = &sprite.config;
            let object = self.objects.get(&id)?;
            let visual = object.sprite?;
            let return_at = config.start_pause
                + (config.duration - config.start_pause - config.endpoint_pause) * 0.5
                + config.endpoint_pause;
            if !config.pettable
                || !sprite.gift_visit
                || !visual.visible
                || elapsed > return_at
                || sprite.fragment
            {
                return None;
            }
            let yaw = object.transform.rotation.y;
            let heading = Vec3::new(yaw.sin(), 0., yaw.cos());
            let heading_right = Vec3::new(heading.z, 0., -heading.x);
            let directional = visual
                .directional_view
                .is_some_and(|v| visual.frame < v.frames);
            let delta = viewer
                .filter(|v| v.is_finite())
                .map_or(Vec3::Z, |v| v - object.transform.position);
            let toward = if delta.length_squared() > 0.0000000001 {
                delta.normalize()
            } else {
                Vec3::Z
            };
            let mut right = Vec3::new(toward.z, 0., -toward.x).normalize_or_zero();
            if right.length_squared() < 0.5 {
                right = if directional { -heading_right } else { Vec3::X };
            }
            let elevation = toward.y.clamp(-1., 1.).asin();
            let mut facing = toward;
            let mut local = glam::Vec2::new(0.28, 0.54);
            let mut depth = 0.;
            if directional {
                let horizontal = Vec3::new(delta.x, 0., delta.z);
                let to_camera = if horizontal.length_squared() > 0.0000000001 {
                    horizontal.normalize()
                } else {
                    -heading
                };
                let azimuth = to_camera.dot(heading_right).atan2(to_camera.dot(heading));
                let sector = (azimuth.abs() / std::f32::consts::FRAC_PI_4 + 0.5).floor() as u32;
                local = match sector {
                    0 => glam::Vec2::new(0.10, 0.525),
                    1 => glam::Vec2::new(0.21, 0.54),
                    2 => glam::Vec2::new(0.28, 0.55),
                    3 => glam::Vec2::new(0.18, 0.54),
                    _ => glam::Vec2::new(0., 0.54),
                };
                if azimuth > 0. && sector > 0 && sector < 4 {
                    local.x = -local.x;
                }
                if sector >= 3 {
                    depth = -0.06;
                }
                let pitch = elevation.clamp(-65_f32.to_radians(), 65_f32.to_radians());
                facing = right.cross(Vec3::Y) * pitch.cos() + Vec3::Y * pitch.sin();
            } else if let Some(view) = visual.elevated_view {
                let top = view.elevation > 0. && view.selected(elevation);
                if top && visual.directional_view.is_some() {
                    right = heading_right;
                }
                let pitch = view.viewer_pitch(elevation);
                facing = right.cross(Vec3::Y) * pitch.cos() + Vec3::Y * pitch.sin();
                if view.elevation < 0. && view.selected(elevation) {
                    right = underside_right(facing, heading);
                }
                if top {
                    local = glam::Vec2::new(0.042, 0.230);
                }
            }
            let scale = object.transform.scale;
            let muzzle = object.transform.position
                + right * (local.x * scale.x.abs())
                + facing.cross(right) * (local.y * scale.y.abs())
                + facing * depth;
            // Grip the middle of the can sideways. Its local +Y runs from base
            // to lid, and its printed front (+Z) faces the player's view.
            let up = facing.cross(right).normalize_or_zero();
            let rotation =
                glam::Quat::from_mat3(&glam::Mat3::from_cols(-up, right, right.cross(up)));
            let lip = muzzle - up * 0.025 + right * 0.1125 + facing * 0.039;
            Some(crate::xp_drink::DrinkCarrier {
                visit: (id, sprite.visit_id),
                lip,
                rotation,
                available: elapsed
                    >= config.start_pause
                        + (config.duration - config.start_pause - config.endpoint_pause) * 0.5
                        - config.arrival_pose_lead,
            })
        })
    }
    pub fn set_pet_gift_enabled(&mut self, enabled: bool) {
        for sprite in self
            .sprites
            .values_mut()
            .filter(|sprite| sprite.config.pettable)
        {
            sprite.gift_enabled = enabled;
        }
    }
    /// Round eligibility is separate from the physical cat and its generic cuddle.
    pub fn reset_pet_gifts(&mut self) {
        for sprite in self
            .sprites
            .values_mut()
            .filter(|sprite| sprite.config.pettable)
        {
            sprite.gift_enabled = false;
            sprite.gift_contact_seconds = 0.;
            sprite.gift_contact_sample = false;
            sprite.fetch_pending = false;
            sprite.gift_visit = false;
            sprite.gift_earned = false;
        }
        self.set_pet_drink_visual(None);
    }
    /// Start a fresh cuddle at handover. Time spent earning the gift does not count twice.
    pub fn pet_after_drink_pickup(&mut self, carrier: (u32, u32)) {
        let Some(sprite) = self.sprites.get_mut(&carrier.0) else {
            return;
        };
        if sprite.visit_id != carrier.1 {
            return;
        }
        let visit = sprite.pet_visit.get_or_insert(PetVisit {
            contact_seconds: 0.,
            contact_sample: false,
            after_gift: false,
            elapsed: 0.,
            farewell: None,
            initial_head_y: sprite.gift_viewer_y.unwrap_or(0.),
            current_head_y: sprite.gift_viewer_y,
            hands_tracked: false,
            contact_hand: None,
            without_contact: 0.,
        });
        visit.after_gift = true;
        visit.contact_seconds = 0.;
        visit.contact_sample = false;
        visit.elapsed = 0.;
        visit.farewell = None;
        visit.without_contact = 0.;
        visit.contact_hand = None;
        if let Some(y) = visit.current_head_y {
            visit.initial_head_y = y;
        }
        sprite.pet_hold_remaining = 0.;
    }
    pub fn take_pet_gift_origin(&mut self) -> Option<Vec3> {
        self.pet_gift_origin.take()
    }

    pub fn trigger_combo_sprites(&mut self) {
        let ids: Vec<_> = self
            .sprites
            .iter()
            .filter(|(_, s)| s.config.combo)
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            self.trigger_sprite(id);
        }
    }
    pub(crate) fn update_sprites(&mut self, dt: f32) {
        let ids: Vec<_> = self.sprites.keys().copied().collect();
        let mut flock_waiting = false;
        let mut flock_in_flight_or_due = false;
        for id in ids {
            let Some(object) = self.objects.get_mut(&id) else {
                self.sprites.remove(&id);
                continue;
            };
            let sprite = self.sprites.get_mut(&id).unwrap();
            sprite.since_trigger += dt;
            if sprite.waiting_for_flock {
                flock_waiting = true;
                continue;
            }
            if sprite.elapsed.is_none() {
                sprite.idle += dt;
                if (sprite.fetch_pending && sprite.gift_enabled && sprite.idle >= 0.15)
                    || (sprite.config.ambient_interval > 0.
                        && sprite.idle >= sprite.config.ambient_interval
                        && sprite.since_trigger >= sprite.config.cooldown)
                {
                    if let Some(target) = sprite.config.sword_target {
                        if let Some(head) = sprite.target_player {
                            sprite.origin = sprite.base_origin + Vec3::new(head.x, 0., head.z);
                            if let Some(height) = target.reach_height {
                                // The near-player portion is around 77% of this forward pass.
                                let eye_y = if sprite.config.flock_jitter.is_some() {
                                    self.flock_standing_eye_y
                                } else {
                                    head.y
                                };
                                sprite.origin.y =
                                    eye_y + height - sprite.config.travel_sample(0.77).0.y;
                            }
                        }
                    }
                    sprite.elapsed = Some(0.);
                    sprite.begin_visit();
                    sprite.gift_visit = sprite.fetch_pending && sprite.gift_enabled;
                    if sprite.gift_visit {
                        sprite.fetch_pending = false;
                    }
                    sprite.gift_earned = false;
                    sprite.gift_contact_seconds = 0.;
                    sprite.gift_contact_sample = false;
                    sprite.pet_hold_remaining = 0.;
                    sprite.pet_visit = None;
                    sprite.idle = 0.;
                    sprite.since_trigger = 0.;
                }
            }
            let Some(elapsed) = sprite.elapsed.as_mut() else {
                flock_in_flight_or_due |= sprite.config.flock_jitter.is_some();
                continue;
            };
            let dt = dt.max(0.);
            let held_dt = dt.min(sprite.pet_hold_remaining);
            sprite.pet_hold_remaining = (sprite.pet_hold_remaining - held_dt).max(0.);
            let mut timeline_dt = dt;
            if let Some(visit) = sprite.pet_visit.as_mut() {
                if let Some(farewell) = visit.farewell.as_mut() {
                    let previous = *farewell;
                    *farewell += dt;
                    // Farewell gets a full second at the endpoint before return.
                    timeline_dt = (*farewell - 1.).max(0.) - (previous - 1.).max(0.);
                } else {
                    visit.elapsed += dt;
                    if std::mem::take(&mut visit.contact_sample) {
                        visit.contact_seconds += dt.min(0.08);
                    }
                    visit.without_contact =
                        if held_dt > 0. || !visit.hands_tracked || visit.current_head_y.is_none() {
                            0.
                        } else {
                            visit.without_contact + dt
                        };
                    let standing = visit.hands_tracked
                        && visit.without_contact >= 0.6
                        && visit
                            .current_head_y
                            .is_some_and(|y| y - visit.initial_head_y >= 0.25);
                    if std::mem::take(&mut sprite.gift_contact_sample) && sprite.gift_enabled {
                        sprite.gift_contact_seconds += dt.min(0.08);
                    }
                    if sprite.gift_enabled
                        && !sprite.gift_visit
                        && !sprite.gift_earned
                        && sprite.gift_contact_seconds >= DRINK_PET_SECONDS
                    {
                        sprite.gift_earned = true;
                        sprite.fetch_pending = true;
                        visit.farewell = Some(0.);
                    }
                    let cuddle_complete =
                        !visit.after_gift || visit.contact_seconds >= DRINK_PET_SECONDS;
                    if (cuddle_complete && (standing || visit.elapsed >= 10.))
                        || (visit.elapsed >= 10. && held_dt == 0.)
                    {
                        visit.farewell = Some(0.);
                    }
                    timeline_dt = 0.;
                }
                // Once contact starts, waiting-time remainder is replaced by
                // bounded cuddle/farewell, then the existing return leg resumes.
                let return_at = sprite.config.start_pause
                    + (sprite.config.duration
                        - sprite.config.start_pause
                        - sprite.config.endpoint_pause)
                        * 0.5
                    + sprite.config.endpoint_pause;
                if visit.farewell.is_some_and(|t| t >= 1.) {
                    *elapsed = elapsed.max(return_at);
                }
            }
            *elapsed += timeline_dt;
            let visit_time = elapsed.min(sprite.config.duration);
            let time = (visit_time - sprite.config.start_pause).max(0.);
            let leg =
                (sprite.config.duration - sprite.config.start_pause - sprite.config.endpoint_pause)
                    * 0.5;
            let returning = !sprite.config.one_way && time > leg + sprite.config.endpoint_pause;
            let arrival = leg - sprite.config.arrival_pose_lead;
            let phase = if time <= arrival {
                time / arrival * 0.5
            } else if returning {
                0.5 + (time - leg - sprite.config.endpoint_pause) / leg * 0.5
            } else {
                0.5
            };
            // Smooth legs, with zero velocity at both endpoints and during the pause.
            let offset = if sprite.config.one_way {
                visit_time / sprite.config.duration
            } else {
                (phase * std::f32::consts::PI).sin().powi(2)
            };
            let (path_offset, tangent) = sprite.config.travel_sample(offset);
            object.transform.position = sprite.origin + path_offset;
            // The pause retains the incoming endpoint tangent. Reverse only once
            // the return leg starts, then follow the same curve backwards.
            if sprite.config.directional_view.is_some() && !sprite.fragment {
                let heading = tangent * if returning { -1. } else { 1. };
                object.transform.scale.x = sprite.origin_scale_x.abs();
                if heading.x.hypot(heading.z) > 0.00001 {
                    object.transform.rotation = Vec3::new(0., heading.x.atan2(heading.z), 0.);
                }
            } else if sprite.config.flip_with_travel && tangent.x != 0. {
                let direction = tangent.x.signum() * if returning { -1. } else { 1. };
                object.transform.scale.x = sprite.origin_scale_x * direction;
            }
            let visual = object.sprite.as_mut().unwrap();
            visual.visible = visit_time < sprite.config.duration;
            // Walking time excludes the pause, so return motion resumes its pose.
            let animation_time = if sprite.config.one_way || time <= arrival {
                time
            } else if returning {
                time - sprite.config.endpoint_pause - sprite.config.arrival_pose_lead
            } else {
                arrival
            };
            visual.frame = if sprite.config.idle_frames > 0
                && sprite.config.endpoint_pause > 0.
                && time >= leg - sprite.config.arrival_pose_lead
                && !returning
            {
                let fps = sprite.config.idle_fps.unwrap_or(sprite.config.fps);
                let frame = if let Some(visit) = sprite.pet_visit.as_ref() {
                    if visit.farewell.is_some() {
                        1 % sprite.config.idle_frames
                    } else if visit.after_gift && visit.contact_seconds == 0. {
                        0
                    } else {
                        (3 + (visit.elapsed * fps).floor() as u32) % sprite.config.idle_frames
                    }
                } else if sprite.config.pettable || time < leg {
                    // Calm waiting pose: cuddle keyposes start only after contact.
                    0
                } else {
                    ((time - leg) * fps).floor() as u32 % sprite.config.idle_frames
                };
                sprite.config.idle_start_frame + frame
            } else {
                (animation_time * sprite.config.fps).floor() as u32 % sprite.config.frames
            };
            if visit_time >= sprite.config.duration {
                sprite.elapsed = None;
                sprite.waiting_for_flock = sprite.config.flock_jitter.is_some();
                flock_waiting |= sprite.waiting_for_flock;
                sprite.idle = 0.;
                object.transform.position = sprite.origin;
                object.transform.scale.x = sprite.origin_scale_x;
                if sprite.config.directional_view.is_some() && !sprite.fragment {
                    object.transform.rotation = sprite.origin_rotation;
                }
            }
            flock_in_flight_or_due |=
                sprite.config.flock_jitter.is_some() && !sprite.waiting_for_flock;
            object.refresh_render_data();
            self.changed(id);
        }
        if flock_waiting && !flock_in_flight_or_due {
            // One authoritative draw per flock visit, reproducible in replay.
            // Preserve the authored stagger relative to the first returning bird.
            let members = self.sprites.values().filter(|s| s.waiting_for_flock);
            let first_delay = members
                .clone()
                .map(|s| s.config.ambient_delay.unwrap_or(0.))
                .fold(f32::INFINITY, f32::min);
            let first = members.into_iter().next().unwrap();
            let mut hash = first
                .visit_id
                .wrapping_add(first.config.key)
                .wrapping_mul(0x9e37_79b9);
            hash = (hash ^ (hash >> 16)).wrapping_mul(0x85eb_ca6b);
            let fraction = (hash >> 8) as f32 / 16_777_216.;
            for member in self.sprites.values_mut().filter(|s| s.waiting_for_flock) {
                let stagger = member.config.ambient_delay.unwrap_or(0.) - first_delay;
                member.idle = -stagger - fraction * member.config.flock_jitter.unwrap();
                member.waiting_for_flock = false;
            }
        }
    }
    pub(super) fn ground_drop_candidate(
        &self,
        sequence: u32,
        head: Option<Vec3>,
        visor_clear: bool,
    ) -> Option<GroundDropCandidate> {
        self.sprites
            .iter()
            .filter_map(|(&id, sprite)| {
                let config = sprite.config.ground_drop?;
                let elapsed = sprite.elapsed?;
                let object = self.objects.get(&id)?;
                if sprite.fragment || !sprite.config.one_way || !object.sprite?.visible {
                    return None;
                }
                let progress = elapsed / sprite.config.duration;
                let phase =
                    0.58 + (sprite.config.key.wrapping_mul(2654435761) % 101) as f32 * 0.001;
                let player = sprite
                    .target_player
                    .unwrap_or(sprite.origin + sprite.config.travel_sample(0.77).0);
                let position = object.transform.position;
                let flock = sprite.config.flock_jitter.is_some();
                let floor_half_width = if flock { 2.5 } else { 2. };
                if progress < phase
                    || progress > 0.91
                    || position.y <= config.floor_y
                    || (position.z - player.z).abs() > 2.
                    || (position.x - player.x).abs() > floor_half_width
                {
                    return None;
                }
                let overhead = |head: Vec3, radius: f32| {
                    position.y > head.y + 0.08
                        && (position - head).with_y(0.).length_squared() <= radius * radius
                };
                let (rank, aim) = if flock {
                    match sprite.flock_drop.as_ref()? {
                        FlockDrop::Floor(rank) => (*rank, None),
                        FlockDrop::Overhead(rank) => {
                            let head = head.filter(|&head| visor_clear && overhead(head, 0.30))?;
                            (*rank, Some(head))
                        }
                    }
                } else {
                    let aim = head.filter(|_| visor_clear && sequence.is_multiple_of(3));
                    if aim.is_some_and(|head| !overhead(head, 0.9)) {
                        return None;
                    }
                    let rank = sprite
                        .config
                        .key
                        .wrapping_mul(2654435761)
                        .wrapping_add(sequence.wrapping_mul(2246822519));
                    (rank, aim)
                };
                Some((
                    rank,
                    GroundDropCandidate {
                        emitter: id,
                        position,
                        floor: config.floor_y,
                        interval: config.interval,
                        aim,
                    },
                ))
            })
            .min_by_key(|(rank, _)| *rank)
            .map(|(_, candidate)| candidate)
    }

    pub(super) fn consume_ground_drop(&mut self, emitter: u32) {
        if let Some(sprite) = self.sprites.get_mut(&emitter) {
            sprite.flock_drop = None;
        }
    }
    pub(crate) fn sync_sprite_models(&mut self) {
        let targets: Vec<_> = self
            .sprite_ids
            .iter()
            .filter_map(|&id| {
                self.objects
                    .get(&id)
                    .and_then(|o| o.sprite)
                    .map(|s| (id, s.key))
            })
            .collect();
        for (id, key) in targets {
            if let Some(asset) = self.sprite_assets.get(&key) {
                if self
                    .models
                    .get(&id)
                    .is_none_or(|model| !std::sync::Arc::ptr_eq(&model.asset, asset))
                {
                    if let Ok(model) = crate::gltf_model::ModelInstance::new(asset.clone(), None) {
                        if self.models.insert(id, model).is_some() {
                            self.model_gpu_changes.removed.push(id);
                        }
                    }
                }
            }
            Self::queue_model_gpu_update(&mut self.models, &mut self.model_gpu_changes, id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{publication::RenderPublisher, wire::WirePublisher, MeshKind, Transform};
    use std::sync::Arc;
    fn asset() -> Arc<crate::gltf_model::ModelAsset> {
        Arc::new(
            crate::gltf_loader::parse_model(
                include_bytes!("../assets/models/rim/drone-orbiter.glb"),
                &[],
                &[],
            )
            .unwrap(),
        )
    }
    fn config() -> SpriteConfig {
        SpriteConfig {
            key: 77,
            columns: 4,
            rows: 2,
            frames: 8,
            fps: 6.,
            duration: 2.,
            ambient_interval: 4.,
            ambient_delay: None,
            flock_jitter: None,
            cooldown: 5.,
            travel: [0.5, 0., 0.],
            travel_control: None,
            travel_sway: None,
            sword_target: None,
            ground_drop: None,
            cut_effect_key: None,
            one_way: false,
            flip_with_travel: false,
            endpoint_pause: 0.,
            start_pause: 0.,
            arrival_pose_lead: 0.,
            idle_start_frame: 0,
            idle_frames: 0,
            idle_fps: None,
            pettable: false,
            combo: true,
            elevated_view: None,
            directional_view: None,
            atlas_bands: Vec::new(),
        }
    }
    fn decoded_fragment_clip(packed: u32) -> (Vec2, f32) {
        let angle = (packed & 4095) as f32 * (std::f32::consts::TAU / 4096.);
        (
            Vec2::new(angle.cos(), angle.sin()),
            (((packed >> 12) & 4095) as f32 - 2048.) / 2047.,
        )
    }
    #[test]
    fn sprite_cut_line_horizontal_vertical_diagonal_and_offset() {
        for direction in [Vec3::X, Vec3::Y, (Vec3::X + Vec3::Y).normalize()] {
            let perpendicular = Vec3::new(-direction.y, direction.x, 0.);
            let offset = perpendicular * 0.17;
            // A short blade follows an off-center stroke, rather than always
            // forcing the cut through the artwork's center.
            let previous = [offset - direction; 2];
            let current = [offset + direction; 2];
            let (normal, line, separation) =
                sprite_cut_line(previous, current, Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ONE);
            assert!(separation.abs_diff_eq(perpendicular, 1e-5));
            assert!((separation.dot(direction)).abs() < 1e-5);
            assert!((line - 0.17).abs() < 1e-5);
            let (decoded, decoded_line) = decoded_fragment_clip(fragment_clip(normal, line, false));
            assert!(decoded.dot(normal) > 0.99999);
            assert!((decoded_line - line).abs() < 1. / 2047.);
            let packed_positive = fragment_clip(normal, line, true);
            assert_eq!(
                fragment_clip(normal, line, false) ^ packed_positive,
                1 << 24
            );
            // Both complementary sides meet at the same noncentral diagonal.
            let uv_hit = Vec2::new(offset.x, -offset.y);
            assert!((decoded.dot(uv_hit) - decoded_line).abs() < 0.001);
        }
    }
    #[test]
    fn sprite_cut_line_degenerate_blade_and_view_normal_motion_are_finite() {
        for (previous, current) in [
            ([Vec3::ZERO; 2], [Vec3::ZERO; 2]),
            ([Vec3::ZERO, Vec3::Y], [Vec3::ZERO, Vec3::Y]),
            ([Vec3::ZERO; 2], [Vec3::Z; 2]),
        ] {
            let (normal, offset, separation) =
                sprite_cut_line(previous, current, Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::ONE);
            assert!(normal.is_finite() && offset.is_finite() && separation.is_finite());
            assert!((normal.length() - 1.).abs() < 1e-5);
            assert!((separation.length() - 1.).abs() < 1e-5);
        }
    }
    #[test]
    fn sprite_cut_fragment_full_frame_clip_and_frozen_plane_roundtrip() {
        let mut source = SceneManager::default();
        let mut mirror = SceneManager::default();
        for scene in [&mut source, &mut mirror] {
            let id = scene.add_model(asset()).unwrap();
            let mut c = config();
            c.one_way = true;
            c.flip_with_travel = true;
            c.sword_target = Some(SpriteSwordTarget {
                points: 1000,
                radius: 0.4,
                reach_height: None,
            });
            assert!(scene.configure_sprite(id, c));
            scene.set_sprite_target_player(Some(Vec3::new(0., 1., 2.)), true);
        }
        let bird = *source.sprites.keys().next().unwrap();
        source.split_sprite_target(
            bird,
            [Vec3::new(-1., -0.8, 0.); 2],
            [Vec3::new(1., 1.2, 0.); 2],
        );
        let parts: Vec<_> = source
            .sprites
            .iter()
            .filter(|(_, s)| s.fragment)
            .map(|(&id, _)| id)
            .collect();
        assert_eq!(parts.len(), 2);
        let mut clips = Vec::new();
        for id in &parts {
            let object = &source.objects[id];
            assert!(object.transform.scale.abs().abs_diff_eq(Vec3::ONE, 1e-5));
            let layout = object.sprite.unwrap().atlas_layout.unwrap();
            assert!(layout.valid(1));
            assert_eq!(layout.bands[0].rect, [0., 0., 0.25, 0.5]);
            clips.push(layout.bands[0].tiles[3]);
            assert!(!source.sprites[id].config.flip_with_travel);
        }
        assert_eq!(clips[0] ^ clips[1], 1 << 24);
        mirror
            .apply_wire_publication(&WirePublisher::default().capture(&mut source).unwrap(), &[])
            .unwrap();
        for id in parts {
            assert_eq!(source.objects[&id].sprite, mirror.objects[&id].sprite);
            assert_eq!(
                source.objects[&id].transform.rotation,
                mirror.objects[&id].transform.rotation
            );
            assert!(mirror.models.contains_key(&id));
        }
    }
    #[test]
    fn flock_waits_after_last_bird_and_keeps_first_wave_stagger() {
        let mut scene = SceneManager::default();
        let ids = [0., 2.].map(|delay| {
            let id = scene.add_model(asset()).unwrap();
            let mut cfg = config();
            cfg.key = id + 5;
            cfg.one_way = true;
            cfg.travel = [0.; 3];
            cfg.duration = 12.;
            cfg.cooldown = 0.;
            cfg.ambient_interval = 12.;
            cfg.flock_jitter = Some(3.);
            cfg.ambient_delay = Some(delay);
            cfg.sword_target = Some(SpriteSwordTarget {
                points: 1000,
                radius: 0.25,
                reach_height: None,
            });
            assert!(scene.configure_sprite(id, cfg));
            id
        });
        scene.set_sprite_target_player(Some(Vec3::Y), true);
        scene.update_sprites(0.1);
        let hit = scene.cut_sprite_sweep(
            [-Vec3::X - Vec3::Y, -Vec3::X + Vec3::Y],
            [Vec3::X - Vec3::Y, Vec3::X + Vec3::Y],
        );
        assert_eq!(hit.len(), 1);
        assert!(scene.sprites[&ids[0]].waiting_for_flock);
        let mut exits = Vec::new();
        let mut starts = Vec::new();
        let mut second_starts = Vec::new();
        for tick in 1..4000 {
            let was = ids.map(|id| scene.sprites[&id].elapsed.is_some());
            scene.update(1. / 30.);
            let now = ids.map(|id| scene.sprites[&id].elapsed.is_some());
            if was[1] && !now[1] {
                exits.push(tick);
            }
            if !was[0] && now[0] {
                starts.push(tick);
            }
            if !was[1] && now[1] {
                second_starts.push(tick);
            }
        }
        assert!(starts.len() >= 3);
        let gaps: Vec<_> = starts
            .iter()
            .zip(exits)
            .map(|(start, end)| start - end)
            .collect();
        assert!(gaps.iter().all(|n| (360..=451).contains(n)));
        assert!(gaps.windows(2).any(|pair| pair[0] != pair[1]));
        for (first, second) in starts.iter().zip(second_starts.iter().skip(1)) {
            assert!((59..=61).contains(&(second - first)));
        }
    }
    #[test]
    fn sword_target_sweep_awards_once_splits_and_respawns() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.one_way = true;
        c.travel = [0.; 3];
        c.cooldown = 0.;
        c.ambient_delay = Some(0.);
        c.sword_target = Some(SpriteSwordTarget {
            points: 1000,
            radius: 0.25,
            reach_height: None,
        });
        assert!(scene.configure_sprite(id, c));
        scene.set_sprite_target_player(Some(Vec3::new(0., 1.7, 0.)), true);
        scene.update_sprites(0.1);
        let before = [Vec3::new(-1., -0.5, 0.), Vec3::new(-1., 0.5, 0.)];
        let after = [Vec3::new(1., -0.5, 0.), Vec3::new(1., 0.5, 0.)];
        assert_eq!(
            scene.cut_sprite_sweep(before, after),
            vec![SpriteCutHit {
                original: id,
                points: 1000
            }]
        );
        assert!(scene.cut_sprite_sweep(before, after).is_empty());
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        let pieces: Vec<_> = scene
            .sprites
            .iter()
            .filter(|(_, s)| s.fragment)
            .map(|(&id, _)| id)
            .collect();
        assert_eq!(pieces.len(), 2);
        for piece in &pieces {
            let v = scene.objects[piece].sprite.unwrap();
            assert!(v.visible && v.atlas_layout.unwrap().valid(1));
            assert!(scene.sprites[piece].config.sword_target.is_none());
        }
        scene.set_sprite_target_player(Some(Vec3::new(0., 1.7, 0.)), false);
        scene.update_sprites(4.1);
        assert!(scene.cut_sprite_sweep(before, after).is_empty());
        scene.set_sprite_target_player(Some(Vec3::new(0., 1.7, 0.)), true);
        // A new visit is eligible, never the second hand of the same hit.
        scene.update_sprites(4.1);
        scene.update_sprites(0.1);
        assert!(scene.trigger_sprite(id));
        assert_eq!(scene.cut_sprite_sweep(before, after).len(), 1);
    }
    #[test]
    fn ground_drop_requires_opt_in_visible_uncut_one_way_actor() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.one_way = true;
        c.cooldown = 0.;
        c.duration = 10.;
        c.travel = [0., 0., 5.];
        c.sword_target = Some(SpriteSwordTarget {
            points: 1,
            radius: 0.3,
            reach_height: None,
        });
        c.ground_drop = Some(SpriteGroundDrop {
            interval: 6.,
            floor_y: 0.012,
        });
        scene.set_object_position(id, Vec3::new(0., 2., -3.));
        assert!(scene.configure_sprite(id, c.clone()));
        scene.set_sprite_target_player(Some(Vec3::new(0., 1.7, 0.)), true);
        assert!(scene.ground_drop_candidate(0, None, true).is_none());
        assert!(scene.trigger_sprite(id));
        scene.update_sprites(7.5);
        assert!(scene.ground_drop_candidate(0, None, true).is_some());
        let p = scene.objects[&id].transform.position;
        assert_eq!(
            scene
                .cut_sprite_sweep([p - Vec3::X, p + Vec3::X], [p - Vec3::X, p + Vec3::X])
                .len(),
            1
        );
        assert!(scene.ground_drop_candidate(0, None, true).is_none());
        assert!(scene
            .sprites
            .values()
            .filter(|sprite| sprite.fragment)
            .all(|sprite| sprite.config.ground_drop.is_none()));
        c.one_way = false;
        assert!(!c.valid());
        c.one_way = true;
        c.ground_drop.as_mut().unwrap().interval = f32::NAN;
        assert!(!c.valid());
        // Decorative emitters need no sword target and still receive player tracking.
        c.ground_drop = Some(SpriteGroundDrop {
            interval: 6.,
            floor_y: 0.012,
        });
        c.sword_target = None;
        scene.set_object_position(id, Vec3::new(0., 2., -3.));
        assert!(scene.configure_sprite(id, c));
        scene.set_sprite_target_player(Some(Vec3::new(0., 1.7, 0.)), true);
        assert!(scene.trigger_sprite(id));
        scene.update_sprites(7.5);
        assert!(scene.ground_drop_candidate(1, None, true).is_some());
    }
    #[test]
    fn flock_drop_varies_by_visit_and_sideways_birds_only_release_to_the_floor() {
        let mut scene = SceneManager::default();
        // The first visit picks overhead for key 2, floor for key 5 and skip for key 3.
        let [overhead, floor, _skip] = [(2, 0.33), (5, 2.25), (3, 0.)].map(|(key, x)| {
            let id = scene.add_model(asset()).unwrap();
            let mut c = config();
            c.key = key;
            c.one_way = true;
            c.cooldown = 0.;
            c.duration = 10.;
            c.travel = [0., 0., 5.];
            c.flock_jitter = Some(0.);
            c.ground_drop = Some(SpriteGroundDrop {
                interval: 6.,
                floor_y: 0.012,
            });
            scene.set_object_position(id, Vec3::new(x, 2., -3.75));
            assert!(scene.configure_sprite(id, c));
            assert!(scene.trigger_sprite(id));
            id
        });
        let head = Vec3::new(0., 1.7, 0.);
        scene.set_sprite_target_player(Some(head), true);
        scene.update_sprites(7.5);

        // The floor candidate is released even with no eligible overhead aim.
        let candidate = scene.ground_drop_candidate(0, Some(head), true).unwrap();
        assert_eq!(candidate.emitter, floor);
        assert_eq!(candidate.aim, None);
        scene.consume_ground_drop(candidate.emitter);
        assert!(scene.ground_drop_candidate(0, Some(head), true).is_none());

        scene.set_object_position(overhead, Vec3::new(0.25, 2., 0.));
        assert!(scene.ground_drop_candidate(1, Some(head), false).is_none());
        let candidate = scene.ground_drop_candidate(1, Some(head), true).unwrap();
        assert_eq!(candidate.emitter, overhead);
        assert_eq!(candidate.aim, Some(head));
        scene.consume_ground_drop(candidate.emitter);
        assert!(scene.ground_drop_candidate(1, Some(head), true).is_none());

        scene.update_sprites(2.5);
        assert!(scene.trigger_sprite(overhead));
        scene.update_sprites(7.5);
        scene.set_object_position(overhead, Vec3::new(0.25, 2., 0.));
        // All flock members can start another visit here. Check this bird's
        // second draw individually, independently of what its neighbors drew.
        assert_eq!(scene.sprites[&overhead].visit_id, 2);
        assert!(scene.sprites[&overhead].flock_drop.is_none());
    }

    #[test]
    fn native_cut_publication_resolves_halves_and_effect_then_retires_models() {
        let mut source = SceneManager::default();
        let mut mirror = SceneManager::default();
        for scene in [&mut source, &mut mirror] {
            let template = scene.add_model(asset()).unwrap();
            let mut effect = config();
            effect.key = 900;
            effect.one_way = true;
            effect.ambient_interval = 0.;
            effect.cooldown = 0.;
            effect.duration = 1.;
            assert!(scene.configure_sprite(template, effect));
            let id = scene.add_model(asset()).unwrap();
            let mut bird = config();
            bird.one_way = true;
            bird.cut_effect_key = Some(900);
            bird.sword_target = Some(SpriteSwordTarget {
                points: 1000,
                radius: 0.4,
                reach_height: None,
            });
            assert!(scene.configure_sprite(id, bird));
        }
        let bird = *source.sprites.keys().last().unwrap();
        source.split_sprite_target(bird, [Vec3::ZERO; 2], [Vec3::X; 2]);
        let mut publisher = crate::scene::publication::RenderPublisher::default();
        mirror.apply_render_publication(publisher.capture(&mut source), &[]);
        let parts: Vec<_> = mirror
            .sprite_ids
            .iter()
            .filter(|id| **id > bird)
            .copied()
            .collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|id| mirror.models.contains_key(id)));
        for id in &parts {
            source.remove_object(*id);
        }
        mirror.apply_render_publication(publisher.capture(&mut source), &[]);
        assert!(parts.iter().all(|id| !mirror.models.contains_key(id)));
    }
    #[test]
    fn sword_target_finite_sweep_rejects_outside_and_invalid_tracking() {
        let a = Vec3::ZERO;
        let b = Vec3::X;
        let c = Vec3::Y;
        assert!(sweep_triangle_distance_squared(Vec3::new(2., 2., 0.), a, b, c) > 1.);
        assert_eq!(
            sweep_triangle_distance_squared(Vec3::new(0.2, 0.2, 0.5), a, b, c),
            0.25
        );
        assert!(sweep_triangle_distance_squared(Vec3::Z, a, a, a).is_finite());
    }
    #[test]
    fn sword_target_reach_is_player_relative_and_latched_per_visit() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.one_way = true;
        c.cooldown = 0.;
        c.ambient_delay = Some(0.);
        c.travel = [0., 0.2, 30.];
        c.duration = 12.;
        c.sword_target = Some(SpriteSwordTarget {
            points: 1000,
            radius: 0.3,
            reach_height: Some(-0.35),
        });
        assert!(scene.configure_sprite(id, c));
        let head = Vec3::new(2., 1.65, 3.);
        scene.set_sprite_target_player(Some(head), true);
        scene.update_sprites(12. * 0.77);
        assert!((scene.objects[&id].transform.position.y - 1.3).abs() < 1e-5);
        let origin = scene.sprites[&id].origin;
        scene.set_sprite_target_player(Some(Vec3::new(10., 2., 10.)), true);
        scene.update_sprites(0.01);
        assert_eq!(scene.sprites[&id].origin, origin);
    }
    #[test]
    fn sprite_travel_sway_is_bounded_and_preserves_endpoints_and_forward_motion() {
        let mut c = config();
        c.travel = [0.7, 0.3, -8.];
        c.travel_control = Some([0.2, 0.8, -3.]);
        let original = c.clone();
        c.travel_sway = Some(SpriteTravelSway {
            amplitude: [0.3, 0.15, 0.],
            cycles: 2.7,
            phase: 1.3,
        });
        assert!(c.valid());
        for t in [0., 1.] {
            assert_eq!(c.travel_sample(t), original.travel_sample(t));
        }
        let mut previous_z = 0.;
        let mut peak = Vec3::ZERO;
        for i in 0..=1000 {
            let t = i as f32 / 1000.;
            let (position, tangent) = c.travel_sample(t);
            let delta = (position - original.travel_sample(t).0).abs();
            assert!(delta.x <= 0.300001 && delta.y <= 0.150001 && delta.z == 0.);
            assert!(position.z <= previous_z && tangent.z < 0.);
            peak = peak.max(delta);
            previous_z = position.z;
        }
        assert!(peak.x > 0.15 && peak.y > 0.075);
    }
    #[test]
    fn sprite_travel_sway_tangent_matches_numerical_derivative() {
        let mut c = config();
        c.travel_control = Some([0.2, 0.4, -1.]);
        c.travel_sway = Some(SpriteTravelSway {
            amplitude: [0.3, 0.15, 0.08],
            cycles: 2.3,
            phase: -0.8,
        });
        let h = 0.0001;
        for t in [0.01, 0.13, 0.35, 0.52, 0.78, 0.99] {
            let numerical = (c.travel_sample(t + h).0 - c.travel_sample(t - h).0) / (2. * h);
            assert!((numerical - c.travel_sample(t).1).length() < 0.003, "t={t}");
        }
    }
    #[test]
    fn sprite_travel_sway_optional_json_and_validity() {
        let mut c = config();
        assert!(c.travel_sway.is_none());
        for t in [0., 0.25, 0.5, 1.] {
            assert_eq!(
                c.travel_sample(t),
                (Vec3::from_array(c.travel) * t, Vec3::from_array(c.travel))
            );
        }
        let json = r#"{"key":77,"columns":4,"rows":2,"frames":8,"fps":6,
            "duration":2,"ambientInterval":4,"cooldown":5,"travel":[0.5,0,0]}"#;
        assert!(serde_json::from_str::<SpriteConfig>(json)
            .unwrap()
            .travel_sway
            .is_none());
        let explicit = json.replace(
            "\"travel\"",
            "\"travelSway\":{\"amplitude\":[0.3,0.1,0],\"cycles\":2.5,\"phase\":1},\"travel\"",
        );
        assert!(serde_json::from_str::<SpriteConfig>(&explicit)
            .unwrap()
            .valid());
        let base = SpriteTravelSway {
            amplitude: [0.3, 0.1, 0.],
            cycles: 2.5,
            phase: 1.,
        };
        for invalid in [
            SpriteTravelSway {
                amplitude: [f32::NAN, 0., 0.],
                ..base
            },
            SpriteTravelSway {
                amplitude: [f32::INFINITY, 0., 0.],
                ..base
            },
            SpriteTravelSway {
                amplitude: [f32::MAX, 0., 0.],
                ..base
            },
            SpriteTravelSway {
                cycles: -1.,
                ..base
            },
            SpriteTravelSway {
                cycles: 65.,
                ..base
            },
            SpriteTravelSway {
                cycles: f32::NAN,
                ..base
            },
            SpriteTravelSway {
                phase: f32::INFINITY,
                ..base
            },
        ] {
            c.travel_sway = Some(invalid);
            assert!(!c.valid());
        }
        c.travel_sway = Some(SpriteTravelSway {
            phase: f32::MAX,
            ..base
        });
        assert!(c.valid());
        assert!(c.travel_sample(0.5).0.is_finite() && c.travel_sample(0.5).1.is_finite());
    }
    #[test]
    fn sprite_elevated_view_wire_preserves_authority_and_contact_basis() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.rows = 8;
        c.duration = 4.;
        c.endpoint_pause = 2.;
        c.pettable = true;
        c.elevated_view = Some(SpriteElevatedView {
            start_frame: 16,
            elevation: 50.,
            max_front_pitch: 25.,
            pitch: 65.,
        });
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        source.update(1.1);
        let endpoint = source.objects[&id].transform.position;
        let pitch = 65_f32.to_radians();
        let viewer = endpoint + Vec3::new(0., 2., 0.01);
        let head = endpoint + Vec3::new(0., pitch.cos(), -pitch.sin()) * 0.55;
        assert!(source.pet_sprite(id, head, viewer));
        assert!(!source.pet_sprite(id, endpoint + Vec3::Y * 0.55, viewer));
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        assert!(mirror.bind_sprite(preload, 77));
        let bytes = WirePublisher::default().capture(&mut source).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 8);
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        let visual = source.objects[&id].sprite.unwrap();
        assert_eq!(mirror.objects[&id].sprite, Some(visual));
        mirror.set_billboard_camera(Some(Vec3::new(0., 8., 0.)));
        mirror.update(20.);
        assert_eq!(mirror.objects[&id].sprite, Some(visual));
        assert_eq!(visual.view_uniform()[0], 16.);
        assert!(!mirror.trigger_sprite(id));
    }
    #[test]
    fn sprite_elevated_view_requires_complete_nonoverlapping_band() {
        let mut c = config();
        c.rows = 4;
        c.elevated_view = Some(SpriteElevatedView {
            start_frame: 8,
            elevation: 50.,
            max_front_pitch: 25.,
            pitch: 65.,
        });
        assert!(c.valid());
        c.elevated_view.as_mut().unwrap().start_frame = 7;
        assert!(!c.valid());
        c.elevated_view.as_mut().unwrap().start_frame = 9;
        assert!(!c.valid());
        c.elevated_view.as_mut().unwrap().start_frame = 8;
        c.elevated_view.as_mut().unwrap().pitch = f32::NAN;
        assert!(!c.valid());
    }
    #[test]
    fn sprite_below_view_signed_thresholds_and_validation() {
        let mut view = SpriteElevatedView {
            start_frame: 20,
            elevation: -35.,
            max_front_pitch: 25.,
            pitch: -65.,
        };
        assert!(view.valid(24, 3));
        assert!(view.selected(-35_f32.to_radians()));
        assert!(view.selected(-90_f32.to_radians()));
        assert!(!view.selected(-34_f32.to_radians()));
        assert!(!view.selected(65_f32.to_radians()));
        assert_eq!(
            view.viewer_pitch(-40_f32.to_radians()),
            -65_f32.to_radians()
        );
        assert_eq!(
            view.viewer_pitch(-90_f32.to_radians()),
            -90_f32.to_radians()
        );
        assert_eq!(view.viewer_pitch(40_f32.to_radians()), 25_f32.to_radians());
        for (elevation, pitch, clamp) in [
            (0., -65., 25.),
            (-90., -90., 25.),
            (-35., 65., 25.),
            (-35., -34., 25.),
            (-35., -91., 25.),
            (-35., -65., -1.),
            (-35., -65., 36.),
        ] {
            view.elevation = elevation;
            view.pitch = pitch;
            view.max_front_pitch = clamp;
            assert!(!view.valid(24, 3));
        }
    }
    #[test]
    fn sprite_below_basis_keeps_head_along_flight_across_overhead() {
        let view = SpriteElevatedView {
            start_frame: 20,
            elevation: -35.,
            max_front_pitch: 25.,
            pitch: -65.,
        };
        for yaw in [0_f32, 0.7, -1.4] {
            let heading = Vec3::new(yaw.sin(), 0., yaw.cos());
            let mut previous_up = None;
            for offset in [
                Vec3::new(0., -1., -0.0001),
                Vec3::NEG_Y,
                Vec3::new(0., -1., 0.0001),
                Vec3::new(0.3, -1., 0.2),
                Vec3::new(-0.3, -1., -0.2),
            ] {
                let camera_facing = offset.normalize();
                let elevation = camera_facing.y.asin();
                let pitch = view.viewer_pitch(elevation);
                let horizontal =
                    Vec3::new(camera_facing.x, 0., camera_facing.z).normalize_or_zero();
                let facing = (horizontal * pitch.cos() + Vec3::Y * pitch.sin()).normalize();
                let right = underside_right(facing, heading);
                let up = facing.cross(right);
                assert!(right.is_finite() && up.is_finite());
                assert!(up.dot(heading) > 0.9);
                assert!((right.cross(up).dot(facing) - 1.).abs() < 0.00001);
                if offset.x.hypot(offset.z) < 0.001 {
                    if let Some(previous) = previous_up {
                        assert!(up.dot(previous) > 0.999);
                    }
                    previous_up = Some(up);
                }
            }
        }
    }
    #[test]
    fn sprite_below_directional_atlas_wire_keeps_viewer_selection_local() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.columns = 4;
        c.rows = 6;
        c.frames = 4;
        c.directional_view = Some(SpriteDirectionalView {
            frames: 4,
            front_start_frame: 0,
            front_oblique_start_frame: 4,
            side_start_frame: 8,
            rear_oblique_start_frame: 12,
            rear_start_frame: 16,
        });
        c.elevated_view = Some(SpriteElevatedView {
            start_frame: 20,
            elevation: -35.,
            max_front_pitch: 25.,
            pitch: -65.,
        });
        c.atlas_bands = [0, 1, 1, 2, 2, 3]
            .into_iter()
            .enumerate()
            .map(|(i, row)| SpriteAtlasBand {
                first_frame: i as u32 * 4,
                frame_count: 4,
                columns: 4,
                origin: [0., row as f32 * 0.25],
                tile_size: [0.25, 0.25],
                first_tile: 0,
                repeat: None,
                tile_map: None,
            })
            .collect();
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        source.update(0.2);
        let visual = source.objects[&id].sprite.unwrap();
        assert!(visual.frame < 4);
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        assert!(mirror.bind_sprite(preload, 77));
        let bytes = WirePublisher::default().capture(&mut source).unwrap();
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        for camera in [Vec3::new(0., -8., 0.), Vec3::new(0., 8., 0.)] {
            mirror.set_billboard_camera(Some(camera));
            mirror.update(10.);
            assert_eq!(mirror.objects[&id].sprite, Some(visual));
        }
        assert_eq!(visual.view_uniform()[1], -35_f32.to_radians());
        assert_eq!(visual.view_uniform()[3], -65_f32.to_radians());
    }
    #[test]
    fn sprite_one_way_flight_hides_repeats_and_preserves_heading() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let origin = Vec3::new(4., 3.4, -1.);
        scene.set_object_position(id, origin);
        let mut c = config();
        c.one_way = true;
        c.combo = false;
        c.duration = 4.;
        c.ambient_delay = Some(1.);
        c.ambient_interval = 24.;
        c.cooldown = 0.;
        c.travel = [-8., 0.2, 0.];
        c.travel_control = Some([-4., 0.6, -1.]);
        c.flip_with_travel = true;
        assert!(scene.configure_sprite(id, c));
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        scene.update(0.5);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        scene.update(0.5);
        assert!(scene.objects[&id].sprite.unwrap().visible);
        scene.update(1.5); // Halfway through the first pass.
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(origin + Vec3::new(-4., 0.35, -0.5), 0.00001));
        assert!(scene.objects[&id].transform.scale.x < 0.);
        scene.update(1.);
        assert!(scene.objects[&id].transform.position.x < 0.); // Never retraces.
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 2); // Flapping continues after midpoint.
        scene.update(1.);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        assert_eq!(scene.objects[&id].transform.position, origin);
        scene.update(23.);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        scene.update(1.);
        assert!(scene.objects[&id].sprite.unwrap().visible);
        assert!(scene.objects[&id].transform.scale.x < 0.);
    }
    #[test]
    fn sprite_one_way_rejects_ground_visit_options() {
        let mut c = config();
        c.one_way = true;
        assert!(c.valid());
        c.pettable = true;
        assert!(!c.valid());
        c.pettable = false;
        c.start_pause = 0.1;
        assert!(!c.valid());
        c.start_pause = 0.;
        c.ambient_delay = Some(f32::NAN);
        assert!(!c.valid());
    }
    #[test]
    fn sprite_curved_travel_retraces_route_and_preserves_endpoint_pause() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let origin = Vec3::new(3.2, 0.025, -1.5);
        scene.set_object_position(id, origin);
        let mut c = config();
        c.duration = 6.;
        c.endpoint_pause = 2.;
        c.flip_with_travel = true;
        c.travel = [-3.05, 0., 1.2];
        c.travel_control = Some([0.1, 0., 2.6]);
        let expected_midpoint = origin + Vec3::new(-0.7125, 0., 1.6);
        let endpoint = origin + Vec3::from_array(c.travel);
        assert!(scene.configure_sprite(id, c));
        assert!(scene.trigger_sprite(id));
        // Control starts toward +X, although the final displacement is -X.
        assert!(scene.objects[&id].transform.scale.x > 0.);
        scene.update(1.);
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(expected_midpoint, 0.00001));
        assert!(scene.objects[&id].transform.scale.x < 0.);
        scene.update(1.);
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(endpoint, 0.00001));
        let incoming_scale = scene.objects[&id].transform.scale.x;
        scene.update(1.);
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(endpoint, 0.00001));
        assert_eq!(scene.objects[&id].transform.scale.x, incoming_scale);
        scene.update(2.);
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(expected_midpoint, 0.00001));
        assert!(scene.objects[&id].transform.scale.x > 0.);
        scene.update(1.);
        assert_eq!(scene.objects[&id].transform.position, origin);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
    }
    #[test]
    fn sprite_start_pause_holds_first_pose_and_offsets_pet_window() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.duration = 8.;
        c.start_pause = 2.;
        c.endpoint_pause = 2.;
        c.pettable = true;
        c.rows = 4;
        c.idle_start_frame = 8;
        c.idle_frames = 8;
        assert!(scene.configure_sprite(id, c));
        assert!(scene.trigger_sprite(id));
        let viewer = Vec3::new(0.5, 0., 2.);
        let head = Vec3::new(0.5, 0.55, 0.);
        scene.update(1.);
        assert_eq!(scene.objects[&id].transform.position, Vec3::ZERO);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 0);
        assert!(!scene.pet_sprite(id, head, viewer));
        assert_eq!(scene.pet_sprites(head, viewer), 0);
        scene.update(1.);
        assert_eq!(scene.objects[&id].transform.position, Vec3::ZERO);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 0);
        scene.update(1.);
        assert!(scene.objects[&id]
            .transform
            .position
            .abs_diff_eq(Vec3::new(0.25, 0., 0.), 0.00001));
        scene.update(1.);
        assert_eq!(
            scene.objects[&id].transform.position,
            Vec3::new(0.5, 0., 0.)
        );
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 8);
        assert_eq!(scene.pet_sprites(head, viewer), 1);
    }
    #[test]
    fn sprite_start_pause_must_leave_time_for_both_travel_legs() {
        let mut c = config();
        c.start_pause = 0.5;
        assert!(c.valid());
        c.start_pause = c.duration;
        assert!(!c.valid());
        c.start_pause = f32::NAN;
        assert!(!c.valid());
        c.start_pause = -0.5;
        assert!(!c.valid());
    }
    #[test]
    fn sprite_curved_travel_control_is_optional_and_finite() {
        let mut c = config();
        assert_eq!(c.travel_sample(0.25).0, Vec3::from_array(c.travel) * 0.25);
        c.travel_control = Some([f32::NAN, 0., 0.]);
        assert!(!c.valid());
        c.travel_control = Some([0., f32::INFINITY, 0.]);
        assert!(!c.valid());
        c.travel_control = Some([0., 0., 0.]);
        assert!(c.valid());
    }
    fn mixed_bands() -> Vec<SpriteAtlasBand> {
        let band =
            |first_frame, frame_count, first_tile, repeat, large, tile_map| SpriteAtlasBand {
                first_frame,
                frame_count,
                first_tile,
                repeat: Some(repeat),
                columns: if large { 4 } else { 8 },
                origin: if large { [0., 0.5] } else { [0., 0.] },
                tile_size: if large { [0.25, 0.25] } else { [0.125, 0.125] },
                tile_map,
            };
        vec![
            band(0, 16, 0, 16, false, None),
            band(16, 8, 0, 1, true, None),
            band(24, 8, 0, 8, true, Some(vec![1, 1, 2, 2, 3, 4, 4, 5])),
            band(32, 8, 16, 4, false, None),
            band(40, 8, 20, 4, false, None),
            band(48, 8, 24, 4, false, None),
            band(56, 8, 28, 4, false, None),
        ]
    }
    #[test]
    fn sprite_mixed_atlas_bands_map_all_semantic_frames_without_texture_growth() {
        let layout = SpriteAtlasLayout::pack(&mixed_bands(), 64).unwrap();
        assert_eq!(std::mem::size_of_val(&layout.bands), 384);
        for frame in 0..64 {
            let band = layout.bands[..7]
                .iter()
                .find(|b| frame >= b.frames[0] && frame < b.frames[0] + b.frames[1])
                .unwrap();
            let uv = band.uv_rect(frame);
            assert!(uv[0] >= 0. && uv[1] >= 0. && uv[0] + uv[2] <= 1. && uv[1] + uv[3] <= 1.);
            if (16..32).contains(&frame) {
                assert_eq!(&uv[2..], &[0.25, 0.25]);
            } else {
                assert_eq!(&uv[2..], &[0.125, 0.125]);
            }
        }
        assert_eq!(layout.bands[1].uv_rect(16), layout.bands[1].uv_rect(23));
        assert_eq!(layout.bands[2].uv_rect(24), [0.25, 0.5, 0.25, 0.25]);
        assert_eq!(layout.bands[2].uv_rect(31), [0.25, 0.75, 0.25, 0.25]);
        assert_eq!(layout.bands[3].uv_rect(32), layout.bands[3].uv_rect(36));
        let mut invalid = mixed_bands();
        invalid[1].first_frame = 15;
        assert!(SpriteAtlasLayout::pack(&invalid, 64).is_none());
        invalid = mixed_bands();
        invalid[2].tile_map = Some(vec![16]);
        assert!(SpriteAtlasLayout::pack(&invalid, 64).is_none());
        invalid = mixed_bands();
        invalid[1].first_tile = u32::MAX;
        assert!(SpriteAtlasLayout::pack(&invalid, 64).is_none());
    }
    #[test]
    fn sprite_atlas_pattern_high_nibbles_roundtrip_as_u32() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.columns = 8;
        c.rows = 8;
        c.atlas_bands = vec![SpriteAtlasBand {
            first_frame: 0,
            frame_count: 8,
            columns: 4,
            origin: [0., 0.],
            tile_size: [0.25, 0.25],
            first_tile: 0,
            repeat: None,
            tile_map: Some(vec![15, 14, 13, 12, 11, 10, 9, 8]),
        }];
        assert!(source.configure_sprite(id, c));
        let layout = source.objects[&id].sprite.unwrap().atlas_layout.unwrap();
        assert_eq!(layout.bands[0].tiles[2], 0x89abcdef);
        assert_eq!(layout.bands[0].uv_rect(0), [0.75, 0.75, 0.25, 0.25]);
        assert_eq!(layout.bands[0].uv_rect(7), [0., 0.5, 0.25, 0.25]);
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        mirror.bind_sprite(preload, 77);
        mirror
            .apply_wire_publication(&WirePublisher::default().capture(&mut source).unwrap(), &[])
            .unwrap();
        assert_eq!(
            mirror.objects[&id].sprite.unwrap().atlas_layout,
            Some(layout)
        );
    }
    fn directional() -> SpriteDirectionalView {
        SpriteDirectionalView {
            frames: 8,
            front_start_frame: 32,
            front_oblique_start_frame: 40,
            side_start_frame: 0,
            rear_oblique_start_frame: 48,
            rear_start_frame: 56,
        }
    }
    #[test]
    fn sprite_directional_heading_tracks_full_tangent_reverses_and_roundtrips() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.columns = 8;
        c.rows = 8;
        c.duration = 6.;
        c.endpoint_pause = 2.;
        c.travel = [1., 0., 2.];
        c.travel_control = Some([2., 0., 0.]);
        c.flip_with_travel = true;
        c.directional_view = Some(directional());
        c.atlas_bands = mixed_bands();
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        assert!(
            (source.objects[&id].transform.rotation.y - std::f32::consts::FRAC_PI_2).abs()
                < 0.00001
        );
        source.update(1.);
        let outgoing = source.objects[&id].transform;
        assert!((outgoing.rotation.y - 1_f32.atan2(2.)).abs() < 0.00001);
        assert!(outgoing.scale.x > 0.);
        source.update(1.);
        let incoming = source.objects[&id].transform.rotation;
        source.update(1.);
        assert_eq!(source.objects[&id].transform.rotation, incoming);
        source.update(2.);
        let returning = source.objects[&id].transform;
        assert!(returning.position.abs_diff_eq(outgoing.position, 0.00001));
        let forward = |yaw: f32| Vec3::new(yaw.sin(), 0., yaw.cos());
        assert!(forward(returning.rotation.y).abs_diff_eq(-forward(outgoing.rotation.y), 0.00001));
        assert!(returning.scale.x > 0.);
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        mirror.bind_sprite(preload, 77);
        mirror
            .apply_wire_publication(&WirePublisher::default().capture(&mut source).unwrap(), &[])
            .unwrap();
        assert_eq!(mirror.objects[&id].sprite, source.objects[&id].sprite);
        assert_eq!(mirror.objects[&id].transform.rotation, returning.rotation);
        let presented = mirror.objects[&id].sprite.unwrap();
        assert_eq!(
            presented.direction_uniform(),
            [[8., 32., 40., 0.], [48., 56., 0., 0.]]
        );
        assert_eq!(
            presented.atlas_uniform(),
            SpriteAtlasLayout::pack(&mixed_bands(), 64).unwrap().bands
        );
        assert_eq!(presented.uniform(), [8., 8., presented.frame as f32, 1.]);
        mirror.update(1.);
        assert_eq!(mirror.objects[&id].transform.rotation, returning.rotation);
    }
    #[test]
    fn sprite_directional_bands_require_space_and_preserve_legacy_defaults() {
        let mut c = config();
        c.columns = 8;
        c.rows = 8;
        c.directional_view = Some(directional());
        assert!(c.valid());
        c.rows = 7;
        assert!(!c.valid());
        c.rows = 8;
        c.directional_view.as_mut().unwrap().front_start_frame = 1;
        assert!(!c.valid());
        c.directional_view = Some(directional());
        c.idle_frames = 8;
        c.idle_start_frame = 4;
        assert!(!c.valid());
    }
    #[test]
    fn sprite_authoritative_clock_cooldown_and_lifetime() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        assert!(scene.configure_sprite(id, config()));
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        assert!(scene.trigger_sprite(id));
        scene.update(0.5);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 3);
        assert!(scene.objects[&id].transform.position.x > 0.);
        assert!(!scene.trigger_sprite(id));
        scene.update(1.5);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        assert!(!scene.trigger_sprite(id));
        scene.update(3.);
        assert!(scene.trigger_sprite(id));
        assert!(scene.remove_object(id));
        assert!(scene.sprites.is_empty());
        assert!(scene.sprite_ids.is_empty());
    }
    #[test]
    fn sprite_semantic_binding_and_wire_mirror_never_run_local_clock() {
        let mut source = SceneManager::default();
        source.add_object(
            MeshKind::Cube,
            Transform::new(Vec3::ZERO, Vec3::ZERO, Vec3::ONE),
        );
        let id = source.add_model(asset()).unwrap();
        assert!(source.configure_sprite(id, config()));
        assert!(source.trigger_sprite(id));
        source.update(0.5);
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        assert_ne!(preload, id);
        assert!(mirror.bind_sprite(preload, 77));
        assert!(!mirror.object_exists(preload));
        let mut wire = WirePublisher::default();
        mirror
            .apply_wire_publication(&wire.capture(&mut source).unwrap(), &[])
            .unwrap();
        let state = mirror.objects[&id].sprite;
        assert!(mirror.model(id).is_some());
        mirror.update(10.);
        assert_eq!(mirror.objects[&id].sprite, state);
        assert!(!mirror.trigger_sprite(id));
        let mut publisher = RenderPublisher::default();
        mirror.apply_render_publication(publisher.capture(&mut source), &[]);
        mirror.update(10.);
        assert_eq!(mirror.objects[&id].sprite, state);
    }
    #[test]
    fn sprite_direction_pause_and_wire_transform_are_authoritative() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.duration = 6.;
        c.ambient_interval = 0.;
        c.endpoint_pause = 2.;
        c.flip_with_travel = true;
        c.travel = [-2., 0., 1.];
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        assert_eq!(source.objects[&id].transform.scale.x, -1.);
        source.update(2.);
        let at_endpoint = source.objects[&id].transform;
        let pose = source.objects[&id].sprite.unwrap().frame;
        assert!((at_endpoint.position - Vec3::new(-2., 0., 1.)).length() < 1e-5);
        source.update(1.);
        assert_eq!(source.objects[&id].transform.position, at_endpoint.position);
        assert_eq!(source.objects[&id].transform.scale, at_endpoint.scale);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, pose);
        source.update(1.5);
        assert_eq!(source.objects[&id].transform.scale.x, 1.);
        assert!(source.objects[&id].transform.position.x > -2.);
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        assert!(mirror.bind_sprite(preload, 77));
        let mut wire = WirePublisher::default();
        mirror
            .apply_wire_publication(&wire.capture(&mut source).unwrap(), &[])
            .unwrap();
        assert_eq!(
            mirror.objects[&id].transform.position,
            source.objects[&id].transform.position
        );
        assert_eq!(
            mirror.objects[&id].transform.scale,
            source.objects[&id].transform.scale
        );
        source.update(1.5);
        source.update(5.);
        assert!(source.trigger_sprite(id));
        source.update(0.5);
        mirror.set_mirror_interpolation(100., 10.).unwrap();
        mirror
            .apply_wire_publication(&wire.capture(&mut source).unwrap(), &[])
            .unwrap();
        assert_eq!(mirror.objects[&id].transform.scale.x, -1.);
        mirror.update(0.05);
        let displayed =
            glam::Mat4::from_cols_array_2d(&mirror.objects[&id].render_data.instance.model);
        assert!(displayed.determinant() < 0.);
        assert!((displayed.x_axis.truncate().length() - 1.).abs() < 1e-5);
        let mirrored = mirror.objects[&id].transform;
        mirror.update(20.);
        assert_eq!(mirror.objects[&id].transform.position, mirrored.position);
        assert_eq!(mirror.objects[&id].transform.scale, mirrored.scale);
    }
    #[test]
    fn sprite_endpoint_idle_animation_loops_and_returns_to_walk() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.rows = 4;
        c.fps = 4.;
        c.duration = 6.;
        c.endpoint_pause = 2.;
        c.idle_start_frame = 8;
        c.idle_frames = 8;
        c.ambient_interval = 0.;
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        source.update(1.75);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 7);
        source.update(0.25);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 8);
        let endpoint = source.objects[&id].transform.position;
        for (dt, frame) in [(0.25, 9), (1.5, 15), (0.25, 8)] {
            source.update(dt);
            assert_eq!(source.objects[&id].sprite.unwrap().frame, frame);
            assert_eq!(source.objects[&id].transform.position, endpoint);
        }
        // The existing presentation frame transports idle poses without a codec change.
        let mut mirror = SceneManager::default();
        let preload = mirror.add_model(asset()).unwrap();
        assert!(mirror.bind_sprite(preload, 77));
        let mut wire = WirePublisher::default();
        mirror
            .apply_wire_publication(&wire.capture(&mut source).unwrap(), &[])
            .unwrap();
        mirror.update(10.);
        assert_eq!(mirror.objects[&id].sprite, source.objects[&id].sprite);
        source.update(0.25);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 1);
        assert!(source.objects[&id].transform.position.x < endpoint.x);
    }
    #[test]
    fn sprite_idle_fps_is_independent_of_walking_speed() {
        let mut source = SceneManager::default();
        let id = source.add_model(asset()).unwrap();
        let mut c = config();
        c.rows = 4;
        c.duration = 22.;
        c.endpoint_pause = 12.;
        c.idle_start_frame = 8;
        c.idle_frames = 8;
        c.idle_fps = Some(3.);
        c.ambient_interval = 0.;
        assert!(source.configure_sprite(id, c));
        assert!(source.trigger_sprite(id));
        source.update(0.5);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 3);
        source.update(4.5);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 8);
        source.update(0.5);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 9);
        source.update(0.5);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 11);
        source.update(11.25);
        assert_eq!(source.objects[&id].sprite.unwrap().frame, 7);
    }
    fn petting_scene() -> (SceneManager, u32) {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.rows = 4;
        c.duration = 6.;
        c.endpoint_pause = 2.;
        c.idle_start_frame = 8;
        c.idle_frames = 8;
        c.idle_fps = Some(3.);
        c.ambient_interval = 0.;
        c.pettable = true;
        assert!(scene.configure_sprite(id, c));
        (scene, id)
    }
    #[test]
    fn sprite_earn_fetch_requires_fresh_round_contact_then_delivers_once_automatically() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let contact = endpoint - Vec3::Z * 0.55;
        for _ in 0..40 {
            scene.interact_sprites(Some(viewer), [Some(contact), None]);
            scene.update(0.05);
        }
        assert!(
            scene.pet_drink_carrier(Some(viewer)).is_none(),
            "out-of-round cuddles have no free can"
        );
        assert!(!scene.sprites[&id].fetch_pending);
        let elapsed = scene.sprites[&id].elapsed;
        scene.reset_pet_gifts();
        scene.set_pet_gift_enabled(true);
        assert_eq!(
            scene.sprites[&id].elapsed, elapsed,
            "round start preserves physical visit"
        );
        scene.update(0.01);
        assert!(
            !scene.sprites[&id].fetch_pending,
            "old contact cannot earn on the first live tick"
        );
        for _ in 0..20 {
            scene.interact_sprites(Some(viewer), [Some(contact), None]);
            scene.update(0.05);
        }
        assert!(
            !scene.sprites[&id].fetch_pending,
            "1.0 live contact seconds is insufficient"
        );
        for _ in 0..14 {
            scene.interact_sprites(Some(viewer), [Some(contact), None]);
            scene.update(0.05);
        }
        assert!(scene.sprites[&id].fetch_pending);
        assert!(
            scene.pet_drink_carrier(Some(viewer)).is_none(),
            "earning starts an empty return, never instant can spawn"
        );
        let first = scene.sprites[&id].visit_id;
        scene.interact_sprites(Some(viewer), [None, None]);
        for _ in 0..100 {
            scene.update(0.05);
            if scene.sprites[&id].gift_visit {
                break;
            }
        }
        assert!(
            scene.sprites[&id].gift_visit,
            "earned fetch bypasses ambient interval/cooldown after reaching cabinet"
        );
        assert_ne!(scene.sprites[&id].visit_id, first);
        assert!(
            !scene.sprites[&id].fetch_pending,
            "one earned ticket buys one delivery"
        );
        assert!(scene.pet_drink_carrier(Some(viewer)).is_some());
        let pose = scene.objects[&id].transform.position;
        let elapsed = scene.sprites[&id].elapsed;
        scene.reset_pet_gifts();
        assert!(scene.pet_drink_carrier(Some(viewer)).is_none());
        assert_eq!(scene.objects[&id].transform.position, pose);
        assert_eq!(scene.sprites[&id].elapsed, elapsed);
    }
    #[test]
    fn sprite_drink_carrier_follows_walking_heading_and_has_distinct_visit_identity() {
        let (mut scene, id) = petting_scene();
        scene.set_pet_gift_enabled(true);
        scene.sprites.get_mut(&id).unwrap().fetch_pending = true;
        assert!(scene.pet_drink_carrier(None).is_none());
        scene.trigger_sprite(id);
        scene.update(0.5);
        let before = scene
            .pet_drink_carrier(Some(Vec3::new(2., 1., 0.)))
            .unwrap();
        let turned = scene
            .pet_drink_carrier(Some(Vec3::new(-2., 1., 0.)))
            .unwrap();
        assert!(
            before.lip.distance(turned.lip) > 0.01,
            "opposite viewer sides select mirrored artwork mouth anchors"
        );
        let unchanged = scene
            .pet_drink_carrier(Some(Vec3::new(2., 1., 0.)))
            .unwrap();
        assert_eq!(
            before.lip, unchanged.lip,
            "view orientation alone cannot rotate the can"
        );
        assert!((before.rotation * Vec3::Y).y.abs() < 1e-5);
        assert!(!before.available);
        scene.update(0.5);
        let moved = scene.pet_drink_carrier(None).unwrap();
        assert!(moved.lip.distance(before.lip) > 0.01);
        scene.update(1.);
        let object = &scene.objects[&id];
        let viewer = object.transform.position + Vec3::Z;
        let head = object.transform.position + Vec3::Y * (0.55 * object.transform.scale.y.abs());
        assert!(
            scene.pet_drink_carrier(Some(viewer)).unwrap().available,
            "earned delivery offers the can on arrival"
        );
        assert!(scene.pet_sprite(id, head, viewer));
        // Reconfiguring the same stable scene handle still starts a fresh visit.
        let config = scene.sprites[&id].config.clone();
        scene.configure_sprite(id, config);
        scene.trigger_sprite(id);
        assert!(
            scene.pet_drink_carrier(None).is_none(),
            "ordinary subsequent visit has no free can"
        );
        assert_ne!(scene.sprites[&id].visit_id, before.visit.1);
    }
    #[test]
    fn sprite_drink_front_mouth_matches_pitched_billboard_instead_of_world_chest() {
        let (mut scene, id) = petting_scene();
        scene.set_pet_gift_enabled(true);
        scene.sprites.get_mut(&id).unwrap().fetch_pending = true;
        let mut config = scene.sprites[&id].config.clone();
        config.columns = 8;
        config.rows = 9;
        config.directional_view = Some(directional());
        assert!(scene.configure_sprite(id, config));
        scene.trigger_sprite(id);
        scene.update(0.5);
        let object = &scene.objects[&id];
        let origin = object.transform.position;
        let yaw = object.transform.rotation.y;
        let heading = Vec3::new(yaw.sin(), 0., yaw.cos());
        let right = Vec3::new(heading.z, 0., -heading.x);
        for height in [0.2, 1., 3.] {
            let viewer = origin + heading * 2. + Vec3::Y * height;
            let pitch = (height / 2_f32)
                .atan()
                .clamp(-65_f32.to_radians(), 65_f32.to_radians());
            let facing = heading * pitch.cos() + Vec3::Y * pitch.sin();
            let mouth = origin
                + right * (0.10 * object.transform.scale.x.abs())
                + facing.cross(right) * (0.525 * object.transform.scale.y.abs());
            let carried = scene.pet_drink_carrier(Some(viewer)).unwrap();
            let axis = carried.rotation * Vec3::Y;
            let middle = carried.lip - axis * 0.1125;
            assert!(axis.dot(right) > 0.9999, "can lies across the muzzle");
            assert!(
                middle.distance(mouth - facing.cross(right) * 0.025 + facing * 0.039) < 1e-5,
                "painted jaw meets the cylinder"
            );
        }
    }
    #[test]
    fn sprite_carry_art_flag_roundtrips_without_changing_semantic_frame() {
        let (mut scene, id) = petting_scene();
        scene.set_pet_gift_enabled(true);
        scene.sprites.get_mut(&id).unwrap().fetch_pending = true;
        let mut config = scene.sprites[&id].config.clone();
        config.columns = 8;
        config.rows = 9;
        config.atlas_bands = vec![
            SpriteAtlasBand {
                first_frame: 0,
                frame_count: 64,
                columns: 8,
                origin: [0., 0.],
                tile_size: [0.125, 1. / 9.],
                repeat: None,
                first_tile: 0,
                tile_map: None,
            },
            SpriteAtlasBand {
                first_frame: 64,
                frame_count: 8,
                columns: 8,
                origin: [0., 8. / 9.],
                tile_size: [0.125, 1. / 9.],
                repeat: None,
                first_tile: 0,
                tile_map: None,
            },
        ];
        assert!(scene.configure_sprite(id, config));
        scene.trigger_sprite(id);
        let visit = scene.pet_drink_carrier(None).unwrap().visit;
        scene.set_pet_drink_visual(Some(visit));
        let visual = scene.objects[&id].sprite.unwrap();
        assert_eq!(visual.frame, 0);
        assert_eq!(visual.atlas_layout.unwrap().bands[0].tiles[3], 0x0040_0000);
        assert!(visual.atlas_layout.unwrap().valid(72));
        let bytes = crate::scene::wire::WirePublisher::default()
            .capture(&mut scene)
            .unwrap();
        let mut mirror = SceneManager::default();
        mirror.apply_wire_publication(&bytes, &[]).unwrap();
        assert_eq!(
            mirror.objects[&id]
                .sprite
                .unwrap()
                .atlas_layout
                .unwrap()
                .bands[0]
                .tiles[3],
            0x0040_0000
        );
        scene.set_pet_drink_visual(None);
        assert_eq!(
            scene.objects[&id]
                .sprite
                .unwrap()
                .atlas_layout
                .unwrap()
                .bands[0]
                .tiles[3],
            0
        );
    }
    #[test]
    fn sprite_pet_hand_intent_requires_arrival_gaze_and_radius_hysteresis() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let object = &scene.objects[&id];
        let target = object.transform.position + Vec3::Y * (0.55 * object.transform.scale.y.abs());
        let head = glam::Mat4::from_translation(target + Vec3::Z);
        // Vertical eye offset changes the billboard's up vector; a horizontal
        // viewer at object height makes the head's reference basis exact.
        let head = glam::Mat4::from_translation(
            head.w_axis.truncate().with_y(object.transform.position.y),
        );
        let facing = (head.w_axis.truncate() - object.transform.position).normalize();
        let target = object.transform.position
            + facing.cross(Vec3::X) * (0.55 * object.transform.scale.y.abs());
        let head = glam::Mat4::from_rotation_translation(
            glam::Quat::from_rotation_arc(
                Vec3::NEG_Z,
                (target - head.w_axis.truncate()).normalize(),
            ),
            head.w_axis.truncate(),
        );
        assert!(scene.pet_hand_intent(head, target, false));
        assert!(!scene.pet_hand_intent(head, target + Vec3::X * 0.65, false));
        assert!(scene.pet_hand_intent(head, target + Vec3::X * 0.65, true));
        assert!(!scene.pet_hand_intent(head, target + Vec3::X * 0.80, true));
        assert!(!scene.pet_hand_intent(
            head * glam::Mat4::from_rotation_y(std::f32::consts::PI),
            target,
            true
        ));
        scene
            .objects
            .get_mut(&id)
            .unwrap()
            .sprite
            .as_mut()
            .unwrap()
            .visible = false;
        assert!(!scene.pet_hand_intent(head, target, true));
    }
    #[test]
    fn sprite_pet_top_artwork_back_contact_matches_heading_from_both_view_sides() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let object = scene.objects.get_mut(&id).unwrap();
        object.transform.rotation.y = 0.7;
        let visual = object.sprite.as_mut().unwrap();
        visual.directional_view = Some(directional());
        visual.elevated_view = Some(SpriteElevatedView {
            start_frame: 16,
            elevation: 40.,
            max_front_pitch: 25.,
            pitch: 65.,
        });
        visual.frame = 8;
        let origin = object.transform.position;
        let scale = object.transform.scale.y.abs();
        let right = Vec3::new(0.7_f32.cos(), 0., -0.7_f32.sin());
        let pitch = 65_f32.to_radians();
        let facing = right.cross(Vec3::Y) * pitch.cos() + Vec3::Y * pitch.sin();
        let back = origin + facing.cross(right) * (0.22 * scale);
        for x in [-0.3, 0.3] {
            let viewer = origin + Vec3::new(x, 1., 0.4);
            let head = glam::Mat4::from_rotation_translation(
                glam::Quat::from_rotation_arc(Vec3::NEG_Z, (back - viewer).normalize()),
                viewer,
            );
            let approaching = back + (viewer - back).normalize() * 0.45;
            assert!(scene.pet_hand_intent(head, approaching, false));
            assert!(!scene.pet_sprite(id, approaching, viewer));
            assert!(scene.pet_sprite(id, back, viewer));
        }
    }
    #[test]
    fn sprite_pet_rejects_hidden_moving_missed_and_nonfinite_contacts() {
        let (mut scene, id) = petting_scene();
        let viewer = Vec3::new(0.5, 2., 0.);
        let contact = Vec3::new(0.5, 0., -0.55);
        assert!(!scene.pet_sprite(id, contact, viewer));
        assert!(scene.trigger_sprite(id));
        scene.update(1.);
        assert!(!scene.pet_sprite(id, contact, viewer));
        scene.update(1.);
        assert!(!scene.pet_sprite(id, contact + Vec3::X, viewer));
        assert!(!scene.pet_sprite(id, Vec3::NAN, viewer));
        assert!(!scene.pet_sprite(id, contact, Vec3::splat(f32::INFINITY)));
        assert!(!scene.pet_sprite(u32::MAX, contact, viewer));
        assert_eq!(scene.pet_sprites(contact + Vec3::X, viewer), 0);
        assert!(scene.pet_sprite(id, contact, viewer));
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 11);
        scene.sprites.get_mut(&id).unwrap().config.pettable = false;
        assert!(!scene.pet_sprite(id, contact, viewer));
        assert_eq!(scene.pet_sprites(contact, viewer), 0);
    }
    #[test]
    fn sprite_arrival_pose_lead_reaches_endpoint_and_enables_contact_together() {
        let (mut scene, id) = petting_scene();
        scene.sprites.get_mut(&id).unwrap().config.arrival_pose_lead = 0.5;
        scene.trigger_sprite(id);
        scene.update(1.4);
        let approaching = scene.objects[&id].transform.position;
        assert!(scene.objects[&id].sprite.unwrap().frame < 8);
        assert!(!scene.pet_sprite(id, approaching - Vec3::Z * 0.55, approaching + Vec3::Y * 2.));
        scene.update(0.1);
        let endpoint = scene.objects[&id].transform.position;
        assert!(approaching.distance(endpoint) > 0.001);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 8);
        assert!(scene.pet_sprite(id, endpoint - Vec3::Z * 0.55, endpoint + Vec3::Y * 2.));
        let mut c = config();
        c.arrival_pose_lead = -0.1;
        assert!(!c.valid());
        c.arrival_pose_lead = c.duration;
        assert!(!c.valid());
    }
    #[test]
    fn sprite_pet_waits_calmly_then_bounds_cuddle_and_farewell() {
        let (mut scene, id) = petting_scene();
        assert!(scene.trigger_sprite(id));
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let contact = endpoint - Vec3::Z * 0.55;
        scene.update(0.5);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 8);
        assert_eq!(scene.objects[&id].transform.position, endpoint);
        assert!(scene.pet_sprite(id, contact, viewer));
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 11);
        // Repeated strokes neither restart nor extend the wall-time bound.
        for _ in 0..90 {
            assert_eq!(
                scene.interact_sprites(Some(viewer), [Some(contact), None]),
                1
            );
            scene.update(0.1);
            assert_eq!(scene.objects[&id].transform.position, endpoint);
        }
        scene.update(1.1);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_some());
        assert!(!scene.pet_sprite(id, contact, viewer));
        scene.update(0.5);
        assert_eq!(scene.objects[&id].transform.position, endpoint);
        scene.update(0.6);
        assert!(scene.objects[&id].transform.position.x < endpoint.x);
        scene.update(2.);
        assert!(!scene.objects[&id].sprite.unwrap().visible);
        assert!(scene.trigger_sprite(id));
        assert!(scene.sprites[&id].pet_visit.is_none());
        scene.update(2.);
        assert_eq!(scene.objects[&id].sprite.unwrap().frame, 8);
    }
    #[test]
    fn sprite_pet_after_drink_pickup_requires_fresh_contact_before_standup_farewell() {
        let (mut scene, id) = petting_scene();
        scene.set_pet_gift_enabled(true);
        scene.sprites.get_mut(&id).unwrap().fetch_pending = true;
        scene.trigger_sprite(id);
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let contact = endpoint - Vec3::Z * 0.55;
        // Earn the drink first; none of this contact may count again after pickup.
        for _ in 0..34 {
            assert_eq!(
                scene.interact_sprites(Some(viewer), [Some(contact), None]),
                1
            );
            scene.update(0.05);
        }
        let carrier = scene.pet_drink_carrier(Some(viewer)).unwrap();
        assert!(carrier.available);
        scene.pet_after_drink_pickup(carrier.visit);
        assert_eq!(
            scene.sprites[&id]
                .pet_visit
                .as_ref()
                .unwrap()
                .contact_seconds,
            0.
        );
        for _ in 0..20 {
            assert_eq!(
                scene.interact_sprites(Some(viewer), [Some(contact), None]),
                1
            );
            scene.update(0.05);
        }
        let standing = viewer + Vec3::Y * 0.3;
        let withdrawn = contact + Vec3::X;
        scene.interact_sprites(Some(standing), [Some(withdrawn), None]);
        // Include contact-release grace, then a full tracked withdrawal interval.
        scene.update(0.2);
        scene.update(0.6);
        assert!(
            scene.sprites[&id]
                .pet_visit
                .as_ref()
                .unwrap()
                .farewell
                .is_none(),
            "standing after only 1.0 fresh contact seconds cannot end the second cuddle"
        );
        assert_eq!(scene.objects[&id].transform.position, endpoint);
        for _ in 0..14 {
            assert_eq!(
                scene.interact_sprites(Some(viewer), [Some(contact), None]),
                1
            );
            scene.update(0.05);
        }
        scene.interact_sprites(Some(standing), [Some(withdrawn), None]);
        scene.update(0.2);
        scene.update(0.6);
        assert!(
            scene.sprites[&id]
                .pet_visit
                .as_ref()
                .unwrap()
                .farewell
                .is_some(),
            "standing after another 0.7 contact seconds may begin farewell"
        );
        assert_eq!(scene.objects[&id].transform.position, endpoint);
    }
    #[test]
    fn sprite_pet_standup_requires_withdrawal_and_valid_tracking() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let contact = endpoint - Vec3::Z * 0.55;
        scene.interact_sprites(Some(viewer), [Some(contact), None]);
        // Tracking loss cannot stand in for withdrawal or a raised head.
        scene.interact_sprites(None, [None, None]);
        scene.update(1.);
        scene.update(1.);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
        // Head rise while a grip remains in contact does not finish the visit.
        let standing = viewer + Vec3::Y * 0.3;
        scene.interact_sprites(Some(standing), [Some(contact), None]);
        scene.update(0.1);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
        // Raised head alone, with missing hand tracking, also cannot finish early.
        scene.interact_sprites(Some(standing), [None, None]);
        scene.update(1.);
        scene.update(1.);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
        scene.interact_sprites(Some(standing), [Some(contact + Vec3::X), None]);
        scene.update(0.1);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
        scene.update(0.6);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_some());
        assert_eq!(scene.objects[&id].transform.position, endpoint);
        scene.update(1.1);
        assert!(scene.objects[&id].transform.position.x < endpoint.x);
    }
    #[test]
    fn sprite_pet_withdrawal_tracks_actual_hand_and_allows_right_hand_handoff() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let standing = viewer + Vec3::Y * 0.3;
        let contact = endpoint - Vec3::Z * 0.55;
        let away = contact + Vec3::X;
        scene.interact_sprites(Some(viewer), [Some(contact), Some(away)]);
        assert_eq!(
            scene.sprites[&id].pet_visit.as_ref().unwrap().contact_hand,
            Some(0)
        );
        // Losing the left petting hand while the right remains visible is loss,
        // never evidence that the left hand withdrew.
        scene.interact_sprites(Some(standing), [None, Some(away)]);
        scene.update(1.);
        scene.update(1.);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
        // A right-only controller can take over by actually contacting the cat.
        scene.interact_sprites(Some(viewer), [None, Some(contact)]);
        assert_eq!(
            scene.sprites[&id].pet_visit.as_ref().unwrap().contact_hand,
            Some(1)
        );
        scene.update(0.1);
        scene.interact_sprites(Some(standing), [None, Some(away)]);
        scene.update(0.3);
        scene.update(0.7);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_some());
    }
    #[test]
    fn sprite_pet_unindexed_contact_does_not_enable_early_withdrawal() {
        let (mut scene, id) = petting_scene();
        scene.trigger_sprite(id);
        scene.update(2.);
        let endpoint = scene.objects[&id].transform.position;
        let viewer = endpoint + Vec3::Y * 2.;
        let contact = endpoint - Vec3::Z * 0.55;
        assert!(scene.pet_sprite(id, contact, viewer));
        scene.interact_sprites(
            Some(viewer + Vec3::Y * 0.3),
            [Some(contact + Vec3::X), None],
        );
        scene.update(1.);
        scene.update(1.);
        assert!(scene.sprites[&id]
            .pet_visit
            .as_ref()
            .unwrap()
            .farewell
            .is_none());
    }
    #[test]
    fn sprite_idle_frame_defaults_and_checked_ranges() {
        let legacy = r#"{"key":77,"columns":4,"rows":2,"frames":8,"fps":6,
            "duration":2,"ambientInterval":4,"cooldown":5,"travel":[0.5,0,0]}"#;
        let decoded: SpriteConfig = serde_json::from_str(legacy).unwrap();
        assert_eq!(decoded.travel_control, None);
        assert_eq!(decoded.start_pause, 0.);
        assert_eq!((decoded.idle_start_frame, decoded.idle_frames), (0, 0));
        assert_eq!(decoded.idle_fps, None);
        assert!(!decoded.pettable);
        assert!(decoded.valid());
        let explicit: SpriteConfig = serde_json::from_str(
            r#"{"key":77,"columns":4,"rows":4,"frames":8,"fps":6,
            "duration":22,"endpointPause":12,"ambientInterval":4,"cooldown":5,
            "travel":[0.5,0,0],"idleStartFrame":8,"idleFrames":8,"idleFps":3}"#,
        )
        .unwrap();
        assert_eq!(
            (
                explicit.idle_start_frame,
                explicit.idle_frames,
                explicit.idle_fps
            ),
            (8, 8, Some(3.))
        );
        assert!(explicit.valid());
        for fps in [0., -1., f32::NAN, f32::INFINITY] {
            let mut c = config();
            c.idle_fps = Some(fps);
            assert!(!c.valid(), "invalid idle fps {fps}");
        }
        for (start, count, valid) in [
            (0, 0, true),
            (0, 8, true),
            (7, 1, true),
            (8, 0, false),
            (1, 0, false),
            (7, 2, false),
            (8, 1, false),
            (u32::MAX, 1, false),
            (1, u32::MAX, false),
        ] {
            let mut c = config();
            c.idle_start_frame = start;
            c.idle_frames = count;
            assert_eq!(c.valid(), valid, "idle range {start}+{count}");
        }
    }
    #[test]
    fn sprite_defaults_preserve_facing_and_reject_invalid_pause() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        assert!(scene.configure_sprite(id, config()));
        assert!(scene.trigger_sprite(id));
        scene.update(1.5);
        assert_eq!(scene.objects[&id].transform.scale.x, 1.);
        for pause in [-1., 2., f32::NAN] {
            let mut c = config();
            c.endpoint_pause = pause;
            assert!(!scene.configure_sprite(id, c));
        }
    }
    #[test]
    fn sprite_rejects_invalid_config_and_accepts_zero_travel() {
        let mut scene = SceneManager::default();
        let id = scene.add_model(asset()).unwrap();
        let mut c = config();
        c.columns = 0;
        assert!(!scene.configure_sprite(id, c));
        let mut c = config();
        c.travel = [0.; 3];
        assert!(scene.configure_sprite(id, c));
    }
}
