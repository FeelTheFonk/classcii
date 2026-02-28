use std::path::PathBuf;
use std::sync::Arc;

use af_core::clock::MediaClock;
use anyhow::Result;
use arc_swap::ArcSwap;
use clap::Parser;

pub mod app;
pub mod batch;
pub mod cli;
pub mod generative;
pub mod hotreload;
pub mod pipeline;

fn main() -> Result<()> {
    // 1. Parser CLI
    let cli = cli::Cli::parse();

    // 2. Initialiser le logging
    env_logger::Builder::new()
        .filter_level(cli.log_level.parse().unwrap_or(log::LevelFilter::Warn))
        .init();

    // 3. Valider la source
    let _ = cli.validate_source();

    // Export Par lots
    if let Some(folder) = cli.batch_folder.as_deref() {
        log::info!("Lancement du traitement par lots offline...");
        let config = resolve_config(&cli)?;
        return batch::run_batch_export(
            folder,
            cli.audio.as_ref(),
            cli.batch_out.as_deref(),
            config,
            cli.fps.unwrap_or(30),
        );
    }

    // 4. Charger la config
    let mut config = resolve_config(&cli)?;

    // 4b. Appliquer les overrides CLI
    if let Some(ref mode) = cli.mode {
        config.render_mode = match mode.as_str() {
            "ascii" => af_core::config::RenderMode::Ascii,
            "halfblock" => af_core::config::RenderMode::HalfBlock,
            "braille" => af_core::config::RenderMode::Braille,
            "quadrant" => af_core::config::RenderMode::Quadrant,
            "sextant" => af_core::config::RenderMode::Sextant,
            "octant" => af_core::config::RenderMode::Octant,
            _ => {
                log::warn!("Mode inconnu '{mode}', utilisation du défaut.");
                config.render_mode
            }
        };
    }
    if let Some(fps) = cli.fps {
        config.target_fps = fps;
    }
    if cli.no_color {
        config.color_enabled = false;
    }

    let config = Arc::new(ArcSwap::from_pointee(config));

    // 5. Lancer le hot-reload config (thread interne notify)
    let _watcher = hotreload::spawn_config_watcher(&cli.config, &config)?;

    // 6. Démarrer le thread audio (si --audio fourni)
    let media_clock = Arc::new(MediaClock::new(0));
    let (audio_output, audio_cmd_tx) = init_audio(&cli, &config, &media_clock);

    // 7. Démarrer le source thread (si vidéo/webcam/procédural)
    let has_audio = audio_output.is_some();
    let video_clock = if has_audio {
        Some(Arc::clone(&media_clock))
    } else {
        None
    };
    #[cfg(feature = "video")]
    let (initial_frame, frame_rx, video_cmd_tx) = pipeline::start_source(&cli, video_clock)?;
    #[cfg(not(feature = "video"))]
    let (initial_frame, frame_rx) = pipeline::start_source(&cli, video_clock)?;

    // 8. Initialiser le terminal ratatui
    let terminal = ratatui::init();

    // 9. Construire l'App
    #[cfg(feature = "video")]
    let mut app_instance =
        app::App::new(config, audio_output, frame_rx, video_cmd_tx, audio_cmd_tx)?;
    #[cfg(not(feature = "video"))]
    let mut app_instance = app::App::new(config, audio_output, frame_rx, audio_cmd_tx)?;
    if let Some(frame) = initial_frame {
        app_instance.current_frame = Some(frame);
    }
    if has_audio {
        app_instance.media_clock = Some(media_clock);
    }

    // 9b. Set initial loaded file names from CLI args
    if let Some(ref path) = cli.image {
        app_instance.loaded_visual_name =
            path.file_name().and_then(|n| n.to_str()).map(String::from);
    } else if let Some(ref path) = cli.video {
        app_instance.loaded_visual_name =
            path.file_name().and_then(|n| n.to_str()).map(String::from);
    }
    if let Some(ref audio_arg) = cli.audio {
        let p = std::path::Path::new(audio_arg.as_str());
        app_instance.loaded_audio_name = p.file_name().and_then(|n| n.to_str()).map(String::from);
    }

    // 10. Boucle principale
    let result = app_instance.run(terminal);

    // 11. Restaurer le terminal (TOUJOURS, même en cas d'erreur)
    ratatui::restore();

    result
}

/// Initialize audio pipeline from CLI args (--audio or video fallback).
fn init_audio(
    cli: &cli::Cli,
    config: &Arc<ArcSwap<af_core::config::RenderConfig>>,
    clock: &Arc<MediaClock>,
) -> (
    Option<triple_buffer::Output<af_core::frame::AudioFeatures>>,
    Option<flume::Sender<af_audio::state::AudioCommand>>,
) {
    if let Some(ref audio_arg) = cli.audio {
        match pipeline::start_audio(audio_arg, config, Arc::clone(clock)) {
            Ok((output, tx)) => (Some(output), tx),
            Err(e) => {
                log::warn!("Audio non disponible : {e}");
                (None, None)
            }
        }
    } else if let Some(ref video_arg) = cli.video {
        match pipeline::start_audio(&video_arg.to_string_lossy(), config, Arc::clone(clock)) {
            Ok((output, tx)) => {
                log::info!("Piste audio de la vidéo chargée avec succès.");
                (Some(output), tx)
            }
            Err(e) => {
                log::info!("Pas de piste audio gérée dans la vidéo : {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    }
}

/// Resolve config: preset takes priority over --config.
fn resolve_config(cli: &cli::Cli) -> Result<af_core::config::RenderConfig> {
    if let Some(ref name) = cli.preset {
        let path = PathBuf::from(format!("config/presets/{name}.toml"));
        if path.exists() {
            af_core::config::load_config(&path)
        } else {
            anyhow::bail!(
                "Preset inconnu : {name}. Voir config/presets/ (ex: 01_cyber_braille, 02_matrix, 07_neon_abyss)"
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
