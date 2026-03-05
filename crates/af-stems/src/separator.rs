use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result};

use crate::stem::{STEM_COUNT, SeparationMeta, StemData, StemId, StemSet};

/// Model variant for separation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelVariant {
    Standard,
    Large,
}

impl ModelVariant {
    fn arg(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Large => "large",
        }
    }
}

/// Configuration for the separation subprocess.
#[derive(Clone, Debug)]
pub struct SeparationConfig {
    pub model: ModelVariant,
    /// Path to the Python executable (e.g. `.venv/Scripts/python.exe`).
    pub python_bin: PathBuf,
    /// Path to the `ext/SCNet/` directory.
    pub scnet_dir: PathBuf,
}

impl SeparationConfig {
    /// Build config with defaults relative to the project root.
    #[must_use]
    pub fn from_project_root(root: &Path) -> Self {
        let python_bin = if cfg!(windows) {
            root.join(".venv/Scripts/python.exe")
        } else {
            root.join(".venv/bin/python")
        };
        Self {
            model: ModelVariant::Standard,
            python_bin,
            scnet_dir: root.join("ext/SCNet"),
        }
    }
}

/// Progress updates from the separation subprocess.
#[derive(Clone, Debug)]
pub enum SeparationProgress {
    Starting,
    Progress(f32),
    Complete,
    Error(String),
}

/// Validate that the Python environment is ready for separation.
///
/// # Errors
/// Returns an error if Python, the checkpoint, or the script are missing.
pub fn preflight_check(config: &SeparationConfig) -> Result<()> {
    if !config.python_bin.exists() {
        anyhow::bail!(
            "Python not found at: {}\nInstall with: uv venv && uv pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu",
            config.python_bin.display()
        );
    }

    let checkpoint = match config.model {
        ModelVariant::Standard => config.scnet_dir.join("models/SCNet.th"),
        ModelVariant::Large => config.scnet_dir.join("models/SCNet-large.th"),
    };
    if !checkpoint.exists() {
        anyhow::bail!("SCNet checkpoint not found: {}", checkpoint.display());
    }

    let script = config.scnet_dir.join("separate.py");
    if !script.exists() {
        anyhow::bail!("Separation script not found: {}", script.display());
    }

    Ok(())
}

/// Run SCNet separation in a blocking manner, sending progress via channel.
///
/// This should be called from a dedicated thread. The resulting `StemSet`
/// contains decoded mono samples for each stem, ready for playback and analysis.
///
/// # Errors
/// Returns an error if the subprocess fails, files are missing, or decoding fails.
#[allow(clippy::too_many_lines)]
pub fn separate_file(
    audio_path: &Path,
    config: &SeparationConfig,
    progress_tx: &flume::Sender<SeparationProgress>,
) -> Result<StemSet> {
    let _ = progress_tx.send(SeparationProgress::Starting);

    preflight_check(config)?;

    // Create temp directory for output stems
    let temp_dir = tempfile::Builder::new()
        .prefix("classcii-stems-")
        .tempdir()
        .context("Failed to create temp directory for stems")?;

    let output_dir = temp_dir.path();
    let script = config.scnet_dir.join("separate.py");

    log::info!(
        "Starting stem separation: {} -> {}",
        audio_path.display(),
        output_dir.display()
    );

    // Spawn Python subprocess
    let mut child = Command::new(&config.python_bin)
        .args([
            script.to_str().unwrap_or("separate.py"),
            "--input",
            audio_path.to_str().context("Invalid audio path (non-UTF8)")?,
            "--output-dir",
            output_dir.to_str().context("Invalid output path")?,
            "--model",
            config.model.arg(),
            "--scnet-dir",
            config.scnet_dir.to_str().context("Invalid SCNet dir path")?,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .context(
            "Failed to launch Python for stem separation. \
             Ensure Python is installed in .venv with: \
             uv pip install torch torchaudio soundfile numpy pyyaml einops julius --index-url https://download.pytorch.org/whl/cpu",
        )?;

    // Read stderr for progress lines
    if let Some(stderr) = child.stderr.take() {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Some(pct_str) = line.strip_prefix("PROGRESS:") {
                if let Ok(pct) = pct_str.trim().parse::<f32>() {
                    let _ = progress_tx.send(SeparationProgress::Progress(pct));
                }
            } else if line.starts_with("ERROR:") {
                log::error!("SCNet: {line}");
            } else {
                log::debug!("SCNet: {line}");
            }
        }
    }

    let status = child
        .wait()
        .context("Failed to wait for Python subprocess")?;

    if !status.success() {
        // Try to read error.json
        let error_json_path = output_dir.join("error.json");
        let error_msg = if error_json_path.exists() {
            let content = std::fs::read_to_string(&error_json_path).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&content)
                .ok()
                .and_then(|v| v["error"].as_str().map(String::from))
                .unwrap_or_else(|| format!("Separation failed (exit code {status})"))
        } else {
            format!("Separation failed (exit code {status})")
        };

        let _ = progress_tx.send(SeparationProgress::Error(error_msg.clone()));
        anyhow::bail!("{error_msg}");
    }

    // Read metadata
    let meta_path = output_dir.join("meta.json");
    let meta_content =
        std::fs::read_to_string(&meta_path).context("Failed to read separation meta.json")?;
    let meta: SeparationMeta =
        serde_json::from_str(&meta_content).context("Failed to parse meta.json")?;

    // Decode each stem WAV file
    let mut stems_vec: Vec<StemData> = Vec::with_capacity(STEM_COUNT);

    for stem_id in StemId::ALL {
        let wav_path = output_dir.join(format!("{}.wav", stem_id.scnet_name()));
        if !wav_path.exists() {
            anyhow::bail!("Expected stem file not found: {}", wav_path.display());
        }

        let (samples, sample_rate) = af_audio::decode::decode_file(&wav_path)
            .with_context(|| format!("Failed to decode stem: {}", stem_id.label()))?;

        log::info!(
            "Loaded stem {}: {} samples @ {}Hz",
            stem_id.label(),
            samples.len(),
            sample_rate
        );

        stems_vec.push(StemData {
            id: stem_id,
            samples: Arc::new(samples),
            sample_rate,
        });
    }

    // Convert Vec to fixed-size array
    let stems: [StemData; STEM_COUNT] = stems_vec
        .try_into()
        .map_err(|_| anyhow::anyhow!("Expected exactly 4 stems"))?;

    let sample_rate = stems[0].sample_rate;

    let stem_set = StemSet {
        stems,
        sample_rate,
        duration_secs: meta.duration_secs,
        source_path: audio_path.to_path_buf(),
    };

    let _ = progress_tx.send(SeparationProgress::Complete);
    // Keep temp_dir alive until stems are loaded — WAVs are already decoded into memory.
    // temp_dir is dropped here, cleaning up the files.

    Ok(stem_set)
}
