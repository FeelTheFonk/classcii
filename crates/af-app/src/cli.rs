use std::path::PathBuf;

use clap::Parser;

/// clasSCII — Audio-reactive ASCII art engine.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Source visuelle : chemin vers une image (PNG, JPEG, BMP, GIF).
    #[arg(long)]
    pub image: Option<PathBuf>,

    /// Source visuelle : chemin vers une vidéo. Requiert --features video.
    #[arg(long)]
    pub video: Option<PathBuf>,

    /// Générateur procédural : "mandelbrot".
    #[arg(long)]
    pub procedural: Option<String>,

    /// Source audio : "mic" pour microphone, ou chemin vers fichier audio.
    #[arg(long)]
    pub audio: Option<String>,

    /// Dossier contenant les médias pour l'export génératif par lots.
    #[arg(long)]
    pub batch_folder: Option<PathBuf>,

    /// Fichier de destination final MP4. Requis si --batch-folder est utilisé.
    #[arg(long)]
    pub batch_out: Option<PathBuf>,

    /// Fichier de configuration TOML. Défaut : config/default.toml.
    #[arg(short, long, default_value = "config/default.toml")]
    pub config: PathBuf,

    /// Charger un preset nommé (ignore --config).
    #[arg(long)]
    pub preset: Option<String>,

    /// Multiplicateur d'échelle pour la rasterisation par lots (Upscaling typographique).
    #[arg(long)]
    pub export_scale: Option<f32>,

    /// Mode de rendu initial : ascii, halfblock, braille, quadrant, sextant, octant.
    #[arg(long)]
    pub mode: Option<String>,

    /// FPS cible (30 ou 60).
    #[arg(long)]
    pub fps: Option<u32>,

    /// Désactiver la couleur.
    #[arg(long, default_value_t = false)]
    pub no_color: bool,

    /// Niveau de log : error, warn, info, debug, trace.
    #[arg(long, default_value = "warn")]
    pub log_level: String,
}

impl Cli {
    /// Validate that exactly one visual source is provided.
    ///
    /// # Errors
    /// Returns an error if zero or more than one source is specified.
    pub fn validate_source(&self) -> anyhow::Result<()> {
        let count = usize::from(self.image.is_some())
            + usize::from(self.video.is_some())
            + usize::from(self.procedural.is_some())
            + usize::from(self.batch_folder.is_some());

        if count == 0 {
            anyhow::bail!(
                "Aucune source visuelle spécifiée. Utilisez --image, --video, --procedural, ou --batch-folder."
            );
        }
        if count > 1 {
            anyhow::bail!(
                "Une seule source visuelle à la fois. Spécifiez --image, --video, --procedural, OU --batch-folder."
            );
        }

        // We no longer require `batch_out` and `audio` to be set explicitly.
        // They will be auto-discovered/auto-generated in `run_batch_export`.
        Ok(())
    }
}
