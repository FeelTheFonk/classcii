/// Image and animated GIF sources.
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    })
}

/// Maximum number of frames to decode from a GIF.
const MAX_GIF_FRAMES: usize = 2000;
/// Maximum total decoded memory in bytes (512 MB).
const MAX_GIF_BYTES: usize = 512 * 1024 * 1024;

/// Source de GIF animé. Pré-décode toutes les frames et les boucle avec timing natif.
///
/// # Example
/// ```no_run
/// use af_source::image::GifSource;
/// use std::path::Path;
/// if let Some(source) = GifSource::try_new(Path::new("anim.gif")).unwrap() {
///     assert!(source.frame_count() > 1);
/// }
/// ```
pub struct GifSource {
    frames: Vec<Arc<FrameBuffer>>,
    delays: Vec<Duration>,
    current: usize,
    last_advance: Instant,
}

impl GifSource {
    /// Décode un GIF animé depuis le disque.
    /// Retourne `Ok(None)` si le GIF n'a qu'une seule frame (utiliser `ImageSource`).
    ///
    /// Limits decoding to `MAX_GIF_FRAMES` frames and `MAX_GIF_BYTES` total memory
    /// to prevent OOM on large GIFs.
    ///
    /// # Errors
    /// Retourne une erreur si le fichier ne peut être ouvert ou décodé.
    #[allow(clippy::cast_possible_truncation)]
    pub fn try_new(path: &Path) -> Result<Option<Self>> {
        use image::AnimationDecoder;
        use image::codecs::gif::GifDecoder;
        use std::fs::File;
        use std::io::BufReader;

        let file =
            File::open(path).with_context(|| format!("Impossible d'ouvrir {}", path.display()))?;
        let decoder = GifDecoder::new(BufReader::new(file))
            .with_context(|| format!("GIF invalide: {}", path.display()))?;

        let mut frames = Vec::new();
        let mut delays = Vec::new();
        let mut total_bytes: usize = 0;

        for raw_result in decoder.into_frames() {
            let raw = raw_result
                .with_context(|| format!("Erreur décodage frame GIF: {}", path.display()))?;

            let buf = raw.buffer();
            let frame_bytes = buf.as_raw().len();

            if frames.len() >= MAX_GIF_FRAMES {
                log::warn!(
                    "GIF {}: frame limit reached ({MAX_GIF_FRAMES}), stopping decode ({} frames kept)",
                    path.display(),
                    frames.len()
                );
                break;
            }

            if total_bytes.saturating_add(frame_bytes) > MAX_GIF_BYTES {
                log::warn!(
                    "GIF {}: memory limit reached ({} MB), stopping decode ({} frames kept)",
                    path.display(),
                    MAX_GIF_BYTES / (1024 * 1024),
                    frames.len()
                );
                break;
            }

            let (numer, denom) = raw.delay().numer_denom_ms();
            let ms = if denom == 0 { 100 } else { numer / denom };
            let delay = Duration::from_millis(u64::from(ms.max(10)));

            let (w, h) = (buf.width(), buf.height());
            frames.push(Arc::new(FrameBuffer {
                data: buf.as_raw().clone(),
                width: w,
                height: h,
            }));
            delays.push(delay);
            total_bytes += frame_bytes;
        }

        if frames.len() <= 1 {
            return Ok(None);
        }

        Ok(Some(Self {
            frames,
            delays,
            current: 0,
            last_advance: Instant::now(),
        }))
    }

    /// Nombre total de frames dans le GIF.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

impl Source for GifSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        if self.frames.is_empty() {
            return None;
        }
        if self.last_advance.elapsed() >= self.delays[self.current] {
            self.current = (self.current + 1) % self.frames.len();
            self.last_advance = Instant::now();
        }
        Some(Arc::clone(&self.frames[self.current]))
    }

    fn native_size(&self) -> (u32, u32) {
        self.frames.first().map_or((0, 0), |f| (f.width, f.height))
    }

    fn is_live(&self) -> bool {
        true
    }
}
