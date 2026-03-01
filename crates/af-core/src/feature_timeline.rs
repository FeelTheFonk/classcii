use crate::frame::AudioFeatures;

/// Une timeline complète pré-calculée des features audio d'un morceau.
/// Utilisée pour le rendu offline (batch export).
#[derive(Clone)]
pub struct FeatureTimeline {
    /// Les features extraites pour chaque frame.
    pub frames: Vec<AudioFeatures>,
    /// Durée de chaque frame en secondes (typiquement 1.0 / fps).
    pub frame_duration: f32,
    /// Le taux d'échantillonnage de l'audio source.
    pub sample_rate: u32,
    /// Energy classification per frame (0=low, 1=medium, 2=high).
    pub energy_levels: Vec<u8>,
}

impl FeatureTimeline {
    /// Obtenir les features à un temps `t` (en secondes).
    ///
    /// # Example
    /// ```
    /// use af_core::feature_timeline::FeatureTimeline;
    /// let timeline = FeatureTimeline { frames: vec![], frame_duration: 0.016, sample_rate: 44100, energy_levels: vec![] };
    /// let features = timeline.get_at_time(1.0);
    /// ```
    #[must_use]
    pub fn get_at_time(&self, time: f64) -> AudioFeatures {
        if self.frames.is_empty() {
            return AudioFeatures::default();
        }

        let index = (time as f32 / self.frame_duration) as usize;
        let clamped_index = index.min(self.frames.len().saturating_sub(1));
        self.frames[clamped_index]
    }

    /// Nombre total de frames pré-analysées.
    #[must_use]
    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }

    /// Energy level at a given frame index (0=low, 1=medium, 2=high).
    #[must_use]
    pub fn energy_at(&self, frame_idx: usize) -> u8 {
        self.energy_levels.get(frame_idx).copied().unwrap_or(1) // default medium
    }

    /// Normalize continuous audio features to [0, 1] across the entire timeline.
    ///
    /// Ensures consistent dynamic range regardless of source volume.
    /// Binary/phase/BPM fields are left untouched.
    pub fn normalize(&mut self) {
        if self.frames.len() < 2 {
            return;
        }

        macro_rules! collect_minmax {
            ($field:ident, $min:ident, $max:ident) => {
                let mut $min = f32::MAX;
                let mut $max = f32::MIN;
                for f in &self.frames {
                    $min = $min.min(f.$field);
                    $max = $max.max(f.$field);
                }
            };
        }

        collect_minmax!(rms, rms_min, rms_max);
        collect_minmax!(peak, peak_min, peak_max);
        collect_minmax!(sub_bass, sub_bass_min, sub_bass_max);
        collect_minmax!(bass, bass_min, bass_max);
        collect_minmax!(low_mid, low_mid_min, low_mid_max);
        collect_minmax!(mid, mid_min, mid_max);
        collect_minmax!(high_mid, high_mid_min, high_mid_max);
        collect_minmax!(presence, presence_min, presence_max);
        collect_minmax!(brilliance, brilliance_min, brilliance_max);
        collect_minmax!(spectral_centroid, sc_min, sc_max);
        collect_minmax!(spectral_flux, sf_min, sf_max);
        collect_minmax!(spectral_flatness, sflt_min, sflt_max);
        collect_minmax!(spectral_rolloff, sr_min, sr_max);
        collect_minmax!(zero_crossing_rate, zcr_min, zcr_max);
        collect_minmax!(timbral_brightness, tb_min, tb_max);
        collect_minmax!(timbral_roughness, tr_min, tr_max);
        collect_minmax!(onset_envelope, oe_min, oe_max);

        for f in &mut self.frames {
            f.rms = norm(f.rms, rms_min, rms_max);
            f.peak = norm(f.peak, peak_min, peak_max);
            f.sub_bass = norm(f.sub_bass, sub_bass_min, sub_bass_max);
            f.bass = norm(f.bass, bass_min, bass_max);
            f.low_mid = norm(f.low_mid, low_mid_min, low_mid_max);
            f.mid = norm(f.mid, mid_min, mid_max);
            f.high_mid = norm(f.high_mid, high_mid_min, high_mid_max);
            f.presence = norm(f.presence, presence_min, presence_max);
            f.brilliance = norm(f.brilliance, brilliance_min, brilliance_max);
            f.spectral_centroid = norm(f.spectral_centroid, sc_min, sc_max);
            f.spectral_flux = norm(f.spectral_flux, sf_min, sf_max);
            f.spectral_flatness = norm(f.spectral_flatness, sflt_min, sflt_max);
            f.spectral_rolloff = norm(f.spectral_rolloff, sr_min, sr_max);
            f.zero_crossing_rate = norm(f.zero_crossing_rate, zcr_min, zcr_max);
            f.timbral_brightness = norm(f.timbral_brightness, tb_min, tb_max);
            f.timbral_roughness = norm(f.timbral_roughness, tr_min, tr_max);
            f.onset_envelope = norm(f.onset_envelope, oe_min, oe_max);
        }
    }

    /// Compute per-frame energy levels from smoothed RMS.
    ///
    /// Uses a 5-second sliding window average and 30th/70th percentile thresholds
    /// to classify each frame as low (0), medium (1), or high (2) energy.
    pub fn compute_energy_levels(&mut self) {
        if self.frames.is_empty() {
            return;
        }

        let n = self.frames.len();
        let window = ((5.0 / self.frame_duration) as usize).max(1);
        let half = window / 2;

        // Sliding window average of RMS
        let mut smooth_rms = Vec::with_capacity(n);
        for i in 0..n {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            let sum: f32 = self.frames[lo..hi].iter().map(|f| f.rms).sum();
            smooth_rms.push(sum / (hi - lo) as f32);
        }

        // Percentile thresholds
        let mut sorted = smooth_rms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let p30 = sorted[n * 30 / 100];
        let p70 = sorted[n * 70 / 100];

        self.energy_levels = smooth_rms
            .iter()
            .map(|&v| {
                if v < p30 {
                    0
                } else if v < p70 {
                    1
                } else {
                    2
                }
            })
            .collect();
    }
}

/// Normalize a value to [0, 1] given min/max range. Returns 0.5 for degenerate ranges.
#[inline]
fn norm(val: f32, min: f32, max: f32) -> f32 {
    let range = max - min;
    if range < 1e-6 {
        0.5
    } else {
        ((val - min) / range).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::AudioFeatures;

    #[test]
    fn normalize_includes_onset_envelope() {
        let mut timeline = FeatureTimeline {
            frames: vec![
                AudioFeatures {
                    onset_envelope: 0.0,
                    rms: 0.1,
                    ..AudioFeatures::default()
                },
                AudioFeatures {
                    onset_envelope: 0.5,
                    rms: 0.5,
                    ..AudioFeatures::default()
                },
                AudioFeatures {
                    onset_envelope: 1.0,
                    rms: 1.0,
                    ..AudioFeatures::default()
                },
            ],
            frame_duration: 1.0 / 60.0,
            sample_rate: 44100,
            energy_levels: vec![],
        };
        timeline.normalize();
        assert!(
            (timeline.frames[0].onset_envelope - 0.0).abs() < f32::EPSILON,
            "min onset_envelope should normalize to 0"
        );
        assert!(
            (timeline.frames[2].onset_envelope - 1.0).abs() < f32::EPSILON,
            "max onset_envelope should normalize to 1"
        );
        assert!(
            (timeline.frames[1].onset_envelope - 0.5).abs() < f32::EPSILON,
            "mid onset_envelope should normalize to 0.5"
        );
    }

    #[test]
    fn energy_levels_classification() {
        // 600 frames at 60fps = 10 seconds (larger than 5s sliding window)
        let mut timeline = FeatureTimeline {
            frames: (0..600)
                .map(|i| AudioFeatures {
                    rms: i as f32 / 600.0,
                    ..AudioFeatures::default()
                })
                .collect(),
            frame_duration: 1.0 / 60.0,
            sample_rate: 44100,
            energy_levels: vec![],
        };
        timeline.compute_energy_levels();
        assert_eq!(timeline.energy_levels.len(), 600);
        // All three energy levels should be present
        assert!(
            timeline.energy_levels.contains(&0),
            "should have low energy"
        );
        assert!(
            timeline.energy_levels.contains(&1),
            "should have medium energy"
        );
        assert!(
            timeline.energy_levels.contains(&2),
            "should have high energy"
        );
    }
}
