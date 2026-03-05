//! Music source separation for classcii via SCNet.
//!
//! Separates audio into 4 stems (drums, bass, other, vocals) using a Python
//! subprocess bridge to the SCNet PyTorch model. Provides multi-stem playback
//! with per-stem mute/solo/volume, and per-stem FFT feature extraction.

pub mod analysis;
pub mod playback;
pub mod separator;
pub mod stem;
