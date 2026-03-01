use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::creation::CreationEngine;
use af_ascii::compositor::Compositor;
use af_audio::state::AudioCommand;
use af_core::charset;
use af_core::clock::MediaClock;
use af_core::config::{BgStyle, ColorMode, DitherMode, RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};

use af_render::fps::FpsCounter;
use af_render::ui::{DrawContext, RenderState, SIDEBAR_WIDTH, SPECTRUM_HEIGHT};
use af_source::resize::Resizer;
#[cfg(feature = "video")]
use af_source::video::VideoCommand;
use anyhow::Result;
use arc_swap::ArcSwap;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;

use crate::pipeline;

/// Category of a loaded media file, determined by extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
    Audio,
}

/// Classify a file path into a media type based on extension.
fn classify_media(path: &Path) -> Option<MediaType> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "bmp" | "gif" => Some(MediaType::Image),
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "flv" | "webm" | "m4v" | "ts" | "mpg" | "mpeg" => {
            Some(MediaType::Video)
        }
        "wav" | "mp3" | "flac" | "ogg" | "aac" | "m4a" | "wma" | "opus" => Some(MediaType::Audio),
        _ => None,
    }
}

/// Application state.
///
/// # Example
/// ```
/// use af_app::app::AppState;
/// let state = AppState::Running;
/// assert!(matches!(state, AppState::Running));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppState {
    /// L'application est en cours d'exécution normale.
    Running,
    /// Pause (audio et vidéo gelés, rendu continue sur dernière frame).
    Paused,
    /// Overlay d'aide affiché (touche ?).
    Help,
    /// Éditeur de charset personnalisé affiché (touche C).
    CharsetEdit,
    /// Mode création interactif (effets audio-réactifs avec presets).
    CreationMode,
    /// Fermeture de l'application. doit se terminer au prochain tour de boucle.
    Quitting,
}

/// Main application struct holding all state.
#[allow(clippy::struct_excessive_bools)]
pub struct App {
    /// Current application state.
    pub state: AppState,
    /// Config courante (lecture via arc-swap depuis tous les threads).
    pub config: Arc<ArcSwap<RenderConfig>>,
    /// Features audio courantes (lecture via triple buffer).
    pub audio_output: Option<triple_buffer::Output<AudioFeatures>>,
    /// Frame source courante.
    pub current_frame: Option<Arc<FrameBuffer>>,
    /// Grille ASCII pré-allouée, réutilisée chaque frame.
    pub grid: AsciiGrid,
    /// Frame resizée, pré-allouée.
    pub resized_frame: FrameBuffer,
    /// Frame d'origine transformée par la caméra virtuelle (Zéro-alloc pre-allouée).
    pub transformed_frame: FrameBuffer,
    /// Flag dirty pour la sidebar (éviter redessin inutile).
    pub sidebar_dirty: bool,
    /// Compteur FPS.
    pub fps_counter: FpsCounter,
    /// Récepteur de frames depuis le source thread.
    pub frame_rx: Option<flume::Receiver<Arc<FrameBuffer>>>,
    /// Dernier terminal size connu (pour détecter les resize).
    pub terminal_size: (u16, u16),
    /// Compositor pour la conversion pixel→ASCII.
    pub compositor: Compositor,
    /// Resizer pré-alloué.
    pub resizer: Resizer,
    /// Previous frame's grid for fade trail effects.
    pub prev_grid: AsciiGrid,
    /// Pre-allocated brightness buffer for glow effect (avoids per-frame alloc).
    pub glow_brightness_buf: Vec<u8>,
    /// Live preset engine : liste des chemins .toml disponibles.
    pub presets: Vec<std::path::PathBuf>,
    /// Index courant dans `presets`.
    pub current_preset_idx: usize,
    /// Channel pour les commandes vidéo (Play, Pause, Seek).
    #[cfg(feature = "video")]
    pub video_cmd_tx: Option<flume::Sender<VideoCommand>>,
    /// Channel pour les commandes audio (Play, Pause, Seek).
    pub audio_cmd_tx: Option<flume::Sender<AudioCommand>>,
    /// Nom du fichier visuel chargé (image/vidéo).
    pub loaded_visual_name: Option<String>,
    /// Nom du fichier audio chargé.
    pub loaded_audio_name: Option<String>,
    /// Flag: l'utilisateur a demandé l'ouverture du file dialog visuel.
    pub open_visual_requested: bool,
    /// Flag: l'utilisateur a demandé l'ouverture du file dialog audio.
    pub open_audio_requested: bool,
    /// Flag: l'utilisateur a demandé l'ouverture du file dialog pour le batch folder.
    pub open_batch_folder_requested: bool,
    /// Buffer local pour l'édition de charset en live.
    pub charset_edit_buf: String,
    /// Position du curseur dans l'éditeur de charset.
    pub charset_edit_cursor: usize,
    /// Horloge partagée A/V (None si pas d'audio fichier chargé).
    pub media_clock: Option<Arc<MediaClock>>,
    /// Pre-allocated fg buffer for chromatic aberration effect.
    pub effect_fg_buf: Vec<(u8, u8, u8)>,
    /// Pre-allocated row buffer for wave distortion effect.
    pub effect_row_buf: Vec<af_core::frame::AsciiCell>,
    /// Onset envelope (continuous, decays via strobe_decay).
    pub onset_envelope: f32,
    /// Color pulse accumulated phase [0.0, 1.0).
    pub color_pulse_phase: f32,
    /// Persistent wave distortion phase (advances per frame).
    pub wave_phase: f32,
    /// Per-mapping EMA smooth state for audio mappings.
    pub mapping_smooth_state: Vec<f32>,
    /// Creation mode engine for automated audio-reactive effects.
    pub creation_engine: CreationEngine,
    /// Whether creation mode modulation is active (independent of overlay visibility).
    pub creation_mode_active: bool,
    /// Scratch RenderConfig reused each frame (avoids per-frame Vec/String alloc).
    pub render_config_scratch: RenderConfig,
    /// Performance warning flag (frame budget exceeded for 10+ consecutive frames).
    pub perf_warning: bool,
    /// Consecutive frames exceeding 1.5× frame budget.
    perf_exceed_count: u8,
    /// Whether shape-matching auto-disable warning has been logged.
    shape_warn_logged: bool,
}

impl App {
    /// Create a new App instance.
    ///
    /// # Errors
    /// Returns an error if terminal size cannot be queried.
    pub fn new(
        config: Arc<ArcSwap<RenderConfig>>,
        audio_output: Option<triple_buffer::Output<AudioFeatures>>,
        frame_rx: Option<flume::Receiver<Arc<FrameBuffer>>>,
        #[cfg(feature = "video")] video_cmd_tx: Option<flume::Sender<VideoCommand>>,
        audio_cmd_tx: Option<flume::Sender<AudioCommand>>,
    ) -> Result<Self> {
        let terminal_size = crossterm::terminal::size()?;
        let canvas_width = terminal_size.0.saturating_sub(SIDEBAR_WIDTH);
        let spectrum_h = if config.load().show_spectrum {
            SPECTRUM_HEIGHT
        } else {
            0u16
        };
        let canvas_height = terminal_size.1.saturating_sub(spectrum_h);
        let initial_charset = config.load().charset.clone();

        let mut presets = Vec::new();
        if let Ok(entries) = std::fs::read_dir("config/presets") {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type()
                    && ft.is_file()
                    && entry.path().extension().is_some_and(|e| e == "toml")
                {
                    presets.push(entry.path());
                }
            }
        }
        presets.sort(); // Predictable iteration order

        Ok(Self {
            state: AppState::Running,
            config,
            audio_output,
            current_frame: None,
            grid: AsciiGrid::new(canvas_width, canvas_height),
            resized_frame: FrameBuffer::new(u32::from(canvas_width), u32::from(canvas_height)),
            transformed_frame: FrameBuffer::new(1, 1), // Will be resized on first real frame
            sidebar_dirty: true,
            fps_counter: FpsCounter::new(60),
            frame_rx,
            terminal_size: (0, 0), // Force initial resize trigger
            compositor: Compositor::new(&initial_charset),
            resizer: Resizer::new(),
            prev_grid: AsciiGrid::new(canvas_width, canvas_height),
            glow_brightness_buf: Vec::with_capacity(
                usize::from(canvas_width) * usize::from(canvas_height),
            ),
            presets,
            current_preset_idx: 0,
            #[cfg(feature = "video")]
            video_cmd_tx,
            audio_cmd_tx,
            loaded_visual_name: None,
            loaded_audio_name: None,
            open_visual_requested: false,
            open_audio_requested: false,
            open_batch_folder_requested: false,
            charset_edit_buf: String::new(),
            charset_edit_cursor: 0,

            media_clock: None,
            effect_fg_buf: Vec::new(),
            effect_row_buf: Vec::new(),
            onset_envelope: 0.0,
            color_pulse_phase: 0.0,
            wave_phase: 0.0,
            mapping_smooth_state: Vec::new(),
            creation_engine: CreationEngine::default(),
            creation_mode_active: false,
            render_config_scratch: RenderConfig::default(),
            perf_warning: false,
            perf_exceed_count: 0,
            shape_warn_logged: false,
        })
    }

    /// Main event loop per spec §29.
    ///
    /// # Errors
    /// Returns an error if terminal operations fail.
    #[allow(clippy::too_many_lines)]
    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut last_frame = Instant::now();

        loop {
            // === Sortie si quitting ===
            if self.state == AppState::Quitting {
                break;
            }

            // === Calcul du frame timing ===
            let config_guard = self.config.load();
            let frame_duration = Duration::from_secs_f64(1.0 / f64::from(config_guard.target_fps));
            drop(config_guard);

            let now = Instant::now();
            let elapsed = now - last_frame;

            if elapsed < frame_duration {
                // Dormir le temps restant, mais rester réactif aux événements
                let remaining = frame_duration.saturating_sub(elapsed);
                if event::poll(remaining)? {
                    self.handle_event(&event::read()?);
                }
                continue;
            }
            last_frame = now;

            // === Polling événements non-bloquant ===
            while event::poll(Duration::ZERO)? {
                self.handle_event(&event::read()?);
            }

            // === File dialogs si demandés ===
            if self.open_visual_requested {
                self.open_visual_requested = false;
                self.open_visual_dialog(&mut terminal);
            }
            if self.open_audio_requested {
                self.open_audio_requested = false;
                self.open_audio_dialog(&mut terminal);
            }
            if self.open_batch_folder_requested {
                self.open_batch_folder_requested = false;
                self.open_batch_folder_dialog(&mut terminal);
            }

            // === Vérifier resize terminal ===
            self.check_resize()?;

            // === Lire audio features (non-bloquant) ===
            let audio_features = self.audio_output.as_mut().map(|out| *out.read());

            // === Lire frame source ===
            if let Some(ref rx) = self.frame_rx
                && let Ok(frame) = rx.try_recv()
            {
                self.current_frame = Some(frame);
            }

            // === Appliquer audio mappings à la config ===
            let config = self.config.load();
            // R1: reuse scratch RenderConfig (clone_from preserves Vec/String capacity)
            let mut render_config = std::mem::take(&mut self.render_config_scratch);
            render_config.clone_from(&config);
            if let Some(ref features) = audio_features {
                pipeline::apply_audio_mappings(
                    &mut render_config,
                    features,
                    self.onset_envelope,
                    &mut self.mapping_smooth_state,
                );
            }

            // Update onset envelope (continuous decay — must run even without source frame)
            if let Some(ref features) = audio_features {
                if features.onset {
                    self.onset_envelope = 1.0;
                } else {
                    self.onset_envelope *= render_config.strobe_decay;
                }
            }

            // === Process source frame into ASCII grid ===
            let render_start = Instant::now();
            if let Some(ref source_frame) = self.current_frame {
                // B.4: Auto-disable shape matching on large grids (>10k cells)
                if u32::from(self.grid.width) * u32::from(self.grid.height) > 10_000
                    && render_config.shape_matching
                {
                    render_config.shape_matching = false;
                    if !self.shape_warn_logged {
                        log::warn!(
                            "Shape matching auto-disabled: grid {}×{} > 10k cells",
                            self.grid.width,
                            self.grid.height
                        );
                        self.shape_warn_logged = true;
                    }
                }
                // Apply Virtual Camera transformations (Zoom, Pan, Rot) on pure pixels *before* ASCIIfying
                if self.transformed_frame.width != source_frame.width
                    || self.transformed_frame.height != source_frame.height
                {
                    // Only allocate if source dimensions change (e.g. video switch)
                    self.transformed_frame =
                        FrameBuffer::new(source_frame.width, source_frame.height);
                }

                af_render::camera::VirtualCamera::apply_transform(
                    &render_config,
                    source_frame,
                    &mut self.transformed_frame,
                );

                // Resize transformed source to grid dimensions
                let _ = self
                    .resizer
                    .resize_into(&self.transformed_frame, &mut self.resized_frame);
                // Convert pixels → ASCII
                self.compositor.process(
                    &self.resized_frame,
                    audio_features.as_ref(),
                    &render_config,
                    &mut self.grid,
                );

                // === Post-processing effects ===
                // 0. Temporal stability (anti-flicker, before all effects)
                if render_config.temporal_stability > 0.0 {
                    af_render::effects::apply_temporal_stability(
                        &mut self.grid,
                        &self.prev_grid,
                        render_config.temporal_stability,
                    );
                }

                // Creation mode modulation (runs even when overlay is hidden)
                if self.creation_mode_active {
                    let image_feats = crate::creation::compute_image_features(&self.grid);
                    let dt = 1.0 / render_config.target_fps as f32;
                    if let Some(ref features) = audio_features {
                        self.creation_engine.modulate(
                            features,
                            &image_feats,
                            &mut render_config,
                            self.onset_envelope,
                            dt,
                        );
                    }
                }

                // Update color pulse phase (reset to 0 when speed is 0 to avoid frozen hue)
                if render_config.color_pulse_speed > 0.0 {
                    self.color_pulse_phase = (self.color_pulse_phase
                        + render_config.color_pulse_speed / render_config.target_fps as f32)
                        % 1.0;
                } else {
                    self.color_pulse_phase = 0.0;
                }

                // 1. Wave distortion (persistent phase + beat modulator)
                if render_config.wave_amplitude > 0.001 {
                    self.wave_phase = (self.wave_phase
                        + render_config.wave_speed / render_config.target_fps as f32)
                        % std::f32::consts::TAU;
                }
                let wave_phase_total = self.wave_phase
                    + audio_features
                        .as_ref()
                        .map_or(0.0, |f| f.beat_phase * std::f32::consts::TAU * 0.5);
                af_render::effects::apply_wave_distortion(
                    &mut self.grid,
                    render_config.wave_amplitude,
                    render_config.wave_speed,
                    wave_phase_total,
                    &mut self.effect_row_buf,
                );

                // 2. Chromatic aberration (color channel offset)
                af_render::effects::apply_chromatic_aberration(
                    &mut self.grid,
                    render_config.chromatic_offset,
                    &mut self.effect_fg_buf,
                );

                // 3. Color pulse (hue rotation)
                af_render::effects::apply_color_pulse(&mut self.grid, self.color_pulse_phase);

                // 4. Fade trails (blend with previous frame)
                if render_config.fade_decay > 0.0 {
                    af_render::effects::apply_fade_trails(
                        &mut self.grid,
                        &self.prev_grid,
                        render_config.fade_decay,
                    );
                }

                // 5. Strobe (onset envelope-driven brightness boost)
                af_render::effects::apply_strobe(
                    &mut self.grid,
                    self.onset_envelope,
                    render_config.beat_flash_intensity,
                );

                // 6. Scan lines
                af_render::effects::apply_scan_lines(
                    &mut self.grid,
                    render_config.scanline_gap,
                    render_config.scanline_darken,
                );

                // 7. Glow (halo — last, on final brightness)
                if render_config.glow_intensity > 0.0 {
                    af_render::effects::apply_glow(
                        &mut self.grid,
                        render_config.glow_intensity,
                        &mut self.glow_brightness_buf,
                    );
                }

                // Save current grid for next frame's fade trails (zero-alloc copy)
                self.prev_grid.copy_from(&self.grid);
            }

            // B.1: Frame budget tracking
            let render_elapsed = render_start.elapsed();
            let frame_budget = Duration::from_secs_f64(1.0 / f64::from(render_config.target_fps));
            if render_elapsed > frame_budget + frame_budget / 2 {
                self.perf_exceed_count = self.perf_exceed_count.saturating_add(1);
                if self.perf_exceed_count >= 10 {
                    self.perf_warning = true;
                }
            } else {
                self.perf_exceed_count = self.perf_exceed_count.saturating_sub(1);
                self.perf_warning = self.perf_exceed_count > 0 && self.perf_warning;
            }

            // === Rendu terminal ===
            self.fps_counter.tick();
            let state = self.render_state();
            let grid = &self.grid;
            let fps_counter = &self.fps_counter;
            let preset_name = if self.presets.is_empty() {
                None
            } else {
                self.presets[self.current_preset_idx]
                    .file_stem()
                    .and_then(|s| s.to_str())
            };

            let loaded_visual = self.loaded_visual_name.as_deref();
            let loaded_audio = self.loaded_audio_name.as_deref();

            let layout_charset_edit = if state == RenderState::CharsetEdit {
                Some((self.charset_edit_buf.as_str(), self.charset_edit_cursor))
            } else {
                None
            };

            let layout_creation = if state == RenderState::CreationMode {
                let mut effects = [("", 0.0f32, 0.0f32); 10];
                for (i, slot) in effects
                    .iter_mut()
                    .enumerate()
                    .take(crate::creation::NUM_EFFECTS)
                {
                    *slot = (
                        crate::creation::EFFECT_NAMES[i],
                        self.creation_engine.effect_value(i, &render_config),
                        self.creation_engine.effect_max(i),
                    );
                }
                Some(af_render::ui::CreationOverlayData {
                    auto_mode: self.creation_engine.auto_mode,
                    master_intensity: self.creation_engine.master_intensity,
                    preset_name: self.creation_engine.active_preset.name(),
                    selected_effect: self.creation_engine.selected_effect,
                    effects,
                })
            } else {
                None
            };

            let creation_mode_active = self.creation_mode_active;
            let perf_warning = self.perf_warning;
            let base_config = self.config.load();
            terminal.draw(|frame| {
                let ctx = DrawContext {
                    grid,
                    config: &render_config,
                    base_config: &base_config,
                    audio: audio_features.as_ref(),
                    fps_counter,
                    preset_name,
                    loaded_visual,
                    loaded_audio,
                    state: &state,
                    charset_edit: layout_charset_edit,

                    creation: layout_creation.as_ref(),
                    creation_mode_active,
                    perf_warning,
                };
                af_render::ui::draw(frame, &ctx);
            })?;
            self.sidebar_dirty = false;

            // Restore scratch (preserves internal Vec/String allocations for next frame)
            self.render_config_scratch = render_config;
        }

        Ok(())
    }

    /// Convert `AppState` to `RenderState` for the UI.
    fn render_state(&self) -> RenderState {
        match self.state {
            AppState::Running => RenderState::Running,
            AppState::Paused => RenderState::Paused,
            AppState::Help => RenderState::Help,
            AppState::CharsetEdit => RenderState::CharsetEdit,

            AppState::CreationMode => RenderState::CreationMode,
            AppState::Quitting => RenderState::Quitting,
        }
    }

    /// Handle a terminal event by dispatching to focused sub-handlers.
    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = *event
        {
            if modifiers.contains(KeyModifiers::CONTROL) {
                match code {
                    KeyCode::Char('d') => {
                        self.open_batch_folder_requested = true;
                        self.sidebar_dirty = true;
                    }
                    KeyCode::Char('o') => {
                        self.open_visual_requested = true;
                        self.sidebar_dirty = true;
                    }
                    _ => {}
                }
                return;
            }

            if self.state == AppState::CharsetEdit {
                self.handle_charset_edit_key(code);
                return;
            }
            if self.state == AppState::CreationMode {
                self.handle_creation_key(code);
                return;
            }

            match code {
                KeyCode::Char('q' | '?' | ' ' | 'o' | 'O') | KeyCode::Esc => {
                    self.handle_navigation_key(code);
                }
                KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Char(
                    '1'..='9'
                    | '0'
                    | 'd'
                    | 'D'
                    | 'c'
                    | 'i'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '-'
                    | '+'
                    | '='
                    | 'e'
                    | 'E'
                    | 's'
                    | 'a'
                    | 'm'
                    | 'b'
                    | 'x'
                    | 'v'
                    | 'p'
                    | 'P'
                    | 'C'
                    | 'K'
                    | 'n',
                ) => self.handle_render_key(code),
                KeyCode::Char(
                    'f' | 'F' | 'g' | 'G' | 'r' | 'R' | 'w' | 'W' | 'h' | 'H' | 'l' | 'L' | 't'
                    | 'T' | 'z' | 'Z' | 'y' | 'Y' | 'j' | 'J' | 'u' | 'U' | '<' | '>' | ',' | '.'
                    | ';' | '\'' | ':' | '"',
                ) => self.handle_effect_key(code),
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    self.handle_playback_key(code);
                }
                _ => {}
            }
        }
    }

    /// Charset Editor logic
    #[allow(clippy::assigning_clones)]
    fn handle_charset_edit_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.state = AppState::Running;
                self.sidebar_dirty = true;
            }
            KeyCode::Enter => {
                if self.charset_edit_buf.chars().count() >= 2 {
                    let new_charset = self.charset_edit_buf.clone();
                    self.toggle_config(|c| {
                        c.charset = new_charset;
                        c.charset_index = 10;
                    });
                    self.state = AppState::Running;
                    self.sidebar_dirty = true;
                }
            }
            KeyCode::Backspace => {
                if self.charset_edit_cursor > 0 {
                    let mut chars: Vec<char> = self.charset_edit_buf.chars().collect();
                    chars.remove(self.charset_edit_cursor - 1);
                    self.charset_edit_buf = chars.into_iter().collect();
                    self.charset_edit_cursor -= 1;
                    self.apply_charset_preview();
                }
            }
            KeyCode::Delete => {
                let chars: Vec<char> = self.charset_edit_buf.chars().collect();
                if self.charset_edit_cursor < chars.len() {
                    let mut chars = chars;
                    chars.remove(self.charset_edit_cursor);
                    self.charset_edit_buf = chars.into_iter().collect();
                    self.apply_charset_preview();
                }
            }
            KeyCode::Left => {
                if self.charset_edit_cursor > 0 {
                    self.charset_edit_cursor -= 1;
                }
            }
            KeyCode::Right => {
                if self.charset_edit_cursor < self.charset_edit_buf.chars().count() {
                    self.charset_edit_cursor += 1;
                }
            }
            KeyCode::Home => {
                self.charset_edit_cursor = 0;
            }
            KeyCode::End => {
                self.charset_edit_cursor = self.charset_edit_buf.chars().count();
            }
            KeyCode::Char(ch) => {
                let mut chars: Vec<char> = self.charset_edit_buf.chars().collect();
                chars.insert(self.charset_edit_cursor, ch);
                self.charset_edit_buf = chars.into_iter().collect();
                self.charset_edit_cursor += 1;
                self.apply_charset_preview();
            }
            _ => {}
        }
    }

    #[allow(clippy::assigning_clones)]
    fn apply_charset_preview(&mut self) {
        if self.charset_edit_buf.chars().count() >= 2 {
            let buf = self.charset_edit_buf.clone();
            self.toggle_config(|c| {
                c.charset = buf;
                c.charset_index = 10;
            });
        }
    }

    /// Navigation keys: quit, help, pause/play.
    fn handle_navigation_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => {
                self.state = AppState::Quitting;
                self.send_quit_commands();
            }
            KeyCode::Esc => {
                if self.state == AppState::Help {
                    self.state = AppState::Running;
                    self.sidebar_dirty = true;
                } else {
                    self.state = AppState::Quitting;
                    self.send_quit_commands();
                }
            }
            KeyCode::Char('?') => {
                self.state = if self.state == AppState::Help {
                    AppState::Running
                } else {
                    AppState::Help
                };
                self.sidebar_dirty = true;
            }
            KeyCode::Char(' ') => {
                self.state = if self.state == AppState::Paused {
                    self.send_play_commands();
                    AppState::Running
                } else {
                    self.send_pause_commands();
                    AppState::Paused
                };
                self.sidebar_dirty = true;
            }
            KeyCode::Char('o') => {
                self.open_visual_requested = true;
                self.sidebar_dirty = true;
            }
            KeyCode::Char('O') => {
                self.open_audio_requested = true;
                self.sidebar_dirty = true;
            }
            _ => {}
        }
    }

    /// Creation Mode key handler.
    fn handle_creation_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.state = AppState::Running;
                self.sidebar_dirty = true;
            }
            KeyCode::Up => {
                if self.creation_engine.selected_effect > 0 {
                    self.creation_engine.selected_effect -= 1;
                }
            }
            KeyCode::Down => {
                if self.creation_engine.selected_effect < crate::creation::NUM_EFFECTS - 1 {
                    self.creation_engine.selected_effect += 1;
                }
            }
            KeyCode::Right => {
                self.adjust_creation_effect(0.1);
            }
            KeyCode::Left => {
                self.adjust_creation_effect(-0.1);
            }
            KeyCode::Char('a') => {
                self.creation_engine.auto_mode = !self.creation_engine.auto_mode;
            }
            KeyCode::Char('p') => {
                self.creation_engine.active_preset = self.creation_engine.active_preset.next();
            }
            KeyCode::Char('q') => {
                // Fully deactivate creation mode
                self.creation_mode_active = false;
                self.state = AppState::Running;
                self.sidebar_dirty = true;
            }
            _ => {}
        }
    }

    /// Adjust the selected effect value in creation mode (index 0 = Master).
    fn adjust_creation_effect(&mut self, delta: f32) {
        let idx = self.creation_engine.selected_effect;
        if idx == 0 {
            // Master intensity — not in RenderConfig
            self.creation_engine.master_intensity =
                (self.creation_engine.master_intensity + delta).clamp(0.0, 2.0);
            return;
        }
        let max = self.creation_engine.effect_max(idx);
        self.toggle_config(|c| match idx {
            1 => c.beat_flash_intensity = (c.beat_flash_intensity + delta).clamp(0.0, max),
            2 => c.fade_decay = (c.fade_decay + delta).clamp(0.0, max),
            3 => c.glow_intensity = (c.glow_intensity + delta).clamp(0.0, max),
            4 => c.chromatic_offset = (c.chromatic_offset + delta * 5.0).clamp(0.0, max),
            5 => c.wave_amplitude = (c.wave_amplitude + delta).clamp(0.0, max),
            6 => c.color_pulse_speed = (c.color_pulse_speed + delta * 5.0).clamp(0.0, max),
            7 => {
                let v = (f32::from(c.scanline_gap) + delta * 10.0).clamp(0.0, max) as u8;
                c.scanline_gap = v;
            }
            8 => c.zalgo_intensity = (c.zalgo_intensity + delta * 5.0).clamp(0.0, max),
            9 => c.strobe_decay = (c.strobe_decay + delta).clamp(0.0, max),
            _ => {}
        });
    }

    /// Render parameter keys: mode, charset, density, color, etc.
    #[allow(clippy::too_many_lines)]
    fn handle_render_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Tab => {
                let config = self.config.load();
                let mut new = (**config).clone();
                new.render_mode = match new.render_mode {
                    RenderMode::Ascii => RenderMode::HalfBlock,
                    RenderMode::HalfBlock => RenderMode::Braille,
                    RenderMode::Braille => RenderMode::Quadrant,
                    RenderMode::Quadrant => RenderMode::Sextant,
                    RenderMode::Sextant => RenderMode::Octant,
                    RenderMode::Octant => RenderMode::Ascii,
                };
                self.config.store(Arc::new(new));
                self.sidebar_dirty = true;
                self.terminal_size = (0, 0);
            }
            KeyCode::BackTab => {
                let config = self.config.load();
                let mut new = (**config).clone();
                new.render_mode = match new.render_mode {
                    RenderMode::Ascii => RenderMode::Octant,
                    RenderMode::HalfBlock => RenderMode::Ascii,
                    RenderMode::Braille => RenderMode::HalfBlock,
                    RenderMode::Quadrant => RenderMode::Braille,
                    RenderMode::Sextant => RenderMode::Quadrant,
                    RenderMode::Octant => RenderMode::Sextant,
                };
                self.config.store(Arc::new(new));
                self.sidebar_dirty = true;
                self.terminal_size = (0, 0);
            }
            KeyCode::Char('1') => self.set_charset(0, charset::CHARSET_FULL),
            KeyCode::Char('2') => self.set_charset(1, charset::CHARSET_DENSE),
            KeyCode::Char('3') => self.set_charset(2, charset::CHARSET_SHORT_1),
            KeyCode::Char('4') => self.set_charset(3, charset::CHARSET_BLOCKS),
            KeyCode::Char('5') => self.set_charset(4, charset::CHARSET_MINIMAL),
            KeyCode::Char('6') => self.set_charset(5, charset::CHARSET_GLITCH_1),
            KeyCode::Char('7') => self.set_charset(6, charset::CHARSET_GLITCH_2),
            KeyCode::Char('8') => self.set_charset(7, charset::CHARSET_EDGE),
            KeyCode::Char('9') => self.set_charset(8, charset::CHARSET_DIGITAL),
            KeyCode::Char('0') => self.set_charset(9, charset::CHARSET_BINARY),
            KeyCode::Char('d') => {
                self.toggle_config(|c| c.density_scale = (c.density_scale - 0.25).max(0.25));
                self.terminal_size = (0, 0); // recalcul pixel dimensions
            }
            KeyCode::Char('D') => {
                self.toggle_config(|c| c.density_scale = (c.density_scale + 0.25).min(4.0));
                self.terminal_size = (0, 0); // recalcul pixel dimensions
            }
            KeyCode::Char('c') => self.toggle_config(|c| c.color_enabled = !c.color_enabled),
            KeyCode::Char('i') => self.toggle_config(|c| c.invert = !c.invert),
            KeyCode::Char('[') => self.toggle_config(|c| c.contrast = (c.contrast - 0.1).max(0.1)),
            KeyCode::Char(']') => self.toggle_config(|c| c.contrast = (c.contrast + 0.1).min(3.0)),
            KeyCode::Char('{') => {
                self.toggle_config(|c| c.brightness = (c.brightness - 0.05).max(-1.0));
            }
            KeyCode::Char('}') => {
                self.toggle_config(|c| c.brightness = (c.brightness + 0.05).min(1.0));
            }
            KeyCode::Char('-') => {
                self.toggle_config(|c| c.saturation = (c.saturation - 0.1).max(0.0));
            }
            KeyCode::Char('+' | '=') => {
                self.toggle_config(|c| c.saturation = (c.saturation + 0.1).min(3.0));
            }
            KeyCode::Char('e') => self.toggle_config(|c| {
                c.edge_threshold = if c.edge_threshold > 0.0 { 0.0 } else { 0.3 };
            }),
            KeyCode::Char('E') => self.toggle_config(|c| {
                c.edge_mix = if c.edge_mix >= 1.0 {
                    0.0
                } else {
                    c.edge_mix + 0.25
                };
            }),
            KeyCode::Char('s') => self.toggle_config(|c| c.shape_matching = !c.shape_matching),
            KeyCode::Char('a') => {
                self.toggle_config(|c| {
                    c.aspect_ratio = match c.aspect_ratio {
                        x if (x - 1.5).abs() < 0.01 => 2.0,
                        x if (x - 2.0).abs() < 0.01 => 2.5,
                        _ => 1.5,
                    };
                });
                self.terminal_size = (0, 0); // recalcul pixel dimensions
            }
            KeyCode::Char('m') => self.toggle_config(|c| {
                c.color_mode = match c.color_mode {
                    ColorMode::Direct => ColorMode::HsvBright,
                    ColorMode::HsvBright => ColorMode::Oklab,
                    ColorMode::Oklab => ColorMode::Quantized,
                    ColorMode::Quantized => ColorMode::Direct,
                };
            }),
            KeyCode::Char('b') => self.toggle_config(|c| {
                c.bg_style = match c.bg_style {
                    BgStyle::Black => BgStyle::SourceDim,
                    BgStyle::SourceDim => BgStyle::Transparent,
                    BgStyle::Transparent => BgStyle::Black,
                };
            }),
            KeyCode::Char('x') => {
                self.toggle_config(|c| c.fullscreen = !c.fullscreen);
                // Forcer le recalcul de la taille des buffers au prochain tour
                self.terminal_size = (0, 0);
            }
            KeyCode::Char('v') => {
                self.toggle_config(|c| c.show_spectrum = !c.show_spectrum);
                self.terminal_size = (0, 0); // recalcul layout
            }
            KeyCode::Char('p') => self.cycle_preset(true),
            KeyCode::Char('P') => self.cycle_preset(false),
            KeyCode::Char('C') => {
                let config = self.config.load();
                self.charset_edit_buf.clone_from(&config.charset);
                self.charset_edit_cursor = self.charset_edit_buf.chars().count();
                self.state = AppState::CharsetEdit;
                self.sidebar_dirty = true;
            }
            KeyCode::Char('K') => {
                if self.creation_mode_active {
                    // Toggle overlay visibility (modulation continues)
                    if self.state == AppState::CreationMode {
                        self.state = AppState::Running;
                    } else {
                        self.state = AppState::CreationMode;
                    }
                } else {
                    // Activate creation mode + open overlay
                    self.creation_mode_active = true;
                    self.state = AppState::CreationMode;
                }
                self.sidebar_dirty = true;
            }
            KeyCode::Char('n') => {
                self.toggle_config(|c| {
                    c.dither_mode = match c.dither_mode {
                        DitherMode::Bayer8x8 => DitherMode::BlueNoise16,
                        DitherMode::BlueNoise16 => DitherMode::None,
                        DitherMode::None => DitherMode::Bayer8x8,
                    };
                });
            }
            _ => {}
        }
    }

    /// Effect keys: fade, glow, chromatic, wave, color pulse, scan lines, strobe, camera.
    #[allow(clippy::too_many_lines)]
    fn handle_effect_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('f') => {
                self.toggle_config(|c| c.fade_decay = (c.fade_decay - 0.1).max(0.0));
            }
            KeyCode::Char('F') => {
                self.toggle_config(|c| c.fade_decay = (c.fade_decay + 0.1).min(1.0));
            }
            KeyCode::Char('g') => {
                self.toggle_config(|c| c.glow_intensity = (c.glow_intensity - 0.1).max(0.0));
            }
            KeyCode::Char('G') => {
                self.toggle_config(|c| c.glow_intensity = (c.glow_intensity + 0.1).min(2.0));
            }
            KeyCode::Char('r') => {
                self.toggle_config(|c| c.chromatic_offset = (c.chromatic_offset - 0.5).max(0.0));
            }
            KeyCode::Char('R') => {
                self.toggle_config(|c| c.chromatic_offset = (c.chromatic_offset + 0.5).min(5.0));
            }
            KeyCode::Char('w') => {
                self.toggle_config(|c| c.wave_amplitude = (c.wave_amplitude - 0.1).max(0.0));
            }
            KeyCode::Char('W') => {
                self.toggle_config(|c| c.wave_amplitude = (c.wave_amplitude + 0.1).min(1.0));
            }
            KeyCode::Char('h') => {
                self.toggle_config(|c| c.color_pulse_speed = (c.color_pulse_speed - 0.5).max(0.0));
            }
            KeyCode::Char('H') => {
                self.toggle_config(|c| c.color_pulse_speed = (c.color_pulse_speed + 0.5).min(5.0));
            }
            KeyCode::Char('l') => {
                self.toggle_config(|c| {
                    c.scanline_gap = match c.scanline_gap {
                        0 => 2,
                        2 => 3,
                        3 => 4,
                        4 => 8,
                        _ => 0,
                    };
                });
            }
            KeyCode::Char('L') => {
                self.toggle_config(|c| {
                    c.scanline_gap = match c.scanline_gap {
                        0 => 8,
                        8 => 4,
                        4 => 3,
                        3 => 2,
                        _ => 0,
                    };
                });
            }
            KeyCode::Char('t') => {
                self.toggle_config(|c| {
                    c.beat_flash_intensity = (c.beat_flash_intensity - 0.1).max(0.0);
                });
            }
            KeyCode::Char('T') => {
                self.toggle_config(|c| {
                    c.beat_flash_intensity = (c.beat_flash_intensity + 0.1).min(2.0);
                });
            }
            KeyCode::Char('z') => {
                self.toggle_config(|c| {
                    c.zalgo_intensity = (c.zalgo_intensity - 0.5).max(0.0);
                });
            }
            KeyCode::Char('Z') => {
                self.toggle_config(|c| {
                    c.zalgo_intensity = (c.zalgo_intensity + 0.5).min(5.0);
                });
            }
            KeyCode::Char('y') => {
                self.toggle_config(|c| {
                    c.temporal_stability = (c.temporal_stability - 0.1).max(0.0);
                });
            }
            KeyCode::Char('Y') => {
                self.toggle_config(|c| {
                    c.temporal_stability = (c.temporal_stability + 0.1).min(1.0);
                });
            }
            KeyCode::Char('j') => {
                self.toggle_config(|c| {
                    c.strobe_decay = (c.strobe_decay - 0.05).max(0.5);
                });
            }
            KeyCode::Char('J') => {
                self.toggle_config(|c| {
                    c.strobe_decay = (c.strobe_decay + 0.05).min(0.99);
                });
            }
            KeyCode::Char('u') => {
                self.toggle_config(|c| c.wave_speed = (c.wave_speed - 0.5).max(0.0));
            }
            KeyCode::Char('U') => {
                self.toggle_config(|c| c.wave_speed = (c.wave_speed + 0.5).min(10.0));
            }
            KeyCode::Char('<') => {
                self.toggle_config(|c| {
                    c.camera_zoom_amplitude = (c.camera_zoom_amplitude - 0.1).max(0.1);
                });
            }
            KeyCode::Char('>') => {
                self.toggle_config(|c| {
                    c.camera_zoom_amplitude = (c.camera_zoom_amplitude + 0.1).min(10.0);
                });
            }
            KeyCode::Char(',') => {
                self.toggle_config(|c| c.camera_rotation -= 0.05);
            }
            KeyCode::Char('.') => {
                self.toggle_config(|c| c.camera_rotation += 0.05);
            }
            KeyCode::Char(';') => {
                self.toggle_config(|c| {
                    c.camera_pan_x = (c.camera_pan_x - 0.05).max(-2.0);
                });
            }
            KeyCode::Char('\'') => {
                self.toggle_config(|c| {
                    c.camera_pan_x = (c.camera_pan_x + 0.05).min(2.0);
                });
            }
            KeyCode::Char(':') => {
                self.toggle_config(|c| {
                    c.camera_pan_y = (c.camera_pan_y - 0.05).max(-2.0);
                });
            }
            KeyCode::Char('"') => {
                self.toggle_config(|c| {
                    c.camera_pan_y = (c.camera_pan_y + 0.05).min(2.0);
                });
            }
            _ => {}
        }
    }

    /// Playback / audio keys: sensitivity, seek.
    fn handle_playback_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Up => {
                self.toggle_config(|c| c.audio_sensitivity = (c.audio_sensitivity + 0.1).min(5.0));
            }
            KeyCode::Down => {
                self.toggle_config(|c| c.audio_sensitivity = (c.audio_sensitivity - 0.1).max(0.0));
            }
            KeyCode::Left => {
                #[cfg(feature = "video")]
                if let Some(ref tx) = self.video_cmd_tx {
                    let _ = tx.send(VideoCommand::Seek(-5.0));
                }
                if let Some(ref tx) = self.audio_cmd_tx {
                    let _ = tx.send(AudioCommand::Seek(-5.0));
                }
            }
            KeyCode::Right => {
                #[cfg(feature = "video")]
                if let Some(ref tx) = self.video_cmd_tx {
                    let _ = tx.send(VideoCommand::Seek(5.0));
                }
                if let Some(ref tx) = self.audio_cmd_tx {
                    let _ = tx.send(AudioCommand::Seek(5.0));
                }
            }
            _ => {}
        }
    }

    /// Send quit commands to all threads.
    fn send_quit_commands(&self) {
        #[cfg(feature = "video")]
        if let Some(ref tx) = self.video_cmd_tx {
            let _ = tx.send(VideoCommand::Quit);
        }
        if let Some(ref tx) = self.audio_cmd_tx {
            let _ = tx.send(AudioCommand::Quit);
        }
    }

    /// Send play commands to all threads.
    fn send_play_commands(&self) {
        #[cfg(feature = "video")]
        if let Some(ref tx) = self.video_cmd_tx {
            let _ = tx.send(VideoCommand::Play);
        }
        if let Some(ref tx) = self.audio_cmd_tx {
            let _ = tx.send(AudioCommand::Play);
        }
    }

    /// Send pause commands to all threads.
    fn send_pause_commands(&self) {
        #[cfg(feature = "video")]
        if let Some(ref tx) = self.video_cmd_tx {
            let _ = tx.send(VideoCommand::Pause);
        }
        if let Some(ref tx) = self.audio_cmd_tx {
            let _ = tx.send(AudioCommand::Pause);
        }
    }

    /// Set charset by index.
    fn set_charset(&mut self, index: usize, charset_str: &str) {
        self.toggle_config(|c| {
            c.charset_index = index;
            c.charset = charset_str.to_string();
        });
    }

    /// Helper to atomically update config.
    fn toggle_config(&mut self, mutate: impl FnOnce(&mut RenderConfig)) {
        let config = self.config.load();
        let mut new = (**config).clone();
        mutate(&mut new);
        self.config.store(Arc::new(new));
        self.sidebar_dirty = true;
    }

    /// Check if the terminal has been resized and update buffers accordingly.
    fn check_resize(&mut self) -> Result<()> {
        let new_size = crossterm::terminal::size()?;
        if new_size != self.terminal_size {
            self.terminal_size = new_size;

            let config = self.config.load();

            let (canvas_width, canvas_height) = if config.fullscreen {
                (new_size.0, new_size.1)
            } else {
                let spectrum_h = if config.show_spectrum {
                    SPECTRUM_HEIGHT
                } else {
                    0u16
                };
                (
                    new_size.0.saturating_sub(SIDEBAR_WIDTH),
                    new_size.1.saturating_sub(spectrum_h),
                )
            };

            // Réallouer la grille ASCII (rare, OK d'allouer ici)
            self.grid = AsciiGrid::new(canvas_width, canvas_height);
            self.prev_grid = AsciiGrid::new(canvas_width, canvas_height);

            let density = config.density_scale.clamp(0.25, 4.0);
            let (pixel_w, pixel_h) = match config.render_mode {
                RenderMode::Ascii => (
                    (f32::from(canvas_width) * density) as u32,
                    (f32::from(canvas_height) * density) as u32,
                ),
                RenderMode::HalfBlock => (
                    (f32::from(canvas_width) * density) as u32,
                    (f32::from(canvas_height) * density * 2.0) as u32,
                ),
                RenderMode::Braille | RenderMode::Octant => (
                    (f32::from(canvas_width) * density * 2.0) as u32,
                    (f32::from(canvas_height) * density * 4.0) as u32,
                ),
                RenderMode::Quadrant => (
                    (f32::from(canvas_width) * density * 2.0) as u32,
                    (f32::from(canvas_height) * density * 2.0) as u32,
                ),
                RenderMode::Sextant => (
                    (f32::from(canvas_width) * density * 2.0) as u32,
                    (f32::from(canvas_height) * density * 3.0) as u32,
                ),
            };

            // Appliquer la correction aspect ratio
            let pixel_h_corrected = (pixel_h as f32 / config.aspect_ratio) as u32;
            let final_w = pixel_w.max(1);
            let final_h = pixel_h_corrected.max(1);

            self.resized_frame = FrameBuffer::new(final_w, final_h);

            #[cfg(feature = "video")]
            if let Some(ref tx) = self.video_cmd_tx {
                let _ = tx.send(VideoCommand::Resize(final_w, final_h));
            }

            self.sidebar_dirty = true;

            log::debug!("Terminal resized to {canvas_width}×{canvas_height}");
        }
        Ok(())
    }

    // === File picker & runtime source switching ===
    //
    // Deux canaux indépendants :
    //   o → source visuelle (image OU vidéo) — ne touche pas à l'audio
    //   O → source audio (musique)           — ne touche pas au visuel
    //
    // Cela permet : image+musique, vidéo+musique, vidéo seul, audio seul, etc.

    /// Suspend le TUI, ouvre un dialog natif, restaure le TUI. Retourne le path choisi.
    fn pick_file(
        terminal: &mut DefaultTerminal,
        title: &str,
        filters: &[(&str, &[&str])],
    ) -> Option<std::path::PathBuf> {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).ok();

        let mut dialog = rfd::FileDialog::new().set_title(title);
        for &(name, exts) in filters {
            dialog = dialog.add_filter(name, exts);
        }
        let picked = dialog.pick_file();

        crossterm::terminal::enable_raw_mode().ok();
        crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen).ok();
        terminal.clear().ok();

        picked
    }

    /// Open visual source dialog (image or video).
    fn open_visual_dialog(&mut self, terminal: &mut DefaultTerminal) {
        let filters: &[(&str, &[&str])] = &[
            (
                "Images & Video",
                &[
                    "png", "jpg", "jpeg", "bmp", "gif", "webp", "mp4", "mkv", "avi", "mov", "webm",
                    "flv", "m4v",
                ],
            ),
            ("Images", &["png", "jpg", "jpeg", "bmp", "gif", "webp"]),
            ("Video", &["mp4", "mkv", "avi", "mov", "webm", "flv", "m4v"]),
        ];
        let picked = Self::pick_file(terminal, "Open Visual \u{2014} clasSCII", filters);
        self.terminal_size = (0, 0);

        if let Some(path) = picked {
            if let Some(media_type) = classify_media(&path) {
                match media_type {
                    MediaType::Image | MediaType::Video => {
                        self.shutdown_visual();
                        self.load_visual(&path, media_type);
                    }
                    MediaType::Audio => {
                        log::warn!(
                            "Fichier audio sélectionné dans le dialog visuel — utiliser Shift+O"
                        );
                    }
                }
            } else {
                log::warn!("Extension non reconnue: {}", path.display());
            }
        }
    }

    /// Open audio source dialog.
    fn open_audio_dialog(&mut self, terminal: &mut DefaultTerminal) {
        let filters: &[(&str, &[&str])] = &[(
            "Audio",
            &["wav", "mp3", "flac", "ogg", "aac", "m4a", "opus"],
        )];
        let picked = Self::pick_file(terminal, "Open Audio \u{2014} clasSCII", filters);
        self.terminal_size = (0, 0);

        if let Some(path) = picked {
            self.shutdown_audio();
            self.start_audio_from_path(&path.to_string_lossy());
            self.loaded_audio_name = path.file_name().and_then(|n| n.to_str()).map(String::from);
            self.sidebar_dirty = true;
        }
    }

    /// Open batch folder dialog and run headless batch export.
    fn open_batch_folder_dialog(&mut self, terminal: &mut DefaultTerminal) {
        crossterm::terminal::disable_raw_mode().ok();
        crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen).ok();

        let dialog = rfd::FileDialog::new().set_title("Select Batch Folder \u{2014} clasSCII");
        let picked = dialog.pick_folder();

        if let Some(folder) = picked {
            // Suspend app visually, do the export offline
            println!("\n=== clasSCII BATCH EXPORT ===");
            println!("Target Folder: {}", folder.display());

            #[allow(unused_variables)]
            let config = (**self.config.load()).clone();

            #[cfg(feature = "video")]
            if let Err(e) = crate::batch::run_batch_export(
                &folder, None, None, config, 30, None, false, None, 15.0, None, 1.0,
            ) {
                println!("\n[ERROR] Batch export failed: {e}");
            } else {
                println!("\n[SUCCESS] Batch export completed.");
            }

            println!("\nPress ENTER to return to clasSCII...");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
        }

        crossterm::terminal::enable_raw_mode().ok();
        crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen).ok();
        terminal.clear().ok();
        self.terminal_size = (0, 0);
    }

    /// Stop only the visual source (image/video), leave audio untouched.
    fn shutdown_visual(&mut self) {
        #[cfg(feature = "video")]
        {
            if let Some(ref tx) = self.video_cmd_tx {
                let _ = tx.send(VideoCommand::Quit);
            }
            self.video_cmd_tx = None;
        }
        self.frame_rx = None;
        self.current_frame = None;
    }

    /// Stop only the audio source, leave visual untouched.
    fn shutdown_audio(&mut self) {
        if let Some(ref tx) = self.audio_cmd_tx {
            let _ = tx.send(AudioCommand::Quit);
        }
        self.audio_cmd_tx = None;
        self.audio_output = None;
        self.media_clock = None;
    }

    /// Load a visual source (image or video) and update sidebar name.
    fn load_visual(&mut self, path: &Path, media_type: MediaType) {
        match media_type {
            MediaType::Image => {
                let is_gif = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case("gif"));
                if is_gif {
                    match af_source::image::GifSource::try_new(path) {
                        Ok(Some(gif)) => {
                            log::info!(
                                "GIF animé chargé ({} frames): {}",
                                gif.frame_count(),
                                path.display()
                            );
                            let (frame_tx, frame_rx) = flume::bounded(3);
                            std::thread::spawn(move || {
                                let mut source = gif;
                                loop {
                                    if let Some(frame) =
                                        af_core::traits::Source::next_frame(&mut source)
                                        && frame_tx.send(frame).is_err()
                                    {
                                        break;
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(1));
                                }
                            });
                            self.frame_rx = Some(frame_rx);
                            self.current_frame = None;
                        }
                        Ok(None) => match af_source::image::ImageSource::new(path) {
                            Ok(mut source) => {
                                self.current_frame =
                                    af_core::traits::Source::next_frame(&mut source);
                                self.frame_rx = None;
                                log::info!("Image chargée: {}", path.display());
                            }
                            Err(e) => log::error!("Erreur chargement image: {e}"),
                        },
                        Err(e) => log::error!("Erreur décodage GIF: {e}"),
                    }
                } else {
                    match af_source::image::ImageSource::new(path) {
                        Ok(mut source) => {
                            self.current_frame = af_core::traits::Source::next_frame(&mut source);
                            self.frame_rx = None;
                            log::info!("Image chargée: {}", path.display());
                        }
                        Err(e) => log::error!("Erreur chargement image: {e}"),
                    }
                }
            }
            MediaType::Video => self.start_video(path),
            MediaType::Audio => {} // unreachable in this context
        }
        self.loaded_visual_name = path.file_name().and_then(|n| n.to_str()).map(String::from);
        self.sidebar_dirty = true;
        if self.state == AppState::Paused {
            self.state = AppState::Running;
        }
    }

    /// Start video thread (without touching audio).
    #[cfg(feature = "video")]
    fn start_video(&mut self, path: &Path) {
        let (frame_tx, frame_rx) = flume::bounded(3);
        let (cmd_tx, cmd_rx) = flume::bounded(10);
        let clock = self.media_clock.clone();
        match af_source::video::spawn_video_thread(path.to_path_buf(), frame_tx, cmd_rx, clock) {
            Ok((_handle, _native_dims)) => {
                self.frame_rx = Some(frame_rx);
                self.video_cmd_tx = Some(cmd_tx);
                self.terminal_size = (0, 0);
                log::info!("Vidéo démarrée: {}", path.display());
            }
            Err(e) => log::error!("Erreur démarrage vidéo: {e}"),
        }
    }

    #[cfg(not(feature = "video"))]
    #[allow(clippy::unused_self)]
    fn start_video(&mut self, path: &Path) {
        log::warn!("Feature 'video' non compilée: {}", path.display());
    }

    /// Start audio analysis from a file path.
    fn start_audio_from_path(&mut self, path_str: &str) {
        let clock = Arc::new(MediaClock::new(0));
        match pipeline::start_audio(path_str, &self.config, Arc::clone(&clock)) {
            Ok((output, tx)) => {
                self.audio_output = Some(output);
                self.audio_cmd_tx = tx;
                self.media_clock = Some(clock);
                log::info!("Audio démarré: {path_str}");
            }
            Err(e) => log::warn!("Audio non disponible: {e}"),
        }
    }

    /// Applique le live preset engine.
    fn cycle_preset(&mut self, forward: bool) {
        if self.presets.is_empty() {
            log::warn!("Aucun preset trouvé dans config/presets/");
            return;
        }

        if forward {
            self.current_preset_idx = (self.current_preset_idx + 1) % self.presets.len();
        } else if self.current_preset_idx == 0 {
            self.current_preset_idx = self.presets.len() - 1;
        } else {
            self.current_preset_idx -= 1;
        }

        let path = &self.presets[self.current_preset_idx];
        match af_core::config::load_config(path) {
            Ok(mut new_cfg) => {
                // Conserver l'état de l'interface qui n'est pas censé sauter avec le preset visuel.
                let old_cfg = self.config.load();
                new_cfg.fullscreen = old_cfg.fullscreen;
                new_cfg.show_spectrum = old_cfg.show_spectrum;

                // Only force resize if pixel dimensions would change
                let needs_resize = old_cfg.render_mode != new_cfg.render_mode
                    || (old_cfg.density_scale - new_cfg.density_scale).abs() > f32::EPSILON
                    || (old_cfg.aspect_ratio - new_cfg.aspect_ratio).abs() > f32::EPSILON;

                self.config.store(Arc::new(new_cfg));

                self.sidebar_dirty = true;
                if needs_resize {
                    self.terminal_size = (0, 0);
                }
                log::info!("Preset chargé à vif : {}", path.display());
            }
            Err(e) => {
                log::error!("Erreur de chargement du glitch preset : {e}");
            }
        }
    }
}
