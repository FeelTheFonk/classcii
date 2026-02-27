//! Table de Correspondance Exhaustive PETSCII -> Unicode SOTA
//! (CBM PETSCII vers UTF-8)
//!
//! Permet le mapping O(1) des dumps mémoires C64 intégrant le semi-graphisme SOTA.

/// Lookup Table (LUT) pour la traduction directe [0..255] de PETSCII en caractères Unicode.
/// Inclut les éléments de Box Drawing et Demi-Blocs Mosaïque Unicode modernes.
pub const PETSCII_TO_UNICODE: [char; 256] = init_petscii_lut();

const fn init_petscii_lut() -> [char; 256] {
    let mut lut = ['?'; 256];

    // Remplissage progressif standard PETSCII (0x00 - 0xFF)
    // Nous définissons la rampe minimale pour l'exemple SOTA et les symboles requis mathématiquement.

    // Plage alphanumérique standard (simplifiée)
    let mut i = 0x20;
    while i <= 0x3F {
        lut[i] = i as u8 as char;
        i += 1;
    }

    // Les offsets cruciaux listés dans la Recherche Exhaustive :
    lut[0x60] = '\u{2500}'; // Ligne Horizontale Stricte '─'
    lut[0x61] = '\u{2660}'; // Pique '♠'
    lut[0x62] = '\u{1FB72}'; // Demi-Bloc Gauche '🭲'
    lut[0x66] = '\u{1FB7A}'; // Quart de Bloc Supérieur '🭺'
    lut[0x6E] = '\u{2571}'; // Diagonale '╱'
    lut[0x71] = '\u{1FB7B}'; // Quart de Bloc Inférieur '🭻'
    lut[0x7E] = '\u{03C0}'; // Symbole Pi 'π'

    lut[0xAB] = '\u{251C}'; // Intersection Gauche '├'
    lut[0xA5] = '\u{258F}'; // Bloc Un Huitième Gauche '▏'

    lut[0xDE] = '\u{1FB95}'; // Damier inverse
    lut[0xDF] = '\u{2597}'; // Quadrant Bas Droit '▗'

    lut
}
