use af_core::config::RenderConfig;
use af_core::feature_timeline::FeatureTimeline;
use std::sync::Arc;

/// Moteur génératif offline adaptant config + audio feature timeline
pub struct AutoGenerativeMapper {
    base_config: RenderConfig,
    timeline: FeatureTimeline,
}

impl AutoGenerativeMapper {
    #[must_use]
    pub fn new(base_config: RenderConfig, timeline: FeatureTimeline) -> Self {
        Self {
            base_config,
            timeline,
        }
    }

    /// Génère une configuration temporaire adaptée au contexte audio courant (timestamp_secs)
    #[must_use]
    pub fn config_at(&self, timestamp_secs: f64) -> Arc<RenderConfig> {
        let features = self.timeline.get_at_time(timestamp_secs);
        let mut modulated = self.base_config.clone();

        // Application des mappings auto-génératifs hors-ligne (batch)
        for mapping in &self.base_config.audio_mappings {
            if !mapping.enabled {
                continue;
            }

            let source_val = match mapping.source.as_str() {
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
                // Boolean traits sont convertis en floats [0.0, 1.0]
                "onset" => {
                    if features.onset {
                        1.0
                    } else {
                        0.0
                    }
                }
                "beat_phase" => features.beat_phase,
                "bpm" => features.bpm / 200.0, // Normalize rough max BPM
                _ => continue,
            };

            // Apply global sensitivity and per-mapping amount/offset
            let mut final_val = source_val * self.base_config.audio_sensitivity;
            // Note: In real-time this is smoothed via Ema, but for batch generative
            // we apply the raw feature_timeline mapping directly.
            final_val = (final_val * mapping.amount) + mapping.offset;

            match mapping.target.as_str() {
                "edge_threshold" => {
                    modulated.edge_threshold =
                        (modulated.edge_threshold + final_val).clamp(0.0, 1.0);
                }
                "edge_mix" => {
                    modulated.edge_mix = (modulated.edge_mix + final_val).clamp(0.0, 1.0);
                }
                "contrast" => {
                    modulated.contrast = (modulated.contrast + final_val).clamp(0.0, 2.0);
                }
                "brightness" => {
                    modulated.brightness = (modulated.brightness + final_val).clamp(-1.0, 1.0);
                }
                "saturation" => {
                    modulated.saturation = (modulated.saturation + final_val).clamp(0.0, 2.0);
                }
                "density_scale" => {
                    // Map [0.0, 1.0] source -> [0.25, 4.0] scale roughly
                    modulated.density_scale =
                        (modulated.density_scale + (final_val * 2.0)).clamp(0.25, 4.0);
                }
                "invert" => {
                    if final_val > 0.5 {
                        modulated.invert = !modulated.invert;
                    }
                }
                _ => {}
            }
        }

        Arc::new(modulated)
    }

    /// Extrait la timeline complete.
    #[must_use]
    pub fn get_timeline(&self) -> &FeatureTimeline {
        &self.timeline
    }
}
