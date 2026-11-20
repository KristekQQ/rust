//! Spatial arcade initials keyboard. Geometry exists only during a qualifying entry.
use glam::{EulerRot, Mat4, Quat, Vec3};
use webgpu_wasm::scene::{RenderOptions, SceneManager};

const COLS: usize = 7;
const Z: f32 = -1.18;
const TOP: f32 = 1.55;
const DX: f32 = 0.15;
const DY: f32 = 0.112;
const DELETE: usize = 26;
const ACCEPT: usize = 27;

pub(super) struct Keyboard {
    keys: Vec<u32>,
    pointers: [u32; 2],
    display: u32,
    hint: u32,
    hover: [Option<usize>; 2],
    pub initials: String,
}

fn label(
    scene: &mut SceneManager,
    pos: Vec3,
    width: f32,
    height: f32,
    text: &str,
) -> Result<u32, String> {
    let id = scene.add_info_panel("debug-text", width, height)?;
    scene.set_object_position(id, pos);
    scene.set_render_options(
        id,
        RenderOptions {
            casts_shadow: false,
            receives_shadow: false,
            ..Default::default()
        },
    );
    scene.update_info_panel(id, &[("", text.to_owned())]);
    Ok(id)
}

impl Keyboard {
    pub fn open(scene: &mut SceneManager, pointers: [u32; 2]) -> Result<Self, String> {
        let mut keys = Vec::with_capacity(28);
        for i in 0..28 {
            let x = (i % COLS) as f32 * DX - 3. * DX;
            let y = TOP - (i / COLS) as f32 * DY;
            let text = match i {
                DELETE => "DEL".into(),
                ACCEPT => "OK".into(),
                _ => char::from(b'A' + i as u8).to_string(),
            };
            keys.push(label(
                scene,
                Vec3::new(x, y, Z),
                DX * 0.85,
                DY * 0.70,
                &text,
            )?);
        }
        let display = label(
            scene,
            Vec3::new(0., TOP + 0.15, Z),
            0.95,
            0.085,
            "INICIALY  _ _ _",
        )?;
        let hint = label(
            scene,
            Vec3::new(0., TOP - 4. * DY, Z),
            1.12,
            0.065,
            "NAMIR A STISKNI SPOUST   A PRESKOCIT",
        )?;
        for pointer in pointers {
            scene.set_emission(pointer, 1.5);
        }
        Ok(Self {
            pointers,
            keys,
            display,
            hint,
            hover: [None; 2],
            initials: String::new(),
        })
    }

    fn aim(scene: &mut SceneManager, pointer: u32, ray: Option<Mat4>) -> Option<usize> {
        let Some(ray) = ray.filter(|r| r.is_finite()) else {
            scene.set_object_position(pointer, super::game::HIDDEN);
            return None;
        };
        let start = ray.w_axis.truncate();
        let direction = -ray.z_axis.truncate().normalize_or_zero();
        if direction == Vec3::ZERO {
            scene.set_object_position(pointer, super::game::HIDDEN);
            return None;
        }
        let distance = (Z - start.z) / direction.z;
        let on_plane = direction.z < -0.001 && (0.001..=4.).contains(&distance);
        let length = if on_plane { distance } else { 1.2 };
        // The authored green pointer has local Z tips at -1 and +1.
        let (yaw, pitch, roll) =
            Quat::from_rotation_arc(Vec3::NEG_Z, direction).to_euler(EulerRot::YXZ);
        scene.set_object_transform(
            pointer,
            start + direction * (length * 0.5),
            Vec3::new(pitch, yaw, roll),
            Vec3::new(0.0035, 0.0035, length * 0.5),
        );
        if !on_plane {
            return None;
        }
        let point = start + direction * distance;
        let col = (point.x / DX + 3.).round();
        let row = ((TOP - point.y) / DY).round();
        if !(0. ..=6.).contains(&col) || !(0. ..=3.).contains(&row) {
            return None;
        }
        Some(row as usize * COLS + col as usize)
    }

    pub fn update(
        &mut self,
        scene: &mut SceneManager,
        rays: [Option<Mat4>; 2],
        pressed: [bool; 2],
    ) -> bool {
        let hover = [0, 1].map(|hand| Self::aim(scene, self.pointers[hand], rays[hand]));
        if hover != self.hover {
            for index in self.hover.into_iter().chain(hover).flatten() {
                let selected = hover.contains(&Some(index));
                scene.set_object_scale(
                    self.keys[index],
                    Vec3::splat(if selected { 1.35 } else { 1. }),
                );
                scene.set_emission(self.keys[index], if selected { 1.8 } else { 0. });
            }
            self.hover = hover;
        }
        for hand in 0..2 {
            if !pressed[hand] {
                continue;
            }
            match hover[hand] {
                Some(ACCEPT) if !self.initials.is_empty() => return true,
                Some(DELETE) => {
                    self.initials.pop();
                }
                Some(i) if i < 26 && self.initials.len() < 3 => {
                    self.initials.push(char::from(b'A' + i as u8))
                }
                _ => continue,
            }
            let slots = format!("{:_<3}", self.initials);
            scene.update_info_panel(self.display, &[("", format!("INICIALY  {slots}"))]);
        }
        false
    }

    pub fn close(self, scene: &mut SceneManager) {
        for pointer in self.pointers {
            scene.set_object_position(pointer, super::game::HIDDEN);
        }
        for id in self.keys.into_iter().chain([self.display, self.hint]) {
            scene.remove_object(id);
        }
    }
}
