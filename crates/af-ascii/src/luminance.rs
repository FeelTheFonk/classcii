use af_core::charset::LuminanceLut;
use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};

/// Process a frame into an ASCII grid using luminance mapping.
///
/// For each grid cell, samples the corresponding pixel's luminance,
/// maps it to a character via LUT, and optionally applies color.
///
/// # Example
/// ```
/// use af_core::frame::{FrameBuffer, AsciiGrid};
/// use af_core::config::RenderConfig;
/// use af_core::charset::LuminanceLut;
/// use af_ascii::luminance::process_luminance;
///
/// let frame = FrameBuffer::new(10, 10);
/// let mut grid = AsciiGrid::new(10, 10);
/// let config = RenderConfig::default();
/// let lut = LuminanceLut::new(&config.charset);
/// process_luminance(&frame, &config, &lut, &mut grid);
/// ```
pub fn process_luminance(
    frame: &FrameBuffer,
    config: &RenderConfig,
    lut: &LuminanceLut,
    grid: &mut AsciiGrid,
) {
    for cy in 0..grid.height {
        for cx in 0..grid.width {
            // Map grid coords to pixel coords
            let px = u32::from(cx) * frame.width / u32::from(grid.width).max(1);
            let py = u32::from(cy) * frame.height / u32::from(grid.height).max(1);

            let px = px.min(frame.width.saturating_sub(1));
            let py = py.min(frame.height.saturating_sub(1));

            let mut lum = frame.luminance_linear(px, py);
            if config.invert {
                lum = 255 - lum;
            }

            // Apply contrast and brightness
            let adjusted = apply_contrast_brightness(lum, config.contrast, config.brightness);

            let mut final_lum = adjusted;
            if config.dither_enabled {
                final_lum = crate::dither::apply_bayer_8x8(
                    final_lum,
                    u32::from(cx),
                    u32::from(cy),
                    config.charset.chars().count() as f32,
                );
            }

            let ch = lut.map(final_lum);
            let (r, g, b, _) = frame.pixel(px, py);

            let cell = AsciiCell {
                ch,
                fg: (r, g, b),
                bg: (0, 0, 0),
            };
            grid.set(cx, cy, cell);
        }
    }
}

/// Apply contrast and brightness to a luminance value.
///
/// Contrast: multiply around 128. Brightness: offset.
/// Result clamped to [0, 255].
fn apply_contrast_brightness(lum: u8, contrast: f32, brightness: f32) -> u8 {
    let val = f32::from(lum);
    let adjusted = (val - 128.0) * contrast + 128.0 + brightness * 255.0;
    adjusted.clamp(0.0, 255.0) as u8
}
