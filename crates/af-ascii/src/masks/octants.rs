//! Implémentation mathématique O(1) des Octants (Unicode 16.0)
//! Symbols for Legacy Computing Supplement - U+1CD00 à U+1CDE5
//!
//! Bits activés :
//! +---+---+
//! | 1 | 5 |
//! +---+---+
//! | 2 | 6 |
//! +---+---+
//! | 3 | 7 |
//! +---+---+
//! | 4 | 8 |
//! +---+---+

/// Génère récursivement la LUT Octant lors de la compilation.
/// Promeut les caractères Quadrants (Block Elements) pour les blocs pleins garantis,
/// et dégrade gracieusement grâce à l'isométrie de Braille pour les octants manquants.
const fn generate_octant_lut() -> [char; 256] {
    let mut lut = [' '; 256];
    let mut i = 0;
    while i < 256 {
        lut[i] = match i as u8 {
            0x00 => ' ',
            0xFF => '\u{2588}',
            // 16 Quadrants natifs
            0x05 => '\u{2598}', // TL
            0x0A => '\u{259D}', // TR
            0x50 => '\u{2596}', // BL
            0xA0 => '\u{2597}', // BR
            0x0F => '\u{2580}', // Top Half
            0xF0 => '\u{2584}', // Bot Half
            0x55 => '\u{258C}', // Left Half
            0xAA => '\u{2590}', // Right Half
            0xA5 => '\u{259A}', // TL + BR
            0x5A => '\u{259E}', // TR + BL
            0x5F => '\u{259B}', // TL+TR+BL
            0xAF => '\u{259C}', // TL+TR+BR
            0xF5 => '\u{2599}', // TL+BL+BR
            0xFA => '\u{259F}', // TR+BL+BR
            _ => {
                let b = i as u8;
                let mut braille_mask = 0u32;
                if b & (1 << 0) != 0 {
                    braille_mask |= 0x01;
                } // Braille 1
                if b & (1 << 1) != 0 {
                    braille_mask |= 0x08;
                } // Braille 4
                if b & (1 << 2) != 0 {
                    braille_mask |= 0x02;
                } // Braille 2
                if b & (1 << 3) != 0 {
                    braille_mask |= 0x10;
                } // Braille 5
                if b & (1 << 4) != 0 {
                    braille_mask |= 0x04;
                } // Braille 3
                if b & (1 << 5) != 0 {
                    braille_mask |= 0x20;
                } // Braille 6
                if b & (1 << 6) != 0 {
                    braille_mask |= 0x40;
                } // Braille 7
                if b & (1 << 7) != 0 {
                    braille_mask |= 0x80;
                } // Braille 8

                match std::char::from_u32(0x2800 + braille_mask) {
                    Some(c) => c,
                    None => ' ',
                }
            }
        };
        i += 1;
    }
    lut
}

pub const OCTANT_LUT: [char; 256] = generate_octant_lut();

/// Lookup Table pré-calculée compilée statique. Index 0..=255.
/// Assigne les caractères  Octant via macro-const O(1).
#[must_use]
#[inline(always)]
pub const fn get_octant_char(bitmask: u8) -> char {
    OCTANT_LUT[bitmask as usize]
}

use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
use rayon::prelude::*;

/// Process frame in octant mode (2×4 sub-pixels per terminal cell).
pub fn process_octant(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_w = u32::from(grid.width) * 2;
    let pixel_h = u32::from(grid.height) * 4;
    let threshold: u8 = 128;

    grid.cells
        .par_chunks_mut(grid.width as usize)
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, cell) in row.iter_mut().enumerate() {
                let base_x = (cx as u32) * 2 * frame.width / pixel_w.max(1);
                let base_y = (cy as u32) * 4 * frame.height / pixel_h.max(1);

                let mut bitmask = 0u8;
                let mut avg_r = 0u32;
                let mut avg_g = 0u32;
                let mut avg_b = 0u32;

                // 2 columns, 4 rows
                for dy in 0..4u32 {
                    for dx in 0..2u32 {
                        let px = (base_x + dx * frame.width / pixel_w.max(1))
                            .min(frame.width.saturating_sub(1));
                        let py = (base_y + dy * frame.height / pixel_h.max(1))
                            .min(frame.height.saturating_sub(1));

                        let lum = frame.luminance(px, py);
                        let (r, g, b, _) = frame.pixel(px, py);

                        // Bit order logic (1..8)
                        let bit = dy * 2 + dx;
                        let on = if config.invert {
                            lum < threshold
                        } else {
                            lum > threshold
                        };

                        if on {
                            bitmask |= 1 << bit;
                        }

                        avg_r += u32::from(r);
                        avg_g += u32::from(g);
                        avg_b += u32::from(b);
                    }
                }

                let ch = get_octant_char(bitmask);
                let fg = ((avg_r / 8) as u8, (avg_g / 8) as u8, (avg_b / 8) as u8);

                *cell = AsciiCell {
                    ch,
                    fg,
                    bg: (0, 0, 0),
                };
            }
        });
}
