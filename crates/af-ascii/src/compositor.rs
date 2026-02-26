use af_core::charset::LuminanceLut;
use af_core::config::{RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};

use crate::color_map;
use crate::luminance;

/// Compositor orchestre les différents modes de conversion pixel→ASCII.
///
/// # Example
/// ```
/// use af_ascii::compositor::Compositor;
/// let c = Compositor::new(" .:#@");
/// ```
pub struct Compositor {
    lut: LuminanceLut,
    current_charset: String,
}

impl Compositor {
    /// Create a new compositor with the given charset.
    #[must_use]
    pub fn new(charset: &str) -> Self {
        Self {
            lut: LuminanceLut::new(charset),
            current_charset: charset.to_string(),
        }
    }

    /// Update the LUT if the charset has changed.
    pub fn update_if_needed(&mut self, charset: &str) {
        if self.current_charset != charset {
            self.lut = LuminanceLut::new(charset);
            self.current_charset = charset.to_string();
        }
    }

    /// Process a frame into an ASCII grid, dispatching to the correct mode.
    ///
    /// # Example
    /// ```
    /// use af_ascii::compositor::Compositor;
    /// use af_core::frame::{FrameBuffer, AsciiGrid};
    /// use af_core::config::RenderConfig;
    ///
    /// let mut compositor = Compositor::new(" .:#@");
    /// let frame = FrameBuffer::new(10, 10);
    /// let mut grid = AsciiGrid::new(10, 10);
    /// let config = RenderConfig::default();
    /// compositor.process(&frame, None, &config, &mut grid);
    /// ```
    pub fn process(
        &mut self,
        frame: &FrameBuffer,
        _audio: Option<&AudioFeatures>,
        config: &RenderConfig,
        grid: &mut AsciiGrid,
    ) {
        self.update_if_needed(&config.charset);

        match config.render_mode {
            RenderMode::Ascii => {
                luminance::process_luminance(frame, config, &self.lut, grid);
                // Apply color mapping as post-process for ASCII mode
                if config.color_enabled {
                    apply_color_mapping(grid, frame, config);
                }
            }
            RenderMode::HalfBlock => {
                crate::halfblock::process_halfblock(frame, config, grid);
                // HalfBlock handles its own colors (fg=bottom, bg=top)
            }
            RenderMode::Braille => {
                crate::braille::process_braille(frame, config, grid);
                // Braille handles its own colors (average of block)
            }
            RenderMode::Quadrant => {
                crate::quadrant::process_quadrant(frame, config, grid);
                // Quadrant handles its own colors (average of block)
            }
        }
    }
}

/// Apply color mapping to all cells in the grid.
fn apply_color_mapping(grid: &mut AsciiGrid, frame: &FrameBuffer, config: &RenderConfig) {
    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let px = u32::from(cx) * frame.width / u32::from(grid.width).max(1);
            let py = u32::from(cy) * frame.height / u32::from(grid.height).max(1);
            let px = px.min(frame.width.saturating_sub(1));
            let py = py.min(frame.height.saturating_sub(1));

            let (r, g, b, _) = frame.pixel(px, py);
            let (mr, mg, mb) = color_map::map_color(r, g, b, &config.color_mode, config.saturation);

            let cell = grid.get(cx, cy);
            let new_cell = af_core::frame::AsciiCell {
                ch: cell.ch,
                fg: (mr, mg, mb),
                bg: cell.bg,
            };
            grid.set(cx, cy, new_cell);
        }
    }
}
