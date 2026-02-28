use af_core::frame::FrameBuffer;

/// Detect edge magnitude and angle at pixel (x, y) using simplified Sobel 3×3.
///
/// Returns `(normalized_magnitude [0.0, 1.0], angle_radians [-PI, PI])`.
///
/// # Example
/// ```
/// use af_core::frame::FrameBuffer;
/// use af_ascii::edge::detect_edge;
///
/// let frame = FrameBuffer::new(10, 10);
/// let (mag, _) = detect_edge(&frame, 5, 5);
/// assert!(mag >= 0.0 && mag <= 1.0);
/// ```
#[must_use]
pub fn detect_edge(frame: &FrameBuffer, x: u32, y: u32) -> (f32, f32) {
    if x == 0 || y == 0 || x >= frame.width - 1 || y >= frame.height - 1 {
        return (0.0, 0.0);
    }

    // Sobel kernels
    let tl = f32::from(frame.luminance_linear(x - 1, y - 1));
    let tc = f32::from(frame.luminance_linear(x, y - 1));
    let tr = f32::from(frame.luminance_linear(x + 1, y - 1));
    let ml = f32::from(frame.luminance_linear(x - 1, y));
    let mr = f32::from(frame.luminance_linear(x + 1, y));
    let bl = f32::from(frame.luminance_linear(x - 1, y + 1));
    let bc = f32::from(frame.luminance_linear(x, y + 1));
    let br = f32::from(frame.luminance_linear(x + 1, y + 1));

    let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

    let mag = (gx * gx + gy * gy).sqrt();
    let norm_mag = (mag / 1442.0).min(1.0); // max theoretical: sqrt(2) * 1020 ≈ 1442

    // Si magnitude presque nulle, angle 0
    let angle = if mag < 1.0 { 0.0 } else { gy.atan2(gx) };

    (norm_mag, angle)
}

/// ASCIIfy-Them  logic : Maps an edge angle [-PI, PI] to a strict directional ascii char: `|`, `_`, `/`, `\`.
///
/// Returns `char`
#[must_use]
pub fn ascii_edge_char(angle_rad: f32) -> char {
    // Conversion en degrés normalisés [0, 360)
    let mut deg = angle_rad.to_degrees();
    if deg < 0.0 {
        deg += 360.0;
    }

    // Secteurs asciify-them
    if (80.0..100.0).contains(&deg) || (260.0..280.0).contains(&deg) {
        '|'
    } else if (170.0..190.0).contains(&deg)
        || (350.0..=360.0).contains(&deg)
        || (0.0..10.0).contains(&deg)
    {
        '_'
    } else if (35.0..55.0).contains(&deg) || (215.0..235.0).contains(&deg) {
        '/'
    } else if (125.0..145.0).contains(&deg) || (305.0..325.0).contains(&deg) {
        '\\'
    } else {
        // Fallback for intermediate angles
        '+'
    }
}

///  Vectorial Edge Mapping: Maps gradient (Gx, Gy) to a strict ASCII directional character
/// based on 8-way symmetric quantization.
///
/// Returns one of: '-', '|', '/', '\\', '+', 'o'
///
/// # Example
/// ```
/// use af_ascii::edge::edge_char;
/// let ch = edge_char(1.0, 0.0);
/// assert_eq!(ch, '-');
/// ```
#[must_use]
pub fn edge_char(gx: f32, gy: f32) -> char {
    if gx.abs() < 0.001 && gy.abs() < 0.001 {
        return ' ';
    }

    // Calcul de l'angle en degrés [0, 180) pour symétrie directionnelle.
    let angle = gy.atan2(gx).to_degrees();
    let angle = if angle < 0.0 { angle + 180.0 } else { angle };

    // Cartographie  stricte :
    // 0 / 180 -> Horizontal ('-')
    // 90      -> Vertical   ('|')
    // 45      -> Diagonale Ascendante   ('/')
    // 135     -> Diagonale Descendante  ('\')

    // Tolérance angulaire (22.5 degrés de demi-secteur)
    if !(22.5..157.5).contains(&angle) {
        '-'
    } else if angle < 67.5 {
        '/' // Note: l'axe Y des buffers d'image est inversé (0 en haut), d'où l'inversion de la diagonale par rapport au plan cartésien standard si nécessaire.
    } else if angle < 112.5 {
        '|'
    } else {
        '\\'
    }
}

/// Compute gradient components at pixel (x, y).
///
/// Returns (gx, gy) gradient magnitudes.
#[must_use]
pub fn gradient(frame: &FrameBuffer, x: u32, y: u32) -> (f32, f32) {
    if x == 0 || y == 0 || x >= frame.width - 1 || y >= frame.height - 1 {
        return (0.0, 0.0);
    }

    let tl = f32::from(frame.luminance_linear(x - 1, y - 1));
    let tc = f32::from(frame.luminance_linear(x, y - 1));
    let tr = f32::from(frame.luminance_linear(x + 1, y - 1));
    let ml = f32::from(frame.luminance_linear(x - 1, y));
    let mr = f32::from(frame.luminance_linear(x + 1, y));
    let bl = f32::from(frame.luminance_linear(x - 1, y + 1));
    let bc = f32::from(frame.luminance_linear(x, y + 1));
    let br = f32::from(frame.luminance_linear(x + 1, y + 1));

    let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

    (gx, gy)
}
