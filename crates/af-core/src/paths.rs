use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resolved ffmpeg binary path (set once at startup via [`init_tool_paths`]).
static FFMPEG_BIN: OnceLock<PathBuf> = OnceLock::new();
/// Resolved ffprobe binary path (set once at startup via [`init_tool_paths`]).
static FFPROBE_BIN: OnceLock<PathBuf> = OnceLock::new();

/// Initialize the global ffmpeg/ffprobe paths from `AppPaths`.
/// Must be called once at startup. Safe to call multiple times (idempotent).
pub fn init_tool_paths(paths: &AppPaths) {
    let _ = FFMPEG_BIN.set(paths.ffmpeg());
    let _ = FFPROBE_BIN.set(paths.ffprobe());
}

/// Get the resolved ffmpeg binary path. Falls back to `"ffmpeg"` if not initialized.
#[must_use]
pub fn ffmpeg_bin() -> &'static Path {
    FFMPEG_BIN
        .get()
        .map_or(Path::new("ffmpeg"), |p| p.as_path())
}

/// Get the resolved ffprobe binary path. Falls back to `"ffprobe"` if not initialized.
#[must_use]
pub fn ffprobe_bin() -> &'static Path {
    FFPROBE_BIN
        .get()
        .map_or(Path::new("ffprobe"), |p| p.as_path())
}

/// Centralized path resolution for the entire application.
///
/// All runtime paths (config, presets, workflows, bundle) are derived from
/// a single `base_dir`, resolved once at startup. This replaces all
/// hardcoded relative paths scattered across crates.
#[derive(Clone, Debug)]
pub struct AppPaths {
    /// Root directory from which all other paths are derived.
    pub base_dir: PathBuf,
    /// `base_dir/config/default.toml`
    pub default_config: PathBuf,
    /// `base_dir/config/presets/`
    pub presets_dir: PathBuf,
    /// `base_dir/workflows/`
    pub workflows_dir: PathBuf,
    /// `base_dir/bundle/` — present only if the directory exists.
    pub bundle_dir: Option<PathBuf>,
}

impl AppPaths {
    /// Build `AppPaths` from a resolved base directory.
    fn from_base(base: PathBuf) -> Self {
        let bundle_candidate = base.join("bundle");
        let bundle_dir = if bundle_candidate.is_dir() {
            Some(bundle_candidate)
        } else {
            None
        };
        Self {
            default_config: base.join("config").join("default.toml"),
            presets_dir: base.join("config").join("presets"),
            workflows_dir: base.join("workflows"),
            bundle_dir,
            base_dir: base,
        }
    }

    /// Resolve `AppPaths` using the standard priority order:
    ///
    /// 1. `CLASSCII_HOME` environment variable (explicit override)
    /// 2. Executable's parent directory (portable/release mode)
    /// 3. Current working directory (dev/workspace mode)
    ///
    /// The first directory that contains a `config/` subdirectory wins.
    /// If none do, the exe parent is used as base (embedded configs will
    /// serve as fallback).
    #[must_use]
    pub fn resolve() -> Self {
        // 1. Env var override
        if let Ok(home) = std::env::var("CLASSCII_HOME") {
            let p = PathBuf::from(&home);
            if p.is_dir() {
                log::info!("CLASSCII_HOME={home}");
                return Self::from_base(p);
            }
            log::warn!("CLASSCII_HOME={home} is not a valid directory, ignoring.");
        }

        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf));

        let cwd = std::env::current_dir().ok();

        // 2. Exe parent — if it has config/
        if let Some(ref dir) = exe_dir
            && dir.join("config").is_dir()
        {
            return Self::from_base(dir.clone());
        }

        // 3. CWD — if it has config/ (dev mode)
        if let Some(ref dir) = cwd
            && dir.join("config").is_dir()
        {
            return Self::from_base(dir.clone());
        }

        // No config/ found anywhere — use exe parent as base (embedded fallback)
        let base = exe_dir.or(cwd).unwrap_or_else(|| PathBuf::from("."));
        Self::from_base(base)
    }

    /// Resolve the path to a named preset file on disk.
    /// Returns `None` if the file does not exist.
    #[must_use]
    pub fn preset_path(&self, name: &str) -> Option<PathBuf> {
        let p = self.presets_dir.join(format!("{name}.toml"));
        if p.is_file() { Some(p) } else { None }
    }

    /// Resolve the ffmpeg binary path.
    /// Checks bundle first, then falls back to bare name (PATH lookup).
    #[must_use]
    pub fn ffmpeg(&self) -> PathBuf {
        self.resolve_bundle_tool("ffmpeg")
    }

    /// Resolve the ffprobe binary path.
    #[must_use]
    pub fn ffprobe(&self) -> PathBuf {
        self.resolve_bundle_tool("ffprobe")
    }

    /// Resolve a tool binary: bundle/tool[.exe] → bare name.
    fn resolve_bundle_tool(&self, name: &str) -> PathBuf {
        if let Some(ref bundle) = self.bundle_dir {
            let bin_name = if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.to_string()
            };
            let bundled = bundle.join(&bin_name);
            if bundled.is_file() {
                return bundled;
            }
        }
        PathBuf::from(name)
    }

    /// Resolve the Python binary for stem separation.
    /// Checks bundle/stems/.venv first, then base_dir/.venv.
    #[must_use]
    pub fn python_bin(&self) -> PathBuf {
        let venv_rel = if cfg!(windows) {
            Path::new(".venv/Scripts/python.exe")
        } else {
            Path::new(".venv/bin/python")
        };

        if let Some(ref bundle) = self.bundle_dir {
            let bundled = bundle.join("stems").join(venv_rel);
            if bundled.is_file() {
                return bundled;
            }
        }

        self.base_dir.join(venv_rel)
    }

    /// Resolve the SCNet directory for stem separation.
    /// Checks bundle/stems/SCNet first, then base_dir/ext/SCNet.
    #[must_use]
    pub fn scnet_dir(&self) -> PathBuf {
        if let Some(ref bundle) = self.bundle_dir {
            let bundled = bundle.join("stems").join("SCNet");
            if bundled.is_dir() {
                return bundled;
            }
        }

        self.base_dir.join("ext").join("SCNet")
    }

    /// Check if external preset files exist on disk.
    #[must_use]
    pub fn has_external_presets(&self) -> bool {
        self.presets_dir.is_dir()
    }

    /// Check if external default config exists on disk.
    #[must_use]
    pub fn has_external_config(&self) -> bool {
        self.default_config.is_file()
    }
}
