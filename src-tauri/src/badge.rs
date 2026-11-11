//! The Windows tray icon: a water drop with today's count drawn on as a badge.
//! Pure RGBA drawing, so it is tested on any OS; only used on Windows.
#![cfg_attr(target_os = "macos", allow(dead_code))]

pub const ICON_SIZE: u32 = 32;

/// Draws the water drop as RGBA, `ICON_SIZE` square, 4x4 supersampled.
pub fn drop_rgba() -> Vec<u8> {
    let n = ICON_SIZE as usize;
    let mut px = vec![0u8; n * n * 4];
    let (cx, cy, r, tip) = (16.0_f32, 20.0_f32, 10.0_f32, 2.5_f32);
    for y in 0..n {
        for x in 0..n {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let fx = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let fy = y as f32 + (sy as f32 + 0.5) / 4.0;
                    let in_circle = (fx - cx).powi(2) + (fy - cy).powi(2) <= r * r;
                    let in_cone =
                        fy >= tip && fy <= cy && (fx - cx).abs() <= r * (fy - tip) / (cy - tip);
                    if in_circle || in_cone {
                        hits += 1;
                    }
                }
            }
            let i = (y * n + x) * 4;
            px[i..i + 4].copy_from_slice(&[0x2E, 0x9B, 0xF0, (hits * 255 / 16) as u8]);
        }
    }
    px
}

/// 3x5 bitmap glyphs, one row per entry, most significant of 3 bits = left.
fn glyph(c: char) -> [u8; 5] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '+' => [0b000, 0b010, 0b111, 0b010, 0b000],
        _ => [0; 5],
    }
}

pub fn badge_label(count: u32) -> String {
    if count > 99 {
        "99+".into()
    } else {
        count.to_string()
    }
}

const SCALE: u32 = 2; // glyph pixel size
const GAP: u32 = 1; // px between glyphs
const PAD: u32 = 2; // px around the text
const BADGE_BG: [u8; 4] = [0x1F, 0x5F, 0xA8, 0xFF];
/// Badge colour while a reminder is due: the Windows tray's due marker.
const BADGE_DUE: [u8; 4] = [0xC6, 0x28, 0x28, 0xFF];
const BADGE_FG: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// The drop with `count` drawn in a badge at the bottom right. Returns RGBA,
/// `ICON_SIZE` square.
pub fn badge_rgba(count: u32, prompting: bool) -> Vec<u8> {
    let bg = if prompting { BADGE_DUE } else { BADGE_BG };
    let n = ICON_SIZE;
    let mut px = drop_rgba();
    let label: Vec<char> = badge_label(count).chars().collect();
    let glyphs = label.len() as u32;
    let text_w = glyphs * 3 * SCALE + (glyphs - 1) * GAP;
    let (w, h) = (text_w + 2 * PAD, 5 * SCALE + 2 * PAD);
    let (x0, y0) = (n - w, n - h);
    let mut put = |x: u32, y: u32, c: [u8; 4]| {
        let i = ((y * n + x) * 4) as usize;
        px[i..i + 4].copy_from_slice(&c);
    };
    for y in y0..n {
        for x in x0..n {
            put(x, y, bg);
        }
    }
    for (gi, ch) in label.iter().enumerate() {
        let gx = x0 + PAD + gi as u32 * (3 * SCALE + GAP);
        for (row, bits) in glyph(*ch).iter().enumerate() {
            for col in 0..3u32 {
                if bits & (0b100 >> col) != 0 {
                    for dy in 0..SCALE {
                        for dx in 0..SCALE {
                            put(
                                gx + col * SCALE + dx,
                                y0 + PAD + row as u32 * SCALE + dy,
                                BADGE_FG,
                            );
                        }
                    }
                }
            }
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(px: &[u8], x: u32, y: u32) -> [u8; 4] {
        let i = ((y * ICON_SIZE + x) * 4) as usize;
        px[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn drop_is_opaque_inside_and_clear_outside() {
        let px = drop_rgba();
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert_eq!(pixel(&px, 16, 20)[3], 255); // centre of the bulb
        assert_eq!(pixel(&px, 0, 0)[3], 0); // corner
        assert_eq!(pixel(&px, 16, 0)[3], 0); // above the tip
    }

    #[test]
    fn badge_labels() {
        assert_eq!(badge_label(0), "0");
        assert_eq!(badge_label(42), "42");
        assert_eq!(badge_label(100), "99+");
    }

    #[test]
    fn badge_draws_digits_bottom_right_and_keeps_drop() {
        let one = badge_rgba(1, false);
        let eight = badge_rgba(8, false);
        assert_eq!(one.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        // Different counts give different icons.
        assert_ne!(one, eight);
        // Bottom-right corner is badge background.
        assert_eq!(pixel(&one, ICON_SIZE - 1, ICON_SIZE - 1), BADGE_BG);
        // Top of the drop is untouched by the badge.
        assert_eq!(pixel(&one, 16, 8), pixel(&drop_rgba(), 16, 8));
        // "8" lights its top-left glyph pixel; "1" doesn't.
        let (gx, gy) = (
            ICON_SIZE - (3 * SCALE + 2 * PAD) + PAD,
            ICON_SIZE - (5 * SCALE + 2 * PAD) + PAD,
        );
        assert_eq!(pixel(&eight, gx, gy), BADGE_FG);
        assert_eq!(pixel(&one, gx, gy), BADGE_BG);
    }

    #[test]
    fn due_badge_changes_colour() {
        let due = badge_rgba(3, true);
        assert_eq!(pixel(&due, ICON_SIZE - 1, ICON_SIZE - 1), BADGE_DUE);
        assert_ne!(due, badge_rgba(3, false));
    }

    #[test]
    fn widest_badge_fits() {
        // "99+" must fit in the icon without panicking on out-of-range pixels.
        let px = badge_rgba(1000, false);
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }
}
