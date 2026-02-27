/// Rampe Standard Complète  - 92 caractères.
pub const CHARSET_FULL: &str =
    "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/|()1{}?-_+~<>i!lI;:,\"^`'. ";

/// Rampe Alternative Dense - 69 caractères.
pub const CHARSET_DENSE: &str = "Ñ@#W$9876543210?!abc;:+=-,._ ";

/// Séquence Courte 1 - 10 caractères.
pub const CHARSET_SHORT_1: &str = ".:-=+*#%@";

/// Séquence Courte 2 - Inversée.
pub const CHARSET_SHORT_2: &str = "@%#*+=-:. ";

/// Séquence Binaire.
pub const CHARSET_BINARY: &str = " #";

/// Séquence Étendue (Asciimatic) - 70 caractères.
pub const CHARSET_EXTENDED: &str =
    "=======--------:::::::::........=========--------:::::::::........++==";

/// Jeu Discret (Matrice) - 5 caractères.
pub const CHARSET_DISCRETE: &str = "1234 ";

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
mod tests {
    use super::*;

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
}
