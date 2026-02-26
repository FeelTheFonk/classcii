/// Shape matching engine: bitmap correlation per spec §49.

/// Shape matcher using 5×5 hardcoded bitmaps for ASCII characters.
///
/// # Example
/// ```
/// use af_ascii::shape_match::ShapeMatcher;
/// let matcher = ShapeMatcher::new();
/// ```
pub struct ShapeMatcher {
    /// Pre-computed bitmaps for common characters (char, bitmap_25bit).
    entries: Vec<(char, u32)>,
}

impl ShapeMatcher {
    /// Create a new shape matcher with the hardcoded character set.
    #[must_use]
    pub fn new() -> Self {
        let mut entries = Vec::with_capacity(64);

        // Hardcoded 5×5 bitmaps (row-major, LSB first)
        let table: &[(char, u32)] = &[
            (' ', 0b00000_00000_00000_00000_00000),
            ('.', 0b00000_00000_00000_00100_00000),
            ('-', 0b00000_00000_11111_00000_00000),
            ('|', 0b00100_00100_00100_00100_00100),
            ('+', 0b00100_00100_11111_00100_00100),
            ('/', 0b00001_00010_00100_01000_10000),
            ('\\', 0b10000_01000_00100_00010_00001),
            ('O', 0b01110_10001_10001_10001_01110),
            ('#', 0b01010_11111_01010_11111_01010),
            ('@', 0b01110_10001_10111_10001_01110),
            ('A', 0b01110_10001_11111_10001_10001),
            ('M', 0b10001_11011_10101_10001_10001),
            ('W', 0b10001_10001_10101_11011_10001),
            ('█', 0b11111_11111_11111_11111_11111),
            ('░', 0b10100_01010_10100_01010_10100),
            ('▒', 0b10101_01010_10101_01010_10101),
            ('▓', 0b01011_10101_01011_10101_01011),
        ];

        for &(ch, bm) in table {
            entries.push((ch, bm));
        }
        Self { entries }
    }

    /// Match a 5×5 luminance block to the best character.
    ///
    /// `block` is a 25-element array of luminance values, row-major.
    /// Returns the best matching character.
    ///
    /// # Example
    /// ```
    /// use af_ascii::shape_match::ShapeMatcher;
    /// let matcher = ShapeMatcher::new();
    /// let block = [0u8; 25]; // all black
    /// let ch = matcher.match_cell(&block);
    /// assert_eq!(ch, ' ');
    /// ```
    #[must_use]
    pub fn match_cell(&self, block: &[u8; 25]) -> char {
        // Convert block to binary bitmap (threshold at 128)
        let mut input_bitmap = 0u32;
        for (i, &lum) in block.iter().enumerate() {
            if lum > 128 {
                input_bitmap |= 1 << i;
            }
        }

        // Find best match by bit correlation (popcount of XNOR)
        let mut best_char = ' ';
        let mut best_score = 0u32;

        for &(ch, pattern) in &self.entries {
            let xnor = !(input_bitmap ^ pattern) & 0x01FF_FFFF; // 25 bits
            let score = xnor.count_ones();
            if score > best_score {
                best_score = score;
                best_char = ch;
            }
        }

        best_char
    }
}

impl Default for ShapeMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a hardcoded 5×5 bitmap for a character.
///
/// # Example
/// ```
/// use af_ascii::shape_match::get_bitmap;
/// let bm = get_bitmap('#');
/// assert_ne!(bm, 0);
/// ```
#[must_use]
pub fn get_bitmap(ch: char) -> u32 {
    match ch {
        ' ' => 0b00000_00000_00000_00000_00000,
        '.' => 0b00000_00000_00000_00100_00000,
        '-' => 0b00000_00000_11111_00000_00000,
        '|' => 0b00100_00100_00100_00100_00100,
        '+' => 0b00100_00100_11111_00100_00100,
        '/' => 0b00001_00010_00100_01000_10000,
        '\\' => 0b10000_01000_00100_00010_00001,
        'O' => 0b01110_10001_10001_10001_01110,
        '#' => 0b01010_11111_01010_11111_01010,
        '@' => 0b01110_10001_10111_10001_01110,
        'A' => 0b01110_10001_11111_10001_10001,
        'M' => 0b10001_11011_10101_10001_10001,
        'W' => 0b10001_10001_10101_11011_10001,
        '█' => 0b11111_11111_11111_11111_11111,
        '░' => 0b10100_01010_10100_01010_10100,
        '▒' => 0b10101_01010_10101_01010_10101,
        '▓' => 0b01011_10101_01011_10101_01011,
        _ => estimate_density(ch),
    }
}

fn estimate_density(ch: char) -> u32 {
    let density = match ch {
        'a'..='z' => 12,
        'A'..='Z' => 14,
        '0'..='9' => 13,
        _ => 8,
    };
    // Centre-out fill pattern
    let order: [u32; 25] = [
        12, 7, 2, 8, 14, 6, 1, 0, 3, 9, 11, 5, 4, 10, 16, 13, 17, 18, 19, 23, 20, 21, 22, 24,
        15,
    ];
    let mut bm = 0u32;
    for &bit in order.iter().take(density) {
        bm |= 1 << bit;
    }
    bm
}
