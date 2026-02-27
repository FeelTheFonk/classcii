use std::sync::Arc;
use std::time::{Duration, Instant};

use af_ascii::compositor::Compositor;
use af_audio::state::AudioCommand;
use af_core::charset;
use af_core::config::{BgStyle, ColorMode, RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};
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
    /// Channel pour les commandes vidéo (Play, Pause, Seek).
    #[cfg(feature = "video")]
    pub video_cmd_tx: Option<flume::Sender<VideoCommand>>,
    /// Channel pour les commandes audio (Play, Pause, Seek).
    pub audio_cmd_tx: Option<flume::Sender<AudioCommand>>,
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
            #[cfg(feature = "video")]
            video_cmd_tx,
            audio_cmd_tx,
        })
    }

    /// Main event loop per spec §29.
    ///
    /// # Errors
    /// Returns an error if terminal operations fail.
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

                // Save current grid for next frame's fade trails (zero-alloc swap + copy)
                std::mem::swap(&mut self.grid, &mut self.prev_grid);
                self.grid.copy_from(&self.prev_grid);
            }

            // === Rendu terminal ===
            self.fps_counter.tick();
            let state = self.render_state();
            let sidebar_dirty = self.sidebar_dirty;
            let grid = &self.grid;
            let fps_counter = &self.fps_counter;
            terminal.draw(|frame| {
                af_render::ui::draw(
                    frame,
                    grid,
                    &render_config,
                    audio_features.as_ref(),
                    fps_counter,
                    sidebar_dirty,
                    &state,
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
            match code {
                KeyCode::Char('q' | '?' | ' ') | KeyCode::Esc => self.handle_navigation_key(code),
                KeyCode::Tab
                | KeyCode::Char(
                    '1'..='5'
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
                    | 's'
                    | 'a'
                    | 'm'
                    | 'b',
                ) => self.handle_render_key(code),
                KeyCode::Char('f' | 'F' | 'g' | 'G') => self.handle_effect_key(code),
                KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                    self.handle_playback_key(code);
                }
                _ => {}
            }
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
            _ => {}
        }
    }

    /// Render parameter keys: mode, charset, density, color, etc.
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

            let sidebar_width = 20u16;
            let spectrum_height = 3u16;
            let canvas_width = new_size.0.saturating_sub(sidebar_width);
            let canvas_height = new_size.1.saturating_sub(spectrum_height);

            // Réallouer la grille ASCII (rare, OK d'allouer ici)
            self.grid = AsciiGrid::new(canvas_width, canvas_height);
            self.prev_grid = AsciiGrid::new(canvas_width, canvas_height);

            let config = self.config.load();
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
}
