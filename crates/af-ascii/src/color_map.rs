use af_core::color::apply_hsv_bright;
use af_core::config::ColorMode;

/// Map a pixel color according to the selected color mode.
///
/// # Example
/// ```
/// use af_ascii::color_map::map_color;
/// use af_core::config::ColorMode;
/// let (r, g, b) = map_color(200, 50, 50, &ColorMode::Direct, 1.0);
/// assert_eq!((r, g, b), (200, 50, 50));
/// ```
#[must_use]
pub fn map_color(r: u8, g: u8, b: u8, mode: &ColorMode, saturation: f32) -> (u8, u8, u8) {
    match mode {
        ColorMode::Direct => (r, g, b),
        ColorMode::HsvBright => apply_hsv_bright(r, g, b, saturation),
        ColorMode::Quantized => quantize(r, g, b),
    }
}

/// Quantize a color to a reduced palette (6×6×6 color cube).
fn quantize(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let quantize_channel = |c: u8| -> u8 {
        let level = c / 43; // 256 / 6 ≈ 43
        level * 51 // 255 / 5 = 51
    };
    (quantize_channel(r), quantize_channel(g), quantize_channel(b))
}
