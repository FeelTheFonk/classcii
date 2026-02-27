use crate::features::extract_features;
use crate::fft::FftPipeline;
use af_core::feature_timeline::FeatureTimeline;

/// Analyseur audio pour le traitement offline en lot (Batch Export).
///
/// Divise un vecteur d'échantillons en frames correspondant au framerate cible,
/// et extrait les `AudioFeatures` pour générer une `FeatureTimeline`.
pub struct BatchAnalyzer {
    fft: FftPipeline,
    target_fps: u32,
    sample_rate: u32,
}

impl BatchAnalyzer {
    /// Crée un nouvel analyseur batch.
    ///
    /// # Panics
    /// Panics if `fft_size` is 0.
    ///
    /// # Example
    /// ```
    /// use af_audio::batch_analyzer::BatchAnalyzer;
    /// let analyzer = BatchAnalyzer::new(60, 44100, 2048);
    /// ```
    #[must_use]
    pub fn new(target_fps: u32, sample_rate: u32, fft_size: usize) -> Self {
        Self {
            fft: FftPipeline::new(fft_size),
            target_fps,
            sample_rate,
        }
    }

    /// Analyse l'intégralité d'un buffer audio et génère une `FeatureTimeline`.
    ///
    /// # Example
    /// ```
    /// use af_audio::batch_analyzer::BatchAnalyzer;
    /// let mut analyzer = BatchAnalyzer::new(60, 44100, 2048);
    /// let samples = vec![0.0; 44100]; // 1 seconde de silence
    /// let timeline = analyzer.analyze_all(&samples);
    /// assert_eq!(timeline.frames.len(), 60);
    /// ```
    #[must_use]
    pub fn analyze_all(&mut self, samples: &[f32]) -> FeatureTimeline {
        let frame_duration = 1.0 / self.target_fps as f32;
        let samples_per_frame = (self.sample_rate as f32 * frame_duration) as usize;

        // Zero division protection
        if samples_per_frame == 0 {
            return FeatureTimeline {
                frames: Vec::new(),
                frame_duration,
                sample_rate: self.sample_rate,
            };
        }

        let num_frames = samples.len().div_ceil(samples_per_frame);
        let mut frames = Vec::with_capacity(num_frames);

        for i in 0..num_frames {
            let start = i * samples_per_frame;
            // L'analyseur prend `fft_size` échantillons s'ils sont disponibles,
            // ou zero-pad la fin si ce n'est pas le cas.
            let end = (start + self.fft.fft_size()).min(samples.len());

            let frame_samples = if start < samples.len() {
                &samples[start..end]
            } else {
                &[]
            };

            let magnitudes = self.fft.process(frame_samples);
            let features = extract_features(frame_samples, magnitudes, self.sample_rate);

            // FIXME: Offline Onset detection/smoothing could go here.
            // Pour l'instant on garde les features brutes. Le rendu lissera à la volée.
            // On peut détecter les "major onsets" post-analyse si besoin.

            frames.push(features);
        }

        // Post-processing : identification des onsets majeurs
        Self::detect_onsets(&mut frames);

        FeatureTimeline {
            frames,
            frame_duration,
            sample_rate: self.sample_rate,
        }
    }

    /// Détection offline simplifiée des onsets basée sur le flux spectral.
    fn detect_onsets(frames: &mut [af_core::frame::AudioFeatures]) {
        let mut ema_flux = 0.0;
        let alpha = 0.1; // Smoothing factor
        let threshold = 1.5; // Multiplier over mean

        for frame in frames.iter_mut() {
            let flux = frame.spectral_flux;
            let current_mean = ema_flux;

            ema_flux = alpha * flux + (1.0 - alpha) * ema_flux;

            if flux > current_mean * threshold && flux > 0.01 {
                frame.onset = true;
                frame.beat_intensity = (flux / (current_mean + 1e-5)).min(1.0);
            } else {
                frame.onset = false;
                frame.beat_intensity = 0.0;
            }
        }
    }

    /// Décode un fichier audio et analyse l'intégralité de ses échantillons.
    ///
    /// # Errors
    /// Retourne une erreur si le fichier ne peut être décodé.
    pub fn analyze_file(&mut self, path: &std::path::Path) -> anyhow::Result<FeatureTimeline> {
        let (samples, actual_sr) = crate::decode::decode_file(path)?;
        self.sample_rate = actual_sr;
        Ok(self.analyze_all(&samples))
    }
}
