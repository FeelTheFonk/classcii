use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use af_ascii::compositor::Compositor;
use af_audio::state::AudioCommand;
use af_core::charset;
use af_core::config::{BgStyle, ColorMode, RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};
use af_render::AudioPanelState;
use af_render::fps::FpsCounter;
use af_render::ui::RenderState;
use af_source::resize::Resizer;
#[cfg(feature = "video")]
use af_source::video::VideoCommand;
use anyhow::Result;
use arc_swap::ArcSwap;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
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
        "png" | "jpg" | "jpeg" | "bmp" | "gif" | "tiff" | "tif" | "webp" => Some(MediaType::Image),
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
    /// Panneau de mixage audio affiché (touche A).
    AudioPanel,
    /// L'application doit se terminer au prochain tour de boucle.
    Quitting,
}

/// Main application struct holding all state.
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
    /// Buffer local pour l'édition de charset en live.
    pub charset_edit_buf: String,
    /// Position du curseur dans l'éditeur de charset.
    pub charset_edit_cursor: usize,
    /// État local du panneau de mixage audio.
    pub audio_panel: AudioPanelState,
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
        let canvas_width = terminal_size.0.saturating_sub(20);
        let canvas_height = terminal_size.1.saturating_sub(3);
        let initial_charset = config.load().charset.clone();

        let mut presets = Vec::new();
        if let Ok(entries) = std::fs::read_dir("config/presets") {
            for entry in entries.flatten() {
                if let Ok(ft) = entry.file_type() {
                    if ft.is_file() && entry.path().extension().is_some_and(|e| e == "toml") {
                        presets.push(entry.path());
                    }
                }
            }
        }
        presets.sort(); // Predictable iteration order

        let audio_mappings_len = config.load().audio_mappings.len();

        Ok(Self {
            state: AppState::Running,
            config,
            audio_output,
            current_frame: None,
            grid: AsciiGrid::new(canvas_width, canvas_height),
            resized_frame: FrameBuffer::new(u32::from(canvas_width), u32::from(canvas_height)),
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
            charset_edit_buf: String::new(),
            charset_edit_cursor: 0,
            audio_panel: AudioPanelState::new(audio_mappings_len),
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

            // === Vérifier resize terminal ===
            self.check_resize()?;

            // === Lire audio features (non-bloquant) ===
            let audio_features = self.audio_output.as_mut().map(|out| *out.read());

            // === Lire frame source ===
            if let Some(ref rx) = self.frame_rx {
                if let Ok(frame) = rx.try_recv() {
                    self.current_frame = Some(frame);
                }
            }

            // === Appliquer audio mappings à la config ===
            let config = self.config.load();
            // R1: acceptable — ~200B/frame allocation for mutable audio mapping overlay
            let mut render_config = (**config).clone();
            if let Some(ref features) = audio_features {
                pipeline::apply_audio_mappings(&mut render_config, features);
            }

            // === Process source frame into ASCII grid ===
            if let Some(ref source_frame) = self.current_frame {
                // Resize source to grid dimensions
                let _ = self
                    .resizer
                    .resize_into(source_frame, &mut self.resized_frame);
                // Convert pixels → ASCII
                self.compositor.process(
                    &self.resized_frame,
                    audio_features.as_ref(),
                    &render_config,
                    &mut self.grid,
                );

                // === Post-processing effects ===
                // Fade trails (blend with previous frame)
                if render_config.fade_decay > 0.0 {
                    af_render::effects::apply_fade_trails(
                        &mut self.grid,
                        &self.prev_grid,
                        render_config.fade_decay,
                    );
                }
                // Beat flash
                if let Some(ref features) = audio_features {
                    af_render::effects::apply_beat_flash(&mut self.grid, features);
                }
                // Glow
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

            // === Rendu terminal ===
            self.fps_counter.tick();
            let state = self.render_state();
            let sidebar_dirty = self.sidebar_dirty;
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
            let layout_audio_panel = if state == RenderState::AudioPanel {
                Some((&self.audio_panel, &render_config))
            } else {
                None
            };
            let layout_charset_edit = if state == RenderState::CharsetEdit {
                Some((self.charset_edit_buf.as_str(), self.charset_edit_cursor))
            } else {
                None
            };

            terminal.draw(|frame| {
                af_render::ui::draw(
                    frame,
                    grid,
                    &render_config,
                    audio_features.as_ref(),
                    fps_counter,
                    preset_name,
                    loaded_visual,
                    loaded_audio,
                    sidebar_dirty,
                    &state,
                    layout_charset_edit,
                    layout_audio_panel,
                );
            })?;
            self.sidebar_dirty = false;
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
            AppState::AudioPanel => RenderState::AudioPanel,
            AppState::Quitting => RenderState::Quitting,
        }
    }

    /// Handle a terminal event by dispatching to focused sub-handlers.
    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = *event
        {
            if self.state == AppState::CharsetEdit {
                self.handle_charset_edit_key(code);
                return;
            }
            if self.state == AppState::AudioPanel {
                self.handle_audio_panel_key(code);
                return;
            }

            match code {
                KeyCode::Char('q' | '?' | ' ' | 'o' | 'O') | KeyCode::Esc => {
                    self.handle_navigation_key(code);
                }
                KeyCode::Tab
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
                    | 'A',
                ) => self.handle_render_key(code),
                KeyCode::Char('f' | 'F' | 'g' | 'G') => self.handle_effect_key(code),
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

    /// Audio Panel logic
    fn handle_audio_panel_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.state = AppState::Running;
                self.sidebar_dirty = true;
            }
            KeyCode::Up => {
                if self.audio_panel.selected_row > 0 {
                    self.audio_panel.selected_row -= 1;
                    self.audio_panel.selected_col = 0;
                }
            }
            KeyCode::Down => {
                if self.audio_panel.selected_row + 1 < self.audio_panel.total_rows {
                    self.audio_panel.selected_row += 1;
                    self.audio_panel.selected_col = 0;
                }
            }
            KeyCode::Left => {
                if self.audio_panel.selected_row >= 2 && self.audio_panel.selected_col > 0 {
                    self.audio_panel.selected_col -= 1;
                }
            }
            KeyCode::Right => {
                if self.audio_panel.selected_row >= 2 && self.audio_panel.selected_col < 4 {
                    self.audio_panel.selected_col += 1;
                }
            }
            KeyCode::Char('-') => self.adjust_panel_value(-1.0),
            KeyCode::Char('+' | '=') => self.adjust_panel_value(1.0),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_panel_cell(),
            KeyCode::Char('n') => {
                self.toggle_config(|c| {
                    c.audio_mappings.push(af_core::config::AudioMapping {
                        enabled: true,
                        source: "rms".into(),
                        target: "brightness".into(),
                        amount: 0.5,
                        offset: 0.0,
                    });
                });
                self.audio_panel.total_rows = 2 + self.config.load().audio_mappings.len();
                self.audio_panel.selected_row = self.audio_panel.total_rows - 1;
                self.audio_panel.selected_col = 0;
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if self.audio_panel.selected_row >= 2 {
                    let idx = self.audio_panel.selected_row - 2;
                    self.toggle_config(|c| {
                        if idx < c.audio_mappings.len() {
                            c.audio_mappings.remove(idx);
                        }
                    });
                    self.audio_panel.total_rows = 2 + self.config.load().audio_mappings.len();
                    if self.audio_panel.selected_row >= self.audio_panel.total_rows {
                        self.audio_panel.selected_row =
                            self.audio_panel.total_rows.saturating_sub(1);
                    }
                    self.audio_panel.selected_col = 0;
                }
            }
            _ => {}
        }
    }

    fn adjust_panel_value(&mut self, sign: f32) {
        let row = self.audio_panel.selected_row;
        let col = self.audio_panel.selected_col;
        self.toggle_config(|c| {
            if row == 0 {
                c.audio_sensitivity = (c.audio_sensitivity + sign * 0.1).clamp(0.0, 5.0);
            } else if row == 1 {
                c.audio_smoothing = (c.audio_smoothing + sign * 0.05).clamp(0.0, 1.0);
            } else if row >= 2 {
                let idx = row - 2;
                if idx < c.audio_mappings.len() {
                    let m = &mut c.audio_mappings[idx];
                    if col == 3 {
                        m.amount = (m.amount + sign * 0.1).clamp(0.0, 5.0);
                    } else if col == 4 {
                        m.offset = (m.offset + sign * 0.05).clamp(-1.0, 1.0);
                    }
                }
            }
        });
    }

    fn toggle_panel_cell(&mut self) {
        let row = self.audio_panel.selected_row;
        let col = self.audio_panel.selected_col;
        if row >= 2 {
            let idx = row - 2;
            self.toggle_config(|c| {
                if idx < c.audio_mappings.len() {
                    let m = &mut c.audio_mappings[idx];
                    if col == 0 {
                        m.enabled = !m.enabled;
                    } else if col == 1 {
                        let sources = af_core::config::AUDIO_SOURCES;
                        let pos = sources.iter().position(|&s| s == m.source).unwrap_or(0);
                        m.source = sources[(pos + 1) % sources.len()].to_string();
                    } else if col == 2 {
                        let targets = af_core::config::AUDIO_TARGETS;
                        let pos = targets.iter().position(|&s| s == m.target).unwrap_or(0);
                        m.target = targets[(pos + 1) % targets.len()].to_string();
                    }
                }
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
            }
            KeyCode::Char('O') => {
                self.open_audio_requested = true;
            }
            _ => {}
        }
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
                    RenderMode::Quadrant => RenderMode::Ascii,
                };
                self.config.store(Arc::new(new));
                self.sidebar_dirty = true;
            }
            KeyCode::Char('1') => self.set_charset(0, charset::CHARSET_COMPACT),
            KeyCode::Char('2') => self.set_charset(1, charset::CHARSET_STANDARD),
            KeyCode::Char('3') => self.set_charset(2, charset::CHARSET_FULL),
            KeyCode::Char('4') => self.set_charset(3, charset::CHARSET_BLOCKS),
            KeyCode::Char('5') => self.set_charset(4, charset::CHARSET_MINIMAL),
            KeyCode::Char('6') => self.set_charset(5, charset::CHARSET_GLITCH_1),
            KeyCode::Char('7') => self.set_charset(6, charset::CHARSET_GLITCH_2),
            KeyCode::Char('8') => self.set_charset(7, charset::CHARSET_DIGITAL),
            KeyCode::Char('9') => self.set_charset(8, charset::CHARSET_CLASSIC_GRADIENT),
            KeyCode::Char('0') => self.set_charset(9, charset::CHARSET_EXTENDED_SMOOTH),
            KeyCode::Char('d') => {
                self.toggle_config(|c| c.density_scale = (c.density_scale - 0.25).max(0.25));
            }
            KeyCode::Char('D') => {
                self.toggle_config(|c| c.density_scale = (c.density_scale + 0.25).min(4.0));
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
            KeyCode::Char('a') => self.toggle_config(|c| {
                c.aspect_ratio = match c.aspect_ratio {
                    x if (x - 1.5).abs() < 0.01 => 2.0,
                    x if (x - 2.0).abs() < 0.01 => 2.5,
                    _ => 1.5,
                };
            }),
            KeyCode::Char('m') => self.toggle_config(|c| {
                c.color_mode = match c.color_mode {
                    ColorMode::Direct => ColorMode::HsvBright,
                    ColorMode::HsvBright => ColorMode::Quantized,
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
            KeyCode::Char('A') => {
                self.audio_panel = AudioPanelState::new(self.config.load().audio_mappings.len());
                self.state = AppState::AudioPanel;
                self.sidebar_dirty = true;
            }
            _ => {}
        }
    }

    /// Effect keys: fade, glow.
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
                let sidebar_width = 20u16;
                let spectrum_height = if config.show_spectrum { 3u16 } else { 0u16 };
                (
                    new_size.0.saturating_sub(sidebar_width),
                    new_size.1.saturating_sub(spectrum_height),
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
                RenderMode::Braille => (
                    (f32::from(canvas_width) * density * 2.0) as u32,
                    (f32::from(canvas_height) * density * 4.0) as u32,
                ),
                RenderMode::Quadrant => (
                    (f32::from(canvas_width) * density * 2.0) as u32,
                    (f32::from(canvas_height) * density * 2.0) as u32,
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
    }

    /// Load a visual source (image or video) and update sidebar name.
    fn load_visual(&mut self, path: &Path, media_type: MediaType) {
        match media_type {
            MediaType::Image => match af_source::image::ImageSource::new(path) {
                Ok(mut source) => {
                    self.current_frame = af_core::traits::Source::next_frame(&mut source);
                    self.frame_rx = None;
                    log::info!("Image chargée: {}", path.display());
                }
                Err(e) => log::error!("Erreur chargement image: {e}"),
            },
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
        match af_source::video::spawn_video_thread(path.to_path_buf(), frame_tx, cmd_rx) {
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
        match pipeline::start_audio(path_str, &self.config) {
            Ok((output, tx)) => {
                self.audio_output = Some(output);
                self.audio_cmd_tx = tx;
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

                self.config.store(Arc::new(new_cfg));
                // Recalculate rows if audio panel is open (even barely)
                let new_len = self.config.load().audio_mappings.len();
                self.audio_panel.total_rows = 2 + new_len;
                self.audio_panel.selected_row = self
                    .audio_panel
                    .selected_row
                    .min(self.audio_panel.total_rows.saturating_sub(1));

                self.sidebar_dirty = true;
                self.terminal_size = (0, 0); // Force redraw / reallocation au cas où le mode de rendu (Braille/Ascii) ait changé.
                log::info!("Preset chargé à vif : {}", path.display());
            }
            Err(e) => {
                log::error!("Erreur de chargement du glitch preset : {e}");
            }
        }
    }
}
