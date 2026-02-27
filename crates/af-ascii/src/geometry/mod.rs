//! TUI Geometry & Rétrocompatibilité (Box Drawing, CBM PETSCII)

pub mod petscii;

/// Convertit un motif de connexion 4-way en caractère Unicode Box Drawing.
/// `up`, `down`, `left`, `right`: les branches à connecter.
/// Retourne le caractère strict de la table U+2500 - U+257F.
#[must_use]
#[allow(clippy::fn_params_excessive_bools)]
pub const fn box_drawing_char(up: bool, down: bool, left: bool, right: bool) -> char {
    match (up, down, left, right) {
        (false, false, false, false) => ' ',
        // Lignes droites
        (true, true, false, false) => '\u{2502}', // │
        (false, false, true, true) => '\u{2500}', // ─
        // Coins
        (false, true, false, true) => '\u{250C}', // ┌
        (false, true, true, false) => '\u{2510}', // ┐
        (true, false, false, true) => '\u{2514}', // └
        (true, false, true, false) => '\u{2518}', // ┘
        // T-Junctions
        (false, true, true, true) => '\u{252C}', // ┬
        (true, false, true, true) => '\u{2534}', // ┴
        (true, true, false, true) => '\u{251C}', // ├
        (true, true, true, false) => '\u{2524}', // ┤
        // Croix
        (true, true, true, true) => '\u{253C}', // ┼
        // Demi-lignes solitaires
        (true, false, false, false) => '\u{2575}', // ╵
        (false, true, false, false) => '\u{2577}', // ╷
        (false, false, true, false) => '\u{2574}', // ╴
        (false, false, false, true) => '\u{2576}', // ╶
    }
}
