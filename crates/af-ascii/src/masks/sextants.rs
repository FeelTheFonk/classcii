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

/// Lookup Table pré-calculée. Index 0..=63 correspond à la valeur binaire.
/// Bit 0: HautGauche, Bit 1: MilieuGauche, Bit 2: BasGauche, Bit 3: HautDroit, Bit 4: MilieuDroit, Bit 5: BasDroit.
/// (Selon la norme  `Recherche exhaustive de combinaisons maximales.md` section 3.2).
const SEXTANT_LUT: [char; 64] = [
    ' ',         // 0: Vide
    '\u{1FB00}', // 1
    '\u{1FB01}', // 2
    '\u{1FB02}', // 3
    '\u{1FB03}', // 4
    '\u{1FB04}', // 5
    '\u{1FB05}', // 6
    '\u{1FB06}', // 7
    '\u{1FB07}', // 8
    '\u{1FB08}', // 9
    '\u{1FB0A}', // 10 (1, 2, 4) -> U+1FB0A
    '\u{1FB0B}', // 11
    '\u{1FB0C}', // 12
    '\u{1FB0D}', // 13
    '\u{1FB0E}', // 14
    '\u{1FB0F}', // 15
    '\u{1FB10}', // 16
    '\u{1FB11}', // 17
    '\u{1FB12}', // 18
    '\u{1FB13}', // 19
    '\u{1FB14}', // 20
    '\u{1FB15}', // 21
    '\u{1FB16}', // 22
    '\u{1FB17}', // 23
    '\u{1FB18}', // 24
    '\u{1FB19}', // 25
    '\u{1FB1A}', // 26
    '\u{1FB1B}', // 27
    '\u{1FB1C}', // 28
    '\u{1FB1D}', // 29
    '\u{1FB1E}', // 30
    '\u{1FB1F}', // 31
    '\u{1FB20}', // 32
    '\u{1FB21}', // 33
    '\u{1FB22}', // 34
    '\u{1FB23}', // 35
    '\u{1FB24}', // 36
    '\u{1FB25}', // 37
    '\u{1FB26}', // 38
    '\u{1FB27}', // 39
    '\u{1FB28}', // 40
    '\u{1FB29}', // 41
    '\u{1FB2A}', // 42
    '\u{1FB2B}', // 43
    '\u{1FB2C}', // 44
    '\u{1FB2D}', // 45
    '\u{1FB2E}', // 46
    '\u{1FB2F}', // 47
    '\u{1FB30}', // 48
    '\u{1FB31}', // 49
    '\u{1FB32}', // 50
    '\u{1FB33}', // 51
    '\u{1FB34}', // 52
    '\u{1FB35}', // 53
    '\u{1FB36}', // 54
    '\u{1FB37}', // 55
    '\u{1FB38}', // 56
    '\u{1FB39}', // 57
    '\u{1FB3A}', // 58
    '\u{1FB3B}', // 59
    '\u{1FB3C}', // 60
    '\u{1FB3D}', // 61
    '\u{1FB3E}', // 62
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
