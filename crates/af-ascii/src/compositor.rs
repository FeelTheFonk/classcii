use af_core::charset::LuminanceLut;
use af_core::config::{BgStyle, RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};
use rayon::prelude::*;

use crate::color_map;
use crate::shape_match::ShapeMatcher;

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
    /// Lazy-initialized shape matcher (only created when first needed).
    shape_matcher: Option<ShapeMatcher>,
}

impl Compositor {
    /// Create a new compositor with the given charset.
    #[must_use]
    pub fn new(charset: &str) -> Self {
        Self {
            lut: LuminanceLut::new(charset),
            current_charset: charset.to_string(),
            shape_matcher: None,
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
    #[allow(clippy::too_many_lines)]
    pub fn process(
        &mut self,
        frame: &FrameBuffer,
        _audio: Option<&AudioFeatures>,
        config: &RenderConfig,
        grid: &mut AsciiGrid,
    ) {
        self.update_if_needed(&config.charset);
        let charset_len = self.current_charset.chars().count() as f32;

        // 1. Pré-Rendu des modes algorithmiques complexes (Braille, HalfBlock, Quadrant, Sextant, Octant)
        let is_ascii = matches!(config.render_mode, RenderMode::Ascii);
        if !is_ascii {
            match config.render_mode {
                RenderMode::HalfBlock => crate::halfblock::process_halfblock(frame, config, grid),
                RenderMode::Braille => crate::braille::process_braille(frame, config, grid),
                RenderMode::Quadrant => crate::quadrant::process_quadrant(frame, config, grid),
                RenderMode::Sextant => crate::masks::sextants::process_sextant(frame, config, grid),
                RenderMode::Octant => crate::masks::octants::process_octant(frame, config, grid),
                RenderMode::Ascii => {}
            }
        }

        // Lazy-init shape matcher if needed
        let use_shape = is_ascii && config.shape_matching;
        if use_shape && self.shape_matcher.is_none() {
            self.shape_matcher = Some(ShapeMatcher::new());
        }

        // 2. MEGA-BOUCLE  (SIMD Philosophy)
        let edge_chars = [' ', '.', '-', '|', '/', '\\', '+', '#'];
        let mix = config.edge_mix.clamp(0.0, 1.0);
        let edge_enabled = config.edge_threshold > 0.0 && config.edge_mix > 0.0;
        let apply_bg = matches!(config.bg_style, BgStyle::SourceDim);

        grid.cells
            .par_chunks_mut(grid.width as usize)
            .enumerate()
            .for_each(|(cy, row)| {
                for (cx, cell) in row.iter_mut().enumerate() {
                    let px = (cx as u32) * frame.width / u32::from(grid.width).max(1);
                    let py = (cy as u32) * frame.height / u32::from(grid.height).max(1);
                    let px = px.min(frame.width.saturating_sub(1));
                    let py = py.min(frame.height.saturating_sub(1));

                    let (r, g, b, _) = frame.pixel(px, py);

                    // A. Base Ascii (Luminance + Couleur Directe)
                    if is_ascii {
                        let mut lum = frame.luminance(px, py);
                        if config.invert {
                            lum = 255 - lum;
                        }
                        let val = f32::from(lum);
                        let adjusted =
                            (val - 128.0) * config.contrast + 128.0 + config.brightness * 255.0;

                        let mut final_lum = adjusted.clamp(0.0, 255.0) as u8;
                        if config.dither_enabled && !use_shape {
                            final_lum = crate::dither::apply_bayer_8x8(
                                final_lum,
                                cx as u32,
                                cy as u32,
                                charset_len,
                            );
                        }

                        // Shape matching or standard LUT
                        cell.ch = if use_shape {
                            if let Some(ref matcher) = self.shape_matcher {
                                let mut block = [0u8; 25];
                                for dy in 0..5u32 {
                                    for dx in 0..5u32 {
                                        let sx = (px + dx).min(frame.width.saturating_sub(1));
                                        let sy = (py + dy).min(frame.height.saturating_sub(1));
                                        block[(dy * 5 + dx) as usize] = frame.luminance(sx, sy);
                                    }
                                }
                                matcher.match_cell(&block)
                            } else {
                                self.lut.map(final_lum)
                            }
                        } else {
                            self.lut.map(final_lum)
                        };

                        if config.color_enabled {
                            let (mr, mg, mb) = color_map::map_color(
                                r,
                                g,
                                b,
                                &config.color_mode,
                                config.saturation,
                            );
                            cell.fg = (mr, mg, mb);
                        } else {
                            cell.fg = (r, g, b);
                        }
                        cell.bg = match config.bg_style {
                            BgStyle::Black | BgStyle::Transparent => (0, 0, 0),
                            BgStyle::SourceDim => (r / 4, g / 4, b / 4),
                        };
                    }

                    // B. Edge Blending
                    if edge_enabled {
                        let (normalized_mag, angle) = crate::edge::detect_edge(frame, px, py);
                        if normalized_mag > config.edge_threshold
                            && (mix >= 1.0 || normalized_mag * mix > 0.5)
                        {
                            // En mode ASCII pur, on utilise le mapping directionnel asciify-them
                            if is_ascii && !use_shape {
                                cell.ch = crate::edge::ascii_edge_char(angle);
                            } else {
                                // Fallback pour les modes blocs ou si les shapes sont déjà actives
                                let idx = ((normalized_mag * (edge_chars.len() - 1) as f32)
                                    as usize)
                                    .min(edge_chars.len() - 1);
                                cell.ch = edge_chars[idx];
                            }
                        }
                    }

                    // C. Override Bg Style (for non-ascii modes that don't do it)
                    if !is_ascii && apply_bg {
                        cell.bg = (r / 4, g / 4, b / 4);
                    }
                }
            });
    }
}
