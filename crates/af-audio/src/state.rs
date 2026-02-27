use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use af_core::frame::AudioFeatures;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use triple_buffer::TripleBuffer;

/// Commandes interactives pour le thread audio SOTA.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioCommand {
    Play,
    Pause,
    Seek(f64),
    Quit,
}

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
pub fn spawn_audio_thread(target_fps: u32, audio_smoothing: f32) -> anyhow::Result<triple_buffer::Output<AudioFeatures>> {
    let mut capture = AudioCapture::start_default()?;
    let sample_rate = capture.sample_rate();

    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio".to_string())
        .spawn(move || {
            run_analysis_loop(&mut buf_input, target_fps, sample_rate, audio_smoothing, &mut |out| {
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
    path: &std::path::Path,
    target_fps: u32,
    audio_smoothing: f32,
    cmd_rx: flume::Receiver<AudioCommand>,
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

    let supported_config = output_device.default_output_config()?;
    let out_sample_format = supported_config.sample_format();
    let output_config = supported_config.config();

    let out_channels = output_config.channels as usize;
    let out_sample_rate = output_config.sample_rate.0;
    
    // Resampling ratio for local zero-alloc linear read
    let sample_rate_ratio = f64::from(sample_rate) / f64::from(out_sample_rate);

    let is_paused = Arc::new(AtomicBool::new(false));

    let output_stream = match out_sample_format {
        cpal::SampleFormat::F32 => build_playback_stream::<f32>(
            &output_device,
            &output_config,
            Arc::clone(&samples),
            Arc::clone(&playback_pos),
            Arc::clone(&is_paused),
            sample_rate_ratio,
            out_channels,
        )?,
        cpal::SampleFormat::I16 => build_playback_stream::<i16>(
            &output_device,
            &output_config,
            Arc::clone(&samples),
            Arc::clone(&playback_pos),
            Arc::clone(&is_paused),
            sample_rate_ratio,
            out_channels,
        )?,
        cpal::SampleFormat::U16 => build_playback_stream::<u16>(
            &output_device,
            &output_config,
            Arc::clone(&samples),
            Arc::clone(&playback_pos),
            Arc::clone(&is_paused),
            sample_rate_ratio,
            out_channels,
        )?,
        fmt => anyhow::bail!("Unsupported audio format: {fmt:?}"),
    };

    output_stream.play()?;
    log::info!("Audio playback started @ {out_sample_rate}Hz, {out_channels} channels");

    // --- Start analysis thread ---
    let analysis_samples = Arc::clone(&samples);
    let analysis_pos = Arc::clone(&playback_pos);
    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio-file".to_string())
        .spawn(move || {
            // Keep the output stream alive in this thread
            let _stream = output_stream;
            run_file_analysis_loop(
                &mut buf_input,
                target_fps,
                sample_rate,
                audio_smoothing,
                &analysis_samples,
                &analysis_pos,
                &is_paused,
                &cmd_rx,
            );
        })?;

    Ok(buf_output)
}

/// Core analysis loop for file playback mode.
#[allow(clippy::too_many_arguments)]
fn run_file_analysis_loop(
    buf_input: &mut triple_buffer::Input<AudioFeatures>,
    target_fps: u32,
    sample_rate: u32,
    audio_smoothing: f32,
    samples: &[f32],
    playback_pos: &AtomicUsize,
    is_paused: &AtomicBool,
    cmd_rx: &flume::Receiver<AudioCommand>,
) {
    let fft_size = 2048;
    let mut fft = FftPipeline::new(fft_size);
    let mut beat = BeatDetector::new();
    let mut smoother = FeatureSmoother::new(audio_smoothing);
    let mut window_buf: Vec<f32> = vec![0.0; fft_size];

    let frame_period = std::time::Duration::from_secs_f64(1.0 / f64::from(target_fps.max(1)));

    loop {
        // Command reception
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCommand::Play => is_paused.store(false, Ordering::Relaxed),
                AudioCommand::Pause => is_paused.store(true, Ordering::Relaxed),
                AudioCommand::Seek(delta) => {
                    let total = samples.len() as f64;
                    let current_sec =
                        playback_pos.load(Ordering::Relaxed) as f64 / f64::from(sample_rate);
                    let new_sec = (current_sec + delta).max(0.0);
                    let new_pos = (new_sec * f64::from(sample_rate)) as usize;
                    playback_pos.store(new_pos % total as usize, Ordering::Relaxed);
                }
                AudioCommand::Quit => return,
            }
        }

        if is_paused.load(Ordering::Relaxed) {
            buf_input.write(AudioFeatures::default());
            thread::sleep(frame_period);
            continue;
        }

        let current_pos = playback_pos.load(Ordering::Relaxed);
        let total = samples.len();

        for (i, slot) in window_buf.iter_mut().enumerate() {
            let idx = if current_pos >= fft_size {
                (current_pos - fft_size + i) % total
            } else {
                (total + current_pos - fft_size + i) % total
            };
            *slot = samples[idx];
        }

        let spectrum = fft.process(&window_buf);
        let mut feats = features::extract_features(&window_buf, spectrum, sample_rate);

        let fps = target_fps as f32;
        let (onset, intensity, bpm, phase) = beat.process(spectrum, fps);
        feats.onset = onset;
        feats.beat_intensity = intensity;
        feats.bpm = bpm;
        feats.beat_phase = phase;

        let smoothed = smoother.smooth(&feats);
        buf_input.write(smoothed);

        thread::sleep(frame_period);
    }
}

/// Core analysis loop for capture mode.
fn run_analysis_loop(
    buf_input: &mut triple_buffer::Input<AudioFeatures>,
    target_fps: u32,
    sample_rate: u32,
    audio_smoothing: f32,
    read_fn: &mut dyn FnMut(&mut Vec<f32>),
) {
    let fft_size = 2048;
    let mut fft = FftPipeline::new(fft_size);
    let mut beat = BeatDetector::new();
    let mut smoother = FeatureSmoother::new(audio_smoothing);
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
            let mut feats = features::extract_features(window, spectrum, sample_rate);

            let fps = target_fps as f32;
            let (onset, intensity, bpm, phase) = beat.process(spectrum, fps);
            feats.onset = onset;
            feats.beat_intensity = intensity;
            feats.bpm = bpm;
            feats.beat_phase = phase;

            let smoothed = smoother.smooth(&feats);
            buf_input.write(smoothed);

            sample_buf.clear();
        }

        thread::sleep(frame_period);
    }
}

/// Generic stream builder to support U16, I16, F32 dynamic dispatch for CPAL.
fn build_playback_stream<T>(
    output_device: &cpal::Device,
    output_config: &cpal::StreamConfig,
    playback_samples: Arc<Vec<f32>>,
    playback_pos_write: Arc<AtomicUsize>,
    is_paused_play: Arc<AtomicBool>,
    sample_rate_ratio: f64,
    out_channels: usize,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let mut local_pos_f = playback_pos_write.load(Ordering::Relaxed) as f64;
    let mut last_sync_pos = local_pos_f as usize;

    let stream = output_device.build_output_stream(
        output_config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            if is_paused_play.load(Ordering::Relaxed) {
                for sample in data.iter_mut() {
                    *sample = T::from_sample(0.0f32);
                }
                return;
            }

            let total = playback_samples.len();
            if total == 0 {
                return;
            }

            let current_shared = playback_pos_write.load(Ordering::Relaxed);
            
            // Resync if seek externally modified playback_pos (threshold = 4 frames of drift)
            if current_shared.abs_diff(last_sync_pos) > out_channels * 4 {
                local_pos_f = current_shared as f64;
            }

            for frame in data.chunks_mut(out_channels) {
                let pos_usize = (local_pos_f as usize) % total;
                let sample = playback_samples[pos_usize];
                
                let out_sample = T::from_sample(sample);
                for out_channel in frame.iter_mut() {
                    *out_channel = out_sample;
                }
                
                local_pos_f += sample_rate_ratio;
            }
            
            if local_pos_f >= total as f64 {
                local_pos_f -= total as f64;
            }
            last_sync_pos = local_pos_f as usize;
            playback_pos_write.store(last_sync_pos, Ordering::Relaxed);
        },
        |err| {
            log::error!("Audio output error: {err}");
        },
        None,
    )?;

    Ok(stream)
}
