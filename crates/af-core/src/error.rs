use thiserror::Error;

/// Errors originating from the core module.
#[derive(Error, Debug)]
pub enum CoreError {
    /// Invalid configuration value or structure.
    #[error("Configuration invalide : {0}")]
    Config(String),

    /// Referenced file does not exist.
    #[error("Fichier introuvable : {path}")]
    FileNotFound {
        /// Path that was not found.
        path: String,
    },

    /// Unsupported file or data format.
    #[error("Format non supporté : {format}")]
    UnsupportedFormat {
        /// The format string that is unsupported.
        format: String,
    },

    /// Invalid width/height dimensions.
    #[error("Dimensions invalides : {width}×{height}")]
    InvalidDimensions {
        /// Width value.
        width: u32,
        /// Height value.
        height: u32,
    },
}
