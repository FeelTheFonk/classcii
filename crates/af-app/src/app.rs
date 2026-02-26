use std::sync::Arc;
use std::time::{Duration, Instant};

use af_ascii::compositor::Compositor;
use af_core::charset;
use af_core::config::{BgStyle, ColorMode, RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, AudioFeatures, FrameBuffer};
use af_render::fps::FpsCounter;
use af_render::ui::RenderState;
use af_source::resize::Resizer;
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
    ) -> Result<Self> {
        let terminal_size = crossterm::terminal::size()?;
        let canvas_width = terminal_size.0.saturating_sub(16);
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
            terminal_size,
            compositor: Compositor::new(&initial_charset),
            resizer: Resizer::new(),
            prev_grid: AsciiGrid::new(canvas_width, canvas_height),
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
            let frame_duration =
                Duration::from_secs_f64(1.0 / f64::from(config_guard.target_fps));
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
                let _ = self.resizer.resize_into(source_frame, &mut self.resized_frame);
                // Convert pixels → ASCII
                self.compositor.process(
                    &self.resized_frame,
                    audio_features.as_ref(),
                    &render_config,
                    &mut self.grid,
                );

                // === Post-processing effects ===
                // Fade trails (blend with previous frame)
                af_render::effects::apply_fade_trails(&mut self.grid, &self.prev_grid, 0.3);
                // Beat flash
                if let Some(ref features) = audio_features {
                    af_render::effects::apply_beat_flash(&mut self.grid, features);
                }
                // Glow
                af_render::effects::apply_glow(&mut self.grid, 0.5);

                // Save current grid for next frame's fade trails
                self.prev_grid = self.grid.clone();
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

    /// Handle a terminal event.
    #[allow(clippy::too_many_lines)]
    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = *event
        {
            match (&self.state, code) {
                (_, KeyCode::Char('q')) | (AppState::Running, KeyCode::Esc) => {
                    self.state = AppState::Quitting;
                }
                (AppState::Help, KeyCode::Esc) | (_, KeyCode::Char('?')) => {
                    self.state = if self.state == AppState::Help {
                        AppState::Running
                    } else {
                        AppState::Help
                    };
                    self.sidebar_dirty = true;
                }
                (_, KeyCode::Char(' ')) => {
                    self.state = if self.state == AppState::Paused {
                        AppState::Running
                    } else {
                        AppState::Paused
                    };
                    self.sidebar_dirty = true;
                }
                // === Render Mode ===
                (_, KeyCode::Tab) => {
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
                // === Charset select (1-5) ===
                (_, KeyCode::Char('1')) => self.set_charset(0, charset::CHARSET_COMPACT),
                (_, KeyCode::Char('2')) => self.set_charset(1, charset::CHARSET_STANDARD),
                (_, KeyCode::Char('3')) => self.set_charset(2, charset::CHARSET_FULL),
                (_, KeyCode::Char('4')) => self.set_charset(3, charset::CHARSET_BLOCKS),
                (_, KeyCode::Char('5')) => self.set_charset(4, charset::CHARSET_MINIMAL),
                // === Density scale ===
                (_, KeyCode::Char('d')) => {
                    self.toggle_config(|c| {
                        c.density_scale = (c.density_scale - 0.25).max(0.25);
                    });
                }
                (_, KeyCode::Char('D')) => {
                    self.toggle_config(|c| {
                        c.density_scale = (c.density_scale + 0.25).min(4.0);
                    });
                }
                // === Visual params ===
                (_, KeyCode::Char('c')) => {
                    self.toggle_config(|c| c.color_enabled = !c.color_enabled);
                }
                (_, KeyCode::Char('i')) => {
                    self.toggle_config(|c| c.invert = !c.invert);
                }
                (_, KeyCode::Char('[')) => {
                    self.toggle_config(|c| c.contrast = (c.contrast - 0.1).max(0.1));
                }
                (_, KeyCode::Char(']')) => {
                    self.toggle_config(|c| c.contrast = (c.contrast + 0.1).min(3.0));
                }
                (_, KeyCode::Char('{')) => {
                    self.toggle_config(|c| c.brightness = (c.brightness - 0.05).max(-1.0));
                }
                (_, KeyCode::Char('}')) => {
                    self.toggle_config(|c| c.brightness = (c.brightness + 0.05).min(1.0));
                }
                (_, KeyCode::Char('-')) => {
                    self.toggle_config(|c| c.saturation = (c.saturation - 0.1).max(0.0));
                }
                (_, KeyCode::Char('+' | '=')) => {
                    self.toggle_config(|c| c.saturation = (c.saturation + 0.1).min(3.0));
                }
                // === Edge / Shape ===
                (_, KeyCode::Char('e')) => {
                    self.toggle_config(|c| {
                        c.edge_threshold = if c.edge_threshold > 0.0 { 0.0 } else { 0.3 };
                    });
                }
                (_, KeyCode::Char('s')) => {
                    self.toggle_config(|c| c.shape_matching = !c.shape_matching);
                }
                // === Aspect ratio cycle ===
                (_, KeyCode::Char('a')) => {
                    self.toggle_config(|c| {
                        c.aspect_ratio = match c.aspect_ratio {
                            x if (x - 1.5).abs() < 0.01 => 2.0,
                            x if (x - 2.0).abs() < 0.01 => 2.5,
                            _ => 1.5,
                        };
                    });
                }
                // === Color mode cycle ===
                (_, KeyCode::Char('m')) => {
                    self.toggle_config(|c| {
                        c.color_mode = match c.color_mode {
                            ColorMode::Direct => ColorMode::HsvBright,
                            ColorMode::HsvBright => ColorMode::Quantized,
                            ColorMode::Quantized => ColorMode::Direct,
                        };
                    });
                }
                // === BG style cycle ===
                (_, KeyCode::Char('b')) => {
                    self.toggle_config(|c| {
                        c.bg_style = match c.bg_style {
                            BgStyle::Black => BgStyle::SourceDim,
                            BgStyle::SourceDim => BgStyle::Transparent,
                            BgStyle::Transparent => BgStyle::Black,
                        };
                    });
                }
                // === Audio controls ===
                (_, KeyCode::Up) => {
                    self.toggle_config(|c| {
                        c.audio_sensitivity = (c.audio_sensitivity + 0.1).min(5.0);
                    });
                }
                (_, KeyCode::Down) => {
                    self.toggle_config(|c| {
                        c.audio_sensitivity = (c.audio_sensitivity - 0.1).max(0.0);
                    });
                }
                (_, KeyCode::Left) => {
                    self.toggle_config(|c| {
                        c.audio_smoothing = (c.audio_smoothing - 0.05).max(0.0);
                    });
                }
                (_, KeyCode::Right) => {
                    self.toggle_config(|c| {
                        c.audio_smoothing = (c.audio_smoothing + 0.05).min(1.0);
                    });
                }
                _ => {}
            }
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

            let sidebar_width = 16u16;
            let spectrum_height = 3u16;
            let canvas_width = new_size.0.saturating_sub(sidebar_width);
            let canvas_height = new_size.1.saturating_sub(spectrum_height);

            // Réallouer la grille ASCII (rare, OK d'allouer ici)
            self.grid = AsciiGrid::new(canvas_width, canvas_height);

            let config = self.config.load();
            let (pixel_w, pixel_h) = match config.render_mode {
                RenderMode::Ascii => (u32::from(canvas_width), u32::from(canvas_height)),
                RenderMode::HalfBlock => {
                    (u32::from(canvas_width), u32::from(canvas_height) * 2)
                }
                RenderMode::Braille => {
                    (u32::from(canvas_width) * 2, u32::from(canvas_height) * 4)
                }
                RenderMode::Quadrant => {
                    (u32::from(canvas_width) * 2, u32::from(canvas_height) * 2)
                }
            };

            // Appliquer la correction aspect ratio
            let pixel_h_corrected = (pixel_h as f32 / config.aspect_ratio) as u32;
            self.resized_frame = FrameBuffer::new(pixel_w, pixel_h_corrected.max(1));
            self.sidebar_dirty = true;

            log::debug!("Terminal resized to {canvas_width}×{canvas_height}");
        }
        Ok(())
    }
}
