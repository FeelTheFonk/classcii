use std::sync::Arc;

use crate::config::RenderConfig;
use crate::frame::{AsciiGrid, AudioFeatures, FrameBuffer};

/// Fournit des frames visuelles au pipeline.
///
/// Implémenté par : `ImageSource`, `VideoSource`, `WebcamSource`, `ProceduralSource`.
///
/// # Example
/// ```
/// use af_core::traits::Source;
/// use af_core::frame::FrameBuffer;
/// use std::sync::Arc;
///
/// struct DummySource;
/// impl Source for DummySource {
///     fn next_frame(&mut self) -> Option<Arc<FrameBuffer>> { None }
///     fn native_size(&self) -> (u32, u32) { (0, 0) }
///     fn is_live(&self) -> bool { false }
/// }
/// ```
pub trait Source: Send + 'static {
    /// Retourne la prochaine frame disponible.
    ///
    /// Retourne `None` si la source est épuisée (fin de vidéo).
    /// Ne bloque JAMAIS — retourne la dernière frame connue si pas de nouvelle.
    fn next_frame(&mut self) -> Option<Arc<FrameBuffer>>;

    /// Dimensions natives de la source (avant resize).
    fn native_size(&self) -> (u32, u32);

    /// Indique si la source est infinie (webcam, procédural) ou finie (fichier).
    fn is_live(&self) -> bool;
}

/// Transforme une frame pixel en une grille de cellules ASCII.
///
/// Le pipeline peut chaîner plusieurs `Processor`s.
///
/// # Example
/// ```
/// use af_core::traits::Processor;
/// use af_core::frame::{FrameBuffer, AsciiGrid, AudioFeatures};
/// use af_core::config::RenderConfig;
///
/// struct DummyProcessor;
/// impl Processor for DummyProcessor {
///     fn process(&self, _input: &FrameBuffer, _audio: Option<&AudioFeatures>,
///                _config: &RenderConfig, _output: &mut AsciiGrid) {}
///     fn name(&self) -> &'static str { "dummy" }
/// }
/// ```
pub trait Processor: Send + Sync {
    /// Traite une frame et écrit le résultat dans `output`.
    ///
    /// CONTRAT : ne doit PAS allouer. `output` est pré-alloué et réutilisé.
    fn process(
        &self,
        input: &FrameBuffer,
        audio: Option<&AudioFeatures>,
        config: &RenderConfig,
        output: &mut AsciiGrid,
    );

    /// Nom lisible pour le debug/UI.
    fn name(&self) -> &'static str;
}

/// Analyse un buffer d'échantillons audio et produit des features.
///
/// # Example
/// ```
/// use af_core::traits::AudioAnalyzer;
/// use af_core::frame::AudioFeatures;
///
/// struct DummyAnalyzer;
/// impl AudioAnalyzer for DummyAnalyzer {
///     fn analyze(&mut self, _samples: &[f32], _features: &mut AudioFeatures) {}
/// }
/// ```
pub trait AudioAnalyzer: Send + 'static {
    /// Traite un bloc d'échantillons (mono, f32, normalisé [-1, 1]).
    ///
    /// CONTRAT : ne doit PAS allouer. Tous les buffers internes sont
    /// pré-alloués dans le constructeur.
    fn analyze(&mut self, samples: &[f32], features: &mut AudioFeatures);
}
