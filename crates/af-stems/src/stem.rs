use std::path::PathBuf;
use std::sync::Arc;

use af_core::frame::AudioFeatures;
use serde::{Deserialize, Serialize};

/// Number of stems produced by SCNet.
pub const STEM_COUNT: usize = 4;

/// Identifies one of the four stems.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StemId {
    Drums,
    Bass,
    Other,
    Vocals,
}

impl StemId {
    /// All stem IDs in the order SCNet produces them.
    pub const ALL: [Self; STEM_COUNT] = [Self::Drums, Self::Bass, Self::Other, Self::Vocals];

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Drums => "Drums",
            Self::Bass => "Bass",
            Self::Other => "Other",
            Self::Vocals => "Vocals",
        }
    }

    /// Short 3-char label for compact display.
    #[must_use]
    pub fn short(self) -> &'static str {
        match self {
            Self::Drums => "DRM",
            Self::Bass => "BAS",
            Self::Other => "OTH",
            Self::Vocals => "VOC",
        }
    }

    /// Distinctive RGB color for TUI display.
    #[must_use]
    pub fn color(self) -> (u8, u8, u8) {
        match self {
            Self::Drums => (230, 80, 60),   // Red
            Self::Bass => (230, 170, 40),   // Orange/Yellow
            Self::Other => (60, 200, 90),   // Green
            Self::Vocals => (80, 180, 230), // Cyan
        }
    }

    /// Index into arrays (matches SCNet source order).
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Drums => 0,
            Self::Bass => 1,
            Self::Other => 2,
            Self::Vocals => 3,
        }
    }

    /// SCNet source name string (for file naming).
    #[must_use]
    pub fn scnet_name(self) -> &'static str {
        match self {
            Self::Drums => "drums",
            Self::Bass => "bass",
            Self::Other => "other",
            Self::Vocals => "vocals",
        }
    }
}

/// Per-stem user-controllable state.
#[derive(Clone, Debug)]
pub struct StemState {
    pub id: StemId,
    pub muted: bool,
    pub solo: bool,
    pub volume: f32,
    pub visible: bool,
}

impl StemState {
    #[must_use]
    pub fn new(id: StemId) -> Self {
        Self {
            id,
            muted: false,
            solo: false,
            volume: 1.0,
            visible: true,
        }
    }
}

/// Decoded audio data for a single stem.
pub struct StemData {
    pub id: StemId,
    /// Mono f32 samples for FFT analysis.
    pub samples: Arc<Vec<f32>>,
    pub sample_rate: u32,
}

/// Complete set of 4 separated stems.
pub struct StemSet {
    pub stems: [StemData; STEM_COUNT],
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub source_path: PathBuf,
}

/// Per-stem audio features, published via triple buffer.
#[derive(Clone, Copy, Default)]
pub struct StemFeatures {
    pub features: [AudioFeatures; STEM_COUNT],
}

/// Metadata from the Python separation script.
#[derive(Deserialize)]
pub struct SeparationMeta {
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_secs: f64,
    pub stems: Vec<String>,
    pub model: String,
    pub elapsed_secs: f64,
}
