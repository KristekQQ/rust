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

/// Generic single-panel fallback retained for native layout regression tests.
#[cfg(test)]
pub fn geometry_xr(lines: &[String], width: u32, height: u32) -> Vec<HudRect> {
    let mut rects = geometry(lines, width.max(1), height.max(1));
    let panel = rects[0].rect;
    // Clip-space bounds corresponding to x=20..80%, y=20..40% of the eye image.
    let scale = (1.2 / panel[2]).min(0.4 / -panel[3]);
    let left = -panel[2] * scale * 0.5;
    let top = 0.4 - panel[3] * scale * 0.5;
    for rect in &mut rects {
        rect.rect = [
            left + (rect.rect[0] - panel[0]) * scale,
            top + (rect.rect[1] - panel[1]) * scale,
            rect.rect[2] * scale,
            rect.rect[3] * scale,
        ];
    }
    rects
}

#[derive(Clone, Copy)]
pub struct XrRoundDisplay {
    pub remaining: f32,
    pub finished: bool,
}

pub struct XrHudContent {
    pub scene: Vec<(&'static str, String)>,
    pub cuts: u64,
    pub round: Option<XrRoundDisplay>,
}
pub enum HudContent {
    Desktop(Vec<String>),
    Xr(XrHudContent),
}

/// Two explicit label/value grids inside x=20..80%, y=10..50% of each eye.
pub fn geometry_xr_tables(content: &XrHudContent, width: u32, height: u32) -> Vec<HudRect> {
    let mut score = vec![("ROZKROJENO", content.cuts.to_string())];
    if let Some(round) = content.round {
        score.push(("CAS", format!("{} S", round.remaining.ceil() as u32)));
        score.push(("STAV", if round.finished { "HOTOVO" } else { "HRA" }.into()));
    }
    let tables = [
        ("SCENA", content.scene.as_slice()),
        ("SKORE", score.as_slice()),
    ];
    let label_columns = tables
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(label, _)| label.len())
        .max()
        .unwrap_or(1);
    let value_columns = tables
        .iter()
        .flat_map(|(_, rows)| rows.iter())
        .map(|(_, value)| value.len())
        .max()
        .unwrap_or(1);
    let divider = (label_columns * 6 + 10) as f32;
    let table_width = divider + (value_columns * 6 + 10) as f32;
    let heights = tables.map(|(_, rows)| (rows.len() + 1) as f32 * 12.0 + 4.0);
    let side_by_side = width as f32 >= height as f32 * 1.3;
    let layout_width = if side_by_side {
        table_width * 2.0 + 10.0
    } else {
        table_width
    };
    let layout_height = if side_by_side {
        heights[0].max(heights[1])
    } else {
        heights[0] + heights[1] + 10.0
    };
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    let scale = (width * 0.6 / layout_width).min(height * 0.4 / layout_height);
    let left = (width - layout_width * scale) * 0.5;
    let top = height * 0.3 - layout_height * scale * 0.5;
    let mut result = Vec::new();
    let mut rect = |x: f32, y: f32, w: f32, h: f32, color| {
        result.push(HudRect {
            rect: [
                (left + x * scale) / width * 2.0 - 1.0,
                1.0 - (top + y * scale) / height * 2.0,
                w * scale / width * 2.0,
                -h * scale / height * 2.0,
            ],
            color,
        });
    };
    for (index, (title, rows)) in tables.iter().enumerate() {
        let x = if side_by_side {
            index as f32 * (table_width + 10.0)
        } else {
            0.0
        };
        let y = if !side_by_side && index == 1 {
            heights[0] + 10.0
        } else {
            0.0
        };
        rect(x, y, table_width, heights[index], [0.015, 0.025, 0.04, 0.9]);
        let grid = [0.25, 0.55, 0.65, 1.0];
        for row in 0..=rows.len() + 1 {
            rect(x, y + row as f32 * 12.0 + 2.0, table_width, 0.5, grid);
        }
        rect(x + divider, y + 14.0, 0.5, heights[index] - 16.0, grid);
        for row in 0..=rows.len() {
            let texts = if row == 0 {
                [(*title, 5.0), ("", 0.0)]
            } else {
                [
                    (rows[row - 1].0, 5.0),
                    (
                        rows[row - 1].1.as_str(),
                        table_width - 5.0 - rows[row - 1].1.len() as f32 * 6.0,
                    ),
                ]
            };
            for (text, start) in texts {
                for (column, ch) in text.chars().enumerate() {
                    for (pixel_y, bits) in glyph(ch).iter().enumerate() {
                        for pixel_x in 0..5 {
                            if bits & (1 << (4 - pixel_x)) != 0 {
                                rect(
                                    x + start + (column * 6 + pixel_x) as f32,
                                    y + 5.0 + (row * 12 + pixel_y) as f32,
                                    1.0,
                                    1.0,
                                    [0.85, 0.95, 1.0, 1.0],
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

pub(crate) fn glyph(ch: char) -> [u8; 7] {
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
        'G' => [14, 17, 16, 23, 17, 17, 14],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        'J' => [1, 1, 1, 1, 17, 17, 14],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        '.' => [0, 0, 0, 0, 0, 6, 6],
        '/' => [1, 1, 2, 4, 8, 16, 16],
        '+' => [0, 4, 4, 31, 4, 4, 0],
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
        // Desktop panel remains anchored eight pixels from the top-left corner.
        assert_eq!(
            rects[0].rect,
            [
                -0.98,
                1.0 - 16.0 / 600.0,
                288.0 / 800.0 * 2.0,
                -30.0 / 600.0 * 2.0
            ]
        );
    }
    #[test]
    fn xr_hud_fits_safe_region_at_each_eye_resolution() {
        let lines = [
            "FPS 60.0 FRAME 16.67 MS",
            "CPU 1.23 MS CULL 0.01 MS",
            "CACHED 123 FRUSTUM 456",
            "BVH NODES 123 TESTS 456",
            "DRAWS 123 MAIN 456",
        ]
        .map(String::from);
        for (width, height) in [
            (2048, 2048),
            (1832, 1920),
            (640, 480),
            (320, 720),
            (1, 1),
            (0, 0),
        ] {
            let rects = geometry_xr(&lines, width, height);
            for rect in &rects {
                let [x, y, w, h] = rect.rect;
                assert!(rect.rect.iter().all(|v| v.is_finite()));
                assert!(x >= -0.60001 && x + w <= 0.60001, "{width}x{height}");
                assert!(y <= 0.60001 && y + h >= 0.19999, "{width}x{height}");
            }
            let [x, y, w, h] = rects[0].rect;
            assert!((x + w * 0.5).abs() < 0.00001);
            assert!((y + h * 0.5 - 0.4).abs() < 0.00001);
            let other_eye = geometry_xr(&lines, width, height);
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&rects),
                bytemuck::cast_slice::<_, u8>(&other_eye)
            );
        }
    }
    #[test]
    fn xr_tables_are_separate_safe_and_score_changes_pixels() {
        let mut content = XrHudContent {
            scene: vec![
                ("FPS", "60.0".into()),
                ("FRAME MS", "16.67".into()),
                ("CPU MS", "1.23".into()),
                ("OBJEKTY", "123".into()),
                ("DRAWS", "123".into()),
            ],
            cuts: 0,
            round: Some(XrRoundDisplay {
                remaining: 30.0,
                finished: false,
            }),
        };
        for (width, height) in [
            (2048, 2048),
            (1832, 1920),
            (640, 480),
            (320, 720),
            (1, 1),
            (0, 0),
        ] {
            let rects = geometry_xr_tables(&content, width, height);
            for rect in &rects {
                let [x, y, w, h] = rect.rect;
                assert!(rect.rect.iter().all(|v| v.is_finite()));
                assert!(x >= -0.60001 && x + w <= 0.60001, "{width}x{height}");
                assert!(y <= 0.80001 && y + h >= -0.00001, "{width}x{height}");
            }
            let panels: Vec<_> = rects.iter().filter(|r| r.color[3] == 0.9).collect();
            assert_eq!(panels.len(), 2);
            let [x, y, w, h] = panels[0].rect;
            let [other_x, other_y, _, _] = panels[1].rect;
            assert!(x + w < other_x || y + h > other_y);
            if width as f32 >= height as f32 * 1.3 {
                assert!(x + w < other_x, "wide eyes use horizontal tables");
            } else {
                assert!(y + h > other_y, "square and portrait eyes stack tables");
            }
            let second_eye = geometry_xr_tables(&content, width, height);
            assert_eq!(
                bytemuck::cast_slice::<_, u8>(&rects),
                bytemuck::cast_slice::<_, u8>(&second_eye)
            );
        }
        let before = geometry_xr_tables(&content, 2048, 2048);
        content.cuts = 1;
        let after = geometry_xr_tables(&content, 2048, 2048);
        assert_ne!(
            bytemuck::cast_slice::<_, u8>(&before),
            bytemuck::cast_slice::<_, u8>(&after)
        );
        content.round = Some(XrRoundDisplay {
            remaining: 0.0,
            finished: true,
        });
        let finished = geometry_xr_tables(&content, 2048, 2048);
        assert_ne!(
            bytemuck::cast_slice::<_, u8>(&after),
            bytemuck::cast_slice::<_, u8>(&finished)
        );
    }
    #[test]
    fn all_metric_letters_have_glyphs() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789./".chars() {
            assert_ne!(glyph(c), [0; 7], "{c}");
        }
    }
}
