use af_core::frame::FrameBuffer;
use af_core::traits::Source;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::image::load_image;

#[cfg(feature = "video")]
use crate::video::{VideoInfo, probe_video, read_exact_or_eof, spawn_ffmpeg_pipe};

/// Extensions image reconnues.
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif"];

/// Extensions vidéo reconnues (feature-gated).
#[cfg(feature = "video")]
const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm"];

/// Source qui parcourt itérativement un dossier pour le traitement par lots.
pub struct FolderBatchSource {
    files: Vec<PathBuf>,
    current_idx: usize,

    current_image: Option<Arc<FrameBuffer>>,
    current_gif: Option<crate::image::GifSource>,

    #[cfg(feature = "video")]
    video_child: Option<std::process::Child>,
    #[cfg(feature = "video")]
    video_info: Option<VideoInfo>,
    #[cfg(feature = "video")]
    video_frame: Option<Arc<FrameBuffer>>,

    #[cfg(feature = "video")]
    target_fps: u32,
    /// Frames read from the current clip (reset on media switch).
    clip_frame_count: u32,
    /// Maximum frames per clip (proportional to total_frames / file_count).
    max_clip_frames: u32,

    /// Previous output frame for crossfade transition.
    crossfade_prev: Option<Arc<FrameBuffer>>,
    /// Remaining crossfade frames (countdown).
    crossfade_remaining: u32,
    /// Total crossfade duration in frames.
    crossfade_duration: u32,
    /// Last output frame (for crossfade capture).
    last_output_frame: Option<Arc<FrameBuffer>>,
}

impl FolderBatchSource {
    /// Crée une nouvelle source par lots explorant `folder_path`.
    ///
    /// `total_frames`: total frames in the audio timeline (for proportional clip duration).
    ///
    /// # Errors
    /// Retourne une erreur si le dossier n'existe pas ou ne peut être lu.
    pub fn new(folder_path: &Path, target_fps: u32, total_frames: u32) -> Result<Self> {
        let mut files = Vec::new();
        Self::scan_dir(folder_path, &mut files)?;
        files.sort();

        if files.is_empty() {
            anyhow::bail!("Aucun fichier média trouvé dans {}", folder_path.display());
        }

        let max_clip_frames = (total_frames / files.len() as u32).max(1);

        let crossfade_duration = (target_fps / 2).max(1);

        let mut source = Self {
            files,
            current_idx: 0,
            current_image: None,
            current_gif: None,
            #[cfg(feature = "video")]
            video_child: None,
            #[cfg(feature = "video")]
            video_info: None,
            #[cfg(feature = "video")]
            video_frame: None,
            #[cfg(feature = "video")]
            target_fps,
            clip_frame_count: 0,
            max_clip_frames,
            crossfade_prev: None,
            crossfade_remaining: 0,
            crossfade_duration,
            last_output_frame: None,
        };

        source.load_current();
        Ok(source)
    }

    /// Extrait récursivement les médias reconnus.
    fn scan_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    Self::scan_dir(&path, files)?;
                } else if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    let ext = ext.to_lowercase();
                    let is_image = IMAGE_EXTS.contains(&ext.as_str());

                    #[cfg(feature = "video")]
                    let is_video = VIDEO_EXTS.contains(&ext.as_str());
                    #[cfg(not(feature = "video"))]
                    let is_video = false;

                    if is_image || is_video {
                        files.push(path);
                    }
                }
            }
        }
        Ok(())
    }

    /// Avance au média suivant dans le dossier (typiquement déclenché par un onset).
    pub fn next_media(&mut self) {
        if self.files.is_empty() {
            return;
        }
        // Capture last frame for crossfade
        self.crossfade_prev = self.last_output_frame.take();
        self.crossfade_remaining = self.crossfade_duration;

        self.current_idx = (self.current_idx + 1) % self.files.len();
        self.load_current();
    }

    /// Number of frames read from the current clip.
    #[must_use]
    pub fn clip_frame_count(&self) -> u32 {
        self.clip_frame_count
    }

    /// Maximum frames per clip (proportional budget).
    #[must_use]
    pub fn max_clip_frames(&self) -> u32 {
        self.max_clip_frames
    }

    /// Set crossfade duration in frames (adaptive: shorter in high-energy, longer in low-energy).
    pub fn set_crossfade_duration(&mut self, frames: u32) {
        self.crossfade_duration = frames.max(1);
    }

    /// Charge le média courant.
    fn load_current(&mut self) {
        if self.files.is_empty() {
            return;
        }
        self.clip_frame_count = 0;
        let path = &self.files[self.current_idx];

        self.current_image = None;
        self.current_gif = None;

        #[cfg(feature = "video")]
        {
            if let Some(mut c) = self.video_child.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
            self.video_info = None;
            self.video_frame = None;
        }

        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext == "gif" {
            match crate::image::GifSource::try_new(path) {
                Ok(Some(gif)) => {
                    self.current_gif = Some(gif);
                    return;
                }
                Ok(None) => { /* single-frame GIF, fall through to load_image */ }
                Err(e) => log::warn!("FolderBatchSource: GIF decode error: {e}"),
            }
        }
        if IMAGE_EXTS.contains(&ext.as_str()) {
            if let Some(path_str) = path.to_str() {
                if let Ok(fb) = load_image(path_str) {
                    self.current_image = Some(Arc::new(fb));
                } else {
                    log::warn!("FolderBatchSource: failed to load image {}", path.display());
                }
            } else {
                log::warn!("FolderBatchSource: non-UTF8 path {}", path.display());
            }
        } else {
            #[cfg(feature = "video")]
            {
                if let Ok(info) = probe_video(path) {
                    if let Some(child) =
                        spawn_ffmpeg_pipe(path, info.width, info.height, 0.0, self.target_fps)
                    {
                        self.video_child = Some(child);
                        self.video_info = Some(info);
                        self.video_frame =
                            Some(Arc::new(FrameBuffer::new(info.width, info.height)));
                    } else {
                        log::warn!(
                            "FolderBatchSource: failed to spawn ffmpeg for {}",
                            path.display()
                        );
                    }
                } else {
                    log::warn!(
                        "FolderBatchSource: failed to probe video {}",
                        path.display()
                    );
                }
            }
        }
    }
}

impl FolderBatchSource {
    /// Internal: get raw frame without crossfade processing.
    fn raw_next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        if let Some(gif) = &mut self.current_gif {
            self.clip_frame_count += 1;
            return gif.next_frame();
        }

        if let Some(img) = &self.current_image {
            self.clip_frame_count += 1;
            return Some(Arc::clone(img));
        }

        #[cfg(feature = "video")]
        {
            if let (Some(child), Some(info), Some(frame_arc)) = (
                self.video_child.as_mut(),
                self.video_info.as_ref(),
                self.video_frame.as_mut(),
            ) {
                let frame_bytes = (info.width * info.height * 4) as usize;

                if Arc::strong_count(frame_arc) > 1 {
                    *frame_arc = Arc::new(FrameBuffer::new(info.width, info.height));
                }

                let fb = Arc::get_mut(frame_arc)?;

                let read_result = child.stdout.as_mut().map_or(Ok(false), |stdout| {
                    read_exact_or_eof(stdout, &mut fb.data[..frame_bytes])
                });

                match read_result {
                    Ok(true) => {
                        self.clip_frame_count += 1;
                        Some(Arc::clone(frame_arc))
                    }
                    Ok(false) => {
                        // EOF: advance to next media instead of restarting
                        if let Some(mut c) = self.video_child.take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        self.next_media();
                        self.raw_next_frame()
                    }
                    Err(e) => {
                        log::warn!("FolderBatchSource: pipe read error: {e}");
                        if let Some(mut c) = self.video_child.take() {
                            let _ = c.kill();
                        }
                        None
                    }
                }
            } else {
                None
            }
        }

        #[cfg(not(feature = "video"))]
        {
            None
        }
    }
}

impl Source for FolderBatchSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        // Clip budget is managed by batch.rs — no auto-advance here.
        let raw_frame = self.raw_next_frame();

        // Apply crossfade if a transition is in progress
        if self.crossfade_remaining > 0 {
            if let (Some(prev), Some(curr)) = (&self.crossfade_prev, &raw_frame) {
                let t = 1.0 - (self.crossfade_remaining as f32 / self.crossfade_duration as f32);
                let blended = Arc::new(blend_frames(prev, curr, t));
                self.crossfade_remaining -= 1;
                if self.crossfade_remaining == 0 {
                    self.crossfade_prev = None;
                }
                self.last_output_frame = Some(Arc::clone(&blended));
                return Some(blended);
            }
            self.crossfade_remaining = 0;
            self.crossfade_prev = None;
        }

        if let Some(ref frame) = raw_frame {
            self.last_output_frame = Some(Arc::clone(frame));
        }
        raw_frame
    }

    fn native_size(&self) -> (u32, u32) {
        if let Some(gif) = &self.current_gif {
            return gif.native_size();
        }
        if let Some(img) = &self.current_image {
            return (img.width, img.height);
        }

        #[cfg(feature = "video")]
        {
            if let Some(info) = &self.video_info {
                return (info.width, info.height);
            }
        }

        (0, 0)
    }

    fn is_live(&self) -> bool {
        false
    }
}

/// Linear per-pixel RGBA blend between two frames.
fn blend_frames(a: &FrameBuffer, b: &FrameBuffer, t: f32) -> FrameBuffer {
    let w = a.width.max(b.width);
    let h = a.height.max(b.height);
    let mut out = FrameBuffer::new(w, h);
    let inv_t = 1.0 - t;
    let len = out.data.len().min(a.data.len()).min(b.data.len());
    for i in 0..len {
        out.data[i] = (f32::from(a.data[i]) * inv_t + f32::from(b.data[i]) * t) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_frames_endpoints() {
        let mut a = FrameBuffer::new(2, 2);
        let mut b = FrameBuffer::new(2, 2);
        a.data.fill(0);
        b.data.fill(255);

        let r0 = blend_frames(&a, &b, 0.0);
        assert!(r0.data.iter().all(|&v| v == 0), "t=0 should be all A");

        let r1 = blend_frames(&a, &b, 1.0);
        assert!(r1.data.iter().all(|&v| v == 255), "t=1 should be all B");

        let rmid = blend_frames(&a, &b, 0.5);
        assert!(
            rmid.data
                .iter()
                .all(|&v| (i16::from(v) - 127).unsigned_abs() <= 1),
            "t=0.5 should be ~127"
        );
    }

    #[test]
    fn blend_frames_different_sizes() {
        let a = FrameBuffer::new(2, 2);
        let b = FrameBuffer::new(4, 4);
        let _ = blend_frames(&a, &b, 0.5);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn scan_dir_recognizes_image_extensions() {
        let dir = std::env::temp_dir().join("classcii_test_scan_dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");

        std::fs::write(dir.join("img.png"), b"").expect("write png");
        std::fs::write(dir.join("img.jpg"), b"").expect("write jpg");
        std::fs::write(dir.join("doc.txt"), b"").expect("write txt");
        std::fs::write(dir.join("audio.mp3"), b"").expect("write mp3");

        let mut files = Vec::new();
        FolderBatchSource::scan_dir(&dir, &mut files).expect("scan should succeed");

        assert_eq!(
            files.len(),
            2,
            "Should find 2 image files, found {}",
            files.len()
        );
        assert!(
            files
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "png"))
        );
        assert!(
            files
                .iter()
                .any(|p| p.extension().is_some_and(|e| e == "jpg"))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
