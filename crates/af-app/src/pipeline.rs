use std::sync::Arc;

use af_core::clock::MediaClock;
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
    clock: Arc<MediaClock>,
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
                let out = af_audio::state::spawn_audio_file_thread(
                    audio_path, fps, smoothing, cmd_rx, clock,
                )?;
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
/// For dynamic sources (video), returns a receiver channel.
///
/// # Errors
/// Returns an error if source initialization fails.
#[allow(clippy::needless_pass_by_value, unused_variables)] // Arc consumed by spawn_video_thread under #[cfg(feature = "video")]
pub fn start_source(
    cli: &Cli,
    clock: Option<Arc<MediaClock>>,
    config: Arc<ArcSwap<RenderConfig>>,
) -> anyhow::Result<SourceResult> {
    let _ = &clock; // Utilisé uniquement avec feature="video"
    if let Some(ref path) = cli.image {
        // Animated GIF detection
        let is_gif = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("gif"));
        if is_gif && let Some(gif) = af_source::image::GifSource::try_new(path)? {
            let (frame_tx, frame_rx) = flume::bounded(3);
            std::thread::spawn(move || {
                let mut source = gif;
                loop {
                    if let Some(frame) = af_core::traits::Source::next_frame(&mut source)
                        && frame_tx.send(frame).is_err()
                    {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            });
            let initial_frame = frame_rx.recv().ok();
            #[cfg(feature = "video")]
            return Ok((initial_frame, Some(frame_rx), None));
            #[cfg(not(feature = "video"))]
            return Ok((initial_frame, Some(frame_rx)));
        }
        // Static image (or single-frame GIF)
        let mut source = af_source::image::ImageSource::new(path)?;
        let frame = af_core::traits::Source::next_frame(&mut source);
        #[cfg(feature = "video")]
        return Ok((frame, None, None));
        #[cfg(not(feature = "video"))]
        return Ok((frame, None));
    }

    #[cfg(feature = "procedural")]
    if let Some(ref proc_type) = cli.procedural {
        log::info!("Starting procedural source: {proc_type}");

        // Procedural sources generate frames on demand similar to images but animated.
        // For simplicity in this architecture, since it's live/animated, we spawn a thread.
        let (frame_tx, frame_rx) = flume::bounded(3);
        let pt = proc_type.clone();

        let cfg_clone = config.clone();
        std::thread::Builder::new()
            .name("procedural_generator".into())
            .spawn(move || {
                let mut source =
                    match af_source::procedural::create_procedural_source(&pt, 640, 360, cfg_clone)
                    {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("Erreur création source procédurale: {e}");
                            return;
                        }
                    };

                // Target ~60fps generation rate
                let target_frame_duration = std::time::Duration::from_nanos(16_666_667);
                loop {
                    let start = std::time::Instant::now();
                    if let Some(frame) = af_core::traits::Source::next_frame(&mut *source)
                        && frame_tx.send(frame).is_err()
                    {
                        break; // receiver dropped
                    }
                    let elapsed = start.elapsed();
                    let sleep_dur = target_frame_duration.saturating_sub(elapsed);
                    if !sleep_dur.is_zero() {
                        std::thread::sleep(sleep_dur);
                    }
                }
            })?;

        #[cfg(feature = "video")]
        return Ok((None, Some(frame_rx), None));
        #[cfg(not(feature = "video"))]
        return Ok((None, Some(frame_rx)));
    }

    #[cfg(feature = "video")]
    if let Some(ref path) = cli.video {
        log::info!("Starting video source: {}", path.display());
        let (frame_tx, frame_rx) = flume::bounded(3);
        let (cmd_tx, cmd_rx) = flume::bounded(10);
        af_source::video::spawn_video_thread(path.clone(), frame_tx, cmd_rx, clock)?;
        return Ok((None, Some(frame_rx), Some(cmd_tx)));
    }

    #[cfg(feature = "video")]
    return Ok((None, None, None));
    #[cfg(not(feature = "video"))]
    return Ok((None, None));
}

/// Applique les mappings audio à une copie de la config avant le rendu.
///
/// `onset_envelope` est un signal synthétique calculé dans App (decay exponentiel).
/// `smooth_state` accumule l'EMA per-mapping (redimensionné si nécessaire).
///
/// # Example
/// ```
/// use af_core::config::RenderConfig;
/// use af_core::frame::AudioFeatures;
/// use af_app::pipeline::apply_audio_mappings;
///
/// let mut config = RenderConfig::default();
/// let features = AudioFeatures::default();
/// let mut smooth = vec![];
/// apply_audio_mappings(&mut config, &features, 0.0, &mut smooth);
/// ```
#[allow(clippy::too_many_lines)]
pub fn apply_audio_mappings(
    config: &mut RenderConfig,
    features: &AudioFeatures,
    onset_envelope: f32,
    smooth_state: &mut Vec<f32>,
) {
    use af_core::config::MappingCurve;

    let sensitivity = config.audio_sensitivity;
    let global_smoothing = config.audio_smoothing;

    // Resize smooth_state si le nombre de mappings a changé
    if smooth_state.len() != config.audio_mappings.len() {
        smooth_state.resize(config.audio_mappings.len(), 0.0);
    }

    for (i, mapping) in config.audio_mappings.iter().enumerate() {
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
            "timbral_brightness" => features.timbral_brightness,
            "timbral_roughness" => features.timbral_roughness,
            "onset_envelope" => onset_envelope,
            "spectral_rolloff" => features.spectral_rolloff,
            "zero_crossing_rate" => features.zero_crossing_rate,
            _ => 0.0,
        };

        // Apply response curve
        let shaped = match &mapping.curve {
            MappingCurve::Linear => source_value,
            MappingCurve::Exponential => source_value * source_value,
            MappingCurve::Threshold => {
                if source_value > 0.3 {
                    (source_value - 0.3) / 0.7
                } else {
                    0.0
                }
            }
            MappingCurve::Smooth => source_value * source_value * (3.0 - 2.0 * source_value),
        };

        let raw_delta = shaped * mapping.amount * sensitivity + mapping.offset;

        // Per-mapping EMA smoothing
        let alpha = mapping.smoothing.unwrap_or(global_smoothing);
        smooth_state[i] = smooth_state[i] * (1.0 - alpha) + raw_delta * alpha;
        let delta = smooth_state[i];

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
            "beat_flash_intensity" => {
                config.beat_flash_intensity = (config.beat_flash_intensity + delta).clamp(0.0, 2.0);
            }
            "chromatic_offset" => {
                config.chromatic_offset = (config.chromatic_offset + delta).clamp(0.0, 5.0);
            }
            "wave_amplitude" => {
                config.wave_amplitude = (config.wave_amplitude + delta).clamp(0.0, 1.0);
            }
            "color_pulse_speed" => {
                config.color_pulse_speed = (config.color_pulse_speed + delta).clamp(0.0, 5.0);
            }
            "fade_decay" => {
                config.fade_decay = (config.fade_decay + delta).clamp(0.0, 1.0);
            }
            "glow_intensity" => {
                config.glow_intensity = (config.glow_intensity + delta).clamp(0.0, 2.0);
            }
            "zalgo_intensity" => {
                config.zalgo_intensity = (config.zalgo_intensity + delta).clamp(0.0, 5.0);
            }
            "camera_zoom_amplitude" => {
                // Zoom varies around 1.0. Delta from audio typically modulates positively.
                // An arbitrary practical range like 0.1 (strong unzoom) to 10.0 (high zoom).
                config.camera_zoom_amplitude =
                    (config.camera_zoom_amplitude + delta * 2.0).clamp(0.1, 10.0);
            }
            "camera_rotation" => {
                // Audio delta -> Rotate left/right smoothly. We let it unbounded or wrap at 2PI later if needed.
                config.camera_rotation += delta * 0.1;
            }
            "camera_pan_x" => {
                // Audio delta for panning (wiggling) on X axis
                config.camera_pan_x = (config.camera_pan_x + delta * 0.5).clamp(-2.0, 2.0);
            }
            "camera_pan_y" => {
                // Audio delta for panning (wiggling) on Y axis
                config.camera_pan_y = (config.camera_pan_y + delta * 0.5).clamp(-2.0, 2.0);
            }
            _ => {}
        }
    }
}
