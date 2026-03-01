use anyhow::Result;
use std::path::Path;

#[cfg(feature = "video")]
use af_ascii::compositor::Compositor;
#[cfg(feature = "video")]
use af_audio::batch_analyzer::BatchAnalyzer;
use af_core::config::RenderConfig;
#[cfg(feature = "video")]
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
#[cfg(feature = "video")]
use af_core::traits::Source;
#[cfg(feature = "video")]
use af_export::muxer::{Mp4Muxer, mux_audio_video};
#[cfg(feature = "video")]
use af_export::rasterizer::Rasterizer;

#[cfg(feature = "video")]
use af_source::folder_batch::FolderBatchSource;

#[cfg(feature = "video")]
use crate::generative::AutoGenerativeMapper;

// ─── Constants ─────────────────────────────────────────────────────

/// Minimum frames between mutation events.
#[cfg(feature = "video")]
const MUTATION_COOLDOWN: u32 = 90;
/// Maximum concurrent mutations per beat event.
#[cfg(feature = "video")]
const MAX_MUTATIONS_PER_EVENT: u32 = 2;
/// Effect burst total duration in frames.
#[cfg(feature = "video")]
const EFFECT_BURST_DURATION: u32 = 60;
/// Density pulse total duration in frames.
#[cfg(feature = "video")]
const DENSITY_PULSE_DURATION: u32 = 30;
/// Invert flash duration in frames.
#[cfg(feature = "video")]
const INVERT_FLASH_DURATION: u32 = 90;
/// Mode/color_mode override duration in frames.
#[cfg(feature = "video")]
const MODE_OVERRIDE_DURATION: u32 = 180;
/// Beat intensity threshold for mutation trigger.
#[cfg(feature = "video")]
const MUTATION_BEAT_THRESHOLD: f32 = 0.85;
/// Beat intensity threshold for aggressive clip advance.
#[cfg(feature = "video")]
const CLIP_ADVANCE_BEAT_THRESHOLD: f32 = 0.9;
/// Easing ramp frames for smooth mutation transitions.
#[cfg(feature = "video")]
const MUTATION_RAMP_FRAMES: u32 = 12;
/// Minimum preset duration in seconds (--preset all).
#[cfg(feature = "video")]
const MIN_PRESET_DURATION_SECS: f32 = 5.0;

// Mutation probabilities (base, before energy scaling)
#[cfg(feature = "video")]
const PROB_MODE_CYCLE: f64 = 0.12;
#[cfg(feature = "video")]
const PROB_CHARSET_ROTATION: f64 = 0.15;
#[cfg(feature = "video")]
const PROB_EFFECT_BURST: f64 = 0.06;
#[cfg(feature = "video")]
const PROB_DENSITY_PULSE: f64 = 0.08;
#[cfg(feature = "video")]
const PROB_COLOR_MODE_CYCLE: f64 = 0.05;
#[cfg(feature = "video")]
const PROB_INVERT_FLASH: f64 = 0.10;
#[cfg(feature = "video")]
const PROB_CAMERA_BURST: f64 = 0.04;

// ─── Smooth Override ───────────────────────────────────────────────

/// Smoothed mutation override with ramp-up/hold/ramp-down easing.
#[cfg(feature = "video")]
struct SmoothOverride {
    target_value: f32,
    total_frames: u32,
    ramp_frames: u32,
    elapsed: u32,
}

#[cfg(feature = "video")]
impl SmoothOverride {
    fn new(target: f32, total: u32, ramp: u32) -> Self {
        Self {
            target_value: target,
            total_frames: total,
            ramp_frames: ramp.min(total / 3), // Ramp can't exceed 1/3 of total
            elapsed: 0,
        }
    }

    /// Current value with smoothstep easing.
    fn value(&self) -> f32 {
        let ramp_end = self.ramp_frames;
        let hold_end = self.total_frames.saturating_sub(self.ramp_frames);

        if self.elapsed < ramp_end {
            // Ramp up
            let t = self.elapsed as f32 / ramp_end.max(1) as f32;
            let s = t * t * (3.0 - 2.0 * t); // smoothstep
            self.target_value * s
        } else if self.elapsed < hold_end {
            // Hold
            self.target_value
        } else {
            // Ramp down
            let remaining = self.total_frames.saturating_sub(self.elapsed);
            let t = remaining as f32 / self.ramp_frames.max(1) as f32;
            let s = t * t * (3.0 - 2.0 * t);
            self.target_value * s
        }
    }

    /// Advance one frame. Returns true when expired.
    fn tick(&mut self) -> bool {
        self.elapsed += 1;
        self.elapsed >= self.total_frames
    }
}

// ─── Macro State ───────────────────────────────────────────────────

/// Grouped macro mutation state for the batch pipeline.
#[cfg(feature = "video")]
struct MacroState {
    mode: Option<af_core::config::RenderMode>,
    mode_countdown: u32,
    invert: Option<bool>,
    invert_countdown: u32,
    charset: Option<(usize, String)>,
    density: Option<SmoothOverride>,
    effect_burst: Option<SmoothOverride>,
    effect_burst_id: u8,
    color_mode: Option<af_core::config::ColorMode>,
    color_mode_countdown: u32,
    camera: Option<SmoothOverride>,
    camera_param: u8, // 0=zoom, 1=rotation, 2=pan_x, 3=pan_y
    frames_since_last: u32,
}

#[cfg(feature = "video")]
impl MacroState {
    fn new() -> Self {
        Self {
            mode: None,
            mode_countdown: 0,
            invert: None,
            invert_countdown: 0,
            charset: None,
            density: None,
            effect_burst: None,
            effect_burst_id: 0,
            color_mode: None,
            color_mode_countdown: 0,
            camera: None,
            camera_param: 0,
            frames_since_last: u32::MAX, // allow first mutation immediately
        }
    }

    /// Decay all countdowns. Clear expired overrides.
    fn tick(&mut self) {
        self.frames_since_last = self.frames_since_last.saturating_add(1);

        if self.mode_countdown > 0 {
            self.mode_countdown -= 1;
            if self.mode_countdown == 0 {
                self.mode = None;
            }
        }
        if self.invert_countdown > 0 {
            self.invert_countdown -= 1;
            if self.invert_countdown == 0 {
                self.invert = None;
            }
        }
        if self.color_mode_countdown > 0 {
            self.color_mode_countdown -= 1;
            if self.color_mode_countdown == 0 {
                self.color_mode = None;
            }
        }
        if let Some(ref mut d) = self.density
            && d.tick()
        {
            self.density = None;
        }
        if let Some(ref mut e) = self.effect_burst
            && e.tick()
        {
            self.effect_burst = None;
        }
        if let Some(ref mut c) = self.camera
            && c.tick()
        {
            self.camera = None;
        }
    }

    /// Apply all active overrides to the frame config.
    fn apply(&self, frame_config: &mut RenderConfig) {
        if let Some(ref m) = self.mode {
            frame_config.render_mode = m.clone();
        }
        if let Some(inv) = self.invert {
            frame_config.invert = inv;
        }
        if let Some((idx, ref chars)) = self.charset {
            frame_config.charset_index = idx;
            frame_config.charset.clone_from(chars);
        }
        if let Some(ref d) = self.density {
            frame_config.density_scale = d.value();
        }
        if let Some(ref e) = self.effect_burst {
            let v = e.value();
            match self.effect_burst_id {
                0 => frame_config.glow_intensity = v,
                1 => frame_config.chromatic_offset = v,
                2 => frame_config.wave_amplitude = v,
                3 => frame_config.color_pulse_speed = v,
                4 => frame_config.zalgo_intensity = v,
                5 => frame_config.fade_decay = v,
                _ => {}
            }
        }
        if let Some(ref cm) = self.color_mode {
            frame_config.color_mode = cm.clone();
        }
        if let Some(ref c) = self.camera {
            let v = c.value();
            match self.camera_param {
                0 => frame_config.camera_zoom_amplitude = 1.0 + v, // Base 1.0 + burst
                1 => frame_config.camera_rotation = v,
                2 => frame_config.camera_pan_x = v,
                3 => frame_config.camera_pan_y = v,
                _ => {}
            }
        }
    }
}

// ─── Preset Sequencer ──────────────────────────────────────────────

/// Active transition between two preset configs.
#[cfg(feature = "video")]
struct PresetTransition {
    from: RenderConfig,
    to_idx: usize,
    duration_frames: u32,
    elapsed_frames: u32,
}

/// Sequences through all available presets with smooth transitions.
#[cfg(feature = "video")]
struct PresetSequencer {
    presets: Vec<(String, RenderConfig)>,
    current_idx: usize,
    transition: Option<PresetTransition>,
    frames_at_current: u32,
    prev_energy: u8,
}

#[cfg(feature = "video")]
impl PresetSequencer {
    fn new(presets: Vec<(String, RenderConfig)>) -> Self {
        Self {
            presets,
            current_idx: 0,
            transition: None,
            frames_at_current: 0,
            prev_energy: 1,
        }
    }

    /// Advance to the next preset, starting a smooth transition.
    fn advance(&mut self, transition_duration: u32) {
        if self.presets.len() < 2 {
            return;
        }
        let from = self.presets[self.current_idx].1.clone();
        let next_idx = (self.current_idx + 1) % self.presets.len();

        log::info!(
            "Preset transition: {} → {}",
            self.presets[self.current_idx].0,
            self.presets[next_idx].0
        );

        self.transition = Some(PresetTransition {
            from,
            to_idx: next_idx,
            duration_frames: transition_duration.max(1),
            elapsed_frames: 0,
        });
        self.frames_at_current = 0;
    }

    /// Get the interpolated config into `out`. If transitioning, interpolates fields.
    fn write_config(&mut self, out: &mut RenderConfig) {
        self.frames_at_current += 1;

        if let Some(ref mut trans) = self.transition {
            trans.elapsed_frames += 1;
            let t = trans.elapsed_frames as f32 / trans.duration_frames as f32;

            if t >= 1.0 {
                self.current_idx = trans.to_idx;
                out.clone_from(&self.presets[self.current_idx].1);
                self.transition = None;
            } else {
                interpolate_configs(&trans.from, &self.presets[trans.to_idx].1, t, out);
            }
        } else {
            out.clone_from(&self.presets[self.current_idx].1);
        }
    }

    /// Check if a preset change should be triggered.
    fn should_change(&mut self, energy: u8, preset_duration_frames: u32) -> bool {
        if self.transition.is_some() || self.presets.len() < 2 {
            return false;
        }

        // Energy transition trigger (with minimum duration)
        let min_frames = (MIN_PRESET_DURATION_SECS * 60.0) as u32; // ~5s at 60fps rough
        let energy_changed = energy != self.prev_energy && self.frames_at_current >= min_frames;
        self.prev_energy = energy;

        // Time-based trigger
        let time_expired = self.frames_at_current >= preset_duration_frames;

        energy_changed || time_expired
    }
}

/// Linearly interpolate two RenderConfigs. Numeric fields lerp, discrete fields snap at t=0.5.
#[cfg(feature = "video")]
fn interpolate_configs(from: &RenderConfig, to: &RenderConfig, t: f32, out: &mut RenderConfig) {
    let lerp = |a: f32, b: f32| a + (b - a) * t;

    // Start from `from`, then interpolate
    out.clone_from(from);

    // Numeric fields: linear interpolation
    out.contrast = lerp(from.contrast, to.contrast);
    out.brightness = lerp(from.brightness, to.brightness);
    out.saturation = lerp(from.saturation, to.saturation);
    out.density_scale = lerp(from.density_scale, to.density_scale);
    out.edge_threshold = lerp(from.edge_threshold, to.edge_threshold);
    out.edge_mix = lerp(from.edge_mix, to.edge_mix);
    out.aspect_ratio = lerp(from.aspect_ratio, to.aspect_ratio);
    out.fade_decay = lerp(from.fade_decay, to.fade_decay);
    out.glow_intensity = lerp(from.glow_intensity, to.glow_intensity);
    out.beat_flash_intensity = lerp(from.beat_flash_intensity, to.beat_flash_intensity);
    out.chromatic_offset = lerp(from.chromatic_offset, to.chromatic_offset);
    out.wave_amplitude = lerp(from.wave_amplitude, to.wave_amplitude);
    out.wave_speed = lerp(from.wave_speed, to.wave_speed);
    out.color_pulse_speed = lerp(from.color_pulse_speed, to.color_pulse_speed);
    out.strobe_decay = lerp(from.strobe_decay, to.strobe_decay);
    out.temporal_stability = lerp(from.temporal_stability, to.temporal_stability);
    out.zalgo_intensity = lerp(from.zalgo_intensity, to.zalgo_intensity);
    out.scanline_darken = lerp(from.scanline_darken, to.scanline_darken);
    out.camera_zoom_amplitude = lerp(from.camera_zoom_amplitude, to.camera_zoom_amplitude);
    out.camera_rotation = lerp(from.camera_rotation, to.camera_rotation);
    out.camera_pan_x = lerp(from.camera_pan_x, to.camera_pan_x);
    out.camera_pan_y = lerp(from.camera_pan_y, to.camera_pan_y);
    out.audio_sensitivity = lerp(from.audio_sensitivity, to.audio_sensitivity);
    out.audio_smoothing = lerp(from.audio_smoothing, to.audio_smoothing);

    // Discrete fields: snap at t=0.5
    if t >= 0.5 {
        out.render_mode = to.render_mode.clone();
        out.color_mode = to.color_mode.clone();
        out.bg_style = to.bg_style.clone();
        out.dither_mode = to.dither_mode.clone();
        out.charset.clone_from(&to.charset);
        out.charset_index = to.charset_index;
        out.invert = to.invert;
        out.color_enabled = to.color_enabled;
        out.shape_matching = to.shape_matching;
        out.scanline_gap = to.scanline_gap;
        out.fullscreen = to.fullscreen;
        out.show_spectrum = to.show_spectrum;
        // Use destination audio mappings once past midpoint
        out.audio_mappings.clone_from(&to.audio_mappings);
    }
}

/// Load all presets from config/presets/*.toml, sorted by filename.
#[cfg(feature = "video")]
fn load_all_presets() -> Vec<(String, RenderConfig)> {
    let presets_dir = std::path::Path::new("config/presets");
    if !presets_dir.is_dir() {
        log::warn!("Dossier presets introuvable : {}", presets_dir.display());
        return Vec::new();
    }

    let mut presets: Vec<(String, RenderConfig)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(presets_dir) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            match af_core::config::load_config(&path) {
                Ok(mut config) => {
                    config.clamp_all();
                    presets.push((name, config));
                }
                Err(e) => {
                    log::warn!("Preset {name} ignoré (parse error): {e}");
                }
            }
        }
    }

    presets.sort_by(|(a, _), (b, _)| a.cmp(b));
    presets
}

// ─── Main Export Function ──────────────────────────────────────────

/// Point d'entrée pour l'export génératif par lots.
///
/// # Errors
/// Retourne une erreur si l'analyse audio, le scan du dossier, ou l'encodage échoue.
#[allow(
    clippy::too_many_lines,
    clippy::fn_params_excessive_bools,
    clippy::too_many_arguments
)]
pub fn run_batch_export(
    folder: &Path,
    audio_path_str: Option<&String>,
    final_output: Option<&Path>,
    config: RenderConfig,
    target_fps: u32,
    export_scale: Option<f32>,
    preset_all: bool,
    seed: Option<u64>,
    preset_duration_secs: f32,
    crossfade_ms: Option<u32>,
    mutation_intensity: f32,
) -> Result<()> {
    #[cfg(not(feature = "video"))]
    {
        let _ = (
            folder,
            audio_path_str,
            final_output,
            config,
            target_fps,
            export_scale,
            preset_all,
            seed,
            preset_duration_secs,
            crossfade_ms,
            mutation_intensity,
        );
        anyhow::bail!("L'export par lots requiert la feature 'video' (ffmpeg support).");
    }

    #[cfg(feature = "video")]
    {
        // === Seed for reproducibility ===
        if let Some(s) = seed {
            fastrand::seed(s);
            log::info!("Seed: {s} (reproductible)");
        }

        // === Auto-discovery Audio ===
        let resolved_audio_path = if let Some(path_str) = audio_path_str {
            std::path::PathBuf::from(path_str)
        } else {
            log::info!("Recherche d'un fichier audio dans {}...", folder.display());
            let found = std::fs::read_dir(folder)?
                .filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .find(|p| {
                    if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                        let e = ext.to_lowercase();
                        e == "mp3" || e == "wav" || e == "flac" || e == "ogg" || e == "aac"
                    } else {
                        false
                    }
                });
            match found {
                Some(p) => {
                    log::info!("Audio trouvé : {}", p.display());
                    p
                }
                None => anyhow::bail!("Aucun fichier audio trouvé dans le dossier."),
            }
        };

        // === Auto-naming Output ===
        let resolved_output_path = if let Some(out_path) = final_output {
            out_path.to_path_buf()
        } else {
            let folder_name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("batch");
            let timestamp = {
                use std::time::SystemTime;
                let secs = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let s = secs % 60;
                let m = (secs / 60) % 60;
                let h = (secs / 3600) % 24;
                let days = secs / 86400;
                let mut y = 1970u64;
                let mut remaining = days;
                loop {
                    let days_in_year = if y.is_multiple_of(4)
                        && (!y.is_multiple_of(100) || y.is_multiple_of(400))
                    {
                        366
                    } else {
                        365
                    };
                    if remaining < days_in_year {
                        break;
                    }
                    remaining -= days_in_year;
                    y += 1;
                }
                let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
                let mdays: [u64; 12] = [
                    31,
                    if leap { 29 } else { 28 },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                let mut mo = 0u64;
                for &md in &mdays {
                    if remaining < md {
                        break;
                    }
                    remaining -= md;
                    mo += 1;
                }
                format!("{y}{:02}{:02}_{h:02}{m:02}{s:02}", mo + 1, remaining + 1)
            };
            let default_name = format!("{folder_name}_{timestamp}.mp4");
            let mut p = std::env::current_dir()?;
            p.push(default_name);
            p
        };

        let audio_path = resolved_audio_path.as_path();
        let final_output = resolved_output_path.as_path();

        // === Preset sequencer (--preset all) ===
        let mut preset_seq = if preset_all {
            let all_presets = load_all_presets();
            if all_presets.is_empty() {
                log::warn!("Aucun preset trouvé, utilisation config unique.");
                None
            } else {
                log::info!("Mode --preset all : {} presets chargés", all_presets.len());
                Some(PresetSequencer::new(all_presets))
            }
        } else {
            None
        };

        // Initial config: either first preset or provided config
        let initial_config = if let Some(ref seq) = preset_seq {
            seq.presets[0].1.clone()
        } else {
            config
        };

        // === Étape 1 : Pré-analyse audio complète (offline) ===
        log::info!("Étape 1/4 : Analyse Audio de {}", audio_path.display());
        let mut analyzer = BatchAnalyzer::new(target_fps, 44100, 2048);
        let timeline = analyzer.analyze_file(audio_path)?;

        let mut mapper = AutoGenerativeMapper::new(initial_config, timeline);

        // === Étape 2 : Initialisation de la source dossier ===
        log::info!(
            "Étape 2/4 : Initialisation Media Folder {}",
            folder.display()
        );
        let total_frames_u32 = mapper.get_timeline().total_frames() as u32;
        let mut source = FolderBatchSource::new(folder, target_fps, total_frames_u32)?;

        let (native_w, native_h) = source.native_size();
        let target_w = native_w.max(1280);
        let target_h = native_h.max(720);

        let grid_w = (target_w / 8) as u16;
        let grid_h = (target_h / 16) as u16;

        let mut grid = AsciiGrid::new(grid_w, grid_h);

        // === Étape 3 : Pipeline de rendu (Compositor + Rasterizer + Muxer) ===
        log::info!("Étape 3/4 : Préparation de l'encodeur FFmpeg");

        let font_data = include_bytes!("../../af-export/assets/FiraCode-Regular.ttf");
        let scale_val = export_scale.unwrap_or(16.0);
        let rasterizer = Rasterizer::new(font_data, scale_val)?;

        let (raster_w, raster_h) = rasterizer.target_dimensions(grid_w, grid_h);

        let temp_video = final_output.with_extension("temp.mp4");
        let mut muxer = Mp4Muxer::new(&temp_video, raster_w, raster_h, target_fps)?;

        let mut frame_config = RenderConfig::default();
        mapper.apply_at(0.0, 0.0, &mut frame_config);
        let mut compositor = Compositor::new(&frame_config.charset);
        let mut raster_fb = FrameBuffer::new(raster_w, raster_h);

        let total_frames = mapper.get_timeline().total_frames();
        let frame_duration = 1.0 / f64::from(target_fps);
        let preset_duration_frames = (preset_duration_secs * target_fps as f32) as u32;

        let mut resizer = af_source::resize::Resizer::new();
        let mut resized_source = FrameBuffer::new(target_w, target_h);
        let mut transformed_source = FrameBuffer::new(1, 1);

        // === Pre-allocated effect buffers (R1 compliance) ===
        let mut prev_grid = AsciiGrid::new(grid_w, grid_h);
        let mut glow_brightness_buf: Vec<u8> =
            Vec::with_capacity(usize::from(grid_w) * usize::from(grid_h));
        let mut effect_fg_buf: Vec<(u8, u8, u8)> = Vec::new();
        let mut effect_row_buf: Vec<AsciiCell> = Vec::new();
        let mut onset_envelope: f32 = 0.0;
        let mut color_pulse_phase: f32 = 0.0;
        let mut wave_phase: f32 = 0.0;

        // Pre-allocated charset pool (font-safe for FiraCode export)
        let charset_pool: [&str; 11] = [
            af_core::charset::CHARSET_FULL,
            af_core::charset::CHARSET_DENSE,
            af_core::charset::CHARSET_SHORT_1,
            af_core::charset::CHARSET_SHORT_2,
            af_core::charset::CHARSET_EDGE,
            af_core::charset::CHARSET_GLITCH_1,
            af_core::charset::CHARSET_DISCRETE,
            af_core::charset::CHARSET_DIGITAL,
            af_core::charset::CHARSET_BINARY,
            af_core::charset::CHARSET_EXTENDED,
            af_core::charset::CHARSET_HIRES,
        ];

        log::info!("Boucle de Rendu : {total_frames} frames à {target_fps}fps");

        let mut macros = MacroState::new();
        // Buffer for preset interpolation
        let mut preset_config_buf = RenderConfig::default();

        let render_start = std::time::Instant::now();

        for frame_idx in 0..total_frames {
            let timestamp_secs = frame_idx as f64 * frame_duration;
            let current_features = mapper.get_timeline().get_at_time(timestamp_secs);

            // === 0. PRESET SEQUENCING (--preset all) ===
            let energy = mapper.get_timeline().energy_at(frame_idx);

            if let Some(ref mut seq) = preset_seq {
                if seq.should_change(energy, preset_duration_frames) {
                    let transition_dur = match energy {
                        2 => target_fps,     // ~1s fast
                        0 => target_fps * 3, // ~3s slow
                        _ => target_fps * 2, // ~2s standard
                    };
                    seq.advance(transition_dur);
                }
                seq.write_config(&mut preset_config_buf);
                mapper.set_base_config(preset_config_buf.clone());
            }

            // Apply audio mappings with curve + smoothing (zero-alloc: reuses frame_config)
            mapper.apply_at(timestamp_secs, onset_envelope, &mut frame_config);

            // === 1. CLIP SEQUENCING (decoupled from mutations) ===
            let clip_budget = match energy {
                2 => source.max_clip_frames() / 2,
                0 => source.max_clip_frames() * 3 / 2,
                _ => source.max_clip_frames(),
            };

            let should_advance = source.clip_frame_count() >= clip_budget
                || (energy == 2
                    && current_features.onset
                    && current_features.beat_intensity > CLIP_ADVANCE_BEAT_THRESHOLD);

            if should_advance {
                // Adaptive crossfade duration
                let cf_frames = if let Some(ms) = crossfade_ms {
                    (ms * target_fps / 1000).max(1)
                } else {
                    match energy {
                        2 => target_fps / 4,
                        0 => target_fps,
                        _ => target_fps / 2,
                    }
                };
                source.set_crossfade_duration(cf_frames);
                source.next_media();
            }

            // === 2. MACRO MUTATIONS (cooldown + coordination) ===
            macros.tick();

            if current_features.onset
                && current_features.beat_intensity > MUTATION_BEAT_THRESHOLD
                && macros.frames_since_last >= MUTATION_COOLDOWN
            {
                let mutation_scale = match energy {
                    2 => 1.5,
                    0 => 0.3,
                    _ => 1.0,
                };
                let mi = mutation_scale * f64::from(mutation_intensity);
                let mut mutations: u32 = 0;
                let intensity_scale = current_features.beat_intensity.max(0.5);

                // Mode cycle
                if mutations < MAX_MUTATIONS_PER_EVENT && fastrand::f64() < PROB_MODE_CYCLE * mi {
                    let modes = [
                        af_core::config::RenderMode::Ascii,
                        af_core::config::RenderMode::HalfBlock,
                        af_core::config::RenderMode::Braille,
                        af_core::config::RenderMode::Quadrant,
                        af_core::config::RenderMode::Sextant,
                    ];
                    let current = macros.mode.as_ref().unwrap_or(&frame_config.render_mode);
                    let idx = modes.iter().position(|m| m == current).unwrap_or(0);
                    macros.mode = Some(modes[(idx + 1) % modes.len()].clone());
                    macros.mode_countdown = MODE_OVERRIDE_DURATION;
                    mutations += 1;
                }

                // Charset rotation
                if mutations < MAX_MUTATIONS_PER_EVENT
                    && fastrand::f64() < PROB_CHARSET_ROTATION * mi
                {
                    let current_idx = macros
                        .charset
                        .as_ref()
                        .map_or(frame_config.charset_index, |(i, _)| *i);
                    let new_idx = (current_idx + 1) % charset_pool.len();
                    let mut new_charset = String::new();
                    new_charset.push_str(charset_pool[new_idx]);
                    macros.charset = Some((new_idx, new_charset));
                    mutations += 1;
                }

                // Effect burst (6 types)
                if mutations < MAX_MUTATIONS_PER_EVENT && fastrand::f64() < PROB_EFFECT_BURST * mi {
                    let bursts: [(u8, f32); 6] = [
                        (0, 1.5 * intensity_scale),
                        (1, 2.5 * intensity_scale),
                        (2, 0.4 * intensity_scale),
                        (3, 2.0 * intensity_scale),
                        (4, 0.8 * intensity_scale), // Zalgo
                        (5, 0.7 * intensity_scale), // Fade
                    ];
                    let pick = bursts[fastrand::usize(0..bursts.len())];
                    macros.effect_burst_id = pick.0;
                    macros.effect_burst = Some(SmoothOverride::new(
                        pick.1,
                        EFFECT_BURST_DURATION,
                        MUTATION_RAMP_FRAMES,
                    ));
                    mutations += 1;
                }

                // Density pulse (continuous range)
                if mutations < MAX_MUTATIONS_PER_EVENT && fastrand::f64() < PROB_DENSITY_PULSE * mi
                {
                    let target = 0.4 + fastrand::f32() * 2.1; // [0.4, 2.5]
                    macros.density = Some(SmoothOverride::new(
                        target,
                        DENSITY_PULSE_DURATION,
                        MUTATION_RAMP_FRAMES.min(8),
                    ));
                    mutations += 1;
                }

                // Color mode cycle
                if mutations < MAX_MUTATIONS_PER_EVENT
                    && fastrand::f64() < PROB_COLOR_MODE_CYCLE * mi
                {
                    let modes = [
                        af_core::config::ColorMode::Direct,
                        af_core::config::ColorMode::HsvBright,
                        af_core::config::ColorMode::Oklab,
                        af_core::config::ColorMode::Quantized,
                    ];
                    let current = macros
                        .color_mode
                        .as_ref()
                        .unwrap_or(&frame_config.color_mode);
                    let idx = modes.iter().position(|m| m == current).unwrap_or(0);
                    macros.color_mode = Some(modes[(idx + 1) % modes.len()].clone());
                    macros.color_mode_countdown = MODE_OVERRIDE_DURATION;
                    mutations += 1;
                }

                // Invert flash (with auto-revert)
                if mutations < MAX_MUTATIONS_PER_EVENT && fastrand::f64() < PROB_INVERT_FLASH * mi {
                    let current = macros.invert.unwrap_or(frame_config.invert);
                    macros.invert = Some(!current);
                    macros.invert_countdown = INVERT_FLASH_DURATION;
                    mutations += 1;
                }

                // Camera burst
                if mutations < MAX_MUTATIONS_PER_EVENT && fastrand::f64() < PROB_CAMERA_BURST * mi {
                    let variant = fastrand::u8(0..4);
                    let (param, value, duration) = match variant {
                        0 => (0, 0.3 * intensity_scale, 45u32),  // Zoom pulse
                        1 => (1, 0.15 * intensity_scale, 60u32), // Rotation pulse
                        2 => (2, 0.3 * intensity_scale, 60u32),  // Pan X drift
                        _ => (3, 0.3 * intensity_scale, 60u32),  // Pan Y drift
                    };
                    macros.camera_param = param;
                    macros.camera =
                        Some(SmoothOverride::new(value, duration, MUTATION_RAMP_FRAMES));
                    mutations += 1;
                }

                if mutations > 0 {
                    macros.frames_since_last = 0;
                }
            }

            // === 2b. LOW-ENERGY DRIFT ===
            if energy == 0 && frame_idx % 120 == 0 {
                match fastrand::u8(0..3) {
                    0 => {
                        frame_config.glow_intensity = (frame_config.glow_intensity
                            + (fastrand::f32() - 0.5) * 0.15)
                            .clamp(0.0, 2.0);
                    }
                    1 => {
                        frame_config.saturation = (frame_config.saturation
                            + (fastrand::f32() - 0.5) * 0.2)
                            .clamp(0.0, 3.0);
                    }
                    _ => {
                        frame_config.brightness = (frame_config.brightness
                            + (fastrand::f32() - 0.5) * 0.1)
                            .clamp(-1.0, 1.0);
                    }
                }
            }

            // === 3. APPLY MACRO OVERLAYS ===
            macros.apply(&mut frame_config);

            // === 4. RENDER PIPELINE ===
            let have_source = if let Some(src_frame) = source.next_frame() {
                if transformed_source.width != src_frame.width
                    || transformed_source.height != src_frame.height
                {
                    transformed_source = FrameBuffer::new(src_frame.width, src_frame.height);
                }

                af_render::camera::VirtualCamera::apply_transform(
                    &frame_config,
                    &src_frame,
                    &mut transformed_source,
                );

                let _ = resizer.resize_into(&transformed_source, &mut resized_source);

                compositor.update_if_needed(&frame_config.charset);
                compositor.process(
                    &resized_source,
                    Some(&current_features),
                    &frame_config,
                    &mut grid,
                );
                true
            } else {
                false
            };

            if have_source || prev_grid.width > 0 {
                // 0. Temporal stability (anti-flicker)
                if frame_config.temporal_stability > 0.0 {
                    af_render::effects::apply_temporal_stability(
                        &mut grid,
                        &prev_grid,
                        frame_config.temporal_stability,
                    );
                }

                // Onset envelope tracking
                if current_features.onset {
                    onset_envelope = 1.0;
                } else {
                    onset_envelope *= frame_config.strobe_decay;
                }

                // Color pulse phase
                if frame_config.color_pulse_speed > 0.0 {
                    color_pulse_phase = (color_pulse_phase
                        + frame_config.color_pulse_speed / target_fps as f32)
                        % 1.0;
                } else {
                    color_pulse_phase = 0.0;
                }

                // 1. Wave distortion
                if frame_config.wave_amplitude > 0.001 {
                    wave_phase = (wave_phase + frame_config.wave_speed / target_fps as f32)
                        % std::f32::consts::TAU;
                }
                let wave_phase_total =
                    wave_phase + current_features.beat_phase * std::f32::consts::TAU * 0.5;
                af_render::effects::apply_wave_distortion(
                    &mut grid,
                    frame_config.wave_amplitude,
                    frame_config.wave_speed,
                    wave_phase_total,
                    &mut effect_row_buf,
                );

                // 2. Chromatic aberration
                af_render::effects::apply_chromatic_aberration(
                    &mut grid,
                    frame_config.chromatic_offset,
                    &mut effect_fg_buf,
                );

                // 3. Color pulse
                af_render::effects::apply_color_pulse(&mut grid, color_pulse_phase);

                // 4. Fade trails
                if frame_config.fade_decay > 0.0 {
                    af_render::effects::apply_fade_trails(
                        &mut grid,
                        &prev_grid,
                        frame_config.fade_decay,
                    );
                }

                // 5. Strobe
                af_render::effects::apply_strobe(
                    &mut grid,
                    onset_envelope,
                    frame_config.beat_flash_intensity,
                );

                // 6. Scan lines
                af_render::effects::apply_scan_lines(
                    &mut grid,
                    frame_config.scanline_gap,
                    frame_config.scanline_darken,
                );

                // 7. Glow
                if frame_config.glow_intensity > 0.0 {
                    af_render::effects::apply_glow(
                        &mut grid,
                        frame_config.glow_intensity,
                        &mut glow_brightness_buf,
                    );
                }

                // Save grid for next frame
                prev_grid.copy_from(&grid);

                raster_fb.data.fill(0);
                rasterizer.render(&grid, &mut raster_fb, frame_config.zalgo_intensity);

                if let Err(e) = muxer.write_frame(&raster_fb) {
                    log::warn!("Pipe write failed (likely interrupted): {e}");
                    break;
                }
            }

            // Progress with ETA
            if frame_idx % 100 == 0 && frame_idx > 0 {
                let elapsed = render_start.elapsed().as_secs_f64();
                let fps_actual = frame_idx as f64 / elapsed;
                let remaining = (total_frames - frame_idx) as f64 / fps_actual;
                log::info!(
                    "Progress: {frame_idx}/{total_frames} ({:.1}%) — {:.1} fps — ETA {:.0}s",
                    frame_idx as f64 / total_frames as f64 * 100.0,
                    fps_actual,
                    remaining,
                );
            }
        }

        log::info!("Clôture du flux vidéo...");
        muxer.finish()?;

        // === Étape 4 : Muxage Audio + Vidéo ===
        log::info!("Étape 4/4 : Muxing Audio/Video Final");
        mux_audio_video(&temp_video, audio_path, final_output)?;

        let _ = std::fs::remove_file(temp_video);

        log::info!("Export réussi vers {}", final_output.display());
        Ok(())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(all(test, feature = "video"))]
mod tests {
    use super::*;

    #[test]
    fn smooth_override_ramp_up_hold_down() {
        let ovr = SmoothOverride::new(2.0, 60, 12);
        // Frame 0: start of ramp — near 0
        assert!(ovr.value() < 0.01, "frame 0 should be near 0");

        // At ramp_frames: should be at target
        let mut o = SmoothOverride::new(2.0, 60, 12);
        for _ in 0..12 {
            o.tick();
        }
        assert!(
            (o.value() - 2.0).abs() < 0.01,
            "after ramp should be near target"
        );

        // Mid hold: should be exactly target
        let mut o2 = SmoothOverride::new(2.0, 60, 12);
        for _ in 0..30 {
            o2.tick();
        }
        assert!(
            (o2.value() - 2.0).abs() < f32::EPSILON,
            "hold phase should be at target"
        );

        // Near end: should be ramping down
        let mut o3 = SmoothOverride::new(2.0, 60, 12);
        for _ in 0..58 {
            o3.tick();
        }
        assert!(o3.value() < 1.5, "ramp down should reduce value");
    }

    #[test]
    fn interpolate_configs_endpoints() {
        let a = RenderConfig {
            contrast: 1.0,
            brightness: 0.0,
            ..RenderConfig::default()
        };

        let b = RenderConfig {
            contrast: 2.0,
            brightness: 0.5,
            ..RenderConfig::default()
        };

        let mut out = RenderConfig::default();

        interpolate_configs(&a, &b, 0.0, &mut out);
        assert!((out.contrast - 1.0).abs() < f32::EPSILON, "t=0 -> from");

        interpolate_configs(&a, &b, 1.0, &mut out);
        assert!((out.contrast - 2.0).abs() < f32::EPSILON, "t=1 -> to");

        interpolate_configs(&a, &b, 0.5, &mut out);
        assert!((out.contrast - 1.5).abs() < f32::EPSILON, "t=0.5 -> mid");
        assert!(
            (out.brightness - 0.25).abs() < f32::EPSILON,
            "t=0.5 brightness"
        );
    }

    #[test]
    fn preset_sequencer_cycles() {
        let presets = vec![
            ("a".into(), RenderConfig::default()),
            ("b".into(), RenderConfig::default()),
        ];
        let mut seq = PresetSequencer::new(presets);
        assert_eq!(seq.presets[seq.current_idx].0, "a");
        seq.advance(1);
        // After advance, transition starts; complete it
        let mut out = RenderConfig::default();
        seq.write_config(&mut out);
        assert_eq!(seq.presets[seq.current_idx].0, "b");
        seq.advance(1);
        seq.write_config(&mut out);
        assert_eq!(seq.presets[seq.current_idx].0, "a"); // Cycle
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn load_all_presets_finds_toml_files() {
        // Ensure we're in the workspace root (tests may run from crate dir)
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest.parent().and_then(|p| p.parent()).unwrap();
        std::env::set_current_dir(workspace_root).unwrap();

        let presets = load_all_presets();
        assert!(
            presets.len() >= 20,
            "Expected >=20 presets, found {}",
            presets.len()
        );
        assert!(presets.iter().all(|(name, _)| !name.is_empty()));
        // Should be sorted
        let names: Vec<&str> = presets.iter().map(|(n, _)| n.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted);
    }
}
