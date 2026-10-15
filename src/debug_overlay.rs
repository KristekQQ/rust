//! Tiny bitmap HUD geometry. Called only when the optional overlay refreshes.
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct HudRect {
    pub rect: [f32; 4],
    pub color: [f32; 4],
}

pub fn geometry(lines: &[String], width: u32, height: u32) -> Vec<HudRect> {
    let scale = if width >= 480 { 2.0 } else { 1.0 };
    let mut result = Vec::new();
    let mut rect = |x: f32, y: f32, w: f32, h: f32, color| {
        result.push(HudRect {
            rect: [
                x / width as f32 * 2.0 - 1.0,
                1.0 - y / height as f32 * 2.0,
                w / width as f32 * 2.0,
                -h / height as f32 * 2.0,
            ],
            color,
        });
    };
    let columns = lines.iter().map(String::len).max().unwrap_or(0);
    rect(
        8.0,
        8.0,
        columns as f32 * 6.0 * scale + 12.0,
        lines.len() as f32 * 10.0 * scale + 10.0,
        [0.015, 0.025, 0.04, 0.85],
    );
    for (line, text) in lines.iter().enumerate() {
        for (column, ch) in text.chars().enumerate() {
            for (row, bits) in glyph(ch).iter().enumerate() {
                for pixel in 0..5 {
                    if bits & (1 << (4 - pixel)) != 0 {
                        rect(
                            14.0 + (column * 6 + pixel) as f32 * scale,
                            13.0 + (line * 10 + row) as f32 * scale,
                            scale,
                            scale,
                            [0.85, 0.95, 1.0, 1.0],
                        );
                    }
                }
            }
        }
    }
    result
}

fn glyph(ch: char) -> [u8; 7] {
    match ch {
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        _ => [0; 7],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hud_has_panel_and_finite_pixel_geometry() {
        let rects = geometry(&["FPS 60.0 FRAME 16.67 MS".into()], 800, 600);
        assert!(rects.len() > 100);
        assert!(rects.iter().all(|r| r.rect.iter().all(|v| v.is_finite())));
        assert_eq!(rects[0].color[3], 0.85);
    }
    #[test]
    fn all_metric_letters_have_glyphs() {
        for c in "FPSFRAMECPUMSCULLACHEDFRUSTMBVHNODETESTS0123456789./".chars() {
            assert_ne!(glyph(c), [0; 7], "{c}");
        }
    }
}
