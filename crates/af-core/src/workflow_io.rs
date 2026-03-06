//! Workflow save/load I/O operations.
//!
//! Directory layout for a saved workflow:
//! ```text
//! workflows/<name>/
//!   manifest.toml          — version, timestamp, flags
//!   config.toml            — full RenderConfig snapshot
//!   source.toml            — SourceInfo (original paths)
//!   stems/                 — (optional, if has_stems)
//!     states.toml          — StemStatesSnapshot
//!     metadata.toml        — StemSeparationInfo
//!     drums.wav            — stem audio (copied from separation output)
//!     bass.wav
//!     other.wav
//!     vocals.wav
//! ```

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::RenderConfig;
use crate::feature_timeline::FeatureTimeline;
use crate::workflow::{
    SourceInfo, StemSeparationInfo, StemStatesSnapshot, WorkflowManifest, sanitize_workflow_name,
};

/// A fully loaded workflow ready for replay.
#[derive(Debug)]
pub struct LoadedWorkflow {
    pub manifest: WorkflowManifest,
    pub config: RenderConfig,
    pub source: SourceInfo,
    pub stem_states: Option<StemStatesSnapshot>,
    pub stem_info: Option<StemSeparationInfo>,
    /// Pre-computed feature timeline (bincode), if saved.
    pub feature_timeline: Option<FeatureTimeline>,
    /// Path to the workflow directory (for resolving stem WAVs).
    pub dir: PathBuf,
}

impl LoadedWorkflow {
    /// Path to a stem WAV file within this workflow, if stems were saved.
    #[must_use]
    pub fn stem_wav_path(&self, stem_name: &str) -> Option<PathBuf> {
        if self.manifest.has_stems {
            let p = self.dir.join("stems").join(format!("{stem_name}.wav"));
            if p.exists() { Some(p) } else { None }
        } else {
            None
        }
    }
}

/// Save a complete workflow to disk.
///
/// Creates `workflows/<name>/` with all metadata files.
/// If `stem_wav_sources` is provided, copies stem WAV files into the workflow.
///
/// # Errors
/// Returns an error if directory creation or file writing fails.
pub fn save_workflow(
    name: &str,
    config: &RenderConfig,
    source: &SourceInfo,
    stem_states: Option<&StemStatesSnapshot>,
    stem_info: Option<&StemSeparationInfo>,
    stem_wav_sources: Option<&[PathBuf; 4]>,
    workflows_dir: &Path,
) -> Result<PathBuf> {
    let safe_name = sanitize_workflow_name(name);
    let dir = workflows_dir.join(&safe_name);

    // Create directory tree
    fs::create_dir_all(&dir)
        .with_context(|| format!("Cannot create workflow dir: {}", dir.display()))?;

    // Manifest
    let mut manifest = WorkflowManifest::new();
    manifest.has_stems = stem_wav_sources.is_some();
    manifest.description = format!("Workflow: {safe_name}");

    let manifest_toml = toml::to_string_pretty(&manifest).context("Serialize manifest")?;
    fs::write(dir.join("manifest.toml"), &manifest_toml).context("Write manifest.toml")?;

    // Config
    let config_toml = toml::to_string_pretty(config).context("Serialize config")?;
    fs::write(dir.join("config.toml"), &config_toml).context("Write config.toml")?;

    // Source info
    let source_toml = toml::to_string_pretty(source).context("Serialize source")?;
    fs::write(dir.join("source.toml"), &source_toml).context("Write source.toml")?;

    // Stems (optional)
    if let Some(wav_paths) = stem_wav_sources {
        let stems_dir = dir.join("stems");
        fs::create_dir_all(&stems_dir).context("Create stems/ dir")?;

        let stem_names = ["drums", "bass", "other", "vocals"];
        for (i, src_path) in wav_paths.iter().enumerate() {
            let dst = stems_dir.join(format!("{}.wav", stem_names[i]));
            fs::copy(src_path, &dst)
                .with_context(|| format!("Copy stem {} → {}", src_path.display(), dst.display()))?;
        }

        if let Some(states) = stem_states {
            let states_toml = toml::to_string_pretty(states).context("Serialize stem states")?;
            fs::write(stems_dir.join("states.toml"), &states_toml)
                .context("Write stems/states.toml")?;
        }

        if let Some(info) = stem_info {
            let info_toml = toml::to_string_pretty(info).context("Serialize stem info")?;
            fs::write(stems_dir.join("metadata.toml"), &info_toml)
                .context("Write stems/metadata.toml")?;
        }
    }

    log::info!("Workflow saved to {}", dir.display());
    Ok(dir)
}

/// Write stem audio samples as mono f32 WAV files into a workflow directory.
///
/// `stems` is an array of (samples, sample_rate) for [drums, bass, other, vocals].
/// Returns the paths to the written WAV files.
///
/// # Errors
/// Returns an error if directory creation or file writing fails.
pub fn write_stem_wavs(workflow_dir: &Path, stems: &[(&[f32], u32); 4]) -> Result<[PathBuf; 4]> {
    let stems_dir = workflow_dir.join("stems");
    fs::create_dir_all(&stems_dir).context("Create stems/ dir")?;

    let names = ["drums", "bass", "other", "vocals"];
    let paths: [PathBuf; 4] = std::array::from_fn(|i| stems_dir.join(format!("{}.wav", names[i])));

    for (i, (samples, sr)) in stems.iter().enumerate() {
        write_wav_f32(&paths[i], samples, *sr)
            .with_context(|| format!("Write {}.wav", names[i]))?;
    }

    Ok(paths)
}

/// Write mono f32 PCM samples as a WAV file (IEEE float format, no external deps).
fn write_wav_f32(path: &Path, samples: &[f32], sample_rate: u32) -> Result<()> {
    let data_len = (samples.len() * 4) as u32;
    let file_len = 36 + data_len;
    let mut f = std::io::BufWriter::new(
        fs::File::create(path).with_context(|| format!("Create WAV: {}", path.display()))?,
    );
    f.write_all(b"RIFF")?;
    f.write_all(&file_len.to_le_bytes())?;
    f.write_all(b"WAVE")?;
    // fmt chunk
    f.write_all(b"fmt ")?;
    f.write_all(&16u32.to_le_bytes())?; // chunk size
    f.write_all(&3u16.to_le_bytes())?; // format: IEEE float
    f.write_all(&1u16.to_le_bytes())?; // channels: mono
    f.write_all(&sample_rate.to_le_bytes())?;
    f.write_all(&(sample_rate * 4).to_le_bytes())?; // byte rate
    f.write_all(&4u16.to_le_bytes())?; // block align
    f.write_all(&32u16.to_le_bytes())?; // bits per sample
    // data chunk
    f.write_all(b"data")?;
    f.write_all(&data_len.to_le_bytes())?;
    for &s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    f.flush()?;
    Ok(())
}

/// Save a pre-computed feature timeline as a binary (bincode) file within a workflow.
/// Also updates the manifest to set `has_feature_timeline = true`.
///
/// # Errors
/// Returns an error if serialization or file writing fails.
pub fn save_feature_timeline(workflow_dir: &Path, timeline: &FeatureTimeline) -> Result<()> {
    let encoded = bincode::serialize(timeline).context("Serialize feature timeline")?;
    let path = workflow_dir.join("timeline.bin");
    fs::write(&path, &encoded).with_context(|| format!("Write {}", path.display()))?;

    // Update manifest
    let manifest_path = workflow_dir.join("manifest.toml");
    if manifest_path.exists() {
        let manifest_str =
            fs::read_to_string(&manifest_path).context("Read manifest for timeline update")?;
        if let Ok(mut manifest) = toml::from_str::<WorkflowManifest>(&manifest_str) {
            manifest.has_feature_timeline = true;
            let updated = toml::to_string_pretty(&manifest).context("Re-serialize manifest")?;
            fs::write(&manifest_path, &updated).context("Update manifest.toml")?;
        }
    }

    log::info!(
        "Feature timeline saved: {} frames, {} bytes",
        timeline.total_frames(),
        encoded.len()
    );
    Ok(())
}

/// Load a pre-computed feature timeline from a workflow directory.
///
/// # Errors
/// Returns an error if the file is missing or deserialization fails.
pub fn load_feature_timeline(workflow_dir: &Path) -> Result<FeatureTimeline> {
    let path = workflow_dir.join("timeline.bin");
    let data = fs::read(&path).with_context(|| format!("Read {}", path.display()))?;
    let timeline: FeatureTimeline =
        bincode::deserialize(&data).context("Deserialize feature timeline")?;
    log::info!(
        "Feature timeline loaded: {} frames",
        timeline.total_frames()
    );
    Ok(timeline)
}

/// Load a workflow from a directory path.
///
/// # Errors
/// Returns an error if required files are missing or malformed.
pub fn load_workflow(dir: &Path) -> Result<LoadedWorkflow> {
    // Manifest
    let manifest_str = fs::read_to_string(dir.join("manifest.toml"))
        .with_context(|| format!("Read manifest.toml in {}", dir.display()))?;
    let manifest: WorkflowManifest =
        toml::from_str(&manifest_str).context("Parse manifest.toml")?;

    if !manifest.is_compatible() {
        anyhow::bail!(
            "Workflow version {} is newer than supported version {}",
            manifest.version,
            crate::workflow::WORKFLOW_VERSION,
        );
    }

    // Config
    let config_str = fs::read_to_string(dir.join("config.toml")).context("Read config.toml")?;
    let config: RenderConfig = toml::from_str(&config_str).context("Parse config.toml")?;

    // Source
    let source_str = fs::read_to_string(dir.join("source.toml")).context("Read source.toml")?;
    let source: SourceInfo = toml::from_str(&source_str).context("Parse source.toml")?;

    // Stems (optional)
    let stems_dir = dir.join("stems");
    let stem_states = if stems_dir.join("states.toml").exists() {
        let s =
            fs::read_to_string(stems_dir.join("states.toml")).context("Read stems/states.toml")?;
        Some(toml::from_str::<StemStatesSnapshot>(&s).context("Parse stems/states.toml")?)
    } else {
        None
    };

    let stem_info = if stems_dir.join("metadata.toml").exists() {
        let s = fs::read_to_string(stems_dir.join("metadata.toml"))
            .context("Read stems/metadata.toml")?;
        Some(toml::from_str::<StemSeparationInfo>(&s).context("Parse stems/metadata.toml")?)
    } else {
        None
    };

    // Feature timeline (optional bincode file)
    let feature_timeline = if dir.join("timeline.bin").exists() {
        match load_feature_timeline(dir) {
            Ok(tl) => Some(tl),
            Err(e) => {
                log::warn!("Could not load feature timeline: {e}");
                None
            }
        }
    } else {
        None
    };

    log::info!("Workflow loaded from {}", dir.display());
    Ok(LoadedWorkflow {
        manifest,
        config,
        source,
        stem_states,
        stem_info,
        feature_timeline,
        dir: dir.to_path_buf(),
    })
}

/// Load a workflow by name (looks up in `workflows_dir`).
///
/// # Errors
/// Returns an error if the workflow directory doesn't exist.
pub fn load_workflow_by_name(name: &str, workflows_dir: &Path) -> Result<LoadedWorkflow> {
    let safe_name = sanitize_workflow_name(name);
    let dir = workflows_dir.join(&safe_name);
    if !dir.exists() {
        anyhow::bail!("Workflow '{}' not found at {}", name, dir.display());
    }
    load_workflow(&dir)
}

/// List all saved workflow names.
///
/// # Errors
/// Returns an error if the workflows directory cannot be read.
pub fn list_workflows(workflows_dir: &Path) -> Result<Vec<String>> {
    let base = workflows_dir.to_path_buf();
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(&base).context("Read workflows dir")? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
            && entry.path().join("manifest.toml").exists()
        {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

/// Detailed workflow entry for TUI browsing.
#[derive(Clone, Debug)]
pub struct WorkflowEntry {
    pub name: String,
    pub created_at: String,
    pub description: String,
    pub has_stems: bool,
    pub has_timeline: bool,
}

/// List all saved workflows with metadata (for TUI browse overlay).
///
/// Convenience wrapper for [`list_workflows_detailed_in`].
///
/// # Errors
/// Returns an error if the workflows directory cannot be read.
pub fn list_workflows_detailed(workflows_dir: &Path) -> Result<Vec<WorkflowEntry>> {
    list_workflows_detailed_in(workflows_dir)
}

/// List all saved workflows in a specific directory.
///
/// # Errors
/// Returns an error if the directory cannot be read.
pub fn list_workflows_detailed_in(base: &Path) -> Result<Vec<WorkflowEntry>> {
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(base).context("Read workflows dir")? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.toml");
        if !manifest_path.exists() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        let manifest: WorkflowManifest = match fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        entries.push(WorkflowEntry {
            name,
            created_at: manifest.created_at,
            description: manifest.description,
            has_stems: manifest.has_stems,
            has_timeline: manifest.has_feature_timeline,
        });
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Update the description field in a workflow's manifest.
///
/// Silently ignores errors (best-effort, non-critical).
pub fn update_workflow_description(workflow_dir: &Path, description: &str) {
    let manifest_path = workflow_dir.join("manifest.toml");
    let Ok(s) = fs::read_to_string(&manifest_path) else {
        return;
    };
    let Ok(mut m) = toml::from_str::<WorkflowManifest>(&s) else {
        return;
    };
    m.description = description.to_string();
    if let Ok(updated) = toml::to_string_pretty(&m) {
        let _ = fs::write(&manifest_path, updated);
    }
}

/// Delete a workflow by name.
///
/// # Errors
/// Returns an error if the workflow doesn't exist or cannot be deleted.
pub fn delete_workflow(name: &str, workflows_dir: &Path) -> Result<()> {
    let safe_name = crate::workflow::sanitize_workflow_name(name);
    let dir = workflows_dir.join(&safe_name);
    if !dir.exists() {
        anyhow::bail!("Workflow '{name}' not found");
    }
    fs::remove_dir_all(&dir).with_context(|| format!("Delete workflow dir: {}", dir.display()))?;
    log::info!("Workflow '{name}' deleted");
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::workflow::{MediaType, StemStateEntry};

    fn test_config() -> RenderConfig {
        RenderConfig::default()
    }

    fn test_source() -> SourceInfo {
        SourceInfo {
            path: PathBuf::from("test_image.png"),
            media_type: MediaType::Image,
            audio_path: Some(PathBuf::from("test_audio.mp3")),
        }
    }

    fn test_stem_states() -> StemStatesSnapshot {
        StemStatesSnapshot {
            states: [
                StemStateEntry {
                    id: "drums".into(),
                    muted: false,
                    solo: false,
                    volume: 1.0,
                    visible: true,
                },
                StemStateEntry {
                    id: "bass".into(),
                    muted: false,
                    solo: false,
                    volume: 1.0,
                    visible: true,
                },
                StemStateEntry {
                    id: "other".into(),
                    muted: false,
                    solo: false,
                    volume: 1.0,
                    visible: true,
                },
                StemStateEntry {
                    id: "vocals".into(),
                    muted: false,
                    solo: false,
                    volume: 1.0,
                    visible: true,
                },
            ],
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join("classcii_test_workflow_rt");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // Manually create workflow in temp dir
        let dir = tmp.join("test_wf");
        fs::create_dir_all(&dir).unwrap();

        let manifest = WorkflowManifest::new();
        fs::write(
            dir.join("manifest.toml"),
            toml::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&test_config()).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("source.toml"),
            toml::to_string_pretty(&test_source()).unwrap(),
        )
        .unwrap();

        let loaded = load_workflow(&dir).unwrap();
        assert!(loaded.manifest.is_compatible());
        assert_eq!(loaded.source.path, PathBuf::from("test_image.png"));
        assert!(loaded.stem_states.is_none());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn save_and_load_with_stems() {
        let tmp = std::env::temp_dir().join("classcii_test_workflow_stems");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let dir = tmp.join("stem_wf");
        fs::create_dir_all(dir.join("stems")).unwrap();

        let mut manifest = WorkflowManifest::new();
        manifest.has_stems = true;
        fs::write(
            dir.join("manifest.toml"),
            toml::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&test_config()).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("source.toml"),
            toml::to_string_pretty(&test_source()).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("stems").join("states.toml"),
            toml::to_string_pretty(&test_stem_states()).unwrap(),
        )
        .unwrap();

        let info = StemSeparationInfo {
            sample_rate: 44100,
            channels: 2,
            duration_secs: 180.0,
            model: "standard".into(),
            elapsed_secs: 12.5,
        };
        fs::write(
            dir.join("stems").join("metadata.toml"),
            toml::to_string_pretty(&info).unwrap(),
        )
        .unwrap();

        let loaded = load_workflow(&dir).unwrap();
        assert!(loaded.manifest.has_stems);
        assert!(loaded.stem_states.is_some());
        let states = loaded.stem_states.unwrap();
        assert_eq!(states.states[0].id, "drums");
        assert!(loaded.stem_info.is_some());
        assert_eq!(loaded.stem_info.unwrap().sample_rate, 44100);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn list_workflows_empty() {
        // Just ensure it doesn't panic on a missing dir
        let nonexistent = std::env::temp_dir().join("classcii_test_wf_noexist_xyz");
        let result = list_workflows(&nonexistent);
        assert!(result.is_ok());
    }

    #[test]
    fn incompatible_version_rejected() {
        let tmp = std::env::temp_dir().join("classcii_test_workflow_ver");
        let _ = fs::remove_dir_all(&tmp);
        let dir = tmp.join("future_wf");
        fs::create_dir_all(&dir).unwrap();

        let mut manifest = WorkflowManifest::new();
        manifest.version = 999;
        fs::write(
            dir.join("manifest.toml"),
            toml::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&test_config()).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.join("source.toml"),
            toml::to_string_pretty(&test_source()).unwrap(),
        )
        .unwrap();

        let result = load_workflow(&dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("newer"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
