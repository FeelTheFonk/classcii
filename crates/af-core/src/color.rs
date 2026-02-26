/// Convertit RGB [0,255] → HSV. H ∈ [0.0, 1.0), S ∈ [0.0, 1.0], V ∈ [0.0, 1.0].
///
/// # Example
/// ```
/// use af_core::color::rgb_to_hsv;
/// let (h, s, v) = rgb_to_hsv(255, 0, 0);
/// assert!((h - 0.0).abs() < 0.01);
/// assert!((s - 1.0).abs() < 0.01);
/// assert!((v - 1.0).abs() < 0.01);
/// ```
#[must_use]
pub fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r = f32::from(r) / 255.0;
    let g = f32::from(g) / 255.0;
    let b = f32::from(b) / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let h = if delta == 0.0 {
        0.0
    } else if (max - r).abs() < f32::EPSILON {
        (((g - b) / delta) % 6.0) / 6.0
    } else if (max - g).abs() < f32::EPSILON {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };
    let h = if h < 0.0 { h + 1.0 } else { h };

    (h, s, v)
}

/// Convertit HSV → RGB [0,255]. H ∈ [0.0, 1.0), S ∈ [0.0, 1.0], V ∈ [0.0, 1.0].
///
/// # Example
/// ```
/// use af_core::color::hsv_to_rgb;
/// let (r, g, b) = hsv_to_rgb(0.0, 1.0, 1.0);
/// assert_eq!(r, 255);
/// assert_eq!(g, 0);
/// assert_eq!(b, 0);
/// ```
#[must_use]
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h * 6.0;
    let i = h.floor() as u32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Technique HSV Bright : force V=1.0, le char ASCII encode la luminance.
///
/// Produit des couleurs vibrantes sur fond noir.
///
/// # Example
/// ```
/// use af_core::color::apply_hsv_bright;
/// let (r, g, b) = apply_hsv_bright(200, 50, 50, 1.0);
/// // V forced to 1.0, so result is brighter
/// assert!(r > 200 || g > 50 || b > 50);
/// ```
#[must_use]
pub fn apply_hsv_bright(r: u8, g: u8, b: u8, saturation_boost: f32) -> (u8, u8, u8) {
    let (h, s, _v) = rgb_to_hsv(r, g, b);
    let s = (s * saturation_boost).min(1.0);
    hsv_to_rgb(h, s, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_hsv_roundtrip() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let r = r as u8;
                    let g = g as u8;
                    let b = b as u8;
                    let (h, s, v) = rgb_to_hsv(r, g, b);
                    let (r2, g2, b2) = hsv_to_rgb(h, s, v);
                    assert!(
                        (i16::from(r) - i16::from(r2)).abs() <= 1,
                        "R mismatch: {r} vs {r2} (h={h}, s={s}, v={v})"
                    );
                    assert!(
                        (i16::from(g) - i16::from(g2)).abs() <= 1,
                        "G mismatch: {g} vs {g2}"
                    );
                    assert!(
                        (i16::from(b) - i16::from(b2)).abs() <= 1,
                        "B mismatch: {b} vs {b2}"
                    );
                }
            }
        }
    }

    #[test]
    fn hsv_bright_keeps_hue() {
        let (h, _s, _v) = rgb_to_hsv(200, 50, 50);
        let (r2, g2, b2) = apply_hsv_bright(200, 50, 50, 1.0);
        let (h2, _s2, v2) = rgb_to_hsv(r2, g2, b2);
        assert!((h - h2).abs() < 0.01, "Hue shifted: {h} vs {h2}");
        assert!((v2 - 1.0).abs() < 0.01, "V not 1.0: {v2}");
    }
}
