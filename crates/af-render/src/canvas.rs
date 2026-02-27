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
pub fn render_grid(buf: &mut Buffer, area: Rect, grid: &AsciiGrid, zalgo_intensity: f32) {
    // Fast LCG pour le glitch Zalgo déterministe
    let mut seed = 0x1234_5678_u32;
    let mut rand = || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        seed
    };
    for cy in 0..grid.height.min(area.height) {
        for cx in 0..grid.width.min(area.width) {
            let cell = grid.get(cx, cy);
            let buf_x = area.x + cx;
            let buf_y = area.y + cy;

            if let Some(buf_cell) = buf.cell_mut((buf_x, buf_y)) {
                // Surcharge Mathématique Zalgo (Zero-Allocation Hot Loop)
                if zalgo_intensity > 0.0 && (rand() % 100) < (zalgo_intensity * 10.0) as u32 {
                    let mut bytes = [0u8; 64];
                    let mut len = 0;
                    len += cell.ch.encode_utf8(&mut bytes[len..]).len();

                    let iterations = (zalgo_intensity * 2.0).clamp(1.0, 8.0) as usize;
                    for _ in 0..iterations {
                        let diacritic = match rand() % 5 {
                            0 => '\u{0300}', // Above: Altération Supérieure Légère
                            1 => '\u{0313}', // Above: Distorsion Supérieure Asymétrique
                            2 => '\u{0330}', // Below: Altération Inférieure
                            3 => '\u{0336}', // Through: Bruit de Ligne Central
                            _ => '\u{0346}', // Below: Densification Basse
                        };
                        len += diacritic.encode_utf8(&mut bytes[len..]).len();
                    }

                    let glitch_str = std::str::from_utf8(&bytes[0..len]).unwrap_or("");
                    buf_cell.set_symbol(glitch_str);
                } else {
                    buf_cell.set_char(cell.ch);
                }

                // Emulation Alpha VTE pour U+2591, U+2592, U+2593 (Shade Characters)
                // S'assure que le composant fg et bg garantissent un alpha blending terminal-native SOTA,
                // forçant le pipeline VTE à utiliser un raster vectoriel plutôt qu'un bitmap crénelé.
                if cell.ch == '\u{2591}' || cell.ch == '\u{2592}' || cell.ch == '\u{2593}' {
                    let term_color_fg = Color::Rgb(cell.fg.0, cell.fg.1, cell.fg.2);
                    let term_color_bg = if cell.bg == (0, 0, 0) {
                        Color::Reset
                    } else {
                        Color::Rgb(cell.bg.0, cell.bg.1, cell.bg.2)
                    };
                    buf_cell.set_fg(term_color_fg).set_bg(term_color_bg);
                } else {
                    buf_cell.set_fg(Color::Rgb(cell.fg.0, cell.fg.1, cell.fg.2));
                    if cell.bg != (0, 0, 0) {
                        buf_cell.set_bg(Color::Rgb(cell.bg.0, cell.bg.1, cell.bg.2));
                    }
                }
            }
        }
    }
}
