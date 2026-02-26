use std::path::Path;
use std::sync::Arc;

use af_core::config::RenderConfig;
use anyhow::Result;
use arc_swap::ArcSwap;
use notify::{Event, EventKind, RecursiveMode, Watcher};

/// Lance un thread qui surveille le fichier config et met à jour l'ArcSwap.
///
/// Retourne le Watcher (doit rester vivant tant que l'app tourne).
///
/// # Errors
/// Returns an error if the watcher cannot be created or the path cannot be watched.
///
/// # Example
/// ```no_run
/// use std::sync::Arc;
/// use arc_swap::ArcSwap;
/// use af_core::config::RenderConfig;
/// use af_app::hotreload::spawn_config_watcher;
/// use std::path::Path;
///
/// let config = Arc::new(ArcSwap::from_pointee(RenderConfig::default()));
/// let _watcher = spawn_config_watcher(Path::new("config/default.toml"), &config);
/// ```
pub fn spawn_config_watcher(
    config_path: &Path,
    config: &Arc<ArcSwap<RenderConfig>>,
) -> Result<impl Watcher + use<>> {
    let config = Arc::clone(config);
    let path = config_path.to_path_buf();

    let mut watcher =
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Modify(_)) {
                    match af_core::config::load_config(&path) {
                        Ok(new_config) => {
                            config.store(Arc::new(new_config));
                            log::info!("Config rechargée depuis {}", path.display());
                        }
                        Err(e) => {
                            log::warn!("Erreur de rechargement config : {e}");
                            // On garde l'ancienne config. Pas de panic.
                        }
                    }
                }
            }
        })?;

    watcher.watch(config_path, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
