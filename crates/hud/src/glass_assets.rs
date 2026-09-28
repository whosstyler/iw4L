use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use assets::{FontDef, GlyphCapture};
use std::sync::OnceLock;

struct Atlas {
    font: FontDef,
    rgba: Vec<u8>,
}
static FONTS: OnceLock<[Atlas; 2]> = OnceLock::new();
fn atlases() -> &'static [Atlas; 2] {
    FONTS.get_or_init(|| {
        let font = FontRef::try_from_slice(include_bytes!("../../ui/assets/Oxanium-Regular.ttf"))
            .expect("bundled Oxanium font");
        let scaled = font.as_scaled(PxScale::from(60.0));
        [false, true].map(|bold| {
            let spread = if bold { 2 } else { 0 };
            let mut rgba = vec![0u8; 1024 * 384 * 4];
            let mut glyphs = Vec::new();
            for code in 32u8..128 {
                let id = scaled.glyph_id(code as char);
                let mut glyph = id.with_scale_and_position(60.0, point(0.0, 0.0));
                glyph.position = point(0.0, 0.0);
                let index = (code - 32) as usize;
                let x = index % 16 * 64 + 2;
                let y = index / 16 * 64 + 2;
                let mut capture = GlyphCapture {
                    letter: code as u16,
                    x0: 0,
                    y0: 0,
                    dx: scaled.h_advance(id).round() as u8,
                    pixel_width: 0,
                    pixel_height: 0,
                    s0: x as f32 / 1024.0,
                    t0: y as f32 / 384.0,
                    s1: x as f32 / 1024.0,
                    t1: y as f32 / 384.0,
                };
                if let Some(outline) = font.outline_glyph(glyph) {
                    let bounds = outline.px_bounds();
                    let width = bounds.width() as usize;
                    let height = bounds.height() as usize;
                    capture.x0 = bounds.min.x as i8;
                    capture.y0 = bounds.min.y as i8;
                    capture.pixel_width = (width + spread) as u8;
                    capture.pixel_height = height as u8;
                    capture.s1 = (x + capture.pixel_width as usize) as f32 / 1024.0;
                    capture.t1 = (y + height) as f32 / 384.0;
                    outline.draw(|gx, gy, coverage| {
                        for offset in 0..=spread {
                            let i = ((y + gy as usize) * 1024 + x + gx as usize + offset) * 4;
                            rgba[i..i + 3].fill(255);
                            rgba[i + 3] = rgba[i + 3].max((coverage * 255.0).round() as u8);
                        }
                    });
                }
                glyphs.push(capture);
            }
            Atlas {
                font: FontDef {
                    name: if bold {
                        "fonts/glass_bold"
                    } else {
                        "fonts/glass_regular"
                    }
                    .into(),
                    pixel_height: 60,
                    material: if bold {
                        "glass_font_bold"
                    } else {
                        "glass_font_regular"
                    }
                    .into(),
                    glow_material: String::new(),
                    glyphs,
                },
                rgba,
            }
        })
    })
}
pub(crate) fn font(bold: bool) -> &'static FontDef {
    &atlases()[usize::from(bold)].font
}
pub(crate) fn font_named(name: &str) -> Option<&'static FontDef> {
    match name {
        "fonts/glass_regular" => Some(font(false)),
        "fonts/glass_bold" => Some(font(true)),
        _ => None,
    }
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
pub(crate) fn image(name: &str) -> Option<(u32, u32, Vec<u8>)> {
    if matches!(name, "glass_font_regular" | "glass_font_bold") {
        return Some((
            1024,
            384,
            atlases()[usize::from(name == "glass_font_bold")]
                .rgba
                .clone(),
        ));
    }
    let (w, h, radius) = match name {
        "glass_panel" => (248u32, 244u32, 6.0),
        "glass_score_panel" => (380, 142, 6.0),
        "glass_ammo_panel" => (460, 106, 6.0),
        "glass_banner" => (540, 38, 0.0),
        "glass_key" => (40, 40, 4.0),
        "glass_glow" => (64, 32, 0.0),
        _ => return None,
    };
    let mut rgba = vec![0; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32;
            let qx = (x as f32 + 0.5 - w as f32 * 0.5).abs() - w as f32 * 0.5 + radius;
            let qy = (y as f32 + 0.5 - h as f32 * 0.5).abs() - h as f32 * 0.5 + radius;
            let distance = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius;
            let coverage = 1.0 - smooth(-0.7, 0.7, distance);
            let fade = match name {
                "glass_score_panel" => 1.0 - smooth(0.67, 1.0, u),
                "glass_ammo_panel" => smooth(0.0, 0.26, u),
                "glass_banner" => smooth(0.0, 0.18, u) * (1.0 - smooth(0.88, 1.0, u)),
                _ => 1.0,
            };
            let edge = smooth(-2.0, -0.7, distance);
            let sheen = (1.0 - v).powi(3) * 0.025;
            let tint = frame::glass::TINT;
            let border = frame::glass::BORDER;
            let (rgb, alpha) = if name == "glass_glow" {
                (
                    [0.35, 0.88, 0.96],
                    (-(u - 0.5).powi(2) * 28.0 - (v - 0.5).powi(2) * 9.0).exp() * 0.30,
                )
            } else {
                (
                    [
                        (tint[0] + sheen) * (1.0 - edge) + border[0] * edge,
                        (tint[1] + sheen) * (1.0 - edge) + border[1] * edge,
                        (tint[2] + sheen) * (1.0 - edge) + border[2] * edge,
                    ],
                    (tint[3] * (1.0 - edge) + border[3] * edge) * coverage * fade,
                )
            };
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[
                (rgb[0] * 255.0) as u8,
                (rgb[1] * 255.0) as u8,
                (rgb[2] * 255.0) as u8,
                (alpha * 255.0) as u8,
            ]);
        }
    }
    Some((w, h, rgba))
}
