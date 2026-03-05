use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use af_core::clock::MediaClock;
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::stem::{STEM_COUNT, StemId, StemSet, StemState};

/// Commands for the stem playback system.
#[derive(Debug, Clone)]
pub enum StemCommand {
    Play,
    Pause,
    Seek(f64),
    SetMuted(StemId, bool),
    SetSolo(StemId, bool),
    SetVolume(StemId, f32),
    ClearSolo,
    Quit,
}

/// Shared stem state that the cpal callback reads atomically.
///
/// We pack mute/solo/volume into atomics to avoid locks in the audio callback.
pub struct StemPlaybackState {
    /// Per-stem mute flags (atomic bool array).
    muted: [AtomicBool; STEM_COUNT],
    /// Per-stem solo flags.
    solo: [AtomicBool; STEM_COUNT],
    /// Per-stem volume × 1000 (stored as u32 atomic for lock-free access).
    volume_millibels: [AtomicUsize; STEM_COUNT],
}

impl StemPlaybackState {
    fn new(states: &[StemState; STEM_COUNT]) -> Self {
        Self {
            muted: std::array::from_fn(|i| AtomicBool::new(states[i].muted)),
            solo: std::array::from_fn(|i| AtomicBool::new(states[i].solo)),
            volume_millibels: std::array::from_fn(|i| {
                AtomicUsize::new((states[i].volume * 1000.0) as usize)
            }),
        }
    }

    fn set_muted(&self, id: StemId, muted: bool) {
        self.muted[id.index()].store(muted, Ordering::Relaxed);
    }

    fn set_solo(&self, id: StemId, solo: bool) {
        self.solo[id.index()].store(solo, Ordering::Relaxed);
    }

    fn set_volume(&self, id: StemId, volume: f32) {
        self.volume_millibels[id.index()].store((volume * 1000.0) as usize, Ordering::Relaxed);
    }

    fn clear_solo(&self) {
        for s in &self.solo {
            s.store(false, Ordering::Relaxed);
        }
    }

    /// Compute the effective gain for a stem given current mute/solo state.
    #[inline]
    fn effective_gain(&self, idx: usize) -> f32 {
        if self.muted[idx].load(Ordering::Relaxed) {
            return 0.0;
        }

        let any_solo = self.solo.iter().any(|s| s.load(Ordering::Relaxed));
        if any_solo && !self.solo[idx].load(Ordering::Relaxed) {
            return 0.0;
        }

        self.volume_millibels[idx].load(Ordering::Relaxed) as f32 / 1000.0
    }
}

/// Spawn the stem playback thread with per-stem mixing.
///
/// Returns the cpal output stream (must be kept alive) and a command sender.
///
/// # Errors
/// Returns an error if no audio output device is found or stream creation fails.
#[allow(clippy::too_many_lines, clippy::needless_pass_by_value)]
pub fn spawn_stem_playback(
    stem_set: &StemSet,
    initial_states: &[StemState; STEM_COUNT],
    cmd_rx: flume::Receiver<StemCommand>,
    clock: Arc<MediaClock>,
) -> Result<cpal::Stream> {
    let host = cpal::default_host();
    let output_device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No audio output device found"))?;

    let supported_config = output_device.default_output_config()?;
    let output_config = supported_config.config();
    let out_channels = output_config.channels as usize;
    let out_sample_rate = output_config.sample_rate.0;
    let sample_rate_ratio = f64::from(stem_set.sample_rate) / f64::from(out_sample_rate);

    // Collect stem sample arcs
    let stem_samples: [Arc<Vec<f32>>; STEM_COUNT] =
        std::array::from_fn(|i| Arc::clone(&stem_set.stems[i].samples));

    let playback_state = Arc::new(StemPlaybackState::new(initial_states));
    let playback_pos = Arc::new(AtomicUsize::new(0));
    let is_paused = Arc::new(AtomicBool::new(false));

    // Clone handles for the command processing thread
    let ps_cmd = Arc::clone(&playback_state);
    let pos_cmd = Arc::clone(&playback_pos);
    let paused_cmd = Arc::clone(&is_paused);
    let clock_cmd = Arc::clone(&clock);
    let sr = stem_set.sample_rate;
    let total_samples = stem_samples[0].len();

    // Command processing thread
    std::thread::Builder::new()
        .name("af-stems-cmd".into())
        .spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    StemCommand::Play => {
                        paused_cmd.store(false, Ordering::Relaxed);
                        clock_cmd.set_paused(false);
                    }
                    StemCommand::Pause => {
                        paused_cmd.store(true, Ordering::Relaxed);
                        clock_cmd.set_paused(true);
                    }
                    StemCommand::Seek(delta) => {
                        let current_sec = pos_cmd.load(Ordering::Relaxed) as f64 / f64::from(sr);
                        let new_sec = (current_sec + delta).max(0.0);
                        let new_pos = (new_sec * f64::from(sr)) as usize % total_samples;
                        pos_cmd.store(new_pos, Ordering::Relaxed);
                        clock_cmd.set_sample_pos(new_pos);
                    }
                    StemCommand::SetMuted(id, muted) => ps_cmd.set_muted(id, muted),
                    StemCommand::SetSolo(id, solo) => ps_cmd.set_solo(id, solo),
                    StemCommand::SetVolume(id, vol) => ps_cmd.set_volume(id, vol),
                    StemCommand::ClearSolo => ps_cmd.clear_solo(),
                    StemCommand::Quit => break,
                }
            }
        })?;

    // Build cpal output stream
    let ps_audio = Arc::clone(&playback_state);
    let pos_audio = Arc::clone(&playback_pos);
    let paused_audio = Arc::clone(&is_paused);
    let clock_audio = Arc::clone(&clock);

    let mut local_pos_f = 0.0f64;
    let mut last_sync_pos = 0usize;
    let mut first_callback = true;

    let stream = output_device.build_output_stream(
        &output_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            if first_callback {
                clock_audio.mark_started();
                first_callback = false;
            }

            if paused_audio.load(Ordering::Relaxed) {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
                return;
            }

            let total = stem_samples[0].len();
            if total == 0 {
                return;
            }

            // Resync if seek changed position externally
            let current_shared = pos_audio.load(Ordering::Relaxed);
            if current_shared.abs_diff(last_sync_pos) > out_channels * 4 {
                local_pos_f = current_shared as f64;
            }

            // Pre-compute per-stem gains
            let gains: [f32; STEM_COUNT] = std::array::from_fn(|i| ps_audio.effective_gain(i));

            for frame in data.chunks_mut(out_channels) {
                let pos_floor = (local_pos_f as usize) % total;
                let pos_ceil = (pos_floor + 1) % total;
                let frac = (local_pos_f - local_pos_f.floor()) as f32;

                let mut mix = 0.0f32;
                for (i, stem) in stem_samples.iter().enumerate() {
                    let s = stem[pos_floor] + (stem[pos_ceil] - stem[pos_floor]) * frac;
                    mix += s * gains[i];
                }

                let out = mix.clamp(-1.0, 1.0);
                for channel in frame.iter_mut() {
                    *channel = out;
                }

                local_pos_f += sample_rate_ratio;
            }

            if local_pos_f >= total as f64 {
                local_pos_f -= total as f64;
            }
            last_sync_pos = local_pos_f as usize;
            pos_audio.store(last_sync_pos, Ordering::Relaxed);
            clock_audio.set_sample_pos(last_sync_pos);
        },
        |err| {
            log::error!("Stem audio output error: {err}");
        },
        None,
    )?;

    stream.play()?;
    log::info!("Stem playback started @ {out_sample_rate}Hz, {out_channels}ch");

    Ok(stream)
}

/// Get the current playback position as an `Arc<AtomicUsize>` for the analysis thread.
///
/// This is a convenience function — in practice, the playback position is shared
/// through the MediaClock. The analysis thread reads clock.sample_pos().
pub fn playback_pos_from_clock(clock: &MediaClock) -> usize {
    clock.sample_pos()
}
