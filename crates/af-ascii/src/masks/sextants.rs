//! Implémentation mathématique O(1) des Sextants (Unicode 13.0)
//! Symbols for Legacy Computing - U+1FB00 à U+1FB3B
//!
//! Bits activés :
//! +---+---+
//! | 1 | 4 |
//! +---+---+
//! | 2 | 5 |
//! +---+---+
//! | 3 | 6 |
//! +---+---+

/// Map un entier 6-bits (0 à 63) vers le caractère Sextant Unicode correspondant.
/// L'index 0 (aucun bit activé) renvoie un espace, et l'index 63 (tous les bits) renvoie un block plein U+2588.
/// La plage U+1FB00 démarre formellement à l'index 1.
#[must_use]
#[inline(always)]
pub const fn get_sextant_char(bitmask: u8) -> char {
    debug_assert!(bitmask < 64, "Sextant bitmask must be 6-bits (0-63)");

    match bitmask {
        0 => ' ',
        // Offset de base pour les sextants : U+1FB00.
        // Puisque U+1FB00 correspond au bitmask '1' (Haut-Gauche seul), et que la séquence se suit...
        // on translate. (0x1FB00 - 1) + bitmask si bitmask < 63
        // Mais attention, la spécification Unicode a des particularités logiques.
        // La table dans "Recherche exhaustive" liste les index de manière incrémentale.
        // Conformément à U+1FB00 - U+1FB3B, on a 60 caractères (64 combinaisons - vide - plein - 2 existants ?).
        // On préfère un tableau `const` statique de taille 64 pour `O(1)` absolu et sans saut conditionnel coûteux.
        _ => SEXTANT_LUT[bitmask as usize],
    }
}

/// Lookup Table pré-calculée. Index 0..=63 correspond au bitmask 6-bit.
/// Bit 0: HautGauche (pos 1), Bit 1: MilieuGauche (pos 2), Bit 2: BasGauche (pos 3),
/// Bit 3: HautDroit (pos 4), Bit 4: MilieuDroit (pos 5), Bit 5: BasDroit (pos 6).
///
/// Mapping vérifié contre la table Unicode officielle (U+1FB00-U+1FB3B, 60 codepoints).
/// Bitmasks 21 (Sextant-135) et 42 (Sextant-246) sont absents de Unicode 13.0
/// (motifs en damier) — fallback vers U+2592 MEDIUM SHADE.
const SEXTANT_LUT: [char; 64] = [
    ' ',         //  0: Vide
    '\u{1FB00}', //  1: Sextant-1
    '\u{1FB01}', //  2: Sextant-2
    '\u{1FB02}', //  3: Sextant-12
    '\u{1FB03}', //  4: Sextant-3
    '\u{1FB04}', //  5: Sextant-13
    '\u{1FB05}', //  6: Sextant-23
    '\u{1FB06}', //  7: Sextant-123
    '\u{1FB07}', //  8: Sextant-4
    '\u{1FB08}', //  9: Sextant-14
    '\u{1FB09}', // 10: Sextant-24
    '\u{1FB0A}', // 11: Sextant-124
    '\u{1FB0B}', // 12: Sextant-34
    '\u{1FB0C}', // 13: Sextant-134
    '\u{1FB0D}', // 14: Sextant-234
    '\u{1FB0E}', // 15: Sextant-1234
    '\u{1FB0F}', // 16: Sextant-5
    '\u{1FB10}', // 17: Sextant-15
    '\u{1FB11}', // 18: Sextant-25
    '\u{1FB12}', // 19: Sextant-125
    '\u{1FB13}', // 20: Sextant-35
    '\u{2592}',  // 21: ▒ (Sextant-135 absent de Unicode — damier, fallback MEDIUM SHADE)
    '\u{1FB14}', // 22: Sextant-235
    '\u{1FB15}', // 23: Sextant-1235
    '\u{1FB16}', // 24: Sextant-45
    '\u{1FB17}', // 25: Sextant-145
    '\u{1FB18}', // 26: Sextant-245
    '\u{1FB19}', // 27: Sextant-1245
    '\u{1FB1A}', // 28: Sextant-345
    '\u{1FB1B}', // 29: Sextant-1345
    '\u{1FB1C}', // 30: Sextant-2345
    '\u{1FB1D}', // 31: Sextant-12345
    '\u{1FB1E}', // 32: Sextant-6
    '\u{1FB1F}', // 33: Sextant-16
    '\u{1FB20}', // 34: Sextant-26
    '\u{1FB21}', // 35: Sextant-126
    '\u{1FB22}', // 36: Sextant-36
    '\u{1FB23}', // 37: Sextant-136
    '\u{1FB24}', // 38: Sextant-236
    '\u{1FB25}', // 39: Sextant-1236
    '\u{1FB26}', // 40: Sextant-46
    '\u{1FB27}', // 41: Sextant-146
    '\u{2592}',  // 42: ▒ (Sextant-246 absent de Unicode — damier, fallback MEDIUM SHADE)
    '\u{1FB28}', // 43: Sextant-1246
    '\u{1FB29}', // 44: Sextant-346
    '\u{1FB2A}', // 45: Sextant-1346
    '\u{1FB2B}', // 46: Sextant-2346
    '\u{1FB2C}', // 47: Sextant-12346
    '\u{1FB2D}', // 48: Sextant-56
    '\u{1FB2E}', // 49: Sextant-156
    '\u{1FB2F}', // 50: Sextant-256
    '\u{1FB30}', // 51: Sextant-1256
    '\u{1FB31}', // 52: Sextant-356
    '\u{1FB32}', // 53: Sextant-1356
    '\u{1FB33}', // 54: Sextant-2356
    '\u{1FB34}', // 55: Sextant-12356
    '\u{1FB35}', // 56: Sextant-456
    '\u{1FB36}', // 57: Sextant-1456
    '\u{1FB37}', // 58: Sextant-2456
    '\u{1FB38}', // 59: Sextant-12456
    '\u{1FB39}', // 60: Sextant-3456
    '\u{1FB3A}', // 61: Sextant-13456
    '\u{1FB3B}', // 62: Sextant-23456
    '\u{2588}',  // 63: Full Block
];

use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
use rayon::prelude::*;

/// Process frame in sextant mode (2×3 sub-pixels per terminal cell).
pub fn process_sextant(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_w = u32::from(grid.width) * 2;
    let pixel_h = u32::from(grid.height) * 3;
    grid.cells
        .par_chunks_mut(grid.width as usize)
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, cell) in row.iter_mut().enumerate() {
                let base_x = (cx as u32) * 2 * frame.width / pixel_w.max(1);
                let base_y = (cy as u32) * 3 * frame.height / pixel_h.max(1);

                // Passe 1 : collecter luminances et couleurs
                let mut lum_values = [0u8; 6];
                let mut lum_sum = 0u32;
                let mut avg_r = 0u32;
                let mut avg_g = 0u32;
                let mut avg_b = 0u32;

                for dy in 0..3u32 {
                    for dx in 0..2u32 {
                        let px = (base_x + dx * frame.width / pixel_w.max(1))
                            .min(frame.width.saturating_sub(1));
                        let py = (base_y + dy * frame.height / pixel_h.max(1))
                            .min(frame.height.saturating_sub(1));

                        let lum = frame.luminance_linear(px, py);
                        let (r, g, b, _) = frame.pixel(px, py);
                        let idx = (dy * 2 + dx) as usize;
                        lum_values[idx] = lum;
                        lum_sum += u32::from(lum);

                        avg_r += u32::from(r);
                        avg_g += u32::from(g);
                        avg_b += u32::from(b);
                    }
                }

                // Passe 2 : seuil adaptatif (moyenne locale)
                let local_threshold = (lum_sum / 6) as u8;
                let mut bitmask = 0u8;
                for bit in 0..6u8 {
                    let on = if config.invert {
                        lum_values[bit as usize] < local_threshold
                    } else {
                        lum_values[bit as usize] > local_threshold
                    };
                    if on {
                        bitmask |= 1 << bit;
                    }
                }

                let ch = get_sextant_char(bitmask);
                let fg = ((avg_r / 6) as u8, (avg_g / 6) as u8, (avg_b / 6) as u8);

                *cell = AsciiCell {
                    ch,
                    fg,
                    bg: (0, 0, 0),
                };
            }
        });
}
