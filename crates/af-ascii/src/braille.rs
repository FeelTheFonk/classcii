use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
use rayon::prelude::*;

/// Braille base codepoint (U+2800).
const BRAILLE_BASE: u32 = 0x2800;

/// Encode 2×4 pixel block into a Braille Unicode character.
///
/// Braille dot numbering (column-major):
/// ```text
///  1 4
///  2 5
///  3 6
///  7 8
/// ```
///
/// # Example
/// ```
/// use af_ascii::braille::encode_braille;
/// assert_eq!(encode_braille([false; 8]), '\u{2800}'); // empty
/// assert_eq!(encode_braille([true; 8]),  '\u{28FF}'); // full
/// ```
#[must_use]
pub fn encode_braille(dots: [bool; 8]) -> char {
    let mut code = 0u32;
    // Dot numbering to bit mapping:
    // dot 1 → bit 0, dot 2 → bit 1, dot 3 → bit 2,
    // dot 4 → bit 3, dot 5 → bit 4, dot 6 → bit 5,
    // dot 7 → bit 6, dot 8 → bit 7
    for (i, &dot) in dots.iter().enumerate() {
        if dot {
            code |= 1 << i;
        }
    }
    char::from_u32(BRAILLE_BASE + code).unwrap_or(' ')
}

/// Process frame in Braille mode (2×4 sub-pixels per terminal cell).
///
/// Each terminal cell samples a 2×4 pixel block. A pixel is "on" if its
/// luminance exceeds a threshold.
///
/// # Example
/// ```
/// use af_core::frame::{FrameBuffer, AsciiGrid};
/// use af_core::config::RenderConfig;
/// use af_ascii::braille::process_braille;
///
/// let frame = FrameBuffer::new(4, 8);
/// let mut grid = AsciiGrid::new(2, 2);
/// let config = RenderConfig::default();
/// process_braille(&frame, &config, &mut grid);
/// ```
pub fn process_braille(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_w = u32::from(grid.width) * 2;
    let pixel_h = u32::from(grid.height) * 4;

    grid.cells
        .par_chunks_mut(grid.width as usize)
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, cell) in row.iter_mut().enumerate() {
                let base_x = (cx as u32) * 2 * frame.width / pixel_w.max(1);
                let base_y = (cy as u32) * 4 * frame.height / pixel_h.max(1);

                // Passe 1 : collecter luminances, couleurs, et indices dot
                let mut lum_values = [0u8; 8];
                let mut lum_sum = 0u32;
                let mut dot_indices = [0usize; 8];
                let mut avg_r = 0u32;
                let mut avg_g = 0u32;
                let mut avg_b = 0u32;
                let mut count = 0u32;
                let mut sub_idx = 0usize;

                for dy in 0..4u32 {
                    for dx in 0..2u32 {
                        let px = (base_x + dx * frame.width / pixel_w.max(1))
                            .min(frame.width.saturating_sub(1));
                        let py = (base_y + dy * frame.height / pixel_h.max(1))
                            .min(frame.height.saturating_sub(1));

                        let lum = frame.luminance_linear(px, py);
                        let (r, g, b, _) = frame.pixel(px, py);

                        let dot_idx = if dx == 0 {
                            match dy {
                                0 => 0,
                                1 => 1,
                                2 => 2,
                                _ => 6,
                            }
                        } else {
                            match dy {
                                0 => 3,
                                1 => 4,
                                2 => 5,
                                _ => 7,
                            }
                        };

                        lum_values[sub_idx] = lum;
                        dot_indices[sub_idx] = dot_idx;
                        lum_sum += u32::from(lum);
                        sub_idx += 1;

                        avg_r += u32::from(r);
                        avg_g += u32::from(g);
                        avg_b += u32::from(b);
                        count += 1;
                    }
                }

                // Passe 2 : seuil adaptatif (moyenne locale)
                let local_threshold = if count > 0 {
                    (lum_sum / count) as u8
                } else {
                    128
                };
                let mut dots = [false; 8];
                for i in 0..sub_idx {
                    let on = if config.invert {
                        lum_values[i] < local_threshold
                    } else {
                        lum_values[i] > local_threshold
                    };
                    dots[dot_indices[i]] = on;
                }

                let ch = encode_braille(dots);
                let fg = if count > 0 {
                    (
                        (avg_r / count) as u8,
                        (avg_g / count) as u8,
                        (avg_b / count) as u8,
                    )
                } else {
                    (255, 255, 255)
                };

                *cell = AsciiCell {
                    ch,
                    fg,
                    bg: (0, 0, 0),
                };
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_empty_is_blank() {
        let ch = encode_braille([false; 8]);
        assert_eq!(ch, '\u{2800}');
    }

    #[test]
    fn braille_full_is_solid() {
        let ch = encode_braille([true; 8]);
        assert_eq!(ch, '\u{28FF}');
    }
}
