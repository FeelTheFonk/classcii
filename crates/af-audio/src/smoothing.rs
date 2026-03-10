use af_core::frame::AudioFeatures;

/// Exponential moving average smoothing with attack/release asymmetry.
///
/// Fast attack (responds quickly to increases), slow release (decays slowly).
///
/// # Example
/// ```
/// use af_audio::smoothing::FeatureSmoother;
/// let smoother = FeatureSmoother::new(0.3);
/// ```
pub struct FeatureSmoother {
    attack: f32,
    release: f32,
    prev: AudioFeatures,
    initialized: bool,
}

impl FeatureSmoother {
    /// Create a new smoother.
    ///
    /// `alpha` controls base responsiveness. Attack = alpha * 2, release = alpha * 0.5.
    #[must_use]
    pub fn new(alpha: f32) -> Self {
        let a = alpha.clamp(0.01, 1.0);
        Self {
            attack: (a * 2.0).min(1.0),
            release: (a * 0.5).max(0.01),
            prev: AudioFeatures::default(),
            initialized: false,
        }
    }

    /// Smooth with attack/release asymmetry per feature.
    pub fn smooth(&mut self, current: &AudioFeatures) -> AudioFeatures {
        if !self.initialized {
            self.prev = *current;
            self.initialized = true;
            return *current;
        }

        let mut smoothed = *current;

        // Adaptive smoothing per frequency band category:
        // Low = ×0.8 (punchy), Mid = ×1.0, High = ×0.7 (fast), Events = passthrough
        smoothed.rms = self.ar(current.rms, self.prev.rms);
        smoothed.peak = self.ar(current.peak, self.prev.peak);
        smoothed.sub_bass = self.ar_scaled(current.sub_bass, self.prev.sub_bass, 0.8);
        smoothed.bass = self.ar_scaled(current.bass, self.prev.bass, 0.8);
        smoothed.low_mid = self.ar(current.low_mid, self.prev.low_mid);
        smoothed.mid = self.ar(current.mid, self.prev.mid);
        smoothed.high_mid = self.ar_scaled(current.high_mid, self.prev.high_mid, 0.7);
        smoothed.presence = self.ar_scaled(current.presence, self.prev.presence, 0.7);
        smoothed.brilliance = self.ar_scaled(current.brilliance, self.prev.brilliance, 0.7);
        smoothed.spectral_centroid =
            self.ar(current.spectral_centroid, self.prev.spectral_centroid);
        smoothed.spectral_flux = self.ar(current.spectral_flux, self.prev.spectral_flux);
        smoothed.spectral_flatness =
            self.ar(current.spectral_flatness, self.prev.spectral_flatness);
        smoothed.bpm = self.ar(current.bpm, self.prev.bpm);
        smoothed.timbral_brightness =
            self.ar(current.timbral_brightness, self.prev.timbral_brightness);
        smoothed.timbral_roughness =
            self.ar(current.timbral_roughness, self.prev.timbral_roughness);
        smoothed.spectral_rolloff = self.ar(current.spectral_rolloff, self.prev.spectral_rolloff);
        smoothed.zero_crossing_rate = self.ar_scaled(
            current.zero_crossing_rate,
            self.prev.zero_crossing_rate,
            0.7,
        );

        // Events: NO smoothing — transient signals must pass through unattenuated.
        // beat_intensity, onset_envelope need full amplitude on first frame for punch.
        smoothed.onset = current.onset;
        smoothed.beat_phase = current.beat_phase;
        smoothed.beat_intensity = current.beat_intensity;
        smoothed.onset_envelope = current.onset_envelope;

        // Spectrum bands
        for i in 0..32 {
            smoothed.spectrum_bands[i] =
                self.ar(current.spectrum_bands[i], self.prev.spectrum_bands[i]);
        }

        self.prev = smoothed;
        smoothed
    }

    /// Attack/release smoothing for a single value.
    #[inline(always)]
    fn ar(&self, current: f32, previous: f32) -> f32 {
        let alpha = if current > previous {
            self.attack
        } else {
            self.release
        };
        alpha * current + (1.0 - alpha) * previous
    }

    /// Attack/release smoothing with per-band scaling factor.
    /// `scale` < 1.0 = slower response (smaller alpha = more smoothing), > 1.0 = faster response.
    #[inline(always)]
    fn ar_scaled(&self, current: f32, previous: f32, scale: f32) -> f32 {
        let base = if current > previous {
            self.attack
        } else {
            self.release
        };
        let alpha = (base * scale).clamp(0.01, 1.0);
        alpha * current + (1.0 - alpha) * previous
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn beat_intensity_bypasses_smoothing() {
        let mut smoother = FeatureSmoother::new(0.3);
        let mut features = AudioFeatures::default();

        // First call initializes
        features.beat_intensity = 0.0;
        smoother.smooth(&features);

        // Second call: beat spike should pass through at full amplitude
        features.beat_intensity = 1.0;
        let smoothed = smoother.smooth(&features);
        assert_eq!(
            smoothed.beat_intensity, 1.0,
            "beat_intensity must bypass smoothing (got {})",
            smoothed.beat_intensity
        );
    }

    #[test]
    fn onset_envelope_bypasses_smoothing() {
        let mut smoother = FeatureSmoother::new(0.3);
        let mut features = AudioFeatures::default();

        smoother.smooth(&features);

        features.onset_envelope = 0.8;
        let smoothed = smoother.smooth(&features);
        assert_eq!(
            smoothed.onset_envelope, 0.8,
            "onset_envelope must bypass smoothing (got {})",
            smoothed.onset_envelope
        );
    }

    #[test]
    fn bass_is_smoothed() {
        let mut smoother = FeatureSmoother::new(0.3);
        let mut features = AudioFeatures::default();

        features.bass = 0.0;
        smoother.smooth(&features);

        features.bass = 1.0;
        let smoothed = smoother.smooth(&features);
        // With scale=0.8, attack = min(0.6*0.8, 1.0) = 0.48
        // smoothed = 0.48 * 1.0 + 0.52 * 0.0 = 0.48
        assert!(
            smoothed.bass < 1.0 && smoothed.bass > 0.3,
            "bass should be smoothed (got {})",
            smoothed.bass
        );
    }

    #[test]
    fn onset_bool_not_smoothed() {
        let mut smoother = FeatureSmoother::new(0.3);
        let mut features = AudioFeatures::default();

        features.onset = false;
        smoother.smooth(&features);

        features.onset = true;
        let smoothed = smoother.smooth(&features);
        assert!(smoothed.onset, "onset bool must pass through directly");
    }
}
