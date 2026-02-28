pub mod mandelbrot;

use af_core::traits::Source;
use mandelbrot::MandelbrotSource;

use af_core::config::RenderConfig;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Fabrique la source procédurale choisie par l'utilisateur.
///
/// # Errors
/// Retourne une erreur si le type n'est pas reconnu.
pub fn create_procedural_source(
    target_type: &str,
    width: u32,
    height: u32,
    config: Arc<ArcSwap<RenderConfig>>,
) -> anyhow::Result<Box<dyn Source>> {
    match target_type.to_lowercase().as_str() {
        "mandelbrot" => Ok(Box::new(MandelbrotSource::new(width, height, config))),
        _ => anyhow::bail!("Générateur procédural inconnu : {target_type}. Supporté : mandelbrot"),
    }
}
