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
    /// Dithering mode.
    pub dither_mode: DitherMode,
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
    /// Beat flash / strobe intensity [0.0, 2.0]. 0.0 = disabled.
    pub beat_flash_intensity: f32,
    /// Chromatic aberration offset [0.0, 5.0]. 0.0 = disabled.
    pub chromatic_offset: f32,
    /// Wave distortion amplitude [0.0, 1.0]. 0.0 = disabled.
    pub wave_amplitude: f32,
    /// Wave distortion speed [0.5, 10.0].
    pub wave_speed: f32,
    /// Color pulse hue rotation speed [0.0, 5.0]. 0.0 = disabled.
    pub color_pulse_speed: f32,
    /// Scan line gap (0 = off, 2-8).
    pub scanline_gap: u8,
    /// Strobe envelope decay [0.5, 0.99].
    pub strobe_decay: f32,
    /// Temporal stability (anti-flicker) [0.0, 1.0]. 0.0 = disabled.
    pub temporal_stability: f32,

    // === Camera & Geometry ===
    /// Zoom affine (1.0 = normal, >1.0 = zoom in, <1.0 = zoom out/infinite fields).
    pub camera_zoom_amplitude: f32,
    /// Rotation en radians.
    pub camera_rotation: f32,
    /// Décalage horizontal (Pan X) normalisé par rapport à la largeur.
    pub camera_pan_x: f32,
    /// Décalage vertical (Pan Y) normalisé par rapport à la hauteur.
    pub camera_pan_y: f32,

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
    "timbral_brightness",
    "timbral_roughness",
    "onset_envelope",
    "spectral_rolloff",
    "zero_crossing_rate",
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
    "beat_flash_intensity",
    "chromatic_offset",
    "wave_amplitude",
    "color_pulse_speed",
    "fade_decay",
    "glow_intensity",
    "camera_zoom_amplitude",
    "camera_rotation",
    "camera_pan_x",
    "camera_pan_y",
];

#[must_use]
pub fn default_true() -> bool {
    true
}

/// Non-linear mapping curve for audio-to-visual shaping.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub enum MappingCurve {
    /// Identity: y = x.
    #[default]
    Linear,
    /// Exponential: y = x² (suppresses low values, amplifies peaks).
    Exponential,
    /// Gate: y = 0 if x < 0.3, else (x-0.3)/0.7.
    Threshold,
    /// Smoothstep: y = 3x² - 2x³.
    Smooth,
}

/// A single audio-to-visual parameter mapping.
///
/// # Example
/// ```
/// use af_core::config::AudioMapping;
/// let m = AudioMapping { enabled: true, source: "bass".into(), target: "contrast".into(), amount: 0.5, offset: 0.0, curve: Default::default(), smoothing: None };
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
    /// Response curve applied before amount/sensitivity.
    #[serde(default)]
    pub curve: MappingCurve,
    /// Per-mapping smoothing override. None = use global audio_smoothing.
    #[serde(default)]
    pub smoothing: Option<f32>,
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum ColorMode {
    /// RGB direct du pixel source.
    Direct,
    /// HSV avec V forcé à 1.0 (char encode la luminance).
    #[default]
    HsvBright,
    /// Quantifié sur palette réduite.
    Quantized,
    /// Oklab avec L forcé à 1.0 (perceptuellement uniforme).
    Oklab,
}

/// Dithering mode for luminance quantization.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum DitherMode {
    /// Bayer 8×8 ordered dithering (default).
    #[default]
    Bayer8x8,
    /// Blue noise 16×16 dithering (perceptually superior).
    #[serde(alias = "BlueNoise64")]
    BlueNoise16,
    /// No dithering.
    None,
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
            dither_mode: DitherMode::Bayer8x8,
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
                    curve: MappingCurve::Linear,
                    smoothing: None,
                },
                AudioMapping {
                    enabled: true,
                    source: "spectral_flux".into(),
                    target: "contrast".into(),
                    amount: 0.5,
                    offset: 0.0,
                    curve: MappingCurve::Linear,
                    smoothing: None,
                },
                AudioMapping {
                    enabled: true,
                    source: "rms".into(),
                    target: "brightness".into(),
                    amount: 0.2,
                    offset: 0.0,
                    curve: MappingCurve::Linear,
                    smoothing: None,
                },
                AudioMapping {
                    enabled: true,
                    source: "beat_intensity".into(),
                    target: "beat_flash_intensity".into(),
                    amount: 0.3,
                    offset: 0.0,
                    curve: MappingCurve::Smooth,
                    smoothing: None,
                },
                AudioMapping {
                    enabled: true,
                    source: "spectral_centroid".into(),
                    target: "glow_intensity".into(),
                    amount: 0.4,
                    offset: 0.0,
                    curve: MappingCurve::Linear,
                    smoothing: None,
                },
            ],
            audio_smoothing: 0.7,
            audio_sensitivity: 1.0,
            fade_decay: 0.3,
            glow_intensity: 0.5,
            zalgo_intensity: 0.0,
            beat_flash_intensity: 0.3,
            chromatic_offset: 0.0,
            wave_amplitude: 0.0,
            wave_speed: 2.0,
            color_pulse_speed: 0.0,
            scanline_gap: 0,
            strobe_decay: 0.75,
            temporal_stability: 0.0,
            camera_zoom_amplitude: 1.0,
            camera_rotation: 0.0,
            camera_pan_x: 0.0,
            camera_pan_y: 0.0,
            target_fps: 30,
            fullscreen: false,
            show_spectrum: true,
        }
    }
}

impl RenderConfig {
    /// Clamp all numeric fields to their valid ranges.
    /// Called after TOML deserialization to prevent out-of-range values.
    pub fn clamp_all(&mut self) {
        self.contrast = self.contrast.clamp(0.1, 3.0);
        self.brightness = self.brightness.clamp(-1.0, 1.0);
        self.saturation = self.saturation.clamp(0.0, 3.0);
        self.edge_threshold = self.edge_threshold.clamp(0.0, 1.0);
        self.edge_mix = self.edge_mix.clamp(0.0, 1.0);
        self.density_scale = self.density_scale.clamp(0.25, 4.0);
        self.fade_decay = self.fade_decay.clamp(0.0, 1.0);
        self.glow_intensity = self.glow_intensity.clamp(0.0, 2.0);
        self.zalgo_intensity = self.zalgo_intensity.clamp(0.0, 5.0);
        self.beat_flash_intensity = self.beat_flash_intensity.clamp(0.0, 2.0);
        self.strobe_decay = self.strobe_decay.clamp(0.5, 0.99);
        self.chromatic_offset = self.chromatic_offset.clamp(0.0, 5.0);
        self.wave_amplitude = self.wave_amplitude.clamp(0.0, 1.0);
        self.wave_speed = self.wave_speed.clamp(0.0, 10.0);
        self.color_pulse_speed = self.color_pulse_speed.clamp(0.0, 5.0);
        self.temporal_stability = self.temporal_stability.clamp(0.0, 1.0);
        self.camera_zoom_amplitude = self.camera_zoom_amplitude.clamp(0.1, 10.0);
        self.camera_pan_x = self.camera_pan_x.clamp(-2.0, 2.0);
        self.camera_pan_y = self.camera_pan_y.clamp(-2.0, 2.0);
        self.scanline_gap = self.scanline_gap.min(8);
        self.target_fps = self.target_fps.clamp(15, 120);
        self.audio_smoothing = self.audio_smoothing.clamp(0.0, 1.0);
        self.audio_sensitivity = self.audio_sensitivity.clamp(0.0, 5.0);
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
    dither_mode: Option<DitherMode>,
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
    beat_flash_intensity: Option<f32>,
    chromatic_offset: Option<f32>,
    wave_amplitude: Option<f32>,
    wave_speed: Option<f32>,
    color_pulse_speed: Option<f32>,
    scanline_gap: Option<u8>,
    strobe_decay: Option<f32>,
    temporal_stability: Option<f32>,
    camera_zoom_amplitude: Option<f32>,
    camera_rotation: Option<f32>,
    camera_pan_x: Option<f32>,
    camera_pan_y: Option<f32>,
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
#[allow(clippy::too_many_lines)]
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
    if let Some(v) = r.dither_mode {
        config.dither_mode = v;
    } else if let Some(v) = r.dither_enabled {
        config.dither_mode = if v {
            DitherMode::Bayer8x8
        } else {
            DitherMode::None
        };
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
    if let Some(v) = r.beat_flash_intensity {
        config.beat_flash_intensity = v;
    }
    if let Some(v) = r.chromatic_offset {
        config.chromatic_offset = v;
    }
    if let Some(v) = r.wave_amplitude {
        config.wave_amplitude = v;
    }
    if let Some(v) = r.wave_speed {
        config.wave_speed = v;
    }
    if let Some(v) = r.color_pulse_speed {
        config.color_pulse_speed = v;
    }
    if let Some(v) = r.scanline_gap {
        config.scanline_gap = v;
    }
    if let Some(v) = r.strobe_decay {
        config.strobe_decay = v;
    }
    if let Some(v) = r.temporal_stability {
        config.temporal_stability = v;
    }
    if let Some(v) = r.camera_zoom_amplitude {
        config.camera_zoom_amplitude = v;
    }
    if let Some(v) = r.camera_rotation {
        config.camera_rotation = v;
    }
    if let Some(v) = r.camera_pan_x {
        config.camera_pan_x = v;
    }
    if let Some(v) = r.camera_pan_y {
        config.camera_pan_y = v;
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

    config.clamp_all();
    Ok(config)
}
