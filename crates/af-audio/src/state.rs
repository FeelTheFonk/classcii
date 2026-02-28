use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;

use af_core::clock::MediaClock;
use af_core::frame::AudioFeatures;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use triple_buffer::TripleBuffer;

/// Commandes interactives pour le thread audio .
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
pub fn spawn_audio_thread(
    target_fps: u32,
    audio_smoothing: f32,
) -> anyhow::Result<triple_buffer::Output<AudioFeatures>> {
    let mut capture = AudioCapture::start_default()?;
    let sample_rate = capture.sample_rate();

    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio".to_string())
        .spawn(move || {
            run_analysis_loop(
                &mut buf_input,
                target_fps,
                sample_rate,
                audio_smoothing,
                &mut |out| {
                    capture.read_samples(out);
                },
            );
        })?;

    Ok(buf_output)
}

/// Spawn both audio playback and analysis from a decoded audio file.
///
/// Le décodage se fait à l'intérieur du thread spawné (non-bloquant pour
/// le thread principal). Le terminal démarre immédiatement ; l'audio démarre
/// une fois le décodage terminé (arrière-plan). Supporté : WAV, MP3, FLAC,
/// OGG, AAC-LC via symphonia ; HE-AAC et autres formats exotiques via ffmpeg.
///
/// # Errors
/// Retourne une erreur uniquement si le spawn du thread OS échoue (très rare).
/// Les erreurs de décodage ou de config audio sont loguées dans le thread.
pub fn spawn_audio_file_thread(
    path: &std::path::Path,
    target_fps: u32,
    audio_smoothing: f32,
    cmd_rx: flume::Receiver<AudioCommand>,
    clock: Arc<MediaClock>,
) -> anyhow::Result<triple_buffer::Output<AudioFeatures>> {
    // Clone du chemin pour le thread (le décodage se fait en arrière-plan).
    let path = path.to_path_buf();
    let (mut buf_input, buf_output) = TripleBuffer::new(&AudioFeatures::default()).split();

    thread::Builder::new()
        .name("af-audio-file".to_string())
        .spawn(move || {
            // --- Décodage INSIDE le thread → thread principal non-bloquant ---
            let (all_samples, sample_rate) = match decode::decode_file(&path) {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("Audio: échec décodage {} : {e}", path.display());
                    return;
                }
            };
            if all_samples.is_empty() {
                log::error!("Audio: fichier vide: {}", path.display());
                return;
            }

            // Écrire le sample rate réel dans le clock partagé
            clock.set_sample_rate(sample_rate);

            let samples = Arc::new(all_samples);
            let playback_pos = Arc::new(AtomicUsize::new(0));

            // --- Config cpal output device ---
            let host = cpal::default_host();
            let Some(output_device) = host.default_output_device() else {
                log::error!("Audio: aucun périphérique de sortie audio trouvé.");
                return;
            };
            let supported_config = match output_device.default_output_config() {
                Ok(c) => c,
                Err(e) => {
                    log::error!("Audio: échec config cpal: {e}");
                    return;
                }
            };
            let out_sample_format = supported_config.sample_format();
            let output_config = supported_config.config();
            let out_channels = output_config.channels as usize;
            let out_sample_rate = output_config.sample_rate.0;
            let sample_rate_ratio = f64::from(sample_rate) / f64::from(out_sample_rate);
            let is_paused = Arc::new(AtomicBool::new(false));

            // --- Build playback stream (dispatch sur le format cpal) ---
            let stream_result = match out_sample_format {
                cpal::SampleFormat::F32 => build_playback_stream::<f32>(
                    &output_device,
                    &output_config,
                    Arc::clone(&samples),
                    Arc::clone(&playback_pos),
                    Arc::clone(&is_paused),
                    sample_rate_ratio,
                    out_channels,
                    Arc::clone(&clock),
                ),
                cpal::SampleFormat::I16 => build_playback_stream::<i16>(
                    &output_device,
                    &output_config,
                    Arc::clone(&samples),
                    Arc::clone(&playback_pos),
                    Arc::clone(&is_paused),
                    sample_rate_ratio,
                    out_channels,
                    Arc::clone(&clock),
                ),
                cpal::SampleFormat::U16 => build_playback_stream::<u16>(
                    &output_device,
                    &output_config,
                    Arc::clone(&samples),
                    Arc::clone(&playback_pos),
                    Arc::clone(&is_paused),
                    sample_rate_ratio,
                    out_channels,
                    Arc::clone(&clock),
                ),
                fmt => {
                    log::error!("Audio: format cpal non supporté: {fmt:?}");
                    return;
                }
            };
            let output_stream = match stream_result {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Audio: impossible de créer le stream de lecture: {e}");
                    return;
                }
            };
            if let Err(e) = output_stream.play() {
                log::error!("Audio: play() échoué: {e}");
                return;
            }
            log::info!("Audio playback started @ {out_sample_rate}Hz, {out_channels} channels");

            // --- Boucle d'analyse (garde le stream vivant via _stream) ---
            let _stream = output_stream;
            run_file_analysis_loop(
                &mut buf_input,
                target_fps,
                sample_rate,
                audio_smoothing,
                &samples,
                &playback_pos,
                &is_paused,
                &cmd_rx,
                &clock,
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
    clock: &MediaClock,
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
                AudioCommand::Play => {
                    is_paused.store(false, Ordering::Relaxed);
                    clock.set_paused(false);
                }
                AudioCommand::Pause => {
                    is_paused.store(true, Ordering::Relaxed);
                    clock.set_paused(true);
                }
                AudioCommand::Seek(delta) => {
                    let total = samples.len() as f64;
                    let current_sec =
                        playback_pos.load(Ordering::Relaxed) as f64 / f64::from(sample_rate);
                    let new_sec = (current_sec + delta).max(0.0);
                    let new_pos = (new_sec * f64::from(sample_rate)) as usize;
                    let final_pos = new_pos % total as usize;
                    playback_pos.store(final_pos, Ordering::Relaxed);
                    clock.set_sample_pos(final_pos);
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
#[allow(clippy::too_many_arguments)]
fn build_playback_stream<T>(
    output_device: &cpal::Device,
    output_config: &cpal::StreamConfig,
    playback_samples: Arc<Vec<f32>>,
    playback_pos_write: Arc<AtomicUsize>,
    is_paused_play: Arc<AtomicBool>,
    sample_rate_ratio: f64,
    out_channels: usize,
    clock: Arc<MediaClock>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let mut local_pos_f = playback_pos_write.load(Ordering::Relaxed) as f64;
    let mut last_sync_pos = local_pos_f as usize;
    let mut first_callback = true;

    let stream = output_device.build_output_stream(
        output_config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            // Marquer le clock comme démarré au premier callback
            if first_callback {
                clock.mark_started();
                first_callback = false;
            }

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
            // Synchroniser le clock partagé avec la position cpal
            clock.set_sample_pos(last_sync_pos);
        },
        |err| {
            log::error!("Audio output error: {err}");
        },
        None,
    )?;

    Ok(stream)
}
