/// Configuration, types, and shared structures for clasSCII.
///
/// This crate contains all shared types, traits, and configuration logic
/// used across the clasSCII workspace.
pub mod charset;
pub mod color;
pub mod config;
pub mod frame;
pub mod traits;

pub use charset::LuminanceLut;
pub use config::RenderConfig;
pub use frame::{AsciiCell, AsciiGrid, AudioFeatures, FrameBuffer};
