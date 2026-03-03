#![allow(clippy::field_reassign_with_default, clippy::needless_borrow)]
use criterion::{Criterion, black_box, criterion_group, criterion_main};

use af_audio::fft::FftPipeline;
use af_audio::features::extract_features;
use af_audio::smoothing::FeatureSmoother;

fn sine_wave(freq_hz: f32, amplitude: f32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|i| {
            amplitude
                * (2.0 * std::f32::consts::PI * freq_hz * i as f32 / 44100.0).sin()
        })
        .collect()
}

fn bench_audio(c: &mut Criterion) {
    let samples = sine_wave(100.0, 0.5, 2048);

    let mut group = c.benchmark_group("audio");

    // FFT
    {
        let mut fft = FftPipeline::new(2048);
        group.bench_function("fft_2048", |b| {
            b.iter(|| {
                fft.process(black_box(&samples));
            });
        });
    }

    // Feature extraction
    {
        let mut fft = FftPipeline::new(2048);
        let spectrum = fft.process(&samples);
        group.bench_function("extract_features", |b| {
            b.iter(|| {
                extract_features(black_box(&samples), black_box(&spectrum), 44100);
            });
        });
    }

    // Smoother
    {
        let mut smoother = FeatureSmoother::new(0.3);
        let features = af_core::frame::AudioFeatures::default();
        group.bench_function("smoother", |b| {
            b.iter(|| {
                smoother.smooth(black_box(&features));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_audio);
criterion_main!(benches);
