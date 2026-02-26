/// Configuration, types, and shared structures for clasSCII.
///
/// This crate contains all shared types, traits, and configuration logic
/// used across the clasSCII workspace.

pub mod charset;
pub mod color;
pub mod config;
pub mod error;
pub mod frame;
pub mod traits;

pub use charset::LuminanceLut;
pub use config::RenderConfig;
pub use error::CoreError;
pub use frame::{AsciiCell, AsciiGrid, AudioFeatures, FrameBuffer};

/// Re-exports pour accès par chemin sémantique.
pub mod grid {
    pub use crate::frame::{AsciiCell, AsciiGrid};
}

/// Re-exports for audio types.
pub mod audio {
    pub use crate::frame::AudioFeatures;
}
