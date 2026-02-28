//! Creation Mode: automated audio-reactive effect modulation with presets.

use af_core::config::RenderConfig;
use af_core::frame::{AsciiGrid, AudioFeatures};

/// Image-level features computed from the current ASCII grid.
pub struct ImageFeatures {
    /// Average luminance [0.0, 1.0].
    pub avg_luminance: f32,
    /// Contrast ratio (stddev / 128) [0.0, 1.0].
    pub contrast_ratio: f32,
    /// Fraction of non-space cells [0.0, 1.0].
    pub edge_density: f32,
    /// Dominant hue bucket [0.0, 1.0).
    pub dominant_hue: f32,
}

/// Compute image-level features from the current ASCII grid.
///
/// O(w*h), zero allocation (stack accumulators only).
#[must_use]
pub fn compute_image_features(grid: &AsciiGrid) -> ImageFeatures {
    let total = grid.cells.len() as f32;
    if total < 1.0 {
        return ImageFeatures {
            avg_luminance: 0.0,
            contrast_ratio: 0.0,
            edge_density: 0.0,
            dominant_hue: 0.0,
        };
    }

    let mut sum_lum: f32 = 0.0;
    let mut sum_lum_sq: f32 = 0.0;
    let mut non_empty: u32 = 0;
    let mut hue_buckets = [0u32; 6]; // 6 hue buckets (60° each)

    for cell in &grid.cells {
        let (r, g, b) = cell.fg;
        let lum = f32::from(r.max(g).max(b)) / 255.0;
        sum_lum += lum;
        sum_lum_sq += lum * lum;

        if cell.ch != ' ' {
            non_empty += 1;
        }

        // Simple hue classification via max channel
        if r > 10 || g > 10 || b > 10 {
            let bucket = if r >= g && r >= b {
                if g >= b { 0 } else { 5 } // red-yellow or red-magenta
            } else if g >= r && g >= b {
                if r >= b { 1 } else { 2 } // yellow-green or green-cyan
            } else if r >= g {
                4 // blue-magenta
            } else {
                3 // cyan-blue
            };
            hue_buckets[bucket] += 1;
        }
    }

    let avg_lum = sum_lum / total;
    let variance = (sum_lum_sq / total - avg_lum * avg_lum).max(0.0);
    let stddev = variance.sqrt();

    let dominant_bucket = hue_buckets
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map_or(0, |(idx, _)| idx);

    ImageFeatures {
        avg_luminance: avg_lum,
        contrast_ratio: (stddev / 0.5).clamp(0.0, 1.0),
        edge_density: non_empty as f32 / total,
        dominant_hue: dominant_bucket as f32 / 6.0,
    }
}

/// Creation mode presets for automated audio-reactive modulation.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum CreationPreset {
    /// Subtle ambient: fade, glow, slow color pulse.
    #[default]
    Ambient,
    /// Beat-driven: strobe, chromatic, wave on beats.
    Percussive,
    /// Full psychedelic: color pulse, wave, chromatic, all maxed.
    Psychedelic,
    /// Film-like: fade, glow, scan lines, subtle.
    Cinematic,
    /// Single dominant effect, clean and focused.
    Minimal,
    /// Sharpest possible rendering, subtle audio response.
    Photoreal,
    /// Non-figurative cross-mapped effects.
    Abstract,
    /// Digital corruption aesthetic.
    Glitch,
    /// Vintage degraded aesthetic with scan lines.
    LoFi,
    /// Each frequency band drives a distinct effect.
    Spectral,
    /// Manual control only (no auto-modulation).
    Custom,
}

impl CreationPreset {
    /// Cycle to next preset.
    #[must_use]
    pub fn next(&self) -> Self {
        match self {
            Self::Ambient => Self::Percussive,
            Self::Percussive => Self::Psychedelic,
            Self::Psychedelic => Self::Cinematic,
            Self::Cinematic => Self::Minimal,
            Self::Minimal => Self::Photoreal,
            Self::Photoreal => Self::Abstract,
            Self::Abstract => Self::Glitch,
            Self::Glitch => Self::LoFi,
            Self::LoFi => Self::Spectral,
            Self::Spectral => Self::Custom,
            Self::Custom => Self::Ambient,
        }
    }

    /// Display name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ambient => "Ambient",
            Self::Percussive => "Percussive",
            Self::Psychedelic => "Psychedelic",
            Self::Cinematic => "Cinematic",
            Self::Minimal => "Minimal",
            Self::Photoreal => "Photoreal",
            Self::Abstract => "Abstract",
            Self::Glitch => "Glitch",
            Self::LoFi => "Lo-Fi",
            Self::Spectral => "Spectral",
            Self::Custom => "Custom",
        }
    }
}

/// Engine for automated creation mode modulation.
pub struct CreationEngine {
    /// Auto-modulation active.
    pub auto_mode: bool,
    /// Master intensity multiplier [0.0, 2.0].
    pub master_intensity: f32,
    /// Active preset.
    pub active_preset: CreationPreset,
    /// Selected effect index in the UI (0-9).
    pub selected_effect: usize,
    /// Internal color pulse phase.
    color_pulse_phase: f32,
    /// Previous density_scale for anti-thrashing (skip if delta < 0.15).
    prev_density: f32,
}

impl Default for CreationEngine {
    fn default() -> Self {
        Self {
            auto_mode: true,
            master_intensity: 1.0,
            active_preset: CreationPreset::Ambient,
            selected_effect: 0,
            color_pulse_phase: 0.0,
            prev_density: 1.0,
        }
    }
}

/// Total number of modulatable effects (index 0 = Master).
pub const NUM_EFFECTS: usize = 10;

/// Effect names for the UI. Index 0 is always "Master".
pub const EFFECT_NAMES: [&str; NUM_EFFECTS] = [
    "Master",
    "Beat Flash",
    "Fade Trails",
    "Glow",
    "Chromatic",
    "Wave",
    "Color Pulse",
    "Scan Lines",
    "Zalgo",
    "Strobe Decay",
];

impl CreationEngine {
    /// Modulate the render config based on audio features and image analysis.
    ///
    /// Sets effect values proportionally each frame (no accumulation).
    /// Only active when `auto_mode` is true and preset is not Custom.
    #[allow(clippy::too_many_lines)]
    pub fn modulate(
        &mut self,
        audio: &AudioFeatures,
        image: &ImageFeatures,
        config: &mut RenderConfig,
        onset_envelope: f32,
        dt: f32,
    ) {
        if !self.auto_mode || self.active_preset == CreationPreset::Custom {
            return;
        }

        let mi = self.master_intensity;

        // Image-adaptive brightness compensation (direct set, no accumulation)
        let bright_comp = if image.avg_luminance < 0.2 {
            (0.2 - image.avg_luminance) * mi
        } else {
            0.0
        };
        config.brightness = bright_comp.clamp(-1.0, 1.0);

        // Advance internal phase
        self.color_pulse_phase += dt * mi;

        match self.active_preset {
            CreationPreset::Ambient => {
                // Smooth, breath-like modulation driven by RMS and spectral centroid
                config.fade_decay = (audio.rms * 0.8 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (audio.spectral_centroid * 0.6 * mi).clamp(0.0, 2.0);
                config.color_pulse_speed = (audio.spectral_centroid * 0.8 * mi).clamp(0.0, 5.0);
                config.wave_amplitude = (audio.rms * 0.15 * mi).clamp(0.0, 1.0);
                config.chromatic_offset = 0.0;
                config.beat_flash_intensity = (onset_envelope * 0.3 * mi).clamp(0.0, 2.0);
            }
            CreationPreset::Percussive => {
                // Beat-locked: heavy strobe, chromatic, wave on hits
                config.beat_flash_intensity = (onset_envelope * 0.8 * mi).clamp(0.0, 2.0);
                config.chromatic_offset = (audio.bass * 3.0 * mi).clamp(0.0, 5.0);
                config.wave_amplitude = (onset_envelope * 0.5 * mi).clamp(0.0, 1.0);
                config.fade_decay = (audio.rms * 0.4 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (audio.mid * 0.5 * mi).clamp(0.0, 2.0);
                config.color_pulse_speed = 0.0;
                config.zalgo_intensity = (onset_envelope * 1.2 * mi).clamp(0.0, 5.0);
            }
            CreationPreset::Psychedelic => {
                // Everything cranked — fast color rotation, heavy visual artifacts
                config.color_pulse_speed = (audio.rms * 3.0 * mi).clamp(0.0, 5.0);
                config.wave_amplitude = (audio.mid * 0.6 * mi).clamp(0.0, 1.0);
                config.chromatic_offset =
                    (audio.spectral_flux * 3.0 * mi + audio.bass * 1.0).clamp(0.0, 5.0);
                config.beat_flash_intensity = (onset_envelope * 1.0 * mi).clamp(0.0, 2.0);
                config.glow_intensity = (audio.rms * 1.2 * mi).clamp(0.0, 2.0);
                config.fade_decay = (audio.spectral_centroid * 0.6 * mi).clamp(0.0, 1.0);
                config.zalgo_intensity = (audio.spectral_flux * 2.0 * mi).clamp(0.0, 5.0);
                let scan = (audio.presence * 4.0 * mi) as u8;
                config.scanline_gap = if scan >= 2 { scan.min(8) } else { 0 };
                // Camera: rotation from centroid, zoom from bass
                config.camera_rotation += audio.spectral_centroid * 0.02 * mi;
                config.camera_zoom_amplitude = (1.0 + audio.bass * 0.3 * mi).clamp(0.1, 10.0);
            }
            CreationPreset::Cinematic => {
                // Smooth, controlled dynamics — fade/glow dominant, subtle scan lines
                config.fade_decay = (audio.rms * 0.9 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (audio.spectral_centroid * 0.7 * mi).clamp(0.0, 2.0);
                config.chromatic_offset = (audio.bass * 0.5 * mi).clamp(0.0, 5.0);
                config.wave_amplitude = 0.0;
                config.color_pulse_speed = (audio.rms * 0.3 * mi).clamp(0.0, 5.0);
                config.beat_flash_intensity = (onset_envelope * 0.5 * mi).clamp(0.0, 2.0);
                let scan = (audio.presence * 3.0 * mi) as u8;
                config.scanline_gap = if scan >= 2 { scan.min(6) } else { 0 };
            }
            CreationPreset::Minimal => {
                // Single dominant effect based on strongest audio feature
                let fade_score = audio.rms;
                let chrom_score = audio.spectral_flux;
                let wave_score = audio.bass;
                if fade_score >= chrom_score && fade_score >= wave_score {
                    config.fade_decay = (fade_score * 0.8 * mi).clamp(0.0, 1.0);
                    config.chromatic_offset = 0.0;
                    config.wave_amplitude = 0.0;
                } else if chrom_score >= wave_score {
                    config.fade_decay = 0.0;
                    config.chromatic_offset = (chrom_score * 2.0 * mi).clamp(0.0, 5.0);
                    config.wave_amplitude = 0.0;
                } else {
                    config.fade_decay = 0.0;
                    config.chromatic_offset = 0.0;
                    config.wave_amplitude = (wave_score * 0.4 * mi).clamp(0.0, 1.0);
                }
                config.glow_intensity = (audio.spectral_centroid * 0.3 * mi).clamp(0.0, 2.0);
                config.beat_flash_intensity = (onset_envelope * 0.3 * mi).clamp(0.0, 2.0);
                config.color_pulse_speed = 0.0;
            }
            CreationPreset::Photoreal => {
                // Sharpest rendering: high stability, subtle glow, minimal effects
                config.temporal_stability = (0.7 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (0.3 * mi).clamp(0.0, 2.0);
                config.fade_decay = (0.2 * mi).clamp(0.0, 1.0);
                config.chromatic_offset = 0.0;
                config.wave_amplitude = 0.0;
                config.zalgo_intensity = 0.0;
                config.color_pulse_speed = 0.0;
                config.beat_flash_intensity = (onset_envelope * 0.15 * mi).clamp(0.0, 2.0);
                // Subtle audio: brightness from RMS, contrast from centroid
                config.contrast = (1.0 + audio.spectral_centroid * 0.3 * mi).clamp(0.1, 3.0);
            }
            CreationPreset::Abstract => {
                // Non-figurative: cross-mapped unusual source→target pairings
                config.wave_amplitude = (audio.spectral_flatness * 0.6 * mi).clamp(0.0, 1.0);
                config.chromatic_offset = (audio.timbral_roughness * 3.0 * mi).clamp(0.0, 5.0);
                config.color_pulse_speed = (audio.spectral_centroid * 2.0 * mi).clamp(0.0, 5.0);
                let scan = (audio.beat_phase * 6.0 * mi) as u8;
                config.scanline_gap = if scan >= 2 { scan.min(8) } else { 0 };
                config.glow_intensity = (audio.mid * 0.8 * mi).clamp(0.0, 2.0);
                config.fade_decay = (audio.spectral_flux * 0.7 * mi).clamp(0.0, 1.0);
                config.beat_flash_intensity = (onset_envelope * 0.6 * mi).clamp(0.0, 2.0);
                config.zalgo_intensity = (audio.timbral_brightness * 1.5 * mi).clamp(0.0, 5.0);
                // Camera: pan from flatness, rotation from roughness
                config.camera_pan_x = (audio.spectral_flatness * 0.3 * mi - 0.15).clamp(-2.0, 2.0);
                config.camera_rotation += audio.timbral_roughness * 0.01 * mi;
            }
            CreationPreset::Glitch => {
                // Digital corruption: zalgo dominant, chromatic aggressive
                config.zalgo_intensity = (audio.spectral_flux * 3.0 * mi).clamp(0.0, 5.0);
                config.chromatic_offset = (audio.bass * 4.0 * mi).clamp(0.0, 5.0);
                config.beat_flash_intensity = (onset_envelope * 1.0 * mi).clamp(0.0, 2.0);
                config.wave_amplitude = (audio.mid * 0.3 * mi).clamp(0.0, 1.0);
                config.fade_decay = (audio.rms * 0.3 * mi).clamp(0.0, 1.0);
                config.glow_intensity = 0.0;
                config.color_pulse_speed = (audio.timbral_roughness * 2.0 * mi).clamp(0.0, 5.0);
                // Invert on onset (via contrast inversion trick)
                if audio.onset {
                    config.invert = !config.invert;
                }
            }
            CreationPreset::LoFi => {
                // Vintage degraded: constant scan lines, high fade, gentle response
                config.scanline_gap = (4.0 * mi).clamp(0.0, 8.0) as u8;
                config.fade_decay = (0.7 * mi).clamp(0.0, 1.0);
                config.temporal_stability = (0.5 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (audio.timbral_roughness * 0.4 * mi).clamp(0.0, 2.0);
                config.chromatic_offset = 0.0;
                config.wave_amplitude = 0.0;
                config.color_pulse_speed = 0.0;
                config.beat_flash_intensity = (onset_envelope * 0.2 * mi).clamp(0.0, 2.0);
                config.zalgo_intensity = 0.0;
            }
            CreationPreset::Spectral => {
                // Each frequency band drives a distinct effect
                config.wave_amplitude = (audio.sub_bass * 0.5 * mi).clamp(0.0, 1.0);
                config.glow_intensity = (audio.bass * 1.0 * mi).clamp(0.0, 2.0);
                config.chromatic_offset = (audio.mid * 2.5 * mi).clamp(0.0, 5.0);
                config.color_pulse_speed = (audio.high_mid * 2.0 * mi).clamp(0.0, 5.0);
                config.zalgo_intensity = (audio.brilliance * 2.0 * mi).clamp(0.0, 5.0);
                config.fade_decay = (audio.rms * 0.5 * mi).clamp(0.0, 1.0);
                config.beat_flash_intensity = (onset_envelope * 0.5 * mi).clamp(0.0, 2.0);
                // Camera: zoom from sub_bass, pan from brilliance
                config.camera_zoom_amplitude = (1.0 + audio.sub_bass * 0.2 * mi).clamp(0.1, 10.0);
                config.camera_pan_x = ((audio.brilliance - 0.5) * 0.2 * mi).clamp(-2.0, 2.0);
            }
            CreationPreset::Custom => {} // Handled by early return above
        }

        // density_scale modulation for select presets (with anti-thrashing)
        let target_density = match self.active_preset {
            CreationPreset::Percussive => (1.0 + audio.bass * 0.5 * mi).clamp(0.25, 4.0),
            CreationPreset::Abstract => (0.5 + audio.spectral_centroid * 1.5 * mi).clamp(0.25, 4.0),
            CreationPreset::Spectral => (0.75 + audio.rms * 1.0 * mi).clamp(0.25, 4.0),
            _ => config.density_scale,
        };
        if (target_density - self.prev_density).abs() > 0.15 {
            config.density_scale = target_density;
            self.prev_density = target_density;
        } else {
            config.density_scale = self.prev_density;
        }
    }

    /// Get current effect value from config by index (0 = Master).
    #[must_use]
    pub fn effect_value(&self, idx: usize, config: &RenderConfig) -> f32 {
        match idx {
            0 => self.master_intensity,
            1 => config.beat_flash_intensity,
            2 => config.fade_decay,
            3 => config.glow_intensity,
            4 => config.chromatic_offset,
            5 => config.wave_amplitude,
            6 => config.color_pulse_speed,
            7 => f32::from(config.scanline_gap),
            8 => config.zalgo_intensity,
            9 => config.strobe_decay,
            _ => 0.0,
        }
    }

    /// Get max value for effect by index (0 = Master).
    #[must_use]
    pub fn effect_max(&self, idx: usize) -> f32 {
        match idx {
            0 | 3 => 2.0,     // Master, Glow
            4 | 6 | 8 => 5.0, // Chromatic, Color Pulse, Zalgo
            7 => 8.0,         // Scan Lines
            9 => 0.99,        // Strobe Decay
            _ => 1.0,         // Beat Flash, Fade, Wave
        }
    }
}
