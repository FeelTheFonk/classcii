use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Decode an audio file into mono f32 samples.
///
/// Supports WAV, MP3, FLAC, OGG, AAC via symphonia.
///
/// # Errors
/// Returns an error if the file cannot be opened or decoded.
///
/// # Example
/// ```no_run
/// use af_audio::decode::decode_file;
/// let (samples, sample_rate) = decode_file("track.wav").unwrap();
/// ```
pub fn decode_file(path: impl AsRef<Path>) -> Result<(Vec<f32>, u32)> {
    let path = path.as_ref();
    let file =
        File::open(path).with_context(|| format!("Cannot open audio file: {}", path.display()))?;
    let mss = MediaSourceStream::new(
        Box::new(file),
        symphonia::core::io::MediaSourceStreamOptions::default(),
    );

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .context("Failed to probe audio format")?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .context("No default audio track found")?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track
        .codec_params
        .channels
        .map_or(1, symphonia::core::audio::Channels::count);

    // GAP 4 SOTA: Downsample aggressif (div/2) si la fréquence > 24000Hz
    // pour éviter d'exploser la RAM sur un film de 2 heures.
    let downsample_factor = if sample_rate > 24000 { 2 } else { 1 };
    let final_sample_rate = sample_rate / downsample_factor;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create audio decoder")?;

    let track_id = track.id;
    let mut all_samples: Vec<f32> = Vec::new();
    let mut sample_idx = 0;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut max_sample_frames: usize = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(e) => {
                log::warn!("Audio decode packet error: {e}");
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Audio decode frame error: {e}");
                continue;
            }
        };

        let spec = *decoded.spec();
        let num_frames = decoded.capacity();
        // Reuse SampleBuffer: only reallocate if this packet is bigger than current capacity
        if sample_buf.is_none() || num_frames > max_sample_frames {
            sample_buf = Some(SampleBuffer::<f32>::new(num_frames as u64, spec));
            max_sample_frames = num_frames;
        }
        let Some(buf) = sample_buf.as_mut() else {
            continue;
        };
        buf.copy_interleaved_ref(decoded);
        let interleaved = buf.samples();

        // Downmix to mono and decimate
        for chunk in interleaved.chunks(channels) {
            if sample_idx % downsample_factor == 0 {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
            sample_idx += 1;
        }
    }

    log::info!(
        "Decoded {} samples @ {}Hz (Original {}Hz) from {}",
        all_samples.len(),
        final_sample_rate,
        sample_rate,
        path.display()
    );

    Ok((all_samples, final_sample_rate))
}
