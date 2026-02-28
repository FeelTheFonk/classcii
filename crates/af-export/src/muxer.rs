use af_core::frame::FrameBuffer;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Encode des raw frames RGBA dans un fichier MP4 avec ffmpeg (Lossless).
pub struct Mp4Muxer {
    ffmpeg_child: Child,
}

impl Mp4Muxer {
    /// Crée un Muxer vidéo.
    /// Utilise x264 avec `-crf 0` (qualité visuelle lossless absolue).
    ///
    /// # Errors
    /// Retourne une erreur si ffmpeg n'est pas installé ou impossible à démarrer.
    pub fn new(output_path: &Path, width: u32, height: u32, target_fps: u32) -> Result<Self> {
        let path_str = output_path.to_str().context("Chemin invalide")?;

        let child = Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "rawvideo",
                "-vcodec",
                "rawvideo",
                "-s",
                &format!("{width}x{height}"),
                "-pix_fmt",
                "rgba",
                "-r",
                &target_fps.to_string(),
                "-i",
                "-",
                "-c:v",
                "libx264rgb", // FORCE SOTA LOSSLESS RGB ENCODER
                "-crf",
                "0",
                "-preset",
                "veryslow",
                "-pix_fmt",
                "rgb24", // FORCE PURITY OF CHROMA (NO YUV SUBSAMPLING)
                "-color_range",
                "pc", // Use full PC range (0-255) for RGB instead of limited tv range
                "-hide_banner",
                "-loglevel",
                "error",
                path_str,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context(
                "Échec de l'initialisation de l'encodeur Video ffmpeg. (Est-il dans PATH ?)",
            )?;

        Ok(Self {
            ffmpeg_child: child,
        })
    }

    /// Mute une nouvelle frame au flux.
    ///
    /// # Errors
    /// Retourne une erreur I/O si l'écriture dans le pipe échoue.
    pub fn write_frame(&mut self, fb: &FrameBuffer) -> Result<()> {
        if let Some(stdin) = self.ffmpeg_child.stdin.as_mut() {
            stdin.write_all(&fb.data)?;
        }
        Ok(())
    }

    /// Ferme le flux et finalise l'exportation.
    ///
    /// # Errors
    /// Retourne une erreur si ffmpeg signale une erreur de terminaison.
    pub fn finish(mut self) -> Result<()> {
        drop(self.ffmpeg_child.stdin.take());

        let status = self.ffmpeg_child.wait()?;
        if !status.success() {
            anyhow::bail!("ffmpeg s'est terminé avec une erreur: {status}");
        }
        Ok(())
    }
}

/// Fusionne un MP4 sans piste audio avec un fichier source Audio.
///
/// # Errors
/// Retourne une erreur si le muxage ffmpeg échoue.
pub fn mux_audio_video(video_path: &Path, audio_path: &Path, final_path: &Path) -> Result<()> {
    let video_str = video_path.to_str().context("video path invalid")?;
    let audio_str = audio_path.to_str().context("audio path invalid")?;
    let final_str = final_path.to_str().context("final path invalid")?;

    let mut command = Command::new("ffmpeg");
    command.args([
        "-y",
        "-i",
        video_str,
        "-i",
        audio_str,
        "-c:v",
        "copy",
        "-c:a",
        "aac",
        "-b:a",
        "320k",
        "-shortest",
        "-hide_banner",
        "-loglevel",
        "error",
        final_str,
    ]);

    let status = command.spawn()?.wait()?;
    if !status.success() {
        anyhow::bail!("Échec du muxing visuel/audio final : {status}");
    }

    Ok(())
}
