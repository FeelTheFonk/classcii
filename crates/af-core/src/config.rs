use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Configuration complète du rendu, hot-rechargeable.
///
/// Sérialisable en TOML. Chaque champ a une valeur par défaut saine.
///
/// # Example
/// ```
/// use af_core::config::RenderConfig;
/// let config = RenderConfig::default();
/// assert_eq!(config.target_fps, 30);
/// ```
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RenderConfig {
    // === Mode de rendu ===
    /// "Ascii" | "Braille" | "HalfBlock" | "Quadrant"
    pub render_mode: RenderMode,
    /// Charset pour le mode ASCII (du plus clair au plus dense).
    pub charset: String,
    /// Index du charset actif parmi les 5 presets built-in.
    pub charset_index: usize,
    /// Activer le tramage optique de luminance (Bayer 8x8 Dithering).
    pub dither_enabled: bool,
    /// Inverser la luminance (pour fond clair).
    pub invert: bool,
    /// Activer la couleur truecolor.
    pub color_enabled: bool,

    // === Conversion ===
    /// Seuil de détection de contours [0.0, 1.0]. 0 = désactivé.
    pub edge_threshold: f32,
    /// Mix edge/fill [0.0, 1.0]. 0 = fill seulement, 1 = edges seulement.
    pub edge_mix: f32,
    /// Activer le shape-matching (plus lent mais meilleure qualité).
    pub shape_matching: bool,
    /// Correction aspect ratio (typiquement 2.0 pour les polices terminal).
    pub aspect_ratio: f32,
    /// Density scale: multiplier for char resolution [0.25, 4.0]. 1.0 = 1:1 with canvas.
    pub density_scale: f32,

    // === Couleur ===
    /// Méthode de mapping couleur.
    pub color_mode: ColorMode,
    /// Saturation boost [0.0, 2.0]. 1.0 = neutre.
    pub saturation: f32,
    /// Contraste [0.0, 2.0]. 1.0 = neutre.
    pub contrast: f32,
    /// Brightness offset [-1.0, 1.0]. 0.0 = neutre.
    pub brightness: f32,
    /// Background rendering style.
    pub bg_style: BgStyle,

    // === Audio-réactivité ===
    /// Mapping des features audio vers les paramètres visuels.
    pub audio_mappings: Vec<AudioMapping>,
    /// Lissage global des features audio [0.0, 1.0]. 0 = brut, 0.9 = très lissé.
    pub audio_smoothing: f32,
    /// Sensibilité globale de la réactivité audio [0.0, 5.0].
    pub audio_sensitivity: f32,

    // === Post-processing Effects ===
    /// Fade trails decay factor [0.0, 1.0]. 0.0 = disabled.
    pub fade_decay: f32,
    /// Glow intensity factor [0.0, 2.0]. 0.0 = disabled.
    pub glow_intensity: f32,
    /// Zalgo combinatory string intensity. Dynamically driven by audio onset.
    pub zalgo_intensity: f32,

    // === Performance ===
    /// FPS cible. 30 ou 60.
    pub target_fps: u32,

    // === UI ===
    /// Mode plein écran exclusif (sans sidebar/spectrum).
    pub fullscreen: bool,
    /// Afficher le spectre audio sous le visualiseur (si pas en fullscreen).
    pub show_spectrum: bool,
}

pub const AUDIO_SOURCES: &[&str] = &[
    "rms",
    "peak",
    "sub_bass",
    "bass",
    "low_mid",
    "mid",
    "high_mid",
    "presence",
    "brilliance",
    "spectral_centroid",
    "spectral_flux",
    "spectral_flatness",
    "beat_intensity",
    "onset",
    "beat_phase",
    "bpm",
];

pub const AUDIO_TARGETS: &[&str] = &[
    "edge_threshold",
    "edge_mix",
    "contrast",
    "brightness",
    "saturation",
    "density_scale",
    "invert",
    "zalgo_intensity",
];

#[must_use]
pub fn default_true() -> bool {
    true
}

/// A single audio-to-visual parameter mapping.
///
/// # Example
/// ```
/// use af_core::config::AudioMapping;
/// let m = AudioMapping { enabled: true, source: "bass".into(), target: "contrast".into(), amount: 0.5, offset: 0.0 };
/// assert_eq!(m.source, "bass");
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioMapping {
    /// Mapping actif ou désactivé.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Feature source : "rms", "bass", "spectral_flux", "onset", etc.
    pub source: String,
    /// Paramètre cible : "edge_threshold", "contrast", "charset_index", etc.
    pub target: String,
    /// Amplitude du mapping [0.0, ∞).
    pub amount: f32,
    /// Offset ajouté après multiplication.
    pub offset: f32,
}

/// Render mode enumeration.
///
/// # Example
/// ```
/// use af_core::config::RenderMode;
/// let mode = RenderMode::default();
/// assert!(matches!(mode, RenderMode::Ascii));
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum RenderMode {
    /// Standard ASCII character mapping.
    #[default]
    Ascii,
    /// Braille Unicode patterns (2×4 sub-pixels).
    Braille,
    /// Half-block characters (▄/▀) with fg/bg colors.
    HalfBlock,
    /// Quadrant block characters (2×2 sub-pixels).
    Quadrant,
    /// Sextant Unicode 13.0 block characters (2x3 sub-pixels).
    Sextant,
    /// Octant Unicode 16.0 block characters (2x4 sub-pixels).
    Octant,
}

/// Color mapping mode.
///
/// # Example
/// ```
/// use af_core::config::ColorMode;
/// let mode = ColorMode::default();
/// assert!(matches!(mode, ColorMode::HsvBright));
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum ColorMode {
    /// RGB direct du pixel source.
    Direct,
    /// HSV avec V forcé à 1.0 (char encode la luminance).
    #[default]
    HsvBright,
    /// Quantifié sur palette réduite.
    Quantized,
}

/// Background rendering style.
///
/// # Example
/// ```
/// use af_core::config::BgStyle;
/// let bg = BgStyle::default();
/// assert!(matches!(bg, BgStyle::Black));
/// ```
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum BgStyle {
    /// Pure black background.
    #[default]
    Black,
    /// Source pixel color, dimmed.
    SourceDim,
    /// Transparent (terminal default bg).
    Transparent,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Ascii,
            charset: crate::charset::CHARSET_FULL.to_string(),
            charset_index: 0,
            dither_enabled: true,
            invert: false,
            color_enabled: true,
            edge_threshold: 0.3,
            edge_mix: 0.5,
            shape_matching: false,
            aspect_ratio: 2.0,
            density_scale: 1.0,
            color_mode: ColorMode::HsvBright,
            saturation: 1.2,
            contrast: 1.0,
            brightness: 0.0,
            bg_style: BgStyle::Black,
            audio_mappings: vec![
                AudioMapping {
                    enabled: true,
                    source: "bass".into(),
                    target: "edge_threshold".into(),
                    amount: 0.3,
                    offset: 0.0,
                },
                AudioMapping {
                    enabled: true,
                    source: "spectral_flux".into(),
                    target: "contrast".into(),
                    amount: 0.5,
                    offset: 0.0,
                },
                AudioMapping {
                    enabled: true,
                    source: "onset".into(),
                    target: "invert".into(),
                    amount: 1.0,
                    offset: 0.0,
                },
                AudioMapping {
                    enabled: true,
                    source: "rms".into(),
                    target: "brightness".into(),
                    amount: 0.2,
                    offset: 0.0,
                },
            ],
            audio_smoothing: 0.7,
            audio_sensitivity: 1.0,
            fade_decay: 0.3,
            glow_intensity: 0.5,
            zalgo_intensity: 0.0,
            target_fps: 30,
            fullscreen: false,
            show_spectrum: true,
        }
    }
}

/// Structure TOML intermédiaire pour désérialisation avec valeurs optionnelles.
#[derive(Deserialize)]
struct ConfigFile {
    render: RenderSection,
    audio: Option<AudioSection>,
}

/// Render section of the TOML config, all fields optional for partial override.
#[derive(Deserialize)]
struct RenderSection {
    render_mode: Option<RenderMode>,
    charset: Option<String>,
    charset_index: Option<usize>,
    dither_enabled: Option<bool>,
    invert: Option<bool>,
    color_enabled: Option<bool>,
    edge_threshold: Option<f32>,
    edge_mix: Option<f32>,
    shape_matching: Option<bool>,
    aspect_ratio: Option<f32>,
    density_scale: Option<f32>,
    color_mode: Option<ColorMode>,
    saturation: Option<f32>,
    contrast: Option<f32>,
    brightness: Option<f32>,
    bg_style: Option<BgStyle>,
    fade_decay: Option<f32>,
    glow_intensity: Option<f32>,
    zalgo_intensity: Option<f32>,
    target_fps: Option<u32>,
    fullscreen: Option<bool>,
    show_spectrum: Option<bool>,
}

/// Audio section of the TOML config, all fields optional.
#[derive(Deserialize)]
struct AudioSection {
    smoothing: Option<f32>,
    sensitivity: Option<f32>,
    mappings: Option<Vec<AudioMapping>>,
}

/// Charge un fichier TOML et fusionne avec les valeurs par défaut.
///
/// # Errors
/// Returns an error if the file cannot be read or parsed.
///
/// # Example
/// ```no_run
/// use af_core::config::load_config;
/// use std::path::Path;
/// let config = load_config(Path::new("config/default.toml")).unwrap();
/// ```
pub fn load_config(path: &Path) -> Result<RenderConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Impossible de lire {}", path.display()))?;

    let file: ConfigFile = toml::from_str(&content)
        .with_context(|| format!("Erreur de parsing TOML dans {}", path.display()))?;

    let mut config = RenderConfig::default();

    let r = file.render;
    if let Some(v) = r.render_mode {
        config.render_mode = v;
    }
    if let Some(v) = r.charset {
        config.charset = v;
    }
    if let Some(v) = r.charset_index {
        config.charset_index = v;
    }
    if let Some(v) = r.dither_enabled {
        config.dither_enabled = v;
    }
    if let Some(v) = r.invert {
        config.invert = v;
    }
    if let Some(v) = r.color_enabled {
        config.color_enabled = v;
    }
    if let Some(v) = r.edge_threshold {
        config.edge_threshold = v;
    }
    if let Some(v) = r.edge_mix {
        config.edge_mix = v;
    }
    if let Some(v) = r.shape_matching {
        config.shape_matching = v;
    }
    if let Some(v) = r.aspect_ratio {
        config.aspect_ratio = v;
    }
    if let Some(v) = r.density_scale {
        config.density_scale = v;
    }
    if let Some(v) = r.color_mode {
        config.color_mode = v;
    }
    if let Some(v) = r.saturation {
        config.saturation = v;
    }
    if let Some(v) = r.contrast {
        config.contrast = v;
    }
    if let Some(v) = r.brightness {
        config.brightness = v;
    }
    if let Some(v) = r.bg_style {
        config.bg_style = v;
    }
    if let Some(v) = r.fade_decay {
        config.fade_decay = v;
    }
    if let Some(v) = r.glow_intensity {
        config.glow_intensity = v;
    }
    if let Some(v) = r.zalgo_intensity {
        config.zalgo_intensity = v;
    }
    if let Some(v) = r.target_fps {
        config.target_fps = v;
    }
    if let Some(v) = r.fullscreen {
        config.fullscreen = v;
    }
    if let Some(v) = r.show_spectrum {
        config.show_spectrum = v;
    }

    if let Some(a) = file.audio {
        if let Some(v) = a.smoothing {
            config.audio_smoothing = v;
        }
        if let Some(v) = a.sensitivity {
            config.audio_sensitivity = v;
        }
        if let Some(v) = a.mappings {
            config.audio_mappings = v;
        }
    }

    Ok(config)
}
