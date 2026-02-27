use af_core::frame::FrameBuffer;
use anyhow::{Context, Result};
use fast_image_resize::images::Image;
use fast_image_resize::{PixelType, ResizeOptions, Resizer as FirResizer};

/// Resizer réutilisable wrappant fast_image_resize.
///
/// Pré-alloue le resizer pour zéro allocation en hot path.
///
/// # Example
/// ```
/// use af_source::resize::Resizer;
/// let r = Resizer::new();
/// ```
pub struct Resizer {
    inner: FirResizer,
    options: ResizeOptions,
    /// Scratch image for source (owned buffer to avoid the mut borrow issue).
    src_buf: Vec<u8>,
}

impl Resizer {
    /// Create a new resizer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: FirResizer::new(),
            options: ResizeOptions::new(),
            src_buf: Vec::new(),
        }
    }

    /// Resize `src` into `dst`. Dimensions of `dst` determine output size.
    ///
    /// # Errors
    /// Returns an error if the resize operation fails.
    ///
    /// # Example
    /// ```
    /// use af_source::resize::Resizer;
    /// use af_core::frame::FrameBuffer;
    /// let mut r = Resizer::new();
    /// let src = FrameBuffer::new(100, 100);
    /// let mut dst = FrameBuffer::new(50, 50);
    /// r.resize_into(&src, &mut dst).unwrap();
    /// ```
    pub fn resize_into(&mut self, src: &FrameBuffer, dst: &mut FrameBuffer) -> Result<()> {
        if src.width == dst.width && src.height == dst.height {
            dst.data.copy_from_slice(&src.data);
            return Ok(());
        }

        // Copy source data into owned buffer
        // R1: forced copy by fast_image_resize API (requires &mut on source)
        self.src_buf.clear();
        self.src_buf.extend_from_slice(&src.data);

        let src_image =
            Image::from_slice_u8(src.width, src.height, &mut self.src_buf, PixelType::U8x4)
                .context("Invalid source dimensions")?;

        let mut dst_image =
            Image::from_slice_u8(dst.width, dst.height, &mut dst.data, PixelType::U8x4)
                .context("Invalid destination dimensions")?;

        self.inner
            .resize(&src_image, &mut dst_image, Some(&self.options))
            .context("Resize failed")?;

        Ok(())
    }
}

impl Default for Resizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience for one-shot usage. DO NOT use in hot path.
///
/// # Errors
/// Returns an error if the resize operation fails.
///
/// # Example
/// ```
/// use af_source::resize::resize_frame;
/// use af_core::frame::FrameBuffer;
/// let src = FrameBuffer::new(100, 100);
/// let dst = resize_frame(&src, 50, 50).unwrap();
/// assert_eq!(dst.width, 50);
/// ```
pub fn resize_frame(src: &FrameBuffer, width: u32, height: u32) -> Result<FrameBuffer> {
    let mut dst = FrameBuffer::new(width, height);
    let mut resizer = Resizer::new();
    resizer.resize_into(src, &mut dst)?;
    Ok(dst)
}
