use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
use rayon::prelude::*;

/// 16 quadrant block characters (2×2 sub-pixels).
///
/// Index = bitmap: bit0=TL, bit1=TR, bit2=BL, bit3=BR.
const QUADRANT_CHARS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// Process frame in quadrant mode (2×2 sub-pixels per terminal cell).
///
/// # Example
/// ```
/// use af_core::frame::{FrameBuffer, AsciiGrid};
/// use af_core::config::RenderConfig;
/// use af_ascii::quadrant::process_quadrant;
///
/// let frame = FrameBuffer::new(4, 4);
/// let mut grid = AsciiGrid::new(2, 2);
/// let config = RenderConfig::default();
/// process_quadrant(&frame, &config, &mut grid);
/// ```
pub fn process_quadrant(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_w = u32::from(grid.width) * 2;
    let pixel_h = u32::from(grid.height) * 2;

    grid.cells
        .par_chunks_mut(grid.width as usize)
        .enumerate()
        .for_each(|(cy, row)| {
            for (cx, cell) in row.iter_mut().enumerate() {
                let base_x = (cx as u32) * 2 * frame.width / pixel_w.max(1);
                let base_y = (cy as u32) * 2 * frame.height / pixel_h.max(1);

                // Passe 1 : collecter luminances et couleurs
                let mut lum_values = [0u8; 4];
                let mut lum_sum = 0u32;
                let mut avg_r = 0u32;
                let mut avg_g = 0u32;
                let mut avg_b = 0u32;

                for dy in 0..2u32 {
                    for dx in 0..2u32 {
                        let px = (base_x + dx * frame.width / pixel_w.max(1))
                            .min(frame.width.saturating_sub(1));
                        let py = (base_y + dy * frame.height / pixel_h.max(1))
                            .min(frame.height.saturating_sub(1));

                        let lum = frame.luminance_linear(px, py);
                        let (r, g, b, _) = frame.pixel(px, py);
                        let idx = (dy * 2 + dx) as usize;
                        lum_values[idx] = lum;
                        lum_sum += u32::from(lum);

                        avg_r += u32::from(r);
                        avg_g += u32::from(g);
                        avg_b += u32::from(b);
                    }
                }

                // Passe 2 : seuil adaptatif (moyenne locale)
                let local_threshold = (lum_sum / 4) as u8;
                let mut bitmap = 0u8;
                for bit in 0..4u8 {
                    let on = if config.invert {
                        lum_values[bit as usize] < local_threshold
                    } else {
                        lum_values[bit as usize] > local_threshold
                    };
                    if on {
                        bitmap |= 1 << bit;
                    }
                }

                let ch = QUADRANT_CHARS[bitmap as usize];
                let fg = ((avg_r / 4) as u8, (avg_g / 4) as u8, (avg_b / 4) as u8);

                *cell = AsciiCell {
                    ch,
                    fg,
                    bg: (0, 0, 0),
                };
            }
        });
}
