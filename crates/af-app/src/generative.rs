use af_core::config::{MappingCurve, RenderConfig};
use af_core::feature_timeline::FeatureTimeline;
use af_core::frame::AudioFeatures;

/// Moteur génératif offline adaptant config + audio feature timeline.
///
/// Applique les audio mappings avec MappingCurve et EMA smoothing,
/// en parité complète avec le pipeline interactif (`pipeline::apply_audio_mappings`).
pub struct AutoGenerativeMapper {
    base_config: RenderConfig,
    timeline: FeatureTimeline,
    smooth_state: Vec<f32>,
}

impl AutoGenerativeMapper {
    #[must_use]
    pub fn new(base_config: RenderConfig, timeline: FeatureTimeline) -> Self {
        let n = base_config.audio_mappings.len();
        Self {
            base_config,
            timeline,
            smooth_state: vec![0.0; n],
        }
    }

    /// Applique les mappings audio sur `out`, en le réinitialisant depuis `base_config`.
    ///
    /// L'`onset_envelope` est calculé par l'appelant (batch loop) et passé ici.
    /// L'EMA smoothing est appliqué séquentiellement frame-par-frame, exactement
    /// comme le pipeline interactif.
    pub fn apply_at(&mut self, timestamp_secs: f64, onset_envelope: f32, out: &mut RenderConfig) {
        let features = self.timeline.get_at_time(timestamp_secs);
        out.clone_from(&self.base_config);

        let sensitivity = self.base_config.audio_sensitivity;
        let global_smoothing = self.base_config.audio_smoothing;

        if self.smooth_state.len() != self.base_config.audio_mappings.len() {
            self.smooth_state
                .resize(self.base_config.audio_mappings.len(), 0.0);
        }

        for (i, mapping) in self.base_config.audio_mappings.iter().enumerate() {
            if !mapping.enabled {
                continue;
            }

            let source_val = resolve_source(&features, mapping.source.as_str(), onset_envelope);

            // Apply response curve (parité avec pipeline.rs)
            let shaped = apply_curve(&mapping.curve, source_val);

            let raw_delta = shaped * mapping.amount * sensitivity + mapping.offset;

            // Per-mapping EMA smoothing
            let alpha = mapping.smoothing.unwrap_or(global_smoothing);
            self.smooth_state[i] = self.smooth_state[i] * (1.0 - alpha) + raw_delta * alpha;
            let delta = self.smooth_state[i];

            apply_target(out, mapping.target.as_str(), delta);
        }
    }

    /// Extrait la timeline complete.
    #[must_use]
    pub fn get_timeline(&self) -> &FeatureTimeline {
        &self.timeline
    }

    /// Remplace la config de base (utilisé par le preset sequencer en mode --preset all).
    pub fn set_base_config(&mut self, config: RenderConfig) {
        self.base_config = config;
    }
}

fn resolve_source(features: &AudioFeatures, source: &str, onset_envelope: f32) -> f32 {
    match source {
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
    }
}

fn apply_curve(curve: &MappingCurve, value: f32) -> f32 {
    match curve {
        MappingCurve::Linear => value,
        MappingCurve::Exponential => value * value,
        MappingCurve::Threshold => {
            if value > 0.3 {
                (value - 0.3) / 0.7
            } else {
                0.0
            }
        }
        MappingCurve::Smooth => value * value * (3.0 - 2.0 * value),
    }
}

fn apply_target(config: &mut RenderConfig, target: &str, delta: f32) {
    match target {
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
            config.camera_zoom_amplitude =
                (config.camera_zoom_amplitude + delta * 2.0).clamp(0.1, 10.0);
        }
        "camera_rotation" => {
            config.camera_rotation += delta * 0.1;
        }
        "camera_pan_x" => {
            config.camera_pan_x = (config.camera_pan_x + delta * 0.5).clamp(-2.0, 2.0);
        }
        "camera_pan_y" => {
            config.camera_pan_y = (config.camera_pan_y + delta * 0.5).clamp(-2.0, 2.0);
        }
        _ => {}
    }
}
