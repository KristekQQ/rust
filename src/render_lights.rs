//! Shader program identity derived from the exact uploaded light uniforms.
//! Slot order is retained: shadow maps refer to the original light index.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct LightClass {
    active: u8,
    directional_first: bool,
}
impl LightClass {
    pub(crate) fn from_colors(colors: [[f32; 3]; 4], appearance: bool) -> Self {
        let active = colors.iter().enumerate().fold(0, |mask, (i, color)| {
            mask | (u8::from(color.iter().any(|&v| v != 0.0)) << i)
        });
        Self {
            active,
            directional_first: appearance && active & 1 != 0,
        }
    }
    pub(crate) fn specialize(self, source: &mut String) {
        let start = source
            .find("    for(var i=0u;i<LIGHT_COUNT;i++) {")
            .expect("light loop");
        let end = source[start..]
            .find("    color+=emission;")
            .expect("light loop end")
            + start;
        let loop_source = &source[start..end];
        let body_start = loop_source.find("        let delta=").expect("light body");
        let body_end = loop_source.rfind("    }").expect("light loop close");
        let body = &loop_source[body_start..body_end];
        let mut specialized = String::new();
        for i in 0..4 {
            if self.active & (1 << i) == 0 {
                continue;
            }
            // Separate lexical scope preserves the original per-light locals.
            specialized.push_str("    {\n");
            let direction = if i == 0 && self.directional_first {
                "let l=scene.appearance_sun.xyz;"
            } else {
                "let delta=scene.lights[i].position-v.world; let l=unit(delta);"
            };
            let body = body.replace("let delta=scene.lights[i].position-v.world; let l=select(unit(delta),scene.appearance_sun.xyz,scene.appearance_sky.w>0.5 && i==0u);", direction)
                // Naga suffixes repeated locals. `gl_1` is reserved in GLSL;
                // rename this original short local before emitting scoped copies.
                .replace("let gl=", "let geometry_light=")
                .replace("d*gv*gl*f", "d*gv*geometry_light*f")
                .replace("scene.lights[i]", &format!("scene.lights[{i}]"))
                .replace("i<u32(effects.params.x)", &format!("{i}u<u32(effects.params.x)"))
                .replace("shadow_visibility(i,", &format!("shadow_visibility({i}u,"));
            specialized.push_str(&body);
            specialized.push_str("    }\n");
        }
        source.replace_range(start..end, &specialized);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn light_class_uses_exact_nonzero_slots_and_preserves_order() {
        for mask in 0..16u8 {
            for appearance in [false, true] {
                let colors = std::array::from_fn(|i| {
                    if mask & (1 << i) != 0 {
                        [0.0, 0.2, 0.0]
                    } else {
                        [-0.0, 0.0, 0.0]
                    }
                });
                let class = LightClass::from_colors(colors, appearance);
                assert_eq!(class.active, mask);
                assert_eq!(class.directional_first, appearance && mask & 1 != 0);
                let mut source = include_str!("render/gltf_pbr.wgsl").to_owned();
                class.specialize(&mut source);
                assert!(!source.contains("scene.lights[i]"));
                assert!(!source.contains("let gl="));
                assert!(!source.contains("i<u32(effects.params.x)"));
                assert!(!source.contains("for(var i=0u;i<LIGHT_COUNT"));
                let body = source.split("fn surface(v:").nth(1).unwrap();
                let mut previous = 0;
                for i in 0..4 {
                    let needle = format!("shadow_visibility({i}u,");
                    assert_eq!(body.contains(&needle), mask & (1 << i) != 0);
                    if let Some(index) = body.find(&needle) {
                        assert!(index >= previous);
                        previous = index;
                    }
                }
                assert_eq!(
                    body.contains("let l=scene.appearance_sun.xyz;"),
                    class.directional_first
                );
            }
        }
    }
    #[test]
    fn light_class_color_changes_reuse_program_but_zero_and_type_changes_do_not() {
        let mut colors = [[0.0; 3]; 4];
        colors[2] = [1.0, 0.0, 0.0];
        let a = LightClass::from_colors(colors, false);
        colors[2] = [0.0, 1e-30, 0.0];
        assert_eq!(a, LightClass::from_colors(colors, true));
        colors[0] = [1.0; 3];
        assert_ne!(a, LightClass::from_colors(colors, true));
        assert_ne!(
            LightClass::from_colors(colors, false),
            LightClass::from_colors(colors, true)
        );
        colors[2] = [0.0; 3];
        assert_ne!(a, LightClass::from_colors(colors, false));
    }
}
