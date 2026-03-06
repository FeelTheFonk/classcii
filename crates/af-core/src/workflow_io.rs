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
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::RenderConfig;
use crate::workflow::{
    StemSeparationInfo, StemStatesSnapshot, SourceInfo, WorkflowManifest,
    sanitize_workflow_name, workflow_base_dir,
};

/// A fully loaded workflow ready for replay.
#[derive(Debug)]
pub struct LoadedWorkflow {
    pub manifest: WorkflowManifest,
    pub config: RenderConfig,
    pub source: SourceInfo,
    pub stem_states: Option<StemStatesSnapshot>,
    pub stem_info: Option<StemSeparationInfo>,
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
) -> Result<PathBuf> {
    let safe_name = sanitize_workflow_name(name);
    let dir = workflow_base_dir().join(&safe_name);

    // Create directory tree
    fs::create_dir_all(&dir)
        .with_context(|| format!("Cannot create workflow dir: {}", dir.display()))?;

    // Manifest
    let mut manifest = WorkflowManifest::new();
    manifest.has_stems = stem_wav_sources.is_some();
    manifest.description = format!("Workflow: {safe_name}");

    let manifest_toml = toml::to_string_pretty(&manifest)
        .context("Serialize manifest")?;
    fs::write(dir.join("manifest.toml"), &manifest_toml)
        .context("Write manifest.toml")?;

    // Config
    let config_toml = toml::to_string_pretty(config)
        .context("Serialize config")?;
    fs::write(dir.join("config.toml"), &config_toml)
        .context("Write config.toml")?;

    // Source info
    let source_toml = toml::to_string_pretty(source)
        .context("Serialize source")?;
    fs::write(dir.join("source.toml"), &source_toml)
        .context("Write source.toml")?;

    // Stems (optional)
    if let Some(wav_paths) = stem_wav_sources {
        let stems_dir = dir.join("stems");
        fs::create_dir_all(&stems_dir)
            .context("Create stems/ dir")?;

        let stem_names = ["drums", "bass", "other", "vocals"];
        for (i, src_path) in wav_paths.iter().enumerate() {
            let dst = stems_dir.join(format!("{}.wav", stem_names[i]));
            fs::copy(src_path, &dst)
                .with_context(|| format!("Copy stem {} → {}", src_path.display(), dst.display()))?;
        }

        if let Some(states) = stem_states {
            let states_toml = toml::to_string_pretty(states)
                .context("Serialize stem states")?;
            fs::write(stems_dir.join("states.toml"), &states_toml)
                .context("Write stems/states.toml")?;
        }

        if let Some(info) = stem_info {
            let info_toml = toml::to_string_pretty(info)
                .context("Serialize stem info")?;
            fs::write(stems_dir.join("metadata.toml"), &info_toml)
                .context("Write stems/metadata.toml")?;
        }
    }

    log::info!("Workflow saved to {}", dir.display());
    Ok(dir)
}

/// Load a workflow from a directory path.
///
/// # Errors
/// Returns an error if required files are missing or malformed.
pub fn load_workflow(dir: &Path) -> Result<LoadedWorkflow> {
    // Manifest
    let manifest_str = fs::read_to_string(dir.join("manifest.toml"))
        .with_context(|| format!("Read manifest.toml in {}", dir.display()))?;
    let manifest: WorkflowManifest = toml::from_str(&manifest_str)
        .context("Parse manifest.toml")?;

    if !manifest.is_compatible() {
        anyhow::bail!(
            "Workflow version {} is newer than supported version {}",
            manifest.version,
            crate::workflow::WORKFLOW_VERSION,
        );
    }

    // Config
    let config_str = fs::read_to_string(dir.join("config.toml"))
        .context("Read config.toml")?;
    let config: RenderConfig = toml::from_str(&config_str)
        .context("Parse config.toml")?;

    // Source
    let source_str = fs::read_to_string(dir.join("source.toml"))
        .context("Read source.toml")?;
    let source: SourceInfo = toml::from_str(&source_str)
        .context("Parse source.toml")?;

    // Stems (optional)
    let stems_dir = dir.join("stems");
    let stem_states = if stems_dir.join("states.toml").exists() {
        let s = fs::read_to_string(stems_dir.join("states.toml"))
            .context("Read stems/states.toml")?;
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

    log::info!("Workflow loaded from {}", dir.display());
    Ok(LoadedWorkflow {
        manifest,
        config,
        source,
        stem_states,
        stem_info,
        dir: dir.to_path_buf(),
    })
}

/// Load a workflow by name (looks up in `workflow_base_dir()`).
///
/// # Errors
/// Returns an error if the workflow directory doesn't exist.
pub fn load_workflow_by_name(name: &str) -> Result<LoadedWorkflow> {
    let safe_name = sanitize_workflow_name(name);
    let dir = workflow_base_dir().join(&safe_name);
    if !dir.exists() {
        anyhow::bail!("Workflow '{}' not found at {}", name, dir.display());
    }
    load_workflow(&dir)
}

/// List all saved workflow names.
///
/// # Errors
/// Returns an error if the workflows directory cannot be read.
pub fn list_workflows() -> Result<Vec<String>> {
    let base = workflow_base_dir();
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

#[cfg(test)]
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
                StemStateEntry { id: "drums".into(), muted: false, solo: false, volume: 1.0, visible: true },
                StemStateEntry { id: "bass".into(), muted: false, solo: false, volume: 1.0, visible: true },
                StemStateEntry { id: "other".into(), muted: false, solo: false, volume: 1.0, visible: true },
                StemStateEntry { id: "vocals".into(), muted: false, solo: false, volume: 1.0, visible: true },
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
        ).unwrap();
        fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&test_config()).unwrap(),
        ).unwrap();
        fs::write(
            dir.join("source.toml"),
            toml::to_string_pretty(&test_source()).unwrap(),
        ).unwrap();

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
        ).unwrap();
        fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&test_config()).unwrap(),
        ).unwrap();
        fs::write(
            dir.join("source.toml"),
            toml::to_string_pretty(&test_source()).unwrap(),
        ).unwrap();
        fs::write(
            dir.join("stems").join("states.toml"),
            toml::to_string_pretty(&test_stem_states()).unwrap(),
        ).unwrap();

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
        ).unwrap();

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
        let result = list_workflows();
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
        ).unwrap();
        fs::write(dir.join("config.toml"), toml::to_string_pretty(&test_config()).unwrap()).unwrap();
        fs::write(dir.join("source.toml"), toml::to_string_pretty(&test_source()).unwrap()).unwrap();

        let result = load_workflow(&dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("newer"));

        let _ = fs::remove_dir_all(&tmp);
    }
}
