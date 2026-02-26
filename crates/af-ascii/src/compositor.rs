use af_core::charset::LuminanceLut;
use af_core::config::{BgStyle, RenderConfig, RenderMode};
use af_core::frame::{AsciiCell, AsciiGrid, AudioFeatures, FrameBuffer};

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
                if config.color_enabled {
                    apply_color_mapping(grid, frame, config);
                }
            }
            RenderMode::HalfBlock => {
                crate::halfblock::process_halfblock(frame, config, grid);
            }
            RenderMode::Braille => {
                crate::braille::process_braille(frame, config, grid);
            }
            RenderMode::Quadrant => {
                crate::quadrant::process_quadrant(frame, config, grid);
            }
        }

        // Edge blending: if edges enabled, overlay edge chars with mix ratio
        if config.edge_threshold > 0.0 && config.edge_mix > 0.0 {
            apply_edge_blend(grid, frame, config);
        }

        // Background style
        apply_bg_style(grid, frame, config);
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
            grid.set(cx, cy, AsciiCell {
                ch: cell.ch,
                fg: (mr, mg, mb),
                bg: cell.bg,
            });
        }
    }
}

/// Edge blending overlay using edge_mix ratio.
fn apply_edge_blend(grid: &mut AsciiGrid, frame: &FrameBuffer, config: &RenderConfig) {
    let edge_chars = [' ', '.', '-', '|', '/', '\\', '+', '#'];
    let mix = config.edge_mix.clamp(0.0, 1.0);

    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let px = u32::from(cx) * frame.width / u32::from(grid.width).max(1);
            let py = u32::from(cy) * frame.height / u32::from(grid.height).max(1);
            let px = px.min(frame.width.saturating_sub(1));
            let py = py.min(frame.height.saturating_sub(1));

            // detect_edge returns normalized [0.0, 1.0]
            let normalized = crate::edge::detect_edge(frame, px, py);

            if normalized > config.edge_threshold {
                let idx = ((normalized * (edge_chars.len() - 1) as f32) as usize)
                    .min(edge_chars.len() - 1);
                let edge_ch = edge_chars[idx];
                let cell = grid.get(cx, cy);

                // Blend: if mix=1.0, full edge; if mix=0.5, 50% chance of keeping original
                if mix >= 1.0 || normalized * mix > 0.5 {
                    grid.set(cx, cy, AsciiCell {
                        ch: edge_ch,
                        fg: cell.fg,
                        bg: cell.bg,
                    });
                }
            }
        }
    }
}

/// Apply background style to grid cells.
fn apply_bg_style(grid: &mut AsciiGrid, frame: &FrameBuffer, config: &RenderConfig) {
    match config.bg_style {
        BgStyle::Black | BgStyle::Transparent => {} // Already default (0,0,0)
        BgStyle::SourceDim => {
            for cy in 0..grid.height {
                for cx in 0..grid.width {
                    let px = u32::from(cx) * frame.width / u32::from(grid.width).max(1);
                    let py = u32::from(cy) * frame.height / u32::from(grid.height).max(1);
                    let px = px.min(frame.width.saturating_sub(1));
                    let py = py.min(frame.height.saturating_sub(1));

                    let (r, g, b, _) = frame.pixel(px, py);
                    let cell = grid.get(cx, cy);
                    grid.set(cx, cy, AsciiCell {
                        ch: cell.ch,
                        fg: cell.fg,
                        bg: (r / 4, g / 4, b / 4), // 25% brightness
                    });
                }
            }
        }
    }
}

