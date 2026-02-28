/// LUT de décompression gamma sRGB → linéaire (approximation gamma 2.0).
///
/// `SRGB_TO_LINEAR[v]` ≈ `(v/255)^2`. Précision suffisante pour le rendu ASCII.
/// Compile-time const, zero-alloc.
const SRGB_TO_LINEAR: [f32; 256] = {
    let mut lut = [0.0f32; 256];
    let mut i = 0;
    while i < 256 {
        let s = i as f32 / 255.0;
        lut[i] = s * s; // gamma ~2.0
        i += 1;
    }
    lut
};

/// Buffer de pixels réutilisable. Pré-alloué, jamais redimensionné en hot path.
///
/// Stocke les pixels en RGBA row-major, 4 bytes par pixel.
///
/// # Example
/// ```
/// use af_core::frame::FrameBuffer;
/// let fb = FrameBuffer::new(10, 10);
/// assert_eq!(fb.data.len(), 400);
/// ```
pub struct FrameBuffer {
    /// Pixels RGBA, row-major, 4 bytes par pixel.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl FrameBuffer {
    /// Crée un buffer pré-alloué aux dimensions données.
    ///
    /// # Example
    /// ```
    /// use af_core::frame::FrameBuffer;
    /// let fb = FrameBuffer::new(100, 50);
    /// assert_eq!(fb.width, 100);
    /// assert_eq!(fb.height, 50);
    /// assert_eq!(fb.data.len(), 100 * 50 * 4);
    /// ```
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            data: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    /// Accès au pixel (x, y) → (r, g, b, a).
    ///
    /// # Example
    /// ```
    /// use af_core::frame::FrameBuffer;
    /// let fb = FrameBuffer::new(10, 10);
    /// let (r, g, b, a) = fb.pixel(0, 0);
    /// assert_eq!((r, g, b, a), (0, 0, 0, 0));
    /// ```
    #[inline(always)]
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> (u8, u8, u8, u8) {
        debug_assert!(x < self.width && y < self.height, "pixel out of bounds");
        let idx = ((y * self.width + x) * 4) as usize;
        if idx + 3 >= self.data.len() {
            return (0, 0, 0, 0);
        }
        (
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        )
    }

    /// Luminance perceptuelle BT.709.
    ///
    /// # Example
    /// ```
    /// use af_core::frame::FrameBuffer;
    /// let mut fb = FrameBuffer::new(1, 1);
    /// fb.data[0] = 255; fb.data[1] = 255; fb.data[2] = 255; fb.data[3] = 255;
    /// assert_eq!(fb.luminance(0, 0), 255);
    /// ```
    #[inline(always)]
    #[must_use]
    pub fn luminance(&self, x: u32, y: u32) -> u8 {
        let (r, g, b, _) = self.pixel(x, y);
        ((u32::from(r) * 2126 + u32::from(g) * 7152 + u32::from(b) * 722) / 10000) as u8
    }

    /// Luminance perceptuelle avec linéarisation sRGB (gamma ~2.0).
    ///
    /// Plus précise que `luminance()` pour les tons sombres et les gradients.
    /// BT.709 appliqué en espace linéaire, reconversion gamma via sqrt.
    #[inline(always)]
    #[must_use]
    pub fn luminance_linear(&self, x: u32, y: u32) -> u8 {
        let (r, g, b, _) = self.pixel(x, y);
        let lr = SRGB_TO_LINEAR[r as usize];
        let lg = SRGB_TO_LINEAR[g as usize];
        let lb = SRGB_TO_LINEAR[b as usize];
        // BT.709 en espace linéaire
        let linear_lum = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
        // Reconversion gamma ~2.0 via sqrt
        let gamma_lum = linear_lum.sqrt();
        (gamma_lum * 255.0) as u8
    }

    /// Échantillonnage moyenné sur une région rectangulaire.
    ///
    /// Retourne `(avg_r, avg_g, avg_b, avg_luminance_linear)`.
    /// Zero-alloc : arithmétique pure. Fast-path si la région est 1×1.
    #[inline]
    #[must_use]
    pub fn area_sample(&self, x0: u32, y0: u32, x1: u32, y1: u32) -> (u8, u8, u8, u8) {
        let x0 = x0.min(self.width.saturating_sub(1));
        let y0 = y0.min(self.height.saturating_sub(1));
        let x1 = x1.min(self.width);
        let y1 = y1.min(self.height);

        // Fast-path: 1×1 ou dégénéré
        if x1 <= x0 + 1 && y1 <= y0 + 1 {
            let (r, g, b, _) = self.pixel(x0, y0);
            return (r, g, b, self.luminance_linear(x0, y0));
        }

        let mut sr = 0u32;
        let mut sg = 0u32;
        let mut sb = 0u32;
        let mut count = 0u32;
        for py in y0..y1 {
            for px in x0..x1 {
                let idx = ((py * self.width + px) * 4) as usize;
                if idx + 2 < self.data.len() {
                    sr += u32::from(self.data[idx]);
                    sg += u32::from(self.data[idx + 1]);
                    sb += u32::from(self.data[idx + 2]);
                    count += 1;
                }
            }
        }
        if count == 0 {
            return (0, 0, 0, 0);
        }
        let ar = (sr / count) as u8;
        let ag = (sg / count) as u8;
        let ab = (sb / count) as u8;
        // Luminance linéaire depuis la couleur moyennée
        let lr = SRGB_TO_LINEAR[ar as usize];
        let lg = SRGB_TO_LINEAR[ag as usize];
        let lb = SRGB_TO_LINEAR[ab as usize];
        let lum = (0.2126 * lr + 0.7152 * lg + 0.0722 * lb).sqrt();
        (ar, ag, ab, (lum * 255.0) as u8)
    }
}

/// Grille de sortie ASCII. Pré-allouée, réutilisée chaque frame.
///
/// # Example
/// ```
/// use af_core::frame::{AsciiGrid, AsciiCell};
/// let mut grid = AsciiGrid::new(80, 24);
/// grid.set(0, 0, AsciiCell { ch: '@', fg: (255, 0, 0), bg: (0, 0, 0) });
/// assert_eq!(grid.get(0, 0).ch, '@');
/// ```
#[derive(Clone)]
pub struct AsciiGrid {
    /// Flat array of cells, row-major.
    pub cells: Vec<AsciiCell>,
    /// Width in characters.
    pub width: u16,
    /// Height in characters.
    pub height: u16,
}

/// Single cell in the ASCII grid.
///
/// # Example
/// ```
/// use af_core::frame::AsciiCell;
/// let cell = AsciiCell::default();
/// assert_eq!(cell.ch, ' ');
/// ```
#[derive(Clone, Copy)]
pub struct AsciiCell {
    /// Caractère à afficher.
    pub ch: char,
    /// Couleur foreground (RGB).
    pub fg: (u8, u8, u8),
    /// Couleur background (RGB). (0,0,0) = transparent/default.
    pub bg: (u8, u8, u8),
}

impl Default for AsciiCell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: (0, 0, 0),
            bg: (0, 0, 0),
        }
    }
}

impl AsciiGrid {
    /// Crée une grille pré-allouée.
    ///
    /// # Example
    /// ```
    /// use af_core::frame::AsciiGrid;
    /// let grid = AsciiGrid::new(80, 24);
    /// assert_eq!(grid.cells.len(), 80 * 24);
    /// ```
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            cells: vec![AsciiCell::default(); width as usize * height as usize],
            width,
            height,
        }
    }

    /// Set a cell at position (x, y).
    ///
    /// # Example
    /// ```
    /// use af_core::frame::{AsciiGrid, AsciiCell};
    /// let mut grid = AsciiGrid::new(10, 10);
    /// grid.set(5, 5, AsciiCell { ch: '#', fg: (255, 255, 255), bg: (0, 0, 0) });
    /// ```
    #[inline(always)]
    pub fn set(&mut self, x: u16, y: u16, cell: AsciiCell) {
        self.cells[y as usize * self.width as usize + x as usize] = cell;
    }

    /// Get a cell reference at position (x, y).
    ///
    /// # Example
    /// ```
    /// use af_core::frame::AsciiGrid;
    /// let grid = AsciiGrid::new(10, 10);
    /// let cell = grid.get(0, 0);
    /// assert_eq!(cell.ch, ' ');
    /// ```
    #[inline(always)]
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> &AsciiCell {
        &self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Copy all cells from `other` into this grid.
    ///
    /// If dimensions differ, this is a no-op.
    /// Zero allocation — reuses the existing buffer.
    #[inline]
    pub fn copy_from(&mut self, other: &AsciiGrid) {
        if self.width == other.width && self.height == other.height {
            self.cells.copy_from_slice(&other.cells);
        }
    }

    /// Clear all cells to default (space, black).
    ///
    /// # Example
    /// ```
    /// use af_core::frame::{AsciiGrid, AsciiCell};
    /// let mut grid = AsciiGrid::new(10, 10);
    /// grid.set(0, 0, AsciiCell { ch: '#', fg: (255, 0, 0), bg: (0, 0, 0) });
    /// grid.clear();
    /// assert_eq!(grid.get(0, 0).ch, ' ');
    /// ```
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            *cell = AsciiCell::default();
        }
    }
}

/// Résultat de l'analyse audio pour une frame temporelle.
///
/// Écrit par le thread audio, lu par le thread de rendu.
/// Taille fixe, Copy, jamais alloué dynamiquement.
///
/// # Example
/// ```
/// use af_core::frame::AudioFeatures;
/// let f = AudioFeatures::default();
/// assert_eq!(f.rms, 0.0);
/// ```
#[derive(Clone, Copy, Default)]
pub struct AudioFeatures {
    // === Amplitude ===
    /// RMS (Root Mean Square) normalisé [0.0, 1.0].
    pub rms: f32,
    /// Peak amplitude de la fenêtre courante [0.0, 1.0].
    pub peak: f32,

    // === Bandes de fréquence (énergie normalisée [0.0, 1.0]) ===
    /// Sub-bass : 20–60 Hz.
    pub sub_bass: f32,
    /// Bass : 60–250 Hz.
    pub bass: f32,
    /// Low-mid : 250–500 Hz.
    pub low_mid: f32,
    /// Mid : 500–2000 Hz.
    pub mid: f32,
    /// High-mid : 2000–4000 Hz.
    pub high_mid: f32,
    /// Presence : 4000–6000 Hz.
    pub presence: f32,
    /// Brilliance : 6000–20000 Hz.
    pub brilliance: f32,

    // === Features spectrales ===
    /// Centroïde spectral normalisé [0.0, 1.0] (brillance du timbre).
    pub spectral_centroid: f32,
    /// Flux spectral [0.0, 1.0] (changement entre frames).
    pub spectral_flux: f32,
    /// Flatness spectrale [0.0, 1.0] (bruit vs tonal).
    pub spectral_flatness: f32,

    // === Détection d'événements ===
    /// True si un onset (attaque) est détecté dans cette frame.
    pub onset: bool,
    /// Onset strength [0.0, 1.0] (graduated, not just bool).
    pub beat_intensity: f32,
    /// BPM estimé (0.0 si inconnu).
    pub bpm: f32,
    /// Phase du beat [0.0, 1.0] (0.0 = sur le beat, 0.5 = entre deux beats).
    pub beat_phase: f32,

    // === Spectre compressé pour visualisation ===
    /// 32 bandes log-fréquence, normalisées [0.0, 1.0].
    pub spectrum_bands: [f32; 32],
}
