/// Simple onset / beat detection.

/// Simple onset / beat detection.
///
/// Uses spectral flux with an adaptive threshold and onset cooldown.
///
/// # Example
/// ```
/// use af_audio::beat::BeatDetector;
/// let detector = BeatDetector::new();
/// ```
pub struct BeatDetector {
    /// Previous spectrum for flux calculation.
    prev_spectrum: Vec<f32>,
    /// Running average of flux for adaptive threshold.
    flux_avg: f32,
    /// Last known BPM.
    bpm: f32,
    /// Phase accumulator [0.0, 1.0].
    phase: f32,
    /// Timestamp of last onset (in frames).
    last_onset_frame: u64,
    /// Current frame counter.
    frame_count: u64,
    /// Onset interval accumulator for BPM estimation.
    intervals: Vec<u64>,
    /// Minimum frames between onsets (cooldown).
    onset_cooldown: u64,
}

impl BeatDetector {
    /// Create a new beat detector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            prev_spectrum: Vec::new(),
            flux_avg: 0.0,
            bpm: 0.0,
            phase: 0.0,
            last_onset_frame: 0,
            frame_count: 0,
            intervals: Vec::with_capacity(16),
            onset_cooldown: 4, // ~133ms @ 30fps, prevents machine-gun
        }
    }

    /// Process a spectrum frame.
    ///
    /// Returns (onset, beat_intensity, bpm, phase).
    pub fn process(&mut self, spectrum: &[f32], fps: f32) -> (bool, f32, f32, f32) {
        self.frame_count += 1;

        // Spectral flux — weight bass bands (first 1/4 of spectrum) more heavily
        let flux: f32 = if self.prev_spectrum.len() == spectrum.len() {
            let bass_cutoff = spectrum.len() / 4;
            spectrum
                .iter()
                .zip(self.prev_spectrum.iter())
                .enumerate()
                .map(|(i, (&cur, &prev))| {
                    let diff = (cur - prev).max(0.0);
                    if i < bass_cutoff { diff * 2.0 } else { diff }
                })
                .sum()
        } else {
            0.0
        };

        // Adaptive threshold
        self.flux_avg = self.flux_avg * 0.93 + flux * 0.07;
        let threshold = self.flux_avg * 1.5 + 0.01;

        // Beat intensity: how far above threshold
        let beat_intensity = if flux > threshold {
            ((flux - threshold) / (threshold + 0.001)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Onset with cooldown
        let frames_since = self.frame_count - self.last_onset_frame;
        let onset = flux > threshold && frames_since > self.onset_cooldown;

        // BPM estimation from onset intervals
        if onset {
            let interval = frames_since;
            self.last_onset_frame = self.frame_count;

            if interval > 5 && interval < 300 {
                self.intervals.push(interval);
                if self.intervals.len() > 16 {
                    self.intervals.remove(0);
                }

                if self.intervals.len() >= 4 {
                    let avg_interval: f64 =
                        self.intervals.iter().map(|&i| i as f64).sum::<f64>()
                            / self.intervals.len() as f64;
                    if avg_interval > 0.0 {
                        self.bpm = (60.0 * f64::from(fps) / avg_interval) as f32;
                        self.bpm = self.bpm.clamp(30.0, 300.0);
                    }
                }
            }

            self.phase = 0.0;
        } else if self.bpm > 0.0 {
            let beats_per_frame = self.bpm / (60.0 * fps);
            self.phase = (self.phase + beats_per_frame) % 1.0;
        }

        self.prev_spectrum.clear();
        self.prev_spectrum.extend_from_slice(spectrum);

        (onset, beat_intensity, self.bpm, self.phase)
    }
}

impl Default for BeatDetector {
    fn default() -> Self {
        Self::new()
    }
}
