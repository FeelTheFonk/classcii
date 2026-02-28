use af_core::frame::FrameBuffer;
use af_core::traits::Source;
use rayon::prelude::*;
use std::sync::Arc;

use af_core::config::RenderConfig;
use arc_swap::ArcSwap;

/// A mathematical field generator that renders the Mandelbrot set.
/// Evaluates the fractal analytically per-pixel using `rayon` for parallelism.
/// Respects zero-allocation in hot paths by returning a recycled `Arc<FrameBuffer>`.
pub struct MandelbrotSource {
    width: u32,
    height: u32,
    pool: Vec<Arc<FrameBuffer>>,
    frame_count: u64,
    config: Arc<ArcSwap<RenderConfig>>,
}

impl MandelbrotSource {
    /// Creates a new Mandelbrot generator with the specified dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32, config: Arc<ArcSwap<RenderConfig>>) -> Self {
        let pool = (0..6)
            .map(|_| {
                let mut fb = FrameBuffer::new(width, height);
                fb.is_camera_baked = true;
                Arc::new(fb)
            })
            .collect();

        Self {
            width,
            height,
            pool,
            frame_count: 0,
            config,
        }
    }
}

impl Source for MandelbrotSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        // Zero-Alloc: Find a free slot in the pre-allocated pool
        let free_idx = self
            .pool
            .iter()
            .position(|a| Arc::strong_count(a) == 1)
            .unwrap_or(0);
        let Some(fb) = Arc::get_mut(&mut self.pool[free_idx]) else {
            return None; // Safe fallback, skip frame if pool is completely saturated
        };

        let cur_config = self.config.load();

        // Animations temporelles
        let time = self.frame_count as f64 / 60.0;

        // Virtual Camera audio-reactive variables
        let base_zoom = f64::from(cur_config.camera_zoom_amplitude);
        let rot = f64::from(cur_config.camera_rotation);
        let pan_x = f64::from(cur_config.camera_pan_x);
        let pan_y = f64::from(cur_config.camera_pan_y);

        let zoom = base_zoom * f64::exp(time * 0.1);

        // Coordonnées de focus initial de la fractale (Vallée des hippocampes)
        let focus_x = -0.743_643_887_037_151;
        let focus_y = 0.131_825_904_205_330;

        let cos_a = rot.cos();
        let sin_a = rot.sin();

        let w = f64::from(self.width);
        let h = f64::from(self.height);
        // Adaptive max_iter: deeper zoom needs more iterations for detail
        let max_iter = (100.0 + zoom.ln().max(0.0) * 50.0).clamp(100.0, 1000.0) as u32;

        let band_size = (self.width * 4) as usize; // Stride en bytes

        fb.data
            .par_chunks_exact_mut(band_size)
            .enumerate()
            .for_each(|(y_idx, row)| {
                let py = y_idx as f64;
                for px in 0..self.width {
                    // Mapping pixel -> plan complexe centré avec Zoom
                    // Application du Panning (VirtualCamera) -> Décalage de l'observateur *avant* le plan
                    let raw_x = (f64::from(px) - w / 2.0) / (w * zoom) * 3.5;
                    let raw_y = (py - h / 2.0) / (h * zoom) * 3.5;

                    // Application native de la Rotation (VirtualCamera) dans le plan mathématique complexe SOTA
                    let rot_x = raw_x * cos_a - raw_y * sin_a;
                    let rot_y = raw_x * sin_a + raw_y * cos_a;

                    // Position finale sur la fractale
                    let cx = rot_x + focus_x - (pan_x * 3.5 / zoom);
                    let cy = rot_y + focus_y - (pan_y * 3.5 / zoom);

                    let mut x = 0.0;
                    let mut y = 0.0;
                    let mut iter = 0;

                    // Z = Z^2 + C
                    while x * x + y * y <= 4.0 && iter < max_iter {
                        let xtemp = x * x - y * y + cx;
                        y = 2.0 * x * y + cy;
                        x = xtemp;
                        iter += 1;
                    }

                    // Smooth HSV cyclic color palette
                    let idx = (px * 4) as usize;
                    if iter == max_iter {
                        row[idx] = 0;
                        row[idx + 1] = 0;
                        row[idx + 2] = 0;
                    } else {
                        let log_zn = f64::ln(x * x + y * y) / 2.0;
                        let nu = f64::ln(log_zn / f64::ln(2.0)) / f64::ln(2.0);
                        let t = (f64::from(iter) + 1.0 - nu) / f64::from(max_iter);
                        let (r, g, b) = hsv_to_rgb_f64(
                            (t * 360.0 * 3.0) % 360.0,
                            0.85,
                            if t < 0.02 { t / 0.02 } else { 1.0 },
                        );
                        row[idx] = r;
                        row[idx + 1] = g;
                        row[idx + 2] = b;
                    }
                    row[idx + 3] = 255;
                }
            });

        self.frame_count += 1;
        Some(Arc::clone(&self.pool[free_idx]))
    }

    fn native_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn is_live(&self) -> bool {
        true // Continuous mathematical field is infinite
    }
}

/// HSV to RGB conversion for f64 fractal coloring. Zero-alloc, O(1).
/// h: [0, 360), s: [0, 1], v: [0, 1] → (r, g, b) as u8.
#[inline]
fn hsv_to_rgb_f64(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}
