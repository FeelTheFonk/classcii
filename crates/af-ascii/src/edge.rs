use af_core::frame::FrameBuffer;

/// Detect edge magnitude at pixel (x, y) using simplified Sobel 3×3.
///
/// Returns normalized edge magnitude [0.0, 1.0].
///
/// # Example
/// ```
/// use af_core::frame::FrameBuffer;
/// use af_ascii::edge::detect_edge;
///
/// let frame = FrameBuffer::new(10, 10);
/// let edge = detect_edge(&frame, 5, 5);
/// assert!(edge >= 0.0 && edge <= 1.0);
/// ```
#[must_use]
pub fn detect_edge(frame: &FrameBuffer, x: u32, y: u32) -> f32 {
    if x == 0 || y == 0 || x >= frame.width - 1 || y >= frame.height - 1 {
        return 0.0;
    }

    // Sobel kernels
    let tl = f32::from(frame.luminance(x - 1, y - 1));
    let tc = f32::from(frame.luminance(x, y - 1));
    let tr = f32::from(frame.luminance(x + 1, y - 1));
    let ml = f32::from(frame.luminance(x - 1, y));
    let mr = f32::from(frame.luminance(x + 1, y));
    let bl = f32::from(frame.luminance(x - 1, y + 1));
    let bc = f32::from(frame.luminance(x, y + 1));
    let br = f32::from(frame.luminance(x + 1, y + 1));

    let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

    let mag = (gx * gx + gy * gy).sqrt();
    (mag / 1442.0).min(1.0) // max theoretical: sqrt(2) * 1020 ≈ 1442
}

/// Select an edge character based on gradient direction.
///
/// Returns one of: '─', '│', '╱', '╲'
///
/// # Example
/// ```
/// use af_ascii::edge::edge_char;
/// let ch = edge_char(1.0, 0.0);
/// assert_eq!(ch, '─');
/// ```
#[must_use]
pub fn edge_char(gx: f32, gy: f32) -> char {
    if gx.abs() < 0.001 && gy.abs() < 0.001 {
        return ' ';
    }

    let angle = gy.atan2(gx).to_degrees();
    let angle = if angle < 0.0 { angle + 180.0 } else { angle };

    if !(22.5..157.5).contains(&angle) {
        '─'
    } else if angle < 67.5 {
        '╲'
    } else if angle < 112.5 {
        '│'
    } else {
        '╱'
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

    let tl = f32::from(frame.luminance(x - 1, y - 1));
    let tc = f32::from(frame.luminance(x, y - 1));
    let tr = f32::from(frame.luminance(x + 1, y - 1));
    let ml = f32::from(frame.luminance(x - 1, y));
    let mr = f32::from(frame.luminance(x + 1, y));
    let bl = f32::from(frame.luminance(x - 1, y + 1));
    let bc = f32::from(frame.luminance(x, y + 1));
    let br = f32::from(frame.luminance(x + 1, y + 1));

    let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;

    (gx, gy)
}
