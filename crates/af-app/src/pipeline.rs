use std::sync::Arc;

use af_core::config::RenderConfig;
use af_core::frame::{AudioFeatures, FrameBuffer};
use arc_swap::ArcSwap;

use crate::cli::Cli;

#[cfg(feature = "video")]
pub type SourceResult = (
    Option<Arc<FrameBuffer>>,
    Option<flume::Receiver<Arc<FrameBuffer>>>,
    Option<flume::Sender<af_source::video::VideoCommand>>,
);

#[cfg(not(feature = "video"))]
pub type SourceResult = (
    Option<Arc<FrameBuffer>>,
    Option<flume::Receiver<Arc<FrameBuffer>>>,
);
/// Start the audio pipeline.
///
/// `audio_arg` can be `"default"` or `"mic"` for microphone capture,
/// or a file path for audio file analysis.
///
/// # Errors
/// Returns an error if the audio device or file is unavailable.
pub fn start_audio(
    audio_arg: &str,
    config: &Arc<ArcSwap<RenderConfig>>,
) -> anyhow::Result<(
    triple_buffer::Output<AudioFeatures>,
    Option<flume::Sender<af_audio::state::AudioCommand>>,
)> {
    let fps = config.load().target_fps;
    let smoothing = config.load().audio_smoothing;

    match audio_arg {
        "default" | "mic" | "microphone" => {
            log::info!("Starting microphone capture");
            let out = af_audio::state::spawn_audio_thread(fps, smoothing)?;
            Ok((out, None))
        }
        path => {
            let audio_path = std::path::Path::new(path);
            if audio_path.exists() {
                log::info!("Starting audio file analysis: {path}");
                let (cmd_tx, cmd_rx) = flume::bounded(10);
                let out =
                    af_audio::state::spawn_audio_file_thread(audio_path, fps, smoothing, cmd_rx)?;
                Ok((out, Some(cmd_tx)))
            } else {
                anyhow::bail!("Audio source not found: {path}")
            }
        }
    }
}

/// Start the visual source pipeline.
///
/// For static images, returns the image as an Arc-wrapped frame.
/// For dynamic sources (video/webcam), returns a receiver channel.
///
/// # Errors
/// Returns an error if source initialization fails.
pub fn start_source(cli: &Cli) -> anyhow::Result<SourceResult> {
    if let Some(ref path) = cli.image {
        let mut source = af_source::image::ImageSource::new(path)?;
        let frame = af_core::traits::Source::next_frame(&mut source);
        #[cfg(feature = "video")]
        return Ok((frame, None, None));
        #[cfg(not(feature = "video"))]
        return Ok((frame, None));
    }

    #[cfg(feature = "video")]
    if let Some(ref path) = cli.video {
        log::info!("Starting video source: {}", path.display());
        let (frame_tx, frame_rx) = flume::bounded(3);
        let (cmd_tx, cmd_rx) = flume::bounded(10);
        af_source::video::spawn_video_thread(path.clone(), frame_tx, cmd_rx)?;
        return Ok((None, Some(frame_rx), Some(cmd_tx)));
    }

    #[cfg(feature = "video")]
    return Ok((None, None, None));
    #[cfg(not(feature = "video"))]
    return Ok((None, None));
}

/// Applique les mappings audio à une copie de la config avant le rendu.
///
/// # Example
/// ```
/// use af_core::config::RenderConfig;
/// use af_core::frame::AudioFeatures;
/// use af_app::pipeline::apply_audio_mappings;
///
/// let mut config = RenderConfig::default();
/// let features = AudioFeatures::default();
/// apply_audio_mappings(&mut config, &features);
/// ```
pub fn apply_audio_mappings(config: &mut RenderConfig, features: &AudioFeatures) {
    let sensitivity = config.audio_sensitivity;

    for mapping in &config.audio_mappings {
        if !mapping.enabled {
            continue;
        }

        let source_value = match mapping.source.as_str() {
            "rms" => features.rms,
            "peak" => features.peak,
            "sub_bass" => features.sub_bass,
            "bass" => features.bass,
            "low_mid" => features.low_mid,
            "mid" => features.mid,
            "high_mid" => features.high_mid,
            "presence" => features.presence,
            "brilliance" => features.brilliance,
            "spectral_centroid" => features.spectral_centroid,
            "spectral_flux" => features.spectral_flux,
            "spectral_flatness" => features.spectral_flatness,
            "beat_intensity" => features.beat_intensity,
            "onset" => {
                if features.onset {
                    1.0
                } else {
                    0.0
                }
            }
            "beat_phase" => features.beat_phase,
            "bpm" => features.bpm / 200.0,
            _ => 0.0,
        };

        let delta = source_value * mapping.amount * sensitivity + mapping.offset;

        match mapping.target.as_str() {
            "edge_threshold" => {
                config.edge_threshold = (config.edge_threshold + delta).clamp(0.0, 1.0);
            }
            "edge_mix" => {
                config.edge_mix = (config.edge_mix + delta).clamp(0.0, 1.0);
            }
            "contrast" => {
                config.contrast = (config.contrast + delta).clamp(0.1, 3.0);
            }
            "brightness" => {
                config.brightness = (config.brightness + delta).clamp(-1.0, 1.0);
            }
            "saturation" => {
                config.saturation = (config.saturation + delta).clamp(0.0, 3.0);
            }
            "density_scale" => {
                config.density_scale = (config.density_scale + delta).clamp(0.25, 4.0);
            }
            "invert" => {
                if delta > 0.5 {
                    config.invert = !config.invert;
                }
            }
            _ => {}
        }
    }
}
