//! Algorithmique de Tramage Ordonné (Ordered Dithering) 
//! Déploiement des matrices de Bayer pour l'élargissement d'histogramme sans banding.

/// Matrice de Bayer 2x2. Normalisée sur 4 niveaux (0-3).
pub const BAYER_2X2: [[u8; 2]; 2] = [[0, 2], [3, 1]];

/// Matrice de Bayer 4x4. Normalisée sur 16 niveaux (0-15).
pub const BAYER_4X4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Matrice de Bayer 8x8. Normalisée sur 64 niveaux (0-63).
pub const BAYER_8X8: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Applique le tramage (Bayer 8x8) à une valeur de luminance brute [0..255].
/// L'ajout du bruit de quantification organisé casse le banding perceptible
/// sur les dégradés subtils, optimisant la projection sur chaîne ASCII stricte.
///
/// # Arguments
/// * `lum` - Luminance initiale (0 à 255)
/// * `x`, `y` - Coordonnées spatiales (en pixels)
/// * `levels` - Le nombre de sous-paliers du dither (parfait si équivalent à `charset.len()`)
#[must_use]
#[inline(always)]
pub fn apply_bayer_8x8(lum: u8, x: u32, y: u32, levels: f32) -> u8 {
    // Si la luminance est extrêmement proche des bornes, on évite le dither
    if !(2..=253).contains(&lum) {
        return lum;
    }

    let bayer_val = f32::from(BAYER_8X8[(y % 8) as usize][(x % 8) as usize]);
    // La matrice de Bayer 8x8 va de 0 à 63.
    // On centre la pondération entre -0.5 et +0.5 autour du seuil.
    // L'amplitude `1.0 / levels` s'assure que le bruit couvre exactement 1 palier de quantification (distance entre 2 index de char).
    let threshold = (bayer_val / 64.0) - 0.5;

    // Convertir de u8 [0, 255] à f32 [0.0, 1.0]
    let base_val = f32::from(lum) / 255.0;

    // Le décalage ordonné pré-quantification
    let dithered_val = (base_val + threshold * (1.0 / levels.max(2.0))).clamp(0.0, 1.0);

    // On retourne en espace 0..255 optimisé
    (dithered_val * 255.0).round() as u8
}
