//! The Windows tray icon: a water drop with today's count drawn large across
//! it, the drop red while a reminder is due. Pure RGBA drawing, so it's tested
//! on any OS; only used on Windows. Windows shows tray icons at 16 px at 100%
//! scaling, so the digits fill the drop to stay readable (#11).
#![cfg_attr(target_os = "macos", allow(dead_code))]

pub const ICON_SIZE: u32 = 32;

const DROP: [u8; 3] = [0x2E, 0x9B, 0xF0];
/// The drop's colour while a reminder is due: the Windows tray's due marker.
const DROP_DUE: [u8; 3] = [0xD3, 0x2F, 0x2F];
const TEXT: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
/// Outline around the digits, so parts that run past the drop's edge still
/// show on a light or dark taskbar.
const OUTLINE: [u8; 4] = [0x0D, 0x2B, 0x4A, 0xFF];
const OUTLINE_DUE: [u8; 4] = [0x5C, 0x0E, 0x0E, 0xFF];

/// Drop geometry: a circle (the bulb) with a cone up to the tip.
const CX: f32 = 16.0;
const CY: f32 = 20.5;
const R: f32 = 11.0;
const TIP: f32 = 1.5;

/// The digits must fit in this width, centred on the bulb.
const MAX_TEXT_W: u32 = 22;
/// Glyphs are 3x5 cells; never draw them taller than 15 px.
const MAX_SCALE: u32 = 3;
const GAP: u32 = 1; // px between glyphs

/// Draws the water drop as RGBA, `ICON_SIZE` square, 4x4 supersampled.
pub fn drop_rgba(color: [u8; 3]) -> Vec<u8> {
    let n = ICON_SIZE as usize;
    let mut px = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let mut hits = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let fx = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let fy = y as f32 + (sy as f32 + 0.5) / 4.0;
                    let in_circle = (fx - CX).powi(2) + (fy - CY).powi(2) <= R * R;
                    let in_cone =
                        (TIP..=CY).contains(&fy) && (fx - CX).abs() <= R * (fy - TIP) / (CY - TIP);
                    if in_circle || in_cone {
                        hits += 1;
                    }
                }
            }
            let i = (y * n + x) * 4;
            px[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], (hits * 255 / 16) as u8]);
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

fn text_width(glyphs: u32, scale: u32) -> u32 {
    glyphs * 3 * scale + glyphs.saturating_sub(1) * GAP
}

/// The largest glyph scale at which `glyphs` characters fit `MAX_TEXT_W`.
pub fn text_scale(glyphs: u32) -> u32 {
    (1..=MAX_SCALE)
        .rev()
        .find(|&s| text_width(glyphs, s) <= MAX_TEXT_W)
        .unwrap_or(1)
}

/// Top-left of the text, centred on the drop's bulb.
fn text_origin(glyphs: u32, scale: u32) -> (u32, u32) {
    let (w, h) = (text_width(glyphs, scale), 5 * scale);
    let x = (CX - w as f32 / 2.0).round() as u32;
    let y = (CY - h as f32 / 2.0).round() as u32;
    (x, y)
}

/// The drop with `count` drawn across it. Returns RGBA, `ICON_SIZE` square.
pub fn badge_rgba(count: u32, prompting: bool) -> Vec<u8> {
    let (drop, outline) = if prompting {
        (DROP_DUE, OUTLINE_DUE)
    } else {
        (DROP, OUTLINE)
    };
    let n = ICON_SIZE as i32;
    let mut px = drop_rgba(drop);

    let label: Vec<char> = badge_label(count).chars().collect();
    let glyphs = label.len() as u32;
    let scale = text_scale(glyphs);
    let (x0, y0) = text_origin(glyphs, scale);

    // Mark the text pixels first, then outline every non-text neighbour.
    let mut text = vec![false; (n * n) as usize];
    for (gi, ch) in label.iter().enumerate() {
        let gx = x0 + gi as u32 * (3 * scale + GAP);
        for (row, bits) in glyph(*ch).iter().enumerate() {
            for col in 0..3u32 {
                if bits & (0b100 >> col) == 0 {
                    continue;
                }
                for dy in 0..scale {
                    for dx in 0..scale {
                        let (x, y) = (gx + col * scale + dx, y0 + row as u32 * scale + dy);
                        if x < ICON_SIZE && y < ICON_SIZE {
                            text[(y * ICON_SIZE + x) as usize] = true;
                        }
                    }
                }
            }
        }
    }
    let mut put = |x: i32, y: i32, c: [u8; 4]| {
        let i = ((y * n + x) * 4) as usize;
        px[i..i + 4].copy_from_slice(&c);
    };
    for y in 0..n {
        for x in 0..n {
            if text[(y * n + x) as usize] {
                put(x, y, TEXT);
                continue;
            }
            let near_text = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    (0..n).contains(&nx) && (0..n).contains(&ny) && text[(ny * n + nx) as usize]
                })
            });
            if near_text {
                put(x, y, outline);
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

    fn count_of(px: &[u8], c: [u8; 4]) -> usize {
        px.chunks(4).filter(|p| *p == c).count()
    }

    #[test]
    fn drop_is_opaque_inside_and_clear_outside() {
        let px = drop_rgba(DROP);
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
    fn one_and_two_digits_are_drawn_at_full_size() {
        // Scale 3 = 15 px tall at 32 px, about 7 px at 16 px (was 5 px).
        assert_eq!(text_scale(1), 3);
        assert_eq!(text_scale(2), 3);
        assert_eq!(text_scale(3), 2);
        for glyphs in 1..=3 {
            assert!(text_width(glyphs, text_scale(glyphs)) <= MAX_TEXT_W);
        }
    }

    #[test]
    fn digits_are_large_white_and_centred() {
        let eight = badge_rgba(8, false);
        // "8" at scale 3 lights 13 of 15 cells, 9 px each.
        assert_eq!(count_of(&eight, TEXT), 13 * 9);
        // Its top-left cell is lit, at the centred origin.
        let (x0, y0) = text_origin(1, 3);
        assert_eq!(pixel(&eight, x0, y0), TEXT);
        assert!((x0 as f32 + 4.5 - CX).abs() <= 0.5);
        // Different counts give different icons.
        assert_ne!(eight, badge_rgba(1, false));
    }

    #[test]
    fn digits_are_outlined() {
        let px = badge_rgba(1, false);
        assert!(count_of(&px, OUTLINE) > 0);
        // No text pixel sits on the icon edge, so the outline is complete.
        for i in 0..ICON_SIZE {
            for (x, y) in [(i, 0), (i, ICON_SIZE - 1), (0, i), (ICON_SIZE - 1, i)] {
                assert_ne!(pixel(&badge_rgba(88, false), x, y), TEXT);
            }
        }
    }

    #[test]
    fn due_turns_the_drop_red() {
        let (normal, due) = (badge_rgba(3, false), badge_rgba(3, true));
        // Above the digits, inside the bulb.
        let (x, y) = (16, 11);
        assert_eq!(pixel(&normal, x, y), [DROP[0], DROP[1], DROP[2], 255]);
        assert_eq!(
            pixel(&due, x, y),
            [DROP_DUE[0], DROP_DUE[1], DROP_DUE[2], 255]
        );
        assert_eq!(count_of(&due, OUTLINE_DUE), count_of(&normal, OUTLINE));
    }

    #[test]
    fn widest_label_fits() {
        // "99+" must fit in the icon without panicking on out-of-range pixels.
        let px = badge_rgba(1000, false);
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert!(count_of(&px, TEXT) > 0);
    }
}
