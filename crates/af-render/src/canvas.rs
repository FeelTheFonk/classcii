use af_core::frame::AsciiGrid;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Écrit directement une `AsciiGrid` dans un `ratatui::Buffer`.
///
/// Pas de widget Canvas ratatui — écriture directe pour zéro overhead.
///
/// # Example
/// ```
/// use af_core::frame::{AsciiGrid, AsciiCell};
/// use af_render::canvas::render_grid;
/// // render_grid writes directly into a ratatui buffer.
/// ```
pub fn render_grid(buf: &mut Buffer, area: Rect, grid: &AsciiGrid) {
    for cy in 0..grid.height.min(area.height) {
        for cx in 0..grid.width.min(area.width) {
            let cell = grid.get(cx, cy);
            let buf_x = area.x + cx;
            let buf_y = area.y + cy;

            if let Some(buf_cell) = buf.cell_mut((buf_x, buf_y)) {
                buf_cell.set_char(cell.ch);
                buf_cell.set_fg(Color::Rgb(cell.fg.0, cell.fg.1, cell.fg.2));
                if cell.bg != (0, 0, 0) {
                    buf_cell.set_bg(Color::Rgb(cell.bg.0, cell.bg.1, cell.bg.2));
                }
            }
        }
    }
}
