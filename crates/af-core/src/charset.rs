/// Rampe Standard Complète — 70 caractères, lightest→densest.
pub const CHARSET_FULL: &str =
    " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// Rampe Alternative Dense — 29 caractères, lightest→densest.
pub const CHARSET_DENSE: &str = " _.,=-+:;cba!?0123456789$W#@Ñ";

/// Séquence Courte 1 - 10 caractères.
pub const CHARSET_SHORT_1: &str = ".:-=+*#%@";

/// Séquence Courte 2 — lightest→densest.
pub const CHARSET_SHORT_2: &str = " .:-=+*#%@";

/// Séquence Binaire.
pub const CHARSET_BINARY: &str = " #";

/// Séquence Étendue — 11 caractères, ASCII + Unicode léger.
pub const CHARSET_EXTENDED: &str = " .·:;+xX#%@";

/// Jeu Discret (Matrice) — 5 caractères, lightest→densest.
pub const CHARSET_DISCRETE: &str = " 1234";

/// Jeu Edge Detection - 6 caractères.
pub const CHARSET_EDGE: &str = ".,*+#@";

/// Blocs Unicode — pseudo-pixels.
pub const CHARSET_BLOCKS: &str = " ░▒▓█";

/// Minimal — haut contraste.
pub const CHARSET_MINIMAL: &str = " .:░▒▓█";

/// Glitch 1 — contraste brutal organique.
pub const CHARSET_GLITCH_1: &str = " .°*O0@#&%";

/// Glitch 2 — barres de visualisation de spectre / data.
pub const CHARSET_GLITCH_2: &str = " ▂▃▄▅▆▇█";

/// Digital Matrix — purisme binaire et cryptique.
pub const CHARSET_DIGITAL: &str = " 01";

/// Haute Résolution — 34 caractères ASCII purs, gradient fin optimisé pour
/// les grandes cellules de caractères (batch export scale 24-48px).
/// Exclut les lettres minuscules (lisibles et distractantes à grande taille).
pub const CHARSET_HIRES: &str = " .'`:,;_-~\"!|/\\(){}[]<>+*=?^#%&@$";

/// Lookup table mapping luminance [0..255] → character.
///
/// Pre-computed at startup for O(1) per-pixel cost.
///
/// # Example
/// ```
/// use af_core::charset::LuminanceLut;
/// let lut = LuminanceLut::new(" .:#@");
/// assert_eq!(lut.map(0), ' ');
/// assert_eq!(lut.map(255), '@');
/// ```
pub struct LuminanceLut {
    lut: [char; 256],
}

impl LuminanceLut {
    /// Build a LUT from a charset ordered lightest→densest.
    ///
    /// # Panics
    /// Panics if charset has fewer than 2 characters.
    ///
    /// # Example
    /// ```
    /// use af_core::charset::LuminanceLut;
    /// let lut = LuminanceLut::new(" .:#@");
    /// assert_eq!(lut.map(0), ' ');
    /// assert_eq!(lut.map(255), '@');
    /// ```
    #[must_use]
    pub fn new(charset: &str) -> Self {
        let chars: Vec<char> = charset.chars().collect();
        let len = chars.len();
        if len < 2 {
            // Fallback: if charset is too short, use a minimal default.
            return Self::new(" @");
        }
        let mut lut = [' '; 256];
        let max_idx = (len - 1) as f32;

        for (i, slot) in lut.iter_mut().enumerate() {
            // Équation de projection linéaire  avec arrondi : char_idx = round(lum_norm * (N-1))
            let char_idx = ((i as f32 / 255.0) * max_idx).round() as usize;
            *slot = chars[char_idx.min(len - 1)];
        }
        Self { lut }
    }

    /// Map a luminance value [0..255] to a character.
    ///
    /// # Example
    /// ```
    /// use af_core::charset::LuminanceLut;
    /// let lut = LuminanceLut::new(" .:#@");
    /// assert_eq!(lut.map(128), ':');
    /// ```
    #[inline(always)]
    #[must_use]
    pub fn map(&self, luminance: u8) -> char {
        self.lut[luminance as usize]
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    const ALL_CHARSETS: &[(&str, &str)] = &[
        ("FULL", CHARSET_FULL),
        ("DENSE", CHARSET_DENSE),
        ("SHORT_1", CHARSET_SHORT_1),
        ("SHORT_2", CHARSET_SHORT_2),
        ("BINARY", CHARSET_BINARY),
        ("EXTENDED", CHARSET_EXTENDED),
        ("DISCRETE", CHARSET_DISCRETE),
        ("EDGE", CHARSET_EDGE),
        ("BLOCKS", CHARSET_BLOCKS),
        ("MINIMAL", CHARSET_MINIMAL),
        ("GLITCH_1", CHARSET_GLITCH_1),
        ("GLITCH_2", CHARSET_GLITCH_2),
        ("DIGITAL", CHARSET_DIGITAL),
        ("HIRES", CHARSET_HIRES),
    ];

    #[test]
    fn luminance_lut_maps_extremes() {
        let lut = LuminanceLut::new(" .:#@");
        assert_eq!(lut.map(0), ' ');
        assert_eq!(lut.map(255), '@');
    }

    #[test]
    fn luminance_lut_monotonic() {
        let lut = LuminanceLut::new(" .:#@");
        let mut prev_idx = 0usize;
        let chars: Vec<char> = " .:#@".chars().collect();
        for i in 0..=255u8 {
            let ch = lut.map(i);
            let idx = chars.iter().position(|&c| c == ch).unwrap_or(0);
            assert!(idx >= prev_idx, "LUT non monotone à luminance {i}");
            prev_idx = idx;
        }
    }

    #[test]
    fn luminance_lut_hires_monotonic() {
        let lut = LuminanceLut::new(CHARSET_HIRES);
        let chars: Vec<char> = CHARSET_HIRES.chars().collect();
        let mut prev_idx = 0usize;
        for i in 0..=255u8 {
            let ch = lut.map(i);
            let idx = chars.iter().position(|&c| c == ch).unwrap_or(0);
            assert!(idx >= prev_idx, "HIRES LUT non monotone à luminance {i}");
            prev_idx = idx;
        }
    }

    #[test]
    fn all_charsets_minimum_length() {
        for (name, cs) in ALL_CHARSETS {
            let len = cs.chars().count();
            assert!(
                len >= 2,
                "CHARSET_{name} has only {len} chars (minimum 2 required)"
            );
        }
    }

    #[test]
    fn all_charsets_no_replacement_character() {
        for (name, cs) in ALL_CHARSETS {
            for (i, ch) in cs.chars().enumerate() {
                assert!(
                    ch != '\u{FFFD}',
                    "CHARSET_{name}[{i}] is U+FFFD REPLACEMENT CHARACTER"
                );
            }
        }
    }

    #[test]
    fn all_charsets_no_null() {
        for (name, cs) in ALL_CHARSETS {
            for (i, ch) in cs.chars().enumerate() {
                assert!(ch != '\0', "CHARSET_{name}[{i}] is null character");
            }
        }
    }

    #[test]
    fn all_charsets_no_control_chars() {
        for (name, cs) in ALL_CHARSETS {
            for (i, ch) in cs.chars().enumerate() {
                let cp = ch as u32;
                // Allow space (0x20), reject all other C0/C1 controls
                if cp == 0x20 {
                    continue;
                }
                assert!(
                    cp >= 0x20 && !(0x7F..=0x9F).contains(&cp),
                    "CHARSET_{name}[{i}] is control char U+{cp:04X}"
                );
            }
        }
    }

    #[test]
    fn all_charsets_no_zero_width() {
        let zero_width = [
            0x200B, // ZERO WIDTH SPACE
            0x200C, // ZERO WIDTH NON-JOINER
            0x200D, // ZERO WIDTH JOINER
            0x200E, // LEFT-TO-RIGHT MARK
            0x200F, // RIGHT-TO-LEFT MARK
            0xFEFF, // ZERO WIDTH NO-BREAK SPACE (BOM)
        ];
        for (name, cs) in ALL_CHARSETS {
            for (i, ch) in cs.chars().enumerate() {
                let cp = ch as u32;
                assert!(
                    !zero_width.contains(&cp),
                    "CHARSET_{name}[{i}] is zero-width char U+{cp:04X}"
                );
            }
        }
    }

    #[test]
    fn all_charsets_no_duplicates() {
        for (name, cs) in ALL_CHARSETS {
            let chars: Vec<char> = cs.chars().collect();
            for (i, &ch) in chars.iter().enumerate() {
                for (j, &ch2) in chars.iter().enumerate().skip(i + 1) {
                    assert!(
                        ch != ch2,
                        "CHARSET_{name} has duplicate '{ch}' at indices {i} and {j}"
                    );
                }
            }
        }
    }

    #[test]
    fn all_charsets_build_valid_lut() {
        for (name, cs) in ALL_CHARSETS {
            let lut = LuminanceLut::new(cs);
            // Verify no panic and extremes are correct
            let chars: Vec<char> = cs.chars().collect();
            assert_eq!(
                lut.map(0),
                chars[0],
                "CHARSET_{name} LUT[0] should be first char"
            );
            assert_eq!(
                lut.map(255),
                *chars.last().expect("non-empty"),
                "CHARSET_{name} LUT[255] should be last char"
            );
        }
    }
}
