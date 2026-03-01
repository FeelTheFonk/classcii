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

/// Convert sRGB [0,255] to Oklab (L, a, b).
/// L ∈ [0.0, 1.0], a ∈ ~[-0.23, 0.28], b ∈ ~[-0.31, 0.20].
/// Björn Ottosson (2020). Perceptually uniform color space.
#[must_use]
pub fn rgb_to_oklab(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    // sRGB → linear RGB
    let r_lin = srgb_to_linear(r);
    let g_lin = srgb_to_linear(g);
    let b_lin = srgb_to_linear(b);

    // Linear RGB → LMS
    let l = 0.412_221_5_f32.mul_add(r_lin, 0.536_332_55_f32.mul_add(g_lin, 0.051_445_95 * b_lin));
    let m = 0.211_903_5_f32.mul_add(r_lin, 0.680_699_5_f32.mul_add(g_lin, 0.107_396_96 * b_lin));
    let s = 0.088_302_46_f32.mul_add(r_lin, 0.281_718_84_f32.mul_add(g_lin, 0.629_978_7 * b_lin));

    // LMS → LMS^(1/3)
    let l_g = l.cbrt();
    let m_g = m.cbrt();
    let s_g = s.cbrt();

    // LMS^(1/3) → Oklab
    let ok_l = 0.210_454_26_f32.mul_add(l_g, 0.793_617_8_f32.mul_add(m_g, -0.004_072_047 * s_g));
    let ok_a = 1.977_998_5_f32.mul_add(l_g, (-2.428_592_2_f32).mul_add(m_g, 0.450_593_7 * s_g));
    let ok_b = 0.025_904_037_f32.mul_add(l_g, 0.782_771_77_f32.mul_add(m_g, -0.808_675_77 * s_g));

    (ok_l, ok_a, ok_b)
}

/// Convert Oklab (L, a, b) to sRGB [0,255].
#[must_use]
pub fn oklab_to_rgb(l: f32, a: f32, b: f32) -> (u8, u8, u8) {
    // Oklab → LMS^(1/3)
    let l_g = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_g = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_g = l - 0.089_484_18 * a - 1.291_485_5 * b;

    // LMS^(1/3) → LMS (cube)
    let l_c = l_g * l_g * l_g;
    let m_c = m_g * m_g * m_g;
    let s_c = s_g * s_g * s_g;

    // LMS → linear RGB
    let r_lin = 4.076_741_7_f32.mul_add(l_c, (-3.307_711_6_f32).mul_add(m_c, 0.230_969_94 * s_c));
    let g_lin = (-1.268_438_f32).mul_add(l_c, 2.609_757_4_f32.mul_add(m_c, -0.341_319_38 * s_c));
    let b_lin =
        (-0.004_196_086_3_f32).mul_add(l_c, (-0.703_418_6_f32).mul_add(m_c, 1.707_614_7 * s_c));

    // linear RGB → sRGB
    (
        linear_to_srgb(r_lin),
        linear_to_srgb(g_lin),
        linear_to_srgb(b_lin),
    )
}

/// sRGB gamma decode (single channel).
#[inline(always)]
fn srgb_to_linear(v: u8) -> f32 {
    let x = f32::from(v) / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear to sRGB gamma encode (single channel).
#[inline(always)]
fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let s = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// Oklab Bright: force L=1.0 and reconvert to sRGB. Perceptually uniform brightness.
#[must_use]
pub fn apply_oklab_bright(r: u8, g: u8, b: u8, saturation_boost: f32) -> (u8, u8, u8) {
    let (_l, a, ob) = rgb_to_oklab(r, g, b);
    oklab_to_rgb(1.0, a * saturation_boost, ob * saturation_boost)
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

    #[test]
    fn oklab_roundtrip() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let r = r as u8;
                    let g = g as u8;
                    let b = b as u8;
                    let (l, a, ob) = rgb_to_oklab(r, g, b);
                    let (r2, g2, b2) = oklab_to_rgb(l, a, ob);
                    assert!(
                        (i16::from(r) - i16::from(r2)).abs() <= 1,
                        "R drift at ({r},{g},{b}): got {r2}"
                    );
                    assert!(
                        (i16::from(g) - i16::from(g2)).abs() <= 1,
                        "G drift at ({r},{g},{b}): got {g2}"
                    );
                    assert!(
                        (i16::from(b) - i16::from(b2)).abs() <= 1,
                        "B drift at ({r},{g},{b}): got {b2}"
                    );
                }
            }
        }
    }
}
