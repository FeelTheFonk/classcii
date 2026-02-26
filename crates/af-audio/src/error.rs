use thiserror::Error;

/// Errors originating from the audio module.
#[derive(Error, Debug)]
pub enum AudioError {
    /// No audio input device found.
    #[error("Aucun périphérique audio d'entrée trouvé")]
    NoInputDevice,

    /// No audio output device found.
    #[error("Aucun périphérique audio de sortie trouvé")]
    NoOutputDevice,

    /// Unsupported audio format.
    #[error("Format audio non supporté : {0}")]
    UnsupportedFormat(String),

    /// Audio stream error.
    #[error("Erreur de stream audio : {0}")]
    StreamError(String),

    /// Audio decode error.
    #[error("Erreur de décodage : {0}")]
    DecodeError(String),
}
