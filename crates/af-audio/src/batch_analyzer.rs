use std::collections::VecDeque;

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
                energy_levels: Vec::new(),
            };
        }

        let num_frames = samples.len().div_ceil(samples_per_frame);
        let mut frames = Vec::with_capacity(num_frames);

        let mut prev_magnitudes: Vec<f32> = Vec::new();

        for i in 0..num_frames {
            let start = i * samples_per_frame;
            let end = (start + self.fft.fft_size()).min(samples.len());

            let frame_samples = if start < samples.len() {
                &samples[start..end]
            } else {
                &[]
            };

            let magnitudes = self.fft.process(frame_samples);
            let mut features = extract_features(frame_samples, magnitudes, self.sample_rate);

            // Bass-weighted spectral flux (parity with BeatDetector in beat.rs)
            // Normalized by bin count for volume-independent beat detection.
            let mut flux = 0.0f32;
            if prev_magnitudes.len() == magnitudes.len() {
                let bass_cutoff = magnitudes.len() / 4;
                for (j, (curr, prev)) in magnitudes.iter().zip(prev_magnitudes.iter()).enumerate() {
                    let diff = (curr - prev).max(0.0);
                    flux += if j < bass_cutoff { diff * 2.0 } else { diff };
                }
                flux /= magnitudes.len().max(1) as f32;
            } else {
                prev_magnitudes.resize(magnitudes.len(), 0.0);
            }
            features.spectral_flux = flux;
            prev_magnitudes.copy_from_slice(magnitudes);

            frames.push(features);
        }

        // Post-processing: onset detection with BPM/beat_phase (parity with BeatDetector)
        Self::detect_onsets(&mut frames, self.target_fps as f32);

        let mut timeline = FeatureTimeline {
            frames,
            frame_duration,
            sample_rate: self.sample_rate,
            energy_levels: Vec::new(),
        };

        // Normalize features to [0, 1] across the entire track
        timeline.normalize();
        // Compute energy classification for clip pacing
        timeline.compute_energy_levels();

        timeline
    }

    /// Offline onset detection replicating the interactive BeatDetector logic.
    ///
    /// Features: warmup skip, FPS-adaptive cooldown, BPM estimation from
    /// inter-onset intervals, beat_phase accumulator.
    fn detect_onsets(frames: &mut [af_core::frame::AudioFeatures], fps: f32) {
        let cooldown = (fps * 0.13).max(2.0) as usize;
        let mut ema_flux = 0.0f32;
        let mut last_onset: usize = 0;
        let mut intervals: VecDeque<usize> = VecDeque::with_capacity(16);
        let mut bpm: f32 = 0.0;
        let mut phase: f32 = 0.0;
        let mut onset_env: f32 = 0.0;
        let strobe_decay: f32 = 0.85;

        for (i, frame) in frames.iter_mut().enumerate() {
            let flux = frame.spectral_flux;

            // Adaptive threshold (parity with beat.rs EMA alpha=0.07)
            ema_flux = ema_flux * 0.93 + flux * 0.07;
            let threshold = ema_flux * 1.5 + 0.01;

            // Warmup: skip first ~10 frames to avoid false positives
            // Silence guard: reject onsets when RMS is negligible
            let warmup_complete = i > 10;
            let since = i.saturating_sub(last_onset);
            let onset = warmup_complete && frame.rms > 1e-4 && flux > threshold && since > cooldown;

            if onset {
                frame.onset = true;
                frame.beat_intensity = ((flux - threshold) / (threshold + 0.001)).clamp(0.0, 1.0);
                last_onset = i;

                // BPM estimation from onset intervals
                if since > 5 && since < 300 {
                    intervals.push_back(since);
                    if intervals.len() > 16 {
                        intervals.pop_front();
                    }

                    if intervals.len() >= 4 {
                        let avg: f64 = intervals.iter().map(|&v| v as f64).sum::<f64>()
                            / intervals.len() as f64;
                        if avg > 0.0 {
                            bpm = (60.0 * f64::from(fps) / avg) as f32;
                            bpm = bpm.clamp(30.0, 300.0);
                        }
                    }
                }

                phase = 0.0;
                onset_env = 1.0;
            } else {
                frame.onset = false;
                frame.beat_intensity = 0.0;
                onset_env *= strobe_decay;

                // Phase accumulation between beats
                if bpm > 0.0 {
                    phase = (phase + bpm / (60.0 * fps)) % 1.0;
                }
            }

            frame.onset_envelope = onset_env;
            frame.bpm = bpm;
            frame.beat_phase = phase;
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
