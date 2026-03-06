use std::sync::Arc;

use af_core::clock::MediaClock;
use af_core::paths::AppPaths;
use anyhow::Result;
use arc_swap::ArcSwap;
use clap::Parser;

pub mod app;
pub mod batch;
pub mod cli;
pub mod creation;
pub mod generative;
pub mod hotreload;
pub mod pipeline;

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    // 1. Parser CLI
    let cli = cli::Cli::parse();

    // 2. Initialiser le logging
    env_logger::Builder::new()
        .filter_level(cli.log_level.parse().unwrap_or(log::LevelFilter::Warn))
        .init();

    // 2a. Resolve all runtime paths once
    let paths = AppPaths::resolve();
    af_core::paths::init_tool_paths(&paths);
    log::info!("Base dir: {}", paths.base_dir.display());

    // 2b. --init : extract embedded configs to disk, then exit
    if cli.init {
        return extract_embedded_configs(&paths);
    }

    // 2c. --preset-list : scan & display available presets, then exit
    if cli.preset_list {
        list_presets(&paths);
        return Ok(());
    }

    // 2d. --workflow-list : scan & display saved workflows, then exit
    if cli.workflow_list {
        return list_workflows_cli(&paths);
    }

    // 3. Valider la source
    cli.validate_source()?;

    // 3b. --load-workflow : override config and source from saved workflow
    let loaded_wf = if let Some(ref wf_path) = cli.load_workflow {
        if cli.preset.is_some() {
            log::warn!("--load-workflow surcharge --preset. Le preset sera ignoré.");
        }
        let wf = af_core::workflow_io::load_workflow(wf_path)?;
        log::info!(
            "Workflow loaded: v{} from {}",
            wf.manifest.version,
            wf.dir.display()
        );
        Some(wf)
    } else {
        None
    };

    // Export Par lots
    if let Some(folder) = cli.batch_folder.as_deref() {
        log::info!("Lancement du traitement par lots offline...");
        let preset_all = cli.preset.as_deref() == Some("all");
        let config = if let Some(ref wf) = loaded_wf {
            wf.config.clone()
        } else if preset_all {
            // --preset all : start from default config, presets are loaded internally
            af_core::config::RenderConfig::default()
        } else {
            resolve_config(&cli, &paths)?
        };

        // Resolve audio: workflow may provide stem WAVs or original audio path
        let audio_arg = cli.audio.clone().or_else(|| {
            loaded_wf
                .as_ref()
                .and_then(|wf| wf.source.audio_path.as_ref())
                .map(|p| p.to_string_lossy().into_owned())
        });

        let result = batch::run_batch_export(
            folder,
            audio_arg.as_ref(),
            cli.batch_out.as_deref(),
            config.clone(),
            cli.fps.unwrap_or(30),
            cli.export_scale,
            preset_all,
            cli.seed,
            cli.preset_duration.unwrap_or(15.0),
            cli.crossfade_ms,
            cli.mutation_intensity.unwrap_or(1.0),
            cli.stems,
            &cli.stem_model,
            cli.save_workflow.as_deref(),
            &paths,
        );

        return result;
    }

    // 4. Charger la config
    let (mut config, config_file_path) = if let Some(ref wf) = loaded_wf {
        (wf.config.clone(), None)
    } else {
        resolve_config_with_path(&cli, &paths)?
    };

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

    // 5. Lancer le hot-reload config (seulement si fichier externe résolu)
    let _watcher = if let Some(ref path) = config_file_path {
        Some(hotreload::spawn_config_watcher(path, &config)?)
    } else {
        log::info!("Config embarquée utilisée — hot-reload désactivé.");
        None
    };

    // 6. Démarrer le thread audio (si --audio fourni)
    let media_clock = Arc::new(MediaClock::new(0));
    let (audio_output, audio_cmd_tx) = init_audio(&cli, &config, &media_clock);

    // 7. Démarrer le source thread (si vidéo/procédural)
    let has_audio = audio_output.is_some();
    let video_clock = if has_audio {
        Some(Arc::clone(&media_clock))
    } else {
        None
    };
    #[cfg(feature = "video")]
    let (initial_frame, frame_rx, video_cmd_tx) =
        pipeline::start_source(&cli, video_clock, Arc::clone(&config))?;
    #[cfg(not(feature = "video"))]
    let (initial_frame, frame_rx) = pipeline::start_source(&cli, video_clock, Arc::clone(&config))?;

    // 8. Initialiser le terminal ratatui
    let terminal = ratatui::init();
    // Enable mouse capture for camera controls (drag, wheel zoom)
    crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;

    // 9. Construire l'App
    let paths = Arc::new(paths);
    #[cfg(feature = "video")]
    let mut app_instance = app::App::new(
        config,
        audio_output,
        frame_rx,
        video_cmd_tx,
        audio_cmd_tx,
        Arc::clone(&paths),
    )?;
    #[cfg(not(feature = "video"))]
    let mut app_instance = app::App::new(
        config,
        audio_output,
        frame_rx,
        audio_cmd_tx,
        Arc::clone(&paths),
    )?;
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
        app_instance.loaded_audio_path = Some(p.to_path_buf());
    } else if let Some(ref video_arg) = cli.video {
        // Video also provides audio for stem separation
        app_instance.loaded_audio_path = Some(video_arg.clone());
    }

    // 10. Boucle principale
    let result = app_instance.run(terminal);

    // 11. Restaurer le terminal (TOUJOURS, même en cas d'erreur)
    crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture).ok();
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

/// List available presets: external (disk) + embedded (built-in).
fn list_presets(paths: &AppPaths) {
    use std::collections::BTreeSet;

    let mut names = BTreeSet::new();
    let mut external = BTreeSet::new();

    // Scan disk presets
    if paths.presets_dir.is_dir()
        && let Ok(entries) = std::fs::read_dir(&paths.presets_dir)
    {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
            {
                names.insert(stem.to_string());
                external.insert(stem.to_string());
            }
        }
    }

    // Add embedded presets
    for name in af_core::embedded::preset_names() {
        names.insert(name.to_string());
    }

    println!("Available presets ({}):", names.len());
    for name in &names {
        let tag = if external.contains(name) {
            ""
        } else {
            " [built-in]"
        };
        println!("  {name}{tag}");
    }
}

/// Scan saved workflows, print details, and exit.
fn list_workflows_cli(paths: &AppPaths) -> Result<()> {
    let entries = af_core::workflow_io::list_workflows_detailed_in(&paths.workflows_dir)?;
    if entries.is_empty() {
        println!("No saved workflows found.");
        println!("  Save dir: {}", paths.workflows_dir.display());
        return Ok(());
    }

    println!("Saved workflows ({}):", entries.len());
    for e in &entries {
        let stems_tag = if e.has_stems { " [stems]" } else { "" };
        let tl_tag = if e.has_timeline { " [timeline]" } else { "" };
        let desc = if e.description.is_empty() {
            String::new()
        } else {
            format!(" - {}", e.description)
        };
        println!(
            "  {}{}{} ({}){}",
            e.name, stems_tag, tl_tag, e.created_at, desc
        );
    }

    Ok(())
}

/// Resolve config with embedded fallback. Returns the config only.
fn resolve_config(cli: &cli::Cli, paths: &AppPaths) -> Result<af_core::config::RenderConfig> {
    resolve_config_with_path(cli, paths).map(|(cfg, _)| cfg)
}

/// Resolve config with embedded fallback.
/// Returns `(config, Option<PathBuf>)` where path is the external file (for hot-reload).
fn resolve_config_with_path(
    cli: &cli::Cli,
    paths: &AppPaths,
) -> Result<(af_core::config::RenderConfig, Option<std::path::PathBuf>)> {
    // 1. Explicit --config path
    if let Some(ref explicit) = cli.config {
        let cfg = af_core::config::load_config(explicit)?;
        return Ok((cfg, Some(explicit.clone())));
    }

    // 2. --preset <name>: try disk, then embedded
    if let Some(ref name) = cli.preset {
        if let Some(path) = paths.preset_path(name) {
            let cfg = af_core::config::load_config(&path)?;
            return Ok((cfg, Some(path)));
        }
        if let Some(content) = af_core::embedded::find_preset(name) {
            let cfg = af_core::config::load_config_from_str(content)?;
            log::info!("Preset '{name}' chargé depuis l'embarqué.");
            return Ok((cfg, None));
        }
        let available: Vec<&str> = af_core::embedded::preset_names();
        anyhow::bail!(
            "Preset inconnu : {name}.\nDisponibles : {}",
            available.join(", ")
        );
    }

    // 3. Default config: try disk, then embedded
    if paths.has_external_config() {
        let cfg = af_core::config::load_config(&paths.default_config)?;
        return Ok((cfg, Some(paths.default_config.clone())));
    }

    log::info!("Config embarquée par défaut utilisée.");
    let cfg = af_core::config::load_config_from_str(af_core::embedded::DEFAULT_CONFIG)?;
    Ok((cfg, None))
}

/// Extract embedded configs to disk for user customization.
fn extract_embedded_configs(paths: &AppPaths) -> Result<()> {
    let config_dir = paths.base_dir.join("config");
    let presets_dir = config_dir.join("presets");

    std::fs::create_dir_all(&presets_dir)?;

    // Extract default.toml
    let default_path = config_dir.join("default.toml");
    if default_path.exists() {
        println!("  SKIP  {}", default_path.display());
    } else {
        std::fs::write(&default_path, af_core::embedded::DEFAULT_CONFIG)?;
        println!("  WRITE {}", default_path.display());
    }

    // Extract all presets
    for (name, content) in af_core::embedded::EMBEDDED_PRESETS {
        let dest = presets_dir.join(format!("{name}.toml"));
        if dest.exists() {
            println!("  SKIP  {}", dest.display());
        } else {
            std::fs::write(&dest, content)?;
            println!("  WRITE {}", dest.display());
        }
    }

    println!("\nConfigs extraites dans {}", config_dir.display());
    println!("Éditez-les pour personnaliser. Hot-reload actif au prochain lancement.");

    Ok(())
}
