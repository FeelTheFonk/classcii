use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};

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
    let threshold: u8 = 128;

    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let base_x = u32::from(cx) * 2 * frame.width / pixel_w.max(1);
            let base_y = u32::from(cy) * 2 * frame.height / pixel_h.max(1);

            let mut bitmap = 0u8;
            let mut avg_r = 0u32;
            let mut avg_g = 0u32;
            let mut avg_b = 0u32;

            // TL=bit0, TR=bit1, BL=bit2, BR=bit3
            for dy in 0..2u32 {
                for dx in 0..2u32 {
                    let px =
                        (base_x + dx * frame.width / pixel_w.max(1)).min(frame.width.saturating_sub(1));
                    let py = (base_y + dy * frame.height / pixel_h.max(1))
                        .min(frame.height.saturating_sub(1));

                    let lum = frame.luminance(px, py);
                    let (r, g, b, _) = frame.pixel(px, py);

                    let bit = dy * 2 + dx; // TL=0, TR=1, BL=2, BR=3
                    let on = if config.invert {
                        lum < threshold
                    } else {
                        lum > threshold
                    };
                    if on {
                        bitmap |= 1 << bit;
                    }

                    avg_r += u32::from(r);
                    avg_g += u32::from(g);
                    avg_b += u32::from(b);
                }
            }

            let ch = QUADRANT_CHARS[bitmap as usize];
            let fg = ((avg_r / 4) as u8, (avg_g / 4) as u8, (avg_b / 4) as u8);

            grid.set(cx, cy, AsciiCell { ch, fg, bg: (0, 0, 0) });
        }
    }
}
