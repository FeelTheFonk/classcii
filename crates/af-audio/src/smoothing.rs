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

        // Per-field smoothing with attack/release
        smoothed.rms = self.ar(current.rms, self.prev.rms);
        smoothed.peak = self.ar(current.peak, self.prev.peak);
        smoothed.sub_bass = self.ar(current.sub_bass, self.prev.sub_bass);
        smoothed.bass = self.ar(current.bass, self.prev.bass);
        smoothed.low_mid = self.ar(current.low_mid, self.prev.low_mid);
        smoothed.mid = self.ar(current.mid, self.prev.mid);
        smoothed.high_mid = self.ar(current.high_mid, self.prev.high_mid);
        smoothed.presence = self.ar(current.presence, self.prev.presence);
        smoothed.brilliance = self.ar(current.brilliance, self.prev.brilliance);
        smoothed.spectral_centroid =
            self.ar(current.spectral_centroid, self.prev.spectral_centroid);
        smoothed.spectral_flux = self.ar(current.spectral_flux, self.prev.spectral_flux);
        smoothed.spectral_flatness =
            self.ar(current.spectral_flatness, self.prev.spectral_flatness);
        smoothed.bpm = self.ar(current.bpm, self.prev.bpm);
        smoothed.beat_intensity = self.ar(current.beat_intensity, self.prev.beat_intensity);

        // Events: no smoothing
        smoothed.onset = current.onset;
        smoothed.beat_phase = current.beat_phase;

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
}
