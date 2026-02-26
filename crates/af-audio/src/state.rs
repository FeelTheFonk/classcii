use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use af_core::frame::AudioFeatures;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use triple_buffer::TripleBuffer;

use crate::beat::BeatDetector;
use crate::capture::AudioCapture;
use crate::decode;
use crate::features;
use crate::fft::FftPipeline;
use crate::smoothing::FeatureSmoother;

/// Spawn the audio analysis thread from microphone capture.
///
/// # Errors
/// Returns an error if audio capture fails to initialize.
pub fn spawn_audio_thread(target_fps: u32) -> anyhow::Result<triple_buffer::Output<AudioFeatures>> {
    let mut capture = AudioCapture::start_default()?;
    let sample_rate = capture.sample_rate();

    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio".to_string())
        .spawn(move || {
            run_analysis_loop(&mut buf_input, target_fps, sample_rate, &mut |out| {
                capture.read_samples(out);
            });
        })?;

    Ok(buf_output)
}

/// Spawn both audio playback and analysis from a decoded audio file.
///
/// The file is decoded once, played back through the default output device,
/// and analyzed in parallel. The playback loops continuously.
///
/// # Errors
/// Returns an error if decoding or audio output initialization fails.
pub fn spawn_audio_file_thread(
    path: &Path,
    target_fps: u32,
) -> anyhow::Result<triple_buffer::Output<AudioFeatures>> {
    let (all_samples, sample_rate) = decode::decode_file(path)?;

    if all_samples.is_empty() {
        anyhow::bail!("Audio file is empty: {}", path.display());
    }

    // Shared state for playback
    let samples = Arc::new(all_samples);
    let playback_pos = Arc::new(AtomicUsize::new(0));

    // --- Start cpal output stream for playback ---
    let host = cpal::default_host();
    let output_device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No audio output device found"))?;

    let output_config = cpal::StreamConfig {
        channels: 2, // stereo output
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let playback_samples = Arc::clone(&samples);
    let playback_pos_write = Arc::clone(&playback_pos);

    let output_stream = output_device.build_output_stream(
        &output_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let total = playback_samples.len();
            let mut pos = playback_pos_write.load(Ordering::Relaxed);

            // Fill output buffer: mono → stereo duplication
            for frame in data.chunks_mut(2) {
                let sample = playback_samples[pos % total];
                frame[0] = sample;
                if frame.len() > 1 {
                    frame[1] = sample;
                }
                pos += 1;
                if pos >= total {
                    pos = 0;
                }
            }
            playback_pos_write.store(pos, Ordering::Relaxed);
        },
        |err| {
            log::error!("Audio output error: {err}");
        },
        None,
    )?;

    output_stream.play()?;
    log::info!("Audio playback started @ {sample_rate}Hz");

    // --- Start analysis thread ---
    let analysis_samples = Arc::clone(&samples);
    let analysis_pos = Arc::clone(&playback_pos);
    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio-file".to_string())
        .spawn(move || {
            // Keep the output stream alive in this thread
            let _stream = output_stream;

            let fft_size = 2048;
            let mut fft = FftPipeline::new(fft_size);
            let mut beat = BeatDetector::new();
            let mut smoother = FeatureSmoother::new(0.3);
            let mut window_buf: Vec<f32> = vec![0.0; fft_size];

            let frame_period =
                std::time::Duration::from_secs_f64(1.0 / f64::from(target_fps.max(1)));

            loop {
                // Read the current playback position and analyze a window around it
                let current_pos = analysis_pos.load(Ordering::Relaxed);
                let total = analysis_samples.len();

                // Fill analysis window from the current playback region
                for (i, slot) in window_buf.iter_mut().enumerate() {
                    let idx = if current_pos >= fft_size {
                        (current_pos - fft_size + i) % total
                    } else {
                        (total + current_pos - fft_size + i) % total
                    };
                    *slot = analysis_samples[idx];
                }

                let spectrum = fft.process(&window_buf);
                let mut feats =
                    features::extract_features(&window_buf, &spectrum, sample_rate);

                let fps = target_fps as f32;
                let (onset, intensity, bpm, phase) = beat.process(&spectrum, fps);
                feats.onset = onset;
                feats.beat_intensity = intensity;
                feats.bpm = bpm;
                feats.beat_phase = phase;
                feats.spectral_flux = spectrum.iter().sum::<f32>().min(1.0);

                let smoothed = smoother.smooth(&feats);
                buf_input.write(smoothed);

                thread::sleep(frame_period);
            }
        })?;

    Ok(buf_output)
}

/// Core analysis loop for capture mode.
fn run_analysis_loop(
    buf_input: &mut triple_buffer::Input<AudioFeatures>,
    target_fps: u32,
    sample_rate: u32,
    read_fn: &mut dyn FnMut(&mut Vec<f32>),
) {
    let fft_size = 2048;
    let mut fft = FftPipeline::new(fft_size);
    let mut beat = BeatDetector::new();
    let mut smoother = FeatureSmoother::new(0.3);
    let mut sample_buf: Vec<f32> = Vec::with_capacity(fft_size * 2);

    let frame_period = std::time::Duration::from_secs_f64(1.0 / f64::from(target_fps.max(1)));

    loop {
        read_fn(&mut sample_buf);

        if sample_buf.len() >= fft_size {
            let window = if sample_buf.len() > fft_size {
                &sample_buf[sample_buf.len() - fft_size..]
            } else {
                &sample_buf
            };

            let spectrum = fft.process(window);
            let mut feats = features::extract_features(window, &spectrum, sample_rate);

            let fps = target_fps as f32;
            let (onset, intensity, bpm, phase) = beat.process(&spectrum, fps);
            feats.onset = onset;
            feats.beat_intensity = intensity;
            feats.bpm = bpm;
            feats.beat_phase = phase;
            feats.spectral_flux = spectrum.iter().sum::<f32>().min(1.0);

            let smoothed = smoother.smooth(&feats);
            buf_input.write(smoothed);

            sample_buf.clear();
        }

        thread::sleep(frame_period);
    }
}
