use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use af_core::clock::MediaClock;
use af_core::frame::AudioFeatures;
use anyhow::Result;
use triple_buffer::TripleBuffer;

use af_audio::beat::BeatDetector;
use af_audio::fft::FftPipeline;
use af_audio::mfcc::MelFilterbank;
use af_audio::smoothing::FeatureSmoother;

use crate::stem::{STEM_COUNT, StemFeatures, StemSet};

/// MFCC coefficient 1 normalization divisor (same as af-audio/state.rs).
const MFCC_BRIGHTNESS_SCALE: f32 = 50.0;
/// MFCC coefficient 2 normalization divisor.
const MFCC_ROUGHNESS_SCALE: f32 = 30.0;

/// Spawn a thread that performs per-stem FFT analysis synchronized to playback.
///
/// Reads the current playback position from `MediaClock` and extracts features
/// for each of the 4 stems independently. Publishes `StemFeatures` via triple buffer.
///
/// # Errors
/// Returns an error if the analysis thread fails to spawn.
pub fn spawn_stem_analysis_thread(
    stem_set: &StemSet,
    clock: Arc<MediaClock>,
    is_paused: Arc<AtomicBool>,
    is_stopped: Arc<AtomicBool>,
    target_fps: u32,
    smoothing: f32,
    input_gain: f32,
) -> Result<(triple_buffer::Output<StemFeatures>, thread::JoinHandle<()>)> {
    let (mut buf_input, buf_output) = TripleBuffer::new(&StemFeatures::default()).split();

    // Clone stem sample arcs for the analysis thread
    let stem_samples: [Arc<Vec<f32>>; STEM_COUNT] =
        std::array::from_fn(|i| Arc::clone(&stem_set.stems[i].samples));
    let sample_rate = stem_set.sample_rate;

    let handle = thread::Builder::new()
        .name("af-stems-analysis".into())
        .spawn(move || {
            run_analysis_loop(
                &mut buf_input,
                &stem_samples,
                sample_rate,
                &clock,
                &is_paused,
                &is_stopped,
                target_fps,
                smoothing,
                input_gain,
            );
        })?;

    Ok((buf_output, handle))
}

/// Core analysis loop: per-stem FFT + feature extraction at target FPS.
#[allow(clippy::too_many_arguments)]
fn run_analysis_loop(
    buf_input: &mut triple_buffer::Input<StemFeatures>,
    stem_samples: &[Arc<Vec<f32>>; STEM_COUNT],
    sample_rate: u32,
    clock: &MediaClock,
    is_paused: &AtomicBool,
    is_stopped: &AtomicBool,
    target_fps: u32,
    smoothing: f32,
    input_gain: f32,
) {
    let fft_size = 2048;
    let frame_period = Duration::from_secs_f64(1.0 / f64::from(target_fps.max(1)));

    // Per-stem analysis state (4 independent pipelines)
    let mut ffts: [FftPipeline; STEM_COUNT] = std::array::from_fn(|_| FftPipeline::new(fft_size));
    let mut beats: [BeatDetector; STEM_COUNT] = std::array::from_fn(|_| BeatDetector::new());
    let mut smoothers: [FeatureSmoother; STEM_COUNT] =
        std::array::from_fn(|_| FeatureSmoother::new(smoothing));
    let mut filterbanks: [MelFilterbank; STEM_COUNT] =
        std::array::from_fn(|_| MelFilterbank::new(fft_size, sample_rate));
    let mut window_bufs: Vec<Vec<f32>> = (0..STEM_COUNT).map(|_| vec![0.0f32; fft_size]).collect();
    let mut onset_envs: [f32; STEM_COUNT] = [0.0; STEM_COUNT];

    let fps = target_fps as f32;

    loop {
        if is_stopped.load(Ordering::Relaxed) {
            break;
        }

        if is_paused.load(Ordering::Relaxed) {
            buf_input.write(StemFeatures::default());
            thread::sleep(frame_period);
            continue;
        }

        let current_pos = clock.sample_pos();
        let mut stem_features = StemFeatures::default();

        for stem_idx in 0..STEM_COUNT {
            let samples = &stem_samples[stem_idx];
            let total = samples.len();
            if total == 0 {
                continue;
            }

            // Fill window buffer centered on current playback position
            // rem_euclid handles all cases without usize underflow
            // (safe even when total < fft_size; values are audio buffer indices, never near isize::MAX)
            #[allow(clippy::cast_possible_wrap)]
            for (i, slot) in window_bufs[stem_idx].iter_mut().enumerate() {
                let idx = (current_pos as isize - fft_size as isize + i as isize)
                    .rem_euclid(total as isize) as usize;
                *slot = samples[idx];
            }

            // Apply input gain
            if (input_gain - 1.0).abs() > f32::EPSILON {
                for s in &mut window_bufs[stem_idx] {
                    *s *= input_gain;
                }
            }

            // FFT
            let spectrum = ffts[stem_idx].process(&window_bufs[stem_idx]);

            // Extract features
            let mut feats =
                af_audio::features::extract_features(&window_bufs[stem_idx], spectrum, sample_rate);

            // Beat detection
            let (onset, intensity, bpm, phase, flux) = beats[stem_idx].process(spectrum, fps);
            feats.onset = onset;
            feats.beat_intensity = intensity;
            feats.bpm = bpm;
            feats.beat_phase = phase;
            feats.spectral_flux = flux;

            // onset_envelope: strobe-style decay (parity with batch_analyzer)
            if onset {
                onset_envs[stem_idx] = 1.0;
            } else {
                onset_envs[stem_idx] *= 0.85;
            }
            feats.onset_envelope = onset_envs[stem_idx];

            // MFCC timbral features
            let mfcc = filterbanks[stem_idx].compute(spectrum);
            feats.mfcc = mfcc;
            feats.timbral_brightness = (mfcc[1] / MFCC_BRIGHTNESS_SCALE + 0.5).clamp(0.0, 1.0);
            feats.timbral_roughness = (mfcc[2].abs() / MFCC_ROUGHNESS_SCALE).clamp(0.0, 1.0);

            // Smooth
            stem_features.features[stem_idx] = smoothers[stem_idx].smooth(&feats);
        }

        buf_input.write(stem_features);
        thread::sleep(frame_period);
    }
}

/// Compute a combined `AudioFeatures` from stem features, for use with existing mappings.
///
/// Takes the active (non-muted, or solo-selected) stems and averages their features.
/// This allows the standard audio-reactive pipeline to work with stem-separated audio.
#[must_use]
pub fn combine_stem_features(
    stem_features: &StemFeatures,
    gains: &[f32; STEM_COUNT],
) -> AudioFeatures {
    let mut combined = AudioFeatures::default();
    let mut weight_sum = 0.0f32;

    for (i, gain) in gains.iter().enumerate() {
        if *gain <= 0.0 {
            continue;
        }
        let f = &stem_features.features[i];
        let w = *gain;

        combined.rms += f.rms * w;
        combined.peak = combined.peak.max(f.peak * w);
        combined.sub_bass += f.sub_bass * w;
        combined.bass += f.bass * w;
        combined.low_mid += f.low_mid * w;
        combined.mid += f.mid * w;
        combined.high_mid += f.high_mid * w;
        combined.presence += f.presence * w;
        combined.brilliance += f.brilliance * w;
        combined.spectral_centroid += f.spectral_centroid * w;
        combined.spectral_flux += f.spectral_flux * w;
        combined.spectral_flatness += f.spectral_flatness * w;
        combined.beat_intensity = combined.beat_intensity.max(f.beat_intensity);
        combined.bpm = combined.bpm.max(f.bpm);
        combined.timbral_brightness += f.timbral_brightness * w;
        combined.timbral_roughness += f.timbral_roughness * w;
        combined.spectral_rolloff += f.spectral_rolloff * w;
        combined.zero_crossing_rate += f.zero_crossing_rate * w;

        // Onset: OR across stems
        if f.onset {
            combined.onset = true;
        }

        // Spectrum bands: weighted sum
        for (j, band) in f.spectrum_bands.iter().enumerate() {
            combined.spectrum_bands[j] += band * w;
        }

        weight_sum += w;
    }

    // Normalize by total weight
    if weight_sum > 0.0 {
        let inv = 1.0 / weight_sum;
        combined.rms *= inv;
        combined.sub_bass *= inv;
        combined.bass *= inv;
        combined.low_mid *= inv;
        combined.mid *= inv;
        combined.high_mid *= inv;
        combined.presence *= inv;
        combined.brilliance *= inv;
        combined.spectral_centroid *= inv;
        combined.spectral_flux *= inv;
        combined.spectral_flatness *= inv;
        combined.timbral_brightness *= inv;
        combined.timbral_roughness *= inv;
        combined.spectral_rolloff *= inv;
        combined.zero_crossing_rate *= inv;
        for band in &mut combined.spectrum_bands {
            *band *= inv;
        }
    }

    combined
}
