//! Integration test: audio feature extraction pipeline.
//! Verifies: samples → FFT → features → smoother produces sensible values.
#![allow(
    clippy::expect_used,
    clippy::float_cmp,
    clippy::needless_borrow,
    clippy::field_reassign_with_default
)]

use af_audio::features::extract_features;
use af_audio::fft::FftPipeline;
use af_audio::smoothing::FeatureSmoother;

/// Generate a sine wave at `freq_hz` with given `amplitude` and `sample_rate`.
fn sine_wave(freq_hz: f32, amplitude: f32, sample_rate: u32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| {
            amplitude * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / sample_rate as f32).sin()
        })
        .collect()
}

#[test]
fn sine_100hz_produces_bass_energy() {
    let sample_rate = 44100;
    let samples = sine_wave(100.0, 0.5, sample_rate, 2048);

    let mut fft = FftPipeline::new(2048);
    let spectrum = fft.process(&samples);

    let features = extract_features(&samples, &spectrum, sample_rate);

    // 100 Hz is in bass band (60–250 Hz) — must produce energy
    assert!(
        features.bass > 0.1,
        "100 Hz sine should produce bass > 0.1, got {}",
        features.bass
    );

    // RMS for amplitude 0.5 sine ≈ 0.354
    assert!(
        features.rms > 0.2 && features.rms < 0.6,
        "RMS should be ~0.35 for amplitude 0.5, got {}",
        features.rms
    );

    // Spectral centroid should be in lower range (100 Hz is low)
    assert!(
        features.spectral_centroid < 0.3,
        "centroid for 100 Hz sine should be low, got {}",
        features.spectral_centroid
    );
}

#[test]
fn silence_produces_near_zero_features() {
    let samples = vec![0.0f32; 2048];

    let mut fft = FftPipeline::new(2048);
    let spectrum = fft.process(&samples);
    let features = extract_features(&samples, &spectrum, 44100);

    assert!(
        features.rms < 0.001,
        "silence RMS should be ~0, got {}",
        features.rms
    );
    assert!(
        features.bass < 0.01,
        "silence bass should be ~0, got {}",
        features.bass
    );
    assert!(
        features.peak < 0.001,
        "silence peak should be ~0, got {}",
        features.peak
    );
}

#[test]
fn smoother_preserves_beat_intensity() {
    let mut smoother = FeatureSmoother::new(0.3);
    let mut features = af_core::frame::AudioFeatures::default();

    // Initialize
    smoother.smooth(&features);

    // Beat spike: must pass through at full amplitude
    features.beat_intensity = 1.0;
    features.onset_envelope = 0.9;
    let smoothed = smoother.smooth(&features);

    assert_eq!(
        smoothed.beat_intensity, 1.0,
        "beat_intensity must bypass smoother"
    );
    assert_eq!(
        smoothed.onset_envelope, 0.9,
        "onset_envelope must bypass smoother"
    );
}

#[test]
fn high_frequency_sine_produces_brilliance() {
    let sample_rate = 44100;
    let samples = sine_wave(10000.0, 0.5, sample_rate, 2048);

    let mut fft = FftPipeline::new(2048);
    let spectrum = fft.process(&samples);
    let features = extract_features(&samples, &spectrum, sample_rate);

    // 10 kHz is in brilliance band (6000–20000 Hz)
    assert!(
        features.brilliance > 0.05,
        "10 kHz sine should produce brilliance > 0.05, got {}",
        features.brilliance
    );
    // Bass should be near zero
    assert!(
        features.bass < 0.05,
        "10 kHz sine should produce near-zero bass, got {}",
        features.bass
    );
}
