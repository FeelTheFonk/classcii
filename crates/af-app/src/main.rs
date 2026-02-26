use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use arc_swap::ArcSwap;
use clap::Parser;

pub mod app;
pub mod cli;
pub mod hotreload;
pub mod pipeline;

fn main() -> Result<()> {
    // 1. Parser CLI
    let cli = cli::Cli::parse();

    // 2. Initialiser le logging
    env_logger::Builder::new()
        .filter_level(
            cli.log_level
                .parse()
                .unwrap_or(log::LevelFilter::Warn),
        )
        .init();

    // 3. Valider la source (sauf si aucune n'est spécifiée, on affiche l'UI vide)
    // DECISION: On ne force pas la validation en Phase 1 pour permettre le lancement sans source.
    let _ = cli.validate_source();

    // 4. Charger la config
    let config = resolve_config(&cli)?;
    let config = Arc::new(ArcSwap::from_pointee(config));

    // 5. Lancer le hot-reload config (thread interne notify)
    let _watcher = hotreload::spawn_config_watcher(&cli.config, &config)?;

    // 6. Démarrer le thread audio (si --audio fourni)
    let audio_output = if let Some(ref audio_arg) = cli.audio {
        match pipeline::start_audio(audio_arg, &config) {
            Ok(output) => Some(output),
            Err(e) => {
                log::warn!("Audio non disponible : {e}");
                None
            }
        }
    } else {
        None
    };

    // 7. Démarrer le source thread (si vidéo/webcam/procédural)
    let (initial_frame, frame_rx) = pipeline::start_source(&cli)?;

    // 8. Initialiser le terminal ratatui
    let terminal = ratatui::init();

    // 9. Construire l'App
    let mut app_instance = app::App::new(config, audio_output, frame_rx)?;
    if let Some(frame) = initial_frame {
        app_instance.current_frame = Some(frame);
    }

    // 10. Boucle principale
    let result = app_instance.run(terminal);

    // 11. Restaurer le terminal (TOUJOURS, même en cas d'erreur)
    ratatui::restore();

    result
}

/// Resolve config: preset takes priority over --config.
fn resolve_config(cli: &cli::Cli) -> Result<af_core::config::RenderConfig> {
    if let Some(ref name) = cli.preset {
        let path = PathBuf::from(format!("config/presets/{name}.toml"));
        if path.exists() {
            af_core::config::load_config(&path)
        } else {
            anyhow::bail!(
                "Preset inconnu : {name}. Disponibles : ambient, aggressive, minimal, retro, psychedelic"
            );
        }
    } else if cli.config.exists() {
        af_core::config::load_config(&cli.config)
    } else {
        log::warn!(
            "Config introuvable : {}. Utilisation des défauts.",
            cli.config.display()
        );
        Ok(af_core::config::RenderConfig::default())
    }
}
