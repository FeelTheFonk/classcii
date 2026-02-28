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
        let max_iter = 100;

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

                    // Mapping itération -> luminance RGB lissée (nuances de gris)
                    let luma = if iter == max_iter {
                        0
                    } else {
                        // Smooth coloring
                        let log_zn = f64::ln(x * x + y * y) / 2.0;
                        let nu = f64::ln(log_zn / f64::ln(2.0)) / f64::ln(2.0);
                        let i = f64::from(iter) + 1.0 - nu;
                        ((i / f64::from(max_iter)) * 255.0) as u8
                    };

                    let idx = (px * 4) as usize;
                    row[idx] = luma;
                    row[idx + 1] = luma;
                    row[idx + 2] = luma;
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
