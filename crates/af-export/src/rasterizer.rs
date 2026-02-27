use ab_glyph::{Font, FontRef, PxScale, point};
use af_core::frame::{AsciiGrid, FrameBuffer};
use rayon::prelude::*;
use std::collections::HashMap;

/// Convertit une AsciiGrid en pixels RGBA haute résolution.
/// Maintien d'un cache atlas pour éliminer tout surcoût de rasterisation dans le hot-loop.
pub struct Rasterizer {
    char_width: u32,
    char_height: u32,
    /// Maps a char to its 1D alpha buffer (size = char_width * char_height)
    glyph_cache: HashMap<char, Vec<u8>>,
}

impl Rasterizer {
    /// Initialise le rasterizer en pré-calculant (atlas software) tous
    /// les caractères couramment utilisés (ASCII, Braille, HalfBlock).
    ///
    /// # Errors
    /// Retourne une erreur si la police fournie est invalide.
    pub fn new(font_data: &[u8], scale_px: f32) -> anyhow::Result<Self> {
        let font = FontRef::try_from_slice(font_data)?;
        let scale = PxScale::from(scale_px);

        let v_advance = font.ascent_unscaled() - font.descent_unscaled() + font.line_gap_unscaled();
        let height = (v_advance * scale.y / font.height_unscaled()).ceil() as u32;

        let m_glyph = font.glyph_id('M');
        let h_advance = font.h_advance_unscaled(m_glyph);
        let width = (h_advance * scale.x / font.height_unscaled()).ceil() as u32;

        let char_width = width.max(1);
        let char_height = height.max(1);

        let mut rasterizer = Self {
            char_width,
            char_height,
            glyph_cache: HashMap::new(),
        };

        rasterizer.cache_charset(&font, scale, 32..=126);
        rasterizer.cache_charset(&font, scale, 0x2800..=0x28FF);
        rasterizer.cache_charset(&font, scale, 0x2580..=0x259F);

        // Cache combinatory diacritics for Zalgo zero-alloc rasterization
        rasterizer.cache_charset(&font, scale, 0x0300..=0x036F);

        Ok(rasterizer)
    }

    fn cache_charset(
        &mut self,
        font: &FontRef,
        scale: PxScale,
        range: std::ops::RangeInclusive<u32>,
    ) {
        for codepoint in range {
            if let Some(ch) = std::char::from_u32(codepoint) {
                let mut buffer = vec![0u8; (self.char_width * self.char_height) as usize];

                let ascent_px = font.ascent_unscaled() * scale.y / font.height_unscaled();
                let glyph = font
                    .glyph_id(ch)
                    .with_scale_and_position(scale, point(0.0, ascent_px));

                if let Some(outline) = font.outline_glyph(glyph) {
                    let bounds = outline.px_bounds();
                    #[allow(clippy::cast_possible_wrap)]
                    outline.draw(|x, y, v| {
                        let px = (x as i32 + bounds.min.x as i32).max(0) as u32;
                        let py = (y as i32 + bounds.min.y as i32).max(0) as u32;
                        if px < self.char_width && py < self.char_height {
                            let idx = (py * self.char_width + px) as usize;
                            if idx < buffer.len() {
                                buffer[idx] = (v * 255.0).round() as u8;
                            }
                        }
                    });
                }
                self.glyph_cache.insert(ch, buffer);
            }
        }
    }

    /// Rendu de l'AsciiGrid sur le FrameBuffer.
    /// Zéro allocation dans le hot-loop (R1). Parallélisé.
    pub fn render(&self, grid: &AsciiGrid, fb: &mut FrameBuffer, zalgo_intensity: f32) {
        let expected_w = u32::from(grid.width) * self.char_width;
        let expected_h = u32::from(grid.height) * self.char_height;

        debug_assert!(fb.width == expected_w && fb.height == expected_h);

        let empty_glyph = vec![0u8; (self.char_width * self.char_height) as usize];

        let stride = (expected_w * 4) as usize;
        let band_size = stride * self.char_height as usize;

        fb.data
            .par_chunks_exact_mut(band_size)
            .enumerate()
            .for_each(|(gy, band)| {
                // Thread-local LCG for deterministic Zalgo
                let mut seed = 0x1234_5678_u32.wrapping_add(gy as u32 * 1337);
                let mut rand = || {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    seed
                };

                for gx in 0..(grid.width as usize) {
                    let cell = grid.get(gx as u16, gy as u16);
                    let char_alpha = self.glyph_cache.get(&cell.ch).unwrap_or(&empty_glyph);

                    // --- Zalgo Combinatory Stack (Zero-alloc references array) ---
                    let mut diacritics: [&Vec<u8>; 8] = [&empty_glyph; 8];
                    let mut diacritics_count = 0;

                    if zalgo_intensity > 0.0 && (rand() % 100) < (zalgo_intensity * 10.0) as u32 {
                        let iterations = (zalgo_intensity * 2.0).clamp(1.0, 8.0) as usize;
                        for _ in 0..iterations {
                            let ch = match rand() % 5 {
                                0 => '\u{0300}',
                                1 => '\u{0313}',
                                2 => '\u{0330}',
                                3 => '\u{0336}',
                                _ => '\u{0346}',
                            };
                            if let Some(d_cache) = self.glyph_cache.get(&ch) {
                                diacritics[diacritics_count] = d_cache;
                                diacritics_count += 1;
                            }
                        }
                    }

                    let cx_start = gx * self.char_width as usize;

                    for cy in 0..(self.char_height as usize) {
                        let fb_y_offset = cy * stride;
                        for cx in 0..(self.char_width as usize) {
                            let local_idx = cy * self.char_width as usize + cx;
                            let mut alpha = char_alpha[local_idx];

                            // Composite diacritics atop base char (max blending)
                            for d in &diacritics[..diacritics_count] {
                                alpha = alpha.max(d[local_idx]);
                            }

                            let alpha_f = f32::from(alpha) / 255.0;

                            let r = (f32::from(cell.fg.0) * alpha_f
                                + f32::from(cell.bg.0) * (1.0 - alpha_f))
                                as u8;
                            let g = (f32::from(cell.fg.1) * alpha_f
                                + f32::from(cell.bg.1) * (1.0 - alpha_f))
                                as u8;
                            let b = (f32::from(cell.fg.2) * alpha_f
                                + f32::from(cell.bg.2) * (1.0 - alpha_f))
                                as u8;

                            let px_idx = fb_y_offset + (cx_start + cx) * 4;
                            band[px_idx] = r;
                            band[px_idx + 1] = g;
                            band[px_idx + 2] = b;
                            band[px_idx + 3] = 255;
                        }
                    }
                }
            });
    }

    /// Calcule les dimensions prévues du FrameBuffer en fonction d'une taille de grille.
    #[must_use]
    pub fn target_dimensions(&self, grid_w: u16, grid_h: u16) -> (u32, u32) {
        (
            u32::from(grid_w) * self.char_width,
            u32::from(grid_h) * self.char_height,
        )
    }
}
