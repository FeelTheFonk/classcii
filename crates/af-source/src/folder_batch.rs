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
const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg"];

/// Extensions vidéo reconnues (feature-gated).
#[cfg(feature = "video")]
const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm"];

/// Source qui parcourt itérativement un dossier pour le traitement par lots.
#[allow(dead_code)]
pub struct FolderBatchSource {
    files: Vec<PathBuf>,
    current_idx: usize,

    current_image: Option<Arc<FrameBuffer>>,

    #[cfg(feature = "video")]
    video_child: Option<std::process::Child>,
    #[cfg(feature = "video")]
    video_info: Option<VideoInfo>,
    #[cfg(feature = "video")]
    video_frame: Option<Arc<FrameBuffer>>,

    target_fps: u32,
}

impl FolderBatchSource {
    /// Crée une nouvelle source par lots explorant `folder_path`.
    ///
    /// # Errors
    /// Retourne une erreur si le dossier n'existe pas ou ne peut être lu.
    pub fn new(folder_path: &Path, target_fps: u32) -> Result<Self> {
        let mut files = Vec::new();
        Self::scan_dir(folder_path, &mut files)?;
        files.sort();

        let mut source = Self {
            files,
            current_idx: 0,
            current_image: None,
            #[cfg(feature = "video")]
            video_child: None,
            #[cfg(feature = "video")]
            video_info: None,
            #[cfg(feature = "video")]
            video_frame: None,
            target_fps,
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
        self.current_idx = (self.current_idx + 1) % self.files.len();
        self.load_current();
    }

    /// Charge le média courant.
    fn load_current(&mut self) {
        if self.files.is_empty() {
            return;
        }
        let path = &self.files[self.current_idx];

        self.current_image = None;

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
        if IMAGE_EXTS.contains(&ext.as_str()) {
            if let Ok(fb) = load_image(path.to_str().unwrap_or("")) {
                self.current_image = Some(Arc::new(fb));
            } else {
                log::warn!("FolderBatchSource: failed to load image {}", path.display());
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

impl Source for FolderBatchSource {
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> {
        if let Some(img) = &self.current_image {
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
                    Ok(true) => Some(Arc::clone(frame_arc)),
                    Ok(false) => {
                        let path = self.files[self.current_idx].clone();
                        if let Some(mut c) = self.video_child.take() {
                            let _ = c.kill();
                            let _ = c.wait();
                        }
                        if let Some(new_child) =
                            spawn_ffmpeg_pipe(&path, info.width, info.height, 0.0, self.target_fps)
                        {
                            self.video_child = Some(new_child);
                        }
                        Some(Arc::clone(frame_arc))
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

    fn native_size(&self) -> (u32, u32) {
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
