//! Implémentation mathématique O(1) Braille (U+2800)
//!
//! Bits activés :
//! +---+---+
//! | 1 | 4 |
//! +---+---+
//! | 2 | 5 |
//! +---+---+
//! | 3 | 6 |
//! +---+---+
//! | 7 | 8 |
//! +---+---+

/// Map un entier 8-bits (0 à 255) vers le caractère Braille correspondant.
/// Calcule mathématiquement le point de code `U+2800 + offset`.
#[must_use]
#[inline(always)]
pub const fn get_braille_char(bitmask: u8) -> char {
    // Le bloc Unicode Braille est parfaitement mappé bit à bit sur l'offset 0x2800.
    // Zero-cost abstraction SOTA.
    match std::char::from_u32(0x2800 + bitmask as u32) {
        Some(c) => c,
        None => ' ',
    }
}
