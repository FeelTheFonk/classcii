use af_core::frame::AudioFeatures;

/// Extract audio features from a spectrum and raw samples.
///
/// # Example
/// ```
/// use af_audio::features::extract_features;
/// use af_core::frame::AudioFeatures;
///
/// let samples = vec![0.0f32; 1024];
/// let spectrum = vec![0.0f32; 513];
/// let features = extract_features(&samples, &spectrum, 44100);
/// assert!(features.rms.abs() < f32::EPSILON);
/// ```
pub fn extract_features(samples: &[f32], spectrum: &[f32], sample_rate: u32) -> AudioFeatures {
    let mut features = AudioFeatures::default();

    // RMS
    if !samples.is_empty() {
        let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
        features.rms = (sum_sq / samples.len() as f32).sqrt().min(1.0);
    }

    // Peak
    features.peak = samples
        .iter()
        .map(|s| s.abs())
        .fold(0.0f32, f32::max)
        .min(1.0);

    // Frequency band energies
    if spectrum.len() > 1 {
        let bin_hz = sample_rate as f32 / ((spectrum.len() - 1) * 2) as f32;

        features.sub_bass = band_energy(spectrum, 20.0, 60.0, bin_hz);
        features.bass = band_energy(spectrum, 60.0, 250.0, bin_hz);
        features.low_mid = band_energy(spectrum, 250.0, 500.0, bin_hz);
        features.mid = band_energy(spectrum, 500.0, 2000.0, bin_hz);
        features.high_mid = band_energy(spectrum, 2000.0, 4000.0, bin_hz);
        features.presence = band_energy(spectrum, 4000.0, 6000.0, bin_hz);
        features.brilliance = band_energy(spectrum, 6000.0, 20000.0, bin_hz);

        // Spectral centroid
        let total_energy: f32 = spectrum.iter().sum();
        if total_energy > 1e-10 {
            let weighted: f32 = spectrum
                .iter()
                .enumerate()
                .map(|(i, &mag)| i as f32 * bin_hz * mag)
                .sum();
            features.spectral_centroid = (weighted / total_energy / 20000.0).clamp(0.0, 1.0);
        }

        // Spectral flatness (geometric mean / arithmetic mean)
        if total_energy > 1e-10 {
            let n = spectrum.len() as f32;
            let log_sum: f32 = spectrum.iter().map(|&m| (m + 1e-10).ln()).sum();
            let geo_mean = (log_sum / n).exp();
            let arith_mean = total_energy / n;
            features.spectral_flatness = (geo_mean / arith_mean).clamp(0.0, 1.0);
        }

        // Spectrum bands (32 log-frequency bands)
        fill_spectrum_bands(spectrum, bin_hz, &mut features.spectrum_bands);
    }

    // MFCC fields are computed externally by MelFilterbank (stateful)
    // and injected after extract_features returns.

    features
}

/// Compute energy in a frequency band.
fn band_energy(spectrum: &[f32], low_hz: f32, high_hz: f32, bin_hz: f32) -> f32 {
    let lo = (low_hz / bin_hz) as usize;
    let hi = ((high_hz / bin_hz) as usize).min(spectrum.len());
    if lo >= hi {
        return 0.0;
    }
    let sum: f32 = spectrum[lo..hi].iter().sum();
    let count = (hi - lo) as f32;
    (sum / count).min(1.0)
}

/// Fill 32 log-spaced frequency bands for visualization.
fn fill_spectrum_bands(spectrum: &[f32], bin_hz: f32, bands: &mut [f32; 32]) {
    let min_freq = 20.0f32;
    let max_freq = 20000.0f32;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    for (i, band) in bands.iter_mut().enumerate() {
        let f_lo = ((log_min + (log_max - log_min) * i as f32 / 32.0).exp() / bin_hz) as usize;
        let f_hi =
            ((log_min + (log_max - log_min) * (i as f32 + 1.0) / 32.0).exp() / bin_hz) as usize;

        let lo = f_lo.min(spectrum.len());
        let hi = f_hi.min(spectrum.len()).max(lo + 1);

        if lo < spectrum.len() && hi <= spectrum.len() {
            let sum: f32 = spectrum[lo..hi].iter().sum();
            *band = (sum / (hi - lo) as f32).min(1.0);
        } else {
            *band = 0.0;
        }
    }
}
