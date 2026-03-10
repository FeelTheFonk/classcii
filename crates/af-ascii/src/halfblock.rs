use af_core::config::RenderConfig;
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};

/// Process frame in half-block mode (▄ character).
///
/// Each terminal cell covers 2 vertical pixels. The top pixel's color goes
/// to bg, the bottom pixel's color goes to fg, and the character is '▄'.
///
/// # Example
/// ```
/// use af_core::frame::{FrameBuffer, AsciiGrid};
/// use af_core::config::RenderConfig;
/// use af_ascii::halfblock::process_halfblock;
///
/// let frame = FrameBuffer::new(4, 4);
/// let mut grid = AsciiGrid::new(4, 2);
/// let config = RenderConfig::default();
/// process_halfblock(&frame, &config, &mut grid);
/// ```
pub fn process_halfblock(frame: &FrameBuffer, config: &RenderConfig, grid: &mut AsciiGrid) {
    let pixel_h = u32::from(grid.height) * 2;
    let pixel_w = u32::from(grid.width);

    crate::for_each_row(&mut grid.cells, grid.width as usize, |cy, row| {
        for (cx, cell) in row.iter_mut().enumerate() {
            let x0 = (cx as u32) * frame.width / pixel_w.max(1);
            let x1 = ((cx as u32 + 1) * frame.width / pixel_w.max(1)).min(frame.width);
            let y_top = (cy as u32) * 2 * frame.height / pixel_h.max(1);
            let y_mid = ((cy as u32) * 2 + 1) * frame.height / pixel_h.max(1);
            let y_bot = (((cy as u32) * 2 + 2) * frame.height / pixel_h.max(1)).min(frame.height);

            let (tr, tg, tb, _) = frame.area_sample(x0, y_top, x1, y_mid);
            let (br, bg, bb, _) = frame.area_sample(x0, y_mid, x1, y_bot);

            // Apply contrast/brightness to both halves
            let tr = crate::adjust_lum(tr, config.contrast, config.brightness);
            let tg = crate::adjust_lum(tg, config.contrast, config.brightness);
            let tb = crate::adjust_lum(tb, config.contrast, config.brightness);
            let br = crate::adjust_lum(br, config.contrast, config.brightness);
            let bg = crate::adjust_lum(bg, config.contrast, config.brightness);
            let bb = crate::adjust_lum(bb, config.contrast, config.brightness);

            // Invert swaps top/bottom colors
            let (fg, bg_color) = if config.invert {
                ((tr, tg, tb), (br, bg, bb))
            } else {
                ((br, bg, bb), (tr, tg, tb))
            };

            *cell = AsciiCell {
                ch: '▄',
                fg,
                bg: bg_color,
            };
        }
    });
}
