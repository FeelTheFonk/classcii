use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Taux d'échantillonnage cible pour le fallback ffmpeg.
/// 24kHz mono : ~96 KB/s — compromis RAM/qualité pour les longs fichiers.
const FFMPEG_TARGET_SAMPLE_RATE: u32 = 24000;

/// Decode an audio file into mono f32 samples.
///
/// Essaie symphonia en premier. Si symphonia ne peut pas décoder le format
/// (ex: HE-AAC dans MKV), bascule automatiquement vers ffmpeg subprocess.
///
/// # Errors
/// Retourne une erreur si ni symphonia ni ffmpeg ne parviennent à décoder.
///
/// # Example
/// ```no_run
/// use af_audio::decode::decode_file;
/// use std::path::Path;
/// // let (samples, sample_rate) = decode_file(Path::new("track.wav")).unwrap();
/// ```
pub fn decode_file(path: impl AsRef<Path>) -> Result<(Vec<f32>, u32)> {
    let path = path.as_ref();
    match decode_via_symphonia(path) {
        Ok(result) => Ok(result),
        Err(e) => {
            log::warn!(
                "Symphonia n'a pas pu décoder {} ({e}). Tentative via ffmpeg.",
                path.display()
            );
            decode_via_ffmpeg(path)
        }
    }
}

/// Décode via symphonia (WAV, MP3, FLAC, OGG, AAC LC, MKV).
///
/// # Errors
/// Retourne une erreur si le codec n'est pas supporté ou si le fichier est illisible.
fn decode_via_symphonia(path: &Path) -> Result<(Vec<f32>, u32)> {
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

    // Downsample aggressif (div/2) si fréquence > 24kHz
    // pour éviter d'exploser la RAM sur un film de 2 heures.
    let downsample_factor = if sample_rate > 24000 { 2 } else { 1 };
    let final_sample_rate = sample_rate / downsample_factor;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .context("Failed to create audio decoder")?;

    let track_id = track.id;
    let mut all_samples: Vec<f32> = Vec::new();
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
        if sample_buf.is_none() || num_frames > max_sample_frames {
            sample_buf = Some(SampleBuffer::<f32>::new(num_frames as u64, spec));
            max_sample_frames = num_frames;
        }
        let Some(buf) = sample_buf.as_mut() else {
            continue;
        };
        buf.copy_interleaved_ref(decoded);
        let interleaved = buf.samples();

        if downsample_factor > 1 {
            // 2-tap averaging decimation (anti-aliased downsampling)
            let mut accum = 0.0f32;
            for (i, chunk) in interleaved.chunks(channels).enumerate() {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                accum += mono;
                if (i + 1) % downsample_factor as usize == 0 {
                    all_samples.push(accum / downsample_factor as f32);
                    accum = 0.0;
                }
            }
        } else {
            for chunk in interleaved.chunks(channels) {
                let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                all_samples.push(mono);
            }
        }
    }

    log::info!(
        "Decoded {} samples @ {}Hz (Original {}Hz) via symphonia from {}",
        all_samples.len(),
        final_sample_rate,
        sample_rate,
        path.display()
    );

    Ok((all_samples, final_sample_rate))
}

/// Décode via ffmpeg subprocess en PCM f32le 24kHz mono.
///
/// Fallback universel pour les formats non supportés par symphonia (HE-AAC, etc.).
/// Requiert `ffmpeg` en PATH.
///
/// # Errors
/// Retourne une erreur si ffmpeg est introuvable ou si le décodage échoue.
fn decode_via_ffmpeg(path: &Path) -> Result<(Vec<f32>, u32)> {
    let path_str = path.to_str().context("Chemin audio invalide (non-UTF8)")?;

    let mut child = Command::new("ffmpeg")
        .args([
            "-i",
            path_str,
            "-vn", // pas de vidéo
            "-ar",
            "24000", // resample à 24kHz
            "-ac",
            "1", // mono
            "-f",
            "f32le", // raw float32 little-endian
            "-hide_banner",
            "-loglevel",
            "error",
            "pipe:1", // stdout
        ])
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context(
            "Impossible de lancer ffmpeg pour le décodage audio. Vérifiez que ffmpeg est en PATH.",
        )?;

    let mut raw_bytes: Vec<u8> = Vec::new();
    if let Some(ref mut stdout) = child.stdout {
        stdout
            .read_to_end(&mut raw_bytes)
            .context("Erreur lecture stdout ffmpeg audio")?;
    }

    let _ = child.wait();

    if raw_bytes.is_empty() {
        anyhow::bail!(
            "ffmpeg n'a produit aucun sample audio depuis {}",
            path.display()
        );
    }

    // Convertir les bytes bruts en Vec<f32> (f32le = 4 bytes par sample)
    let num_samples = raw_bytes.len() / 4;
    let mut samples = Vec::with_capacity(num_samples);
    for chunk in raw_bytes.chunks_exact(4) {
        // SAFETY: chunks_exact(4) garantit exactement 4 bytes
        let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
        samples.push(f32::from_le_bytes(bytes));
    }

    log::info!(
        "Decoded {} samples @ {}Hz via ffmpeg from {}",
        samples.len(),
        FFMPEG_TARGET_SAMPLE_RATE,
        path.display()
    );

    Ok((samples, FFMPEG_TARGET_SAMPLE_RATE))
}
