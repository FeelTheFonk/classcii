/// Placeholder for image source. Phase 2.
use std::path::Path;
use std::sync::Arc;

use af_core::frame::FrameBuffer;
use af_core::traits::Source;
use anyhow::{Context, Result};

/// Source d'image statique. Retourne toujours la même frame.
///
/// # Example
/// ```no_run
/// use af_source::image::ImageSource;
/// use std::path::Path;
/// let source = ImageSource::new(Path::new("test.png")).unwrap();
/// ```
pub struct ImageSource {
    frame: Arc<FrameBuffer>,
}

impl ImageSource {
    /// Load an image from disk and create a source.
    ///
    /// # Errors
    /// Returns an error if the image cannot be loaded.
    pub fn new(path: &Path) -> Result<Self> {
        let img = image::open(path)
            .with_context(|| format!("Impossible de charger {}", path.display()))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(Self {
            frame: Arc::new(FrameBuffer {
                data: rgba.into_raw(),
                width,
                height,
                is_camera_baked: false,
            }),
        })
    }
}

impl Source for ImageSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        Some(Arc::clone(&self.frame))
    }

    fn native_size(&self) -> (u32, u32) {
        (self.frame.width, self.frame.height)
    }

    fn is_live(&self) -> bool {
        false
    }
}

/// Convenance pour les tests.
///
/// # Errors
/// Returns an error if the image cannot be loaded.
///
/// # Example
/// ```no_run
/// use af_source::image::load_image;
/// let frame = load_image("test.png").unwrap();
/// ```
pub fn load_image(path: &str) -> Result<FrameBuffer> {
    let img = image::open(path).with_context(|| format!("Impossible de charger {path}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Ok(FrameBuffer {
        data: rgba.into_raw(),
        width: w,
        height: h,
        is_camera_baked: false,
    })
}
