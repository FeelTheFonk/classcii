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
    let input_gain = config.load().input_gain;

    match audio_arg {
        "default" | "mic" | "microphone" => {
            log::info!("Starting microphone capture (gain={input_gain:.1})");
            let out = af_audio::state::spawn_audio_thread(fps, smoothing, input_gain)?;
            Ok((out, None))
        }
        path => {
            let audio_path = std::path::Path::new(path);
            if audio_path.exists() {
                log::info!("Starting audio file analysis: {path} (gain={input_gain:.1})");
                let (cmd_tx, cmd_rx) = flume::bounded(10);
                let out = af_audio::state::spawn_audio_file_thread(
                    audio_path, fps, smoothing, input_gain, cmd_rx, clock,
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
/// `target_fps` permet la correction framerate-independent du lissage per-mapping.
///
/// Le lissage per-mapping est **opt-in** : seuls les mappings avec `smoothing: Some(val)`
/// appliquent un EMA supplémentaire. Sans override, les features (déjà lissées par
/// `FeatureSmoother`) sont utilisées directement — évite le double-smoothing.
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
/// apply_audio_mappings(&mut config, &features, None, 0.0, &mut smooth, 60);
/// ```
#[allow(clippy::too_many_lines)]
pub fn apply_audio_mappings(
    config: &mut RenderConfig,
    features: &AudioFeatures,
    stem_features: Option<&af_stems::stem::StemFeatures>,
    onset_envelope: f32,
    smooth_state: &mut Vec<f32>,
    target_fps: u32,
) {
    use af_core::config::MappingCurve;

    let sensitivity = config.audio_sensitivity;

    // Resize smooth_state si le nombre de mappings a changé
    if smooth_state.len() != config.audio_mappings.len() {
        smooth_state.resize(config.audio_mappings.len(), 0.0);
    }

    for (i, mapping) in config.audio_mappings.iter().enumerate() {
        if !mapping.enabled {
            continue;
        }

        // Resolve features from per-stem data if mapping has stem_source
        let effective_features = match (&mapping.stem_source, stem_features) {
            (Some(stem_name), Some(sf)) => {
                let stem_idx = match stem_name.as_str() {
                    "drums" => 0usize,
                    "bass" => 1,
                    "other" => 2,
                    "vocals" => 3,
                    _ => usize::MAX,
                };
                if stem_idx < 4 {
                    &sf.features[stem_idx]
                } else {
                    features
                }
            }
            _ => features,
        };

        let source_value = match mapping.source.as_str() {
            "rms" => effective_features.rms,
            "peak" => effective_features.peak,
            "sub_bass" => effective_features.sub_bass,
            "bass" => effective_features.bass,
            "low_mid" => effective_features.low_mid,
            "mid" => effective_features.mid,
            "high_mid" => effective_features.high_mid,
            "presence" => effective_features.presence,
            "brilliance" => effective_features.brilliance,
            "spectral_centroid" => effective_features.spectral_centroid,
            "spectral_flux" => effective_features.spectral_flux,
            "spectral_flatness" => effective_features.spectral_flatness,
            "beat_intensity" => effective_features.beat_intensity,
            "onset" => {
                if effective_features.onset {
                    1.0
                } else {
                    0.0
                }
            }
            "beat_phase" => effective_features.beat_phase,
            "bpm" => effective_features.bpm / 300.0,
            "timbral_brightness" => effective_features.timbral_brightness,
            "timbral_roughness" => effective_features.timbral_roughness,
            "onset_envelope" => onset_envelope,
            "spectral_rolloff" => effective_features.spectral_rolloff,
            "zero_crossing_rate" => effective_features.zero_crossing_rate,
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

        // Per-mapping EMA smoothing — opt-in only.
        // Without explicit per-mapping smoothing, features pass through directly
        // (already smoothed by FeatureSmoother in the audio thread).
        let delta = if let Some(user_alpha) = mapping.smoothing {
            // Framerate-independent correction: alpha calibrated for 60 FPS baseline.
            let fps = f32::from(target_fps.max(1) as u16);
            let alpha = 1.0 - (1.0 - user_alpha).powf(60.0 / fps);
            smooth_state[i] = smooth_state[i] * (1.0 - alpha) + raw_delta * alpha;
            smooth_state[i]
        } else {
            // No per-mapping smoothing — direct passthrough (eliminates double-smoothing)
            smooth_state[i] = raw_delta;
            raw_delta
        };

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
                config.invert = delta > 0.5;
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
                config.camera_rotation += delta * 0.1;
                // Wrap at TAU to prevent float precision degradation
                config.camera_rotation = config.camera_rotation.rem_euclid(std::f32::consts::TAU);
            }
            "camera_pan_x" => {
                // Audio delta for panning (wiggling) on X axis
                config.camera_pan_x = (config.camera_pan_x + delta * 0.5).clamp(-2.0, 2.0);
            }
            "camera_pan_y" => {
                // Audio delta for panning (wiggling) on Y axis
                config.camera_pan_y = (config.camera_pan_y + delta * 0.5).clamp(-2.0, 2.0);
            }
            "camera_tilt_x" => {
                config.camera_tilt_x = (config.camera_tilt_x + delta * 0.3).clamp(-1.0, 1.0);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use af_core::config::{AudioMapping, MappingCurve, RenderConfig};
    use af_core::frame::AudioFeatures;

    #[test]
    fn no_smoothing_by_default_direct_passthrough() {
        let mut config = RenderConfig::default();
        let mut features = AudioFeatures::default();
        features.bass = 0.5;
        let mut smooth = vec![];

        apply_audio_mappings(&mut config, &features, None, 0.0, &mut smooth, 60);

        // With Smooth curve on bass=0.5: shaped = 3*(0.25) - 2*(0.125) = 0.5
        // delta = 0.5 * 0.7 * 2.0 = 0.7 — direct passthrough (no per-mapping EMA)
        assert!(
            config.edge_threshold > 0.5,
            "bass mapping should produce substantial edge_threshold, got {}",
            config.edge_threshold
        );
    }

    #[test]
    fn explicit_smoothing_applies_ema() {
        let mut config = RenderConfig::default();
        config.audio_mappings = vec![AudioMapping {
            enabled: true,
            source: "rms".into(),
            target: "brightness".into(),
            amount: 1.0,
            offset: 0.0,
            curve: MappingCurve::Linear,
            smoothing: Some(0.3), // Explicit per-mapping smoothing
            stem_source: None,
        }];
        let mut features = AudioFeatures::default();
        features.rms = 1.0;
        let mut smooth = vec![];

        // First frame: EMA with alpha=0.3 → 0.3 * raw_delta + 0.7 * 0
        apply_audio_mappings(&mut config, &features, None, 0.0, &mut smooth, 60);
        let first = config.brightness;

        // With smoothing, first frame should be substantially less than raw delta
        // raw_delta = 1.0 * 1.0 * 2.0 = 2.0, smoothed ≈ 0.3 * 2.0 = 0.6
        assert!(
            first < 1.5,
            "with smoothing=0.3, first frame should be dampened, got {first}"
        );
    }

    #[test]
    fn onset_envelope_passthrough() {
        let mut config = RenderConfig::default();
        config.audio_mappings = vec![AudioMapping {
            enabled: true,
            source: "onset_envelope".into(),
            target: "brightness".into(),
            amount: 1.0,
            offset: 0.0,
            curve: MappingCurve::Linear,
            smoothing: None,
            stem_source: None,
        }];
        let features = AudioFeatures::default();
        let mut smooth = vec![];

        apply_audio_mappings(&mut config, &features, None, 0.75, &mut smooth, 60);
        // delta = 0.75 * 1.0 * 2.0 = 1.5, clamped brightness to 1.0
        assert!(
            config.brightness > 0.5,
            "onset_envelope should pass through directly, got {}",
            config.brightness
        );
    }

    #[test]
    fn stem_source_resolves_per_stem_features() {
        use af_stems::stem::StemFeatures;

        let mut config = RenderConfig::default();
        config.audio_mappings = vec![AudioMapping {
            enabled: true,
            source: "bass".into(),
            target: "brightness".into(),
            amount: 1.0,
            offset: 0.0,
            curve: MappingCurve::Linear,
            smoothing: None,
            stem_source: Some("drums".into()), // stem index 0
        }];

        // Combined features have bass=0.0 (should NOT be used)
        let combined = AudioFeatures::default();

        // Stem features: drums (index 0) has bass=0.8
        let mut stem_feats = StemFeatures::default();
        stem_feats.features[0].bass = 0.8;

        let mut smooth = vec![];
        apply_audio_mappings(
            &mut config,
            &combined,
            Some(&stem_feats),
            0.0,
            &mut smooth,
            60,
        );

        // Should use drums bass=0.8, not combined bass=0.0
        assert!(
            config.brightness > 0.3,
            "stem_source='drums' should resolve from stem features, got brightness={}",
            config.brightness
        );
    }

    #[test]
    fn stem_source_falls_back_without_stem_features() {
        let mut config = RenderConfig::default();
        config.audio_mappings = vec![AudioMapping {
            enabled: true,
            source: "bass".into(),
            target: "brightness".into(),
            amount: 1.0,
            offset: 0.0,
            curve: MappingCurve::Linear,
            smoothing: None,
            stem_source: Some("drums".into()),
        }];

        let mut combined = AudioFeatures::default();
        combined.bass = 0.5;

        let mut smooth = vec![];
        // Pass None for stem_features → should fall back to combined
        apply_audio_mappings(&mut config, &combined, None, 0.0, &mut smooth, 60);

        assert!(
            config.brightness > 0.2,
            "without stem features, should fall back to combined, got brightness={}",
            config.brightness
        );
    }
}
