//! Workflow persistence types for save/load of complete session state.
//!
//! A workflow captures the full rendering state: config, source info, stem states,
//! creation mode, and optionally pre-computed feature timelines for reproducible replay.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Current workflow format version. Bumped on breaking changes.
pub const WORKFLOW_VERSION: u32 = 1;

/// Top-level manifest describing a saved workflow.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowManifest {
    /// Format version for compatibility checking.
    pub version: u32,
    /// ISO 8601 timestamp of creation.
    pub created_at: String,
    /// classcii version that created this workflow.
    pub classcii_version: String,
    /// Optional user description.
    #[serde(default)]
    pub description: String,
    /// Whether stem WAVs are included.
    #[serde(default)]
    pub has_stems: bool,
    /// Whether a feature timeline binary is included.
    #[serde(default)]
    pub has_feature_timeline: bool,
}

impl WorkflowManifest {
    /// Create a new manifest with current timestamp and version.
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: WORKFLOW_VERSION,
            created_at: now_iso8601(),
            classcii_version: env!("CARGO_PKG_VERSION").to_string(),
            description: String::new(),
            has_stems: false,
            has_feature_timeline: false,
        }
    }

    /// Check if this manifest is compatible with the current version.
    #[must_use]
    pub fn is_compatible(&self) -> bool {
        self.version <= WORKFLOW_VERSION
    }
}

impl Default for WorkflowManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about the original media source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceInfo {
    /// Original file path (may not exist on reload).
    pub path: PathBuf,
    /// Media type.
    pub media_type: MediaType,
    /// Audio file path if separate from visual source.
    #[serde(default)]
    pub audio_path: Option<PathBuf>,
}

/// Type of media source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    None,
}

/// Snapshot of per-stem mute/solo/volume/visible states.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemStatesSnapshot {
    pub states: [StemStateEntry; 4],
}

/// Single stem state entry for serialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemStateEntry {
    pub id: String,
    pub muted: bool,
    pub solo: bool,
    pub volume: f32,
    pub visible: bool,
}

/// Snapshot of creation mode state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreationSnapshot {
    pub preset_name: String,
    pub master_intensity: f32,
    pub auto_mode: bool,
}

/// Metadata from stem separation (mirrors SeparationMeta but serializable to TOML).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StemSeparationInfo {
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_secs: f64,
    pub model: String,
    pub elapsed_secs: f64,
}

/// Resolve the base directory for workflow storage.
///
/// Returns `<exe_dir>/workflows/` if the exe directory is writable,
/// otherwise falls back to `<cwd>/workflows/`.
#[must_use]
pub fn workflow_base_dir() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    base.join("workflows")
}

/// Sanitize a workflow name for use as a directory name.
#[must_use]
pub fn sanitize_workflow_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches(|c: char| c == '.' || c == '_');
    if trimmed.is_empty() {
        "workflow".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Generate an ISO 8601 timestamp (simplified, no external dep).
fn now_iso8601() -> String {
    // Use SystemTime for a basic timestamp without chrono dependency.
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Basic UTC timestamp: days since epoch, then hours/minutes/seconds.
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let minutes = (rem % 3600) / 60;
    let seconds = rem % 60;
    // Approximate date from days since 1970-01-01 (good enough for file naming).
    let (year, month, day) = days_to_date(days);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since epoch to (year, month, day). Simplified leap year handling.
fn days_to_date(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_dangerous_chars() {
        assert_eq!(sanitize_workflow_name("my workflow!"), "my_workflow");
        assert_eq!(sanitize_workflow_name("../hack"), "hack");
        assert_eq!(sanitize_workflow_name("good-name_v2"), "good-name_v2");
        assert_eq!(sanitize_workflow_name(""), "workflow");
        assert_eq!(sanitize_workflow_name("..."), "workflow");
    }

    #[test]
    fn manifest_compatible() {
        let m = WorkflowManifest::new();
        assert!(m.is_compatible());
        assert_eq!(m.version, WORKFLOW_VERSION);
    }

    #[test]
    fn manifest_roundtrip_toml() {
        let m = WorkflowManifest::new();
        let toml_str = toml::to_string_pretty(&m).expect("serialize");
        let m2: WorkflowManifest = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(m.version, m2.version);
        assert_eq!(m.classcii_version, m2.classcii_version);
    }

    #[test]
    fn stem_states_roundtrip_toml() {
        let snap = StemStatesSnapshot {
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
                    muted: true,
                    solo: false,
                    volume: 0.8,
                    visible: true,
                },
                StemStateEntry {
                    id: "other".into(),
                    muted: false,
                    solo: true,
                    volume: 1.0,
                    visible: false,
                },
                StemStateEntry {
                    id: "vocals".into(),
                    muted: false,
                    solo: false,
                    volume: 0.6,
                    visible: true,
                },
            ],
        };
        let toml_str = toml::to_string_pretty(&snap).expect("serialize");
        let snap2: StemStatesSnapshot = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(snap2.states[1].volume, 0.8);
        assert!(snap2.states[1].muted);
    }

    #[test]
    fn source_info_roundtrip_toml() {
        let info = SourceInfo {
            path: PathBuf::from("/tmp/test.mp4"),
            media_type: MediaType::Video,
            audio_path: Some(PathBuf::from("/tmp/audio.wav")),
        };
        let toml_str = toml::to_string_pretty(&info).expect("serialize");
        let info2: SourceInfo = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(info2.path, PathBuf::from("/tmp/test.mp4"));
    }

    #[test]
    fn workflow_base_dir_ends_with_workflows() {
        let dir = workflow_base_dir();
        assert!(dir.ends_with("workflows"));
    }

    #[test]
    fn iso8601_format() {
        let ts = now_iso8601();
        assert!(ts.contains('T'));
        assert!(ts.ends_with('Z'));
        assert!(ts.len() >= 20);
    }
}
