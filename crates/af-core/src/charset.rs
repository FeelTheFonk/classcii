/// 10 caractères — compact, bon contraste.
pub const CHARSET_COMPACT: &str = " .:-=+*#%@";

/// 70 caractères — Paul Bourke extended, bon équilibre.
pub const CHARSET_STANDARD: &str =
    " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// 70 caractères — Paul Bourke, résolution maximale (inversé: dense→clair).
pub const CHARSET_FULL: &str =
    "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/\\|()1{}[]?-_+~<>i!lI;:,\"^`'. ";

/// Blocs Unicode — pseudo-pixels.
pub const CHARSET_BLOCKS: &str = " ░▒▓█";

/// Minimal — haut contraste.
pub const CHARSET_MINIMAL: &str = " .:░▒▓█";

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
        let len = if chars.len() >= 2 {
            chars.len()
        } else {
            // Fallback: if charset is too short, use a minimal default.
            return Self::new(" @");
        };
        let mut lut = [' '; 256];
        for (i, slot) in lut.iter_mut().enumerate() {
            *slot = chars[i * (len - 1) / 255];
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
            let idx = chars.iter().position(|&c| c == ch).unwrap();
            assert!(idx >= prev_idx, "LUT non monotone à luminance {i}");
            prev_idx = idx;
        }
    }
}
