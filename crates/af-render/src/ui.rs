use af_core::config::{BgStyle, ColorMode, RenderConfig};
use af_core::frame::{AsciiGrid, AudioFeatures};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use crate::canvas;
use crate::fps::FpsCounter;

// ─── Layout constants ──────────────────────────────────────────────

/// Sidebar width in terminal columns.
pub const SIDEBAR_WIDTH: u16 = 24;
/// Spectrum bar height in terminal rows.
pub const SPECTRUM_HEIGHT: u16 = 3;
/// Minimum terminal width.
pub const MIN_TERM_WIDTH: u16 = 80;
/// Minimum terminal height.
pub const MIN_TERM_HEIGHT: u16 = 20;

// ─── Data structures ───────────────────────────────────────────────

/// Data bundle for the creation mode overlay.
pub struct CreationOverlayData<'a> {
    /// Auto-modulation active.
    pub auto_mode: bool,
    /// Master intensity [0.0, 2.0].
    pub master_intensity: f32,
    /// Active preset name.
    pub preset_name: &'a str,
    /// Selected effect index.
    pub selected_effect: usize,
    /// Effect names and current values.
    pub effects: [(&'a str, f32, f32); 10], // (name, value, max)
}

/// Application state enum (mirrored for rendering decisions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderState {
    /// Normal running state.
    Running,
    /// Paused.
    Paused,
    /// Help overlay visible.
    Help,
    /// Custom charset editor overlay (key C).
    CharsetEdit,
    /// Creation mode interactive overlay.
    CreationMode,
    /// Stem separation mode overlay (key S).
    StemMode,
    /// Workflow save overlay (Ctrl+S).
    WorkflowSave,
    /// Workflow browse/load overlay (Ctrl+W).
    WorkflowBrowse,
    /// Quitting (should not reach draw).
    Quitting,
}

/// Data for the stem separation overlay.
pub struct StemOverlayData {
    /// Per-stem display info (4 stems).
    pub stems: [StemDisplayInfo; 4],
    /// Currently selected stem index (0-3).
    pub selected_idx: usize,
    /// Separation progress (None if not in progress, Some(0.0..1.0) if running).
    pub separation_progress: Option<f32>,
    /// Whether stems have been separated and loaded.
    pub has_stems: bool,
    /// Whether an audio file is loaded (for separation).
    pub has_audio: bool,
}

/// Display info for a single stem in the overlay.
#[allow(clippy::struct_excessive_bools)]
pub struct StemDisplayInfo {
    /// Label (e.g. "Drums").
    pub label: &'static str,
    /// Short label (e.g. "DRM").
    pub short: &'static str,
    /// Distinctive color (r, g, b).
    pub color: (u8, u8, u8),
    /// Muted state.
    pub muted: bool,
    /// Solo state.
    pub solo: bool,
    /// Volume [0.0, 2.0].
    pub volume: f32,
    /// Spectrum visible.
    pub visible: bool,
    /// 32 log-frequency spectrum bands [0.0, 1.0].
    pub spectrum: [f32; 32],
    /// RMS level.
    pub rms: f32,
    /// Onset detected.
    pub onset: bool,
}

/// Data for the workflow save overlay.
pub struct WorkflowSaveData<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub cursor: usize,
    /// 0=name, 1=description
    pub active_field: u8,
}

/// Data for the workflow browse overlay.
pub struct WorkflowBrowseData {
    pub entries: Vec<WorkflowBrowseEntry>,
    pub selected_idx: usize,
}

/// Single entry in the workflow browse list.
pub struct WorkflowBrowseEntry {
    pub name: String,
    pub created_at: String,
    pub description: String,
    pub has_stems: bool,
    pub has_timeline: bool,
}

/// Bundled context for the `draw()` function.
pub struct DrawContext<'a> {
    pub grid: &'a AsciiGrid,
    pub config: &'a RenderConfig,
    pub base_config: &'a RenderConfig,
    pub audio: Option<&'a AudioFeatures>,
    pub fps_counter: &'a FpsCounter,
    pub preset_name: Option<&'a str>,
    pub loaded_visual: Option<&'a str>,
    pub loaded_audio: Option<&'a str>,
    pub state: &'a RenderState,
    pub charset_edit: Option<(&'a str, usize)>,

    pub creation: Option<&'a CreationOverlayData<'a>>,
    pub creation_mode_active: bool,
    pub perf_warning: bool,
    /// Current playback position in seconds (from MediaClock), None if no media loaded.
    pub playback_pos_secs: Option<f64>,
    /// Parameter change flash countdown (>0 means show flash indicator).
    pub param_flash: u8,
    /// Help overlay scroll offset.
    pub help_scroll: u16,
    /// Stem overlay data (None when stem mode is not active / no stem set loaded).
    pub stem: Option<&'a StemOverlayData>,
    /// Workflow save overlay data.
    pub workflow_save: Option<&'a WorkflowSaveData<'a>>,
    /// Workflow browse overlay data.
    pub workflow_browse: Option<&'a WorkflowBrowseData>,
    /// Flash message (workflow saved confirmation, etc.).
    pub flash_msg: Option<&'a str>,
}

// ─── Main draw ─────────────────────────────────────────────────────

/// Draw the full UI: canvas + sidebar + spectrum bar + overlays.
pub fn draw(frame: &mut Frame, ctx: &DrawContext<'_>) {
    let area = frame.area();

    // Minimum terminal size guard
    if area.width < MIN_TERM_WIDTH || area.height < MIN_TERM_HEIGHT {
        let msg = format!(
            "Terminal too small ({}x{}, need {}x{})",
            area.width, area.height, MIN_TERM_WIDTH, MIN_TERM_HEIGHT
        );
        let p = Paragraph::new(msg)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(p, area);
        return;
    }

    if ctx.config.fullscreen {
        canvas::render_grid(
            frame.buffer_mut(),
            area,
            ctx.grid,
            ctx.config.zalgo_intensity,
        );
    } else {
        let h_chunks = Layout::horizontal([Constraint::Min(40), Constraint::Length(SIDEBAR_WIDTH)])
            .split(area);

        let canvas_area = if ctx.config.show_spectrum {
            let v_chunks =
                Layout::vertical([Constraint::Min(10), Constraint::Length(SPECTRUM_HEIGHT)])
                    .split(h_chunks[0]);
            draw_spectrum(frame, v_chunks[1], ctx.audio);
            v_chunks[0]
        } else {
            h_chunks[0]
        };

        canvas::render_grid(
            frame.buffer_mut(),
            canvas_area,
            ctx.grid,
            ctx.config.zalgo_intensity,
        );

        draw_sidebar(
            frame,
            h_chunks[1],
            ctx.base_config,
            ctx.audio,
            ctx.fps_counter,
            ctx.preset_name,
            ctx.loaded_visual,
            ctx.loaded_audio,
            ctx.state,
            ctx.creation_mode_active,
            ctx.perf_warning,
            ctx.playback_pos_secs,
            ctx.param_flash,
        );
    }

    // Overlays (modal, one at a time)
    // Help overlay draws even in fullscreen if toggled
    if *ctx.state == RenderState::Help {
        dim_overlay_background(frame, area);
        draw_help_overlay(frame, area, ctx.help_scroll);
    } else if let Some((buf, cursor)) = ctx.charset_edit {
        dim_overlay_background(frame, area);
        draw_charset_edit_overlay(frame, area, buf, cursor);
    } else if let Some(creation) = ctx.creation {
        draw_creation_overlay(frame, area, creation, ctx.audio);
    } else if let Some(stem) = ctx.stem {
        draw_stem_overlay(frame, area, stem);
    } else if let Some(wf_save) = ctx.workflow_save {
        dim_overlay_background(frame, area);
        draw_workflow_save_overlay(frame, area, wf_save);
    } else if let Some(wf_browse) = ctx.workflow_browse {
        dim_overlay_background(frame, area);
        draw_workflow_browse_overlay(frame, area, wf_browse);
    }

    // Flash message (workflow saved, etc.) — renders on top of everything
    if let Some(msg) = ctx.flash_msg {
        let msg_w = msg.len() as u16 + 4;
        let x = area.x + area.width.saturating_sub(msg_w) / 2;
        let flash_area = Rect::new(x, area.y + 1, msg_w, 3);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green))
            .style(Style::default().bg(Color::Rgb(10, 30, 10)));
        let p = Paragraph::new(Line::from(Span::styled(
            msg.to_string(),
            Style::default().fg(Color::Green),
        )))
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(p, flash_area);
    }
}

/// Dim the entire frame area to create visual separation for overlays.
fn dim_overlay_background(frame: &mut Frame, area: Rect) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                cell.set_bg(Color::Rgb(12, 12, 16));
                // Dim foreground
                if let Color::Rgb(r, g, b) = cell.fg {
                    cell.set_fg(Color::Rgb(r / 3, g / 3, b / 3));
                } else {
                    cell.set_fg(Color::DarkGray);
                }
            }
        }
    }
}

/// Draw the spectrum sparkline bar with intensity coloring.
fn draw_spectrum(frame: &mut Frame, area: Rect, audio: Option<&AudioFeatures>) {
    let mut data = [0u64; 32];
    if let Some(features) = audio {
        for (i, &v) in features.spectrum_bands.iter().enumerate() {
            data[i] = (v * 64.0).clamp(0.0, 64.0) as u64;
        }
    }

    let avg: f32 = if let Some(features) = audio {
        features.spectrum_bands.iter().sum::<f32>() / 32.0
    } else {
        0.0
    };
    let bar_color = if avg > 0.5 {
        Color::Red
    } else if avg > 0.25 {
        Color::Yellow
    } else {
        Color::Cyan
    };

    let sparkline = Sparkline::default()
        .block(Block::default().borders(Borders::TOP).title(" Spectrum "))
        .data(data)
        .style(Style::default().fg(bar_color));

    frame.render_widget(sparkline, area);
}

/// Draw the parameter sidebar with all live values.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    preset_name: Option<&str>,
    loaded_visual: Option<&str>,
    loaded_audio: Option<&str>,
    state: &RenderState,
    creation_mode_active: bool,
    perf_warning: bool,
    playback_pos_secs: Option<f64>,
    param_flash: u8,
) {
    let mode_str = match config.render_mode {
        af_core::config::RenderMode::Ascii => "ASCII",
        af_core::config::RenderMode::Braille => "Braille",
        af_core::config::RenderMode::HalfBlock => "HalfBlock",
        af_core::config::RenderMode::Quadrant => "Quadrant",
        af_core::config::RenderMode::Sextant => "Sextant",
        af_core::config::RenderMode::Octant => "Octant",
    };

    let color_mode_str = match config.color_mode {
        ColorMode::Direct => "Direct",
        ColorMode::HsvBright => "HSV",
        ColorMode::Quantized => "Quant",
        ColorMode::Oklab => "Oklab",
    };

    let bg_str = match config.bg_style {
        BgStyle::Black => "Black",
        BgStyle::SourceDim => "SrcDim",
        BgStyle::Transparent => "Trans",
    };

    let state_str = match state {
        RenderState::Running => "\u{25b6} RUN",
        RenderState::Paused => "\u{23f8} PAUSE",
        RenderState::Help => "? HELP",
        RenderState::CharsetEdit => "C EDIT",

        RenderState::CreationMode => "K CREATE",
        RenderState::StemMode => "S STEMS",
        RenderState::WorkflowSave => "SAVE WF",
        RenderState::WorkflowBrowse => "LOAD WF",
        RenderState::Quitting => "\u{23f9} QUIT",
    };

    let charset_names = [
        "Full", "Dense", "Short", "Blocks", "Minimal", "Glitch1", "Glitch2", "Edge", "Digital",
        "Binary",
    ];
    let charset_name = charset_names.get(config.charset_index).unwrap_or(&"Custom");

    // Onset indicator
    let onset_str = if audio.is_some_and(|a| a.onset) {
        "\u{25cf} BEAT"
    } else {
        "\u{25cb}"
    };

    // Key-value line builder: key 4-pad, label 6-pad, value remainder
    let label = Color::Gray;
    let val_c = Color::White;
    let section = Color::Yellow;
    let kv_line = |k: &str, lbl: &str, val: &str| -> Line {
        Line::from(vec![
            Span::styled(format!(" {k:<4}"), Style::default().fg(label)),
            Span::styled(format!("{lbl:<6}: "), Style::default().fg(label)),
            Span::styled(val.to_owned(), Style::default().fg(val_c)),
        ])
    };

    macro_rules! fmt {
        ($fmt:literal, $val:expr) => {{ format!($fmt, $val) }};
    }

    let dither_str = match config.dither_mode {
        af_core::config::DitherMode::Bayer8x8 => "Bayer8",
        af_core::config::DitherMode::BlueNoise16 => "BNoise",
        af_core::config::DitherMode::None => "OFF",
    };
    let scan_str = if config.scanline_gap == 0 {
        "OFF".to_owned()
    } else {
        fmt!("{}", config.scanline_gap)
    };

    let mut lines = vec![
        Line::from(Span::styled(state_str, Style::default().fg(Color::Green))),
        Line::from(""),
        // ─── Render ─────────────
        Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500} Render \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(section),
        )),
        kv_line("Tab", "Mode", mode_str),
        kv_line("p/P", "Preset", preset_name.unwrap_or("Custom")),
        kv_line("1-0", "Chars", charset_name),
        kv_line("d/D", "Densty", &fmt!("{:.2}", config.density_scale)),
        kv_line(
            "c",
            "Color",
            if config.color_enabled { "ON" } else { "OFF" },
        ),
        kv_line("m", "CMode", color_mode_str),
        kv_line("i", "Invert", if config.invert { "ON" } else { "OFF" }),
        kv_line("[]", "Contrst", &fmt!("{:.1}", config.contrast)),
        kv_line("{}", "Bright", &fmt!("{:.2}", config.brightness)),
        kv_line("-/+", "Satur", &fmt!("{:.1}", config.saturation)),
        kv_line("e", "Edge", &fmt!("{:.1}", config.edge_threshold)),
        kv_line("E", "EdgMix", &fmt!("{:.2}", config.edge_mix)),
        kv_line(
            "s",
            "Shapes",
            if config.shape_matching { "ON" } else { "OFF" },
        ),
        kv_line("a", "Aspect", &fmt!("{:.1}", config.aspect_ratio)),
        kv_line("b", "BG", bg_str),
        kv_line("n", "Dither", dither_str),
        // ─── Effects ────────────
        Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500} Effects \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(section),
        )),
        kv_line("f/F", "Fade", &fmt!("{:.1}", config.fade_decay)),
        kv_line("g/G", "Glow", &fmt!("{:.1}", config.glow_intensity)),
        kv_line("t/T", "Flash", &fmt!("{:.1}", config.beat_flash_intensity)),
        kv_line("r/R", "Chroma", &fmt!("{:.1}", config.chromatic_offset)),
        kv_line("w/W", "Wave", &fmt!("{:.2}", config.wave_amplitude)),
        kv_line("h/H", "Pulse", &fmt!("{:.1}", config.color_pulse_speed)),
        kv_line("l/L", "Scan", &scan_str),
        kv_line("z/Z", "Zalgo", &fmt!("{:.1}", config.zalgo_intensity)),
        kv_line("y/Y", "Stabil", &fmt!("{:.1}", config.temporal_stability)),
        kv_line("j/J", "StDcy", &fmt!("{:.2}", config.strobe_decay)),
        kv_line("u/U", "WSpeed", &fmt!("{:.1}", config.wave_speed)),
    ];

    // Compact mode: hide zero-value effects on small terminals
    let compact_effects = area.height < 30;
    if compact_effects {
        lines.retain(|line| {
            // Keep section headers and lines with non-zero values
            let text = line.to_string();
            // Keep all non-effect lines (they don't start with key hints like "f/F")
            !text.contains(": 0.0") && !text.contains(": OFF")
        });
    }

    // ─── Camera (conditional — hidden on small terminals) ─────────
    let show_camera = area.height >= 35;
    if show_camera {
        lines.push(Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500} Camera \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(section),
        )));
        lines.push(kv_line(
            "\u{2191}\u{2193}\u{2190}\u{2192}",
            "Pan",
            &format!("{:.2},{:.2}", config.camera_pan_x, config.camera_pan_y),
        ));
        lines.push(kv_line(
            "Scrl",
            "Zoom",
            &fmt!("{:.2}", config.camera_zoom_amplitude),
        ));
        lines.push(kv_line(
            ",/.",
            "Rot",
            &fmt!("{:.2}", config.camera_rotation),
        ));
        lines.push(kv_line(
            "Drag",
            "Tilt",
            &fmt!("{:.2}", config.camera_tilt_x),
        ));
    }

    // ─── Audio ──────────────
    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500}\u{2500} Audio \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(section),
    )));
    lines.push(kv_line(
        "S+\u{2191}\u{2193}",
        "Sens",
        &fmt!("{:.1}", config.audio_sensitivity),
    ));
    lines.push(kv_line(
        "",
        "Smooth",
        &fmt!("{:.2}", config.audio_smoothing),
    ));

    if let Some(features) = audio {
        lines.push(kv_line("", "RMS", &fmt!("{:.2}", features.rms)));
        lines.push(kv_line("", "BPM", &fmt!("{:.0}", features.bpm)));
        lines.push(Line::from(Span::styled(
            format!(" {onset_str}"),
            Style::default().fg(if features.onset {
                Color::Red
            } else {
                Color::DarkGray
            }),
        )));
    }

    // Playback position (MM:SS) when media clock is available
    if let Some(secs) = playback_pos_secs {
        let total_s = secs as u64;
        let mm = total_s / 60;
        let ss = total_s % 60;
        lines.push(Line::from(Span::styled(
            format!(" \u{25b6} {mm:02}:{ss:02}"),
            Style::default().fg(Color::DarkGray),
        )));
    }

    // ─── Info (condensed on small terminals) ────────────────
    let show_full_info = area.height >= 28;

    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500}\u{2500} Info \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(section),
    )));
    if perf_warning {
        lines.push(Line::from(vec![
            Span::styled(fmt!(" {:.0} FPS ", fps_counter.fps()), Style::default()),
            Span::styled("!", Style::default().fg(Color::Yellow)),
        ]));
    } else {
        lines.push(Line::from(fmt!(" {:.0} FPS", fps_counter.fps())));
    }
    lines.push(Line::from(fmt!(" {:.1}ms", fps_counter.frame_time_ms)));

    if show_full_info {
        let truncate = |name: &str, max: usize| -> String {
            let count = name.chars().count();
            if count > max {
                let skip = count - (max - 2);
                let suffix: String = name.chars().skip(skip).collect();
                format!("..{suffix}")
            } else {
                name.to_owned()
            }
        };
        if let Some(name) = loaded_visual {
            lines.push(Line::from(vec![
                Span::styled(" V ", Style::default().fg(label)),
                Span::styled(truncate(name, 18), Style::default().fg(Color::Cyan)),
            ]));
        }
        if let Some(name) = loaded_audio {
            lines.push(Line::from(vec![
                Span::styled(" A ", Style::default().fg(label)),
                Span::styled(truncate(name, 18), Style::default().fg(Color::Magenta)),
            ]));
        }

        lines.push(Line::from(""));
        let creation_indicator = if creation_mode_active {
            Span::styled("K\u{25cf}", Style::default().fg(Color::Cyan))
        } else {
            Span::styled("K\u{25cb}", Style::default().fg(Color::DarkGray))
        };
        lines.push(Line::from(vec![
            Span::styled(" [o]Med ", Style::default().fg(Color::DarkGray)),
            Span::styled("[O]Aud ", Style::default().fg(Color::DarkGray)),
            creation_indicator,
        ]));
    }

    let sidebar_title = if param_flash > 0 {
        " Params \u{25cf} "
    } else if perf_warning {
        " Params \u{26a0} "
    } else {
        " Params "
    };
    let title_color = if param_flash > 0 {
        Color::Green
    } else {
        Color::White
    };
    let sidebar = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(sidebar_title)
            .title_style(Style::default().fg(title_color)),
    );

    frame.render_widget(sidebar, area);
}

/// Draw a scrollable help overlay with all keybindings.
#[allow(clippy::too_many_lines)]
fn draw_help_overlay(frame: &mut Frame, area: Rect, scroll: u16) {
    let help_text = vec![
        Line::from(Span::styled(
            " clasSCII \u{2014} Controls ",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Navigation \u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" q/Esc    Quit"),
        Line::from(" Space    Play/Pause"),
        Line::from(" ?        Toggle help"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Render \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" Tab      Cycle mode \u{2192}"),
        Line::from(" Shft+Tab Cycle mode \u{2190}"),
        Line::from(" 1-0      Select charset"),
        Line::from(" d/D      Density \u{00b1}"),
        Line::from(" c        Toggle color"),
        Line::from(" m        Color mode"),
        Line::from(" i        Invert"),
        Line::from(" [ ]      Contrast \u{00b1}"),
        Line::from(" { }      Brightness \u{00b1}"),
        Line::from(" -/+      Saturation \u{00b1}"),
        Line::from(" e/E      Edge toggl/mix"),
        Line::from(" s        Shapes"),
        Line::from(" a        Aspect ratio"),
        Line::from(" b        BG style"),
        Line::from(" n        Dither mode"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Effects \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" f/F      Fade \u{00b1}"),
        Line::from(" g/G      Glow \u{00b1}"),
        Line::from(" t/T      Flash \u{00b1}"),
        Line::from(" r/R      Chromatic \u{00b1}"),
        Line::from(" w/W      Wave \u{00b1}"),
        Line::from(" h/H      Color pulse \u{00b1}"),
        Line::from(" l/L      Scan lines"),
        Line::from(" z/Z      Zalgo \u{00b1}"),
        Line::from(" y/Y      Stability \u{00b1}"),
        Line::from(" j/J      Strobe dcy \u{00b1}"),
        Line::from(" u/U      Wave speed \u{00b1}"),
        Line::from(" N/M      Input gain \u{00b1}"),
        Line::from(Span::styled(
            " Color FX best in ASCII/Quadrant",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Camera \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" \u{2191}\u{2193}\u{2190}\u{2192}    Pan"),
        Line::from(" Scroll   Zoom \u{00b1}"),
        Line::from(" Sh+Scrl  Rotation \u{00b1}"),
        Line::from(" L-Drag   Pan (mouse)"),
        Line::from(" R-Drag   Rotation + Tilt"),
        Line::from(" Bksp     Reset camera"),
        Line::from(" Sh+Bksp  Reset all params"),
        Line::from(" </>      Zoom \u{00b1} (keys)"),
        Line::from(" ,/.      Rotation \u{00b1} (keys)"),
        Line::from(" ;/'      Pan X \u{00b1} (keys)"),
        Line::from(" :/\"      Pan Y \u{00b1} (keys)"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Audio \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" Sh+\u{2191}/\u{2193}   Sensitivity \u{00b1}"),
        Line::from(" Sh+\u{2190}/\u{2192}   Seek \u{00b1}5s"),
        Line::from(" v        Spectrum"),
        Line::from(" p/P      Preset cycle"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Overlays \u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" o        Open visual"),
        Line::from(" O        Open audio"),
        Line::from(" Ctrl+D   Open batch folder"),
        Line::from(" C        Charset editor"),
        Line::from(" K        Creation (Esc=hide q=off)"),
        Line::from(" S        Stem separation mode"),
        Line::from(" Ctrl+S   Save workflow"),
        Line::from(" Ctrl+W   Load workflow"),
        Line::from(" x        Fullscreen"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? or Esc to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let total_lines = help_text.len() as u16;
    let help_width = 42u16.min(area.width.saturating_sub(4));
    let max_height = area.height.saturating_sub(2);
    let help_height = (total_lines + 2).min(max_height);
    let x = area.x + area.width.saturating_sub(help_width) / 2;
    let y = area.y + area.height.saturating_sub(help_height) / 2;
    let help_area = Rect::new(x, y, help_width, help_height);

    // Scroll indicators in title
    let inner_height = help_height.saturating_sub(2);
    let can_scroll_up = scroll > 0;
    let can_scroll_down = total_lines > scroll + inner_height;
    let title = if can_scroll_up && can_scroll_down {
        " Help \u{25b2}\u{25bc} "
    } else if can_scroll_up {
        " Help \u{25b2} "
    } else if can_scroll_down {
        " Help \u{25bc} "
    } else {
        " Help "
    };

    let help = Paragraph::new(help_text).scroll((scroll, 0)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(help, help_area);
}

/// Draw the Charset Editor overlay.
fn draw_charset_edit_overlay(frame: &mut Frame, area: Rect, buf: &str, cursor: usize) {
    let mut text_lines = Vec::new();

    text_lines.push(Line::from(Span::styled(
        "Order: lightest -> densest",
        Style::default().fg(Color::DarkGray),
    )));
    text_lines.push(Line::from(""));

    let chars: Vec<char> = buf.chars().collect();
    let mut preview_spans = Vec::new();
    preview_spans.push(Span::raw(" > "));

    for (i, &ch) in chars.iter().enumerate() {
        if i == cursor {
            preview_spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            ));
        } else {
            preview_spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(Color::Cyan),
            ));
        }
    }
    // Append cursor if at the very end
    if cursor == chars.len() {
        preview_spans.push(Span::styled(" ", Style::default().bg(Color::White)));
    }
    text_lines.push(Line::from(preview_spans));

    text_lines.push(Line::from(""));
    let len = chars.len();
    if len >= 2 {
        text_lines.push(Line::from(Span::styled(
            format!("Length: {len} | OK \u{2014} live preview"),
            Style::default().fg(Color::Green),
        )));
    } else {
        text_lines.push(Line::from(Span::styled(
            format!("Length: {len} | Need 2+ chars"),
            Style::default().fg(Color::Red),
        )));
    }

    text_lines.push(Line::from(""));
    text_lines.push(Line::from(Span::styled(
        "Enter=apply  Esc=cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 44u16.min(area.width.saturating_sub(4));
    let overlay_height = text_lines.len() as u16 + 2;
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(text_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Charset Editor ")
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(widget, overlay_area);
}

/// Draw the creation mode overlay with effect list and audio meters.
#[allow(clippy::too_many_lines)]
fn draw_creation_overlay(
    frame: &mut Frame,
    area: Rect,
    creation: &CreationOverlayData<'_>,
    audio: Option<&AudioFeatures>,
) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(24);

    // Header — [AUTO] / [MANUAL] indicator
    let (mode_label, mode_color) = if creation.auto_mode {
        ("[AUTO]", Color::Green)
    } else {
        ("[MANUAL]", Color::Red)
    };

    lines.push(Line::from(vec![
        Span::styled("  [a] ", Style::default().fg(Color::Gray)),
        Span::styled(mode_label, Style::default().fg(mode_color)),
        Span::styled("  [p] Preset: ", Style::default().fg(Color::Gray)),
        Span::styled(creation.preset_name, Style::default().fg(Color::Yellow)),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  \u{2500}\u{2500} Effects \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));

    // Effects list (index 0 = Master, 1-9 = effects)
    for (i, (name, value, max)) in creation.effects.iter().enumerate() {
        let selected = i == creation.selected_effect;
        let prefix = if selected { " \u{25b8} " } else { "   " };
        let bar_len = if *max > 0.0 {
            (value / max * 10.0).min(10.0) as usize
        } else {
            0
        };
        let bar_color = if i == 0 { Color::Cyan } else { Color::Green };
        let bar: String =
            "\u{2588}".repeat(bar_len) + &"\u{2591}".repeat(10_usize.saturating_sub(bar_len));
        let name_color = if selected { Color::White } else { Color::Gray };
        // Auto-modulated indicator: ~ for effects (not Master) when auto is on
        let auto_mark = if i > 0 && creation.auto_mode { "~" } else { "" };

        // Use higher precision for Strobe Decay (max 0.99)
        let val_fmt = if i == 9 {
            format!(" {value:.2}/{max:.2}")
        } else {
            format!(" {value:.1}/{max:.1}")
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{name:<12}"), Style::default().fg(name_color)),
            Span::styled(bar, Style::default().fg(bar_color)),
            Span::styled(val_fmt, Style::default().fg(Color::White)),
            Span::styled(auto_mark, Style::default().fg(Color::DarkGray)),
        ]));
    }

    // Audio meters
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  \u{2500}\u{2500} Audio Live \u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));

    if let Some(af) = audio {
        // Clamp bars to prevent overflow on high sensitivity
        let bar = |v: f32| "\u{2588}".repeat((v * 4.0).clamp(0.0, 8.0) as usize);
        let rms_bar = bar(af.rms);
        let bass_bar = bar(af.bass);
        let mid_bar = bar(af.mid);
        let high_bar = bar(af.brilliance);
        lines.push(Line::from(vec![
            Span::styled("  RMS ", Style::default().fg(Color::Gray)),
            Span::styled(rms_bar, Style::default().fg(Color::Green)),
            Span::styled("  Bass ", Style::default().fg(Color::Gray)),
            Span::styled(bass_bar, Style::default().fg(Color::Red)),
            Span::styled("  Mid ", Style::default().fg(Color::Gray)),
            Span::styled(mid_bar, Style::default().fg(Color::Yellow)),
            Span::styled("  Hi ", Style::default().fg(Color::Gray)),
            Span::styled(high_bar, Style::default().fg(Color::Cyan)),
        ]));

        let bpm_str = if af.bpm > 0.0 {
            format!("{:.0}", af.bpm)
        } else {
            "--".into()
        };
        let beat_str = if af.onset {
            "\u{25cf} BEAT"
        } else {
            "\u{25cb}"
        };
        let beat_color = if af.onset {
            Color::Red
        } else {
            Color::DarkGray
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  BPM: {bpm_str}"),
                Style::default().fg(Color::White),
            ),
            Span::styled("  ", Style::default()),
            Span::styled(beat_str, Style::default().fg(beat_color)),
        ]));
    } else {
        lines.push(Line::from(Span::styled(
            "  (no audio)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Footer
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " [\u{2191}\u{2193}]Sel [\u{2190}\u{2192}]Adj [a]Auto [p]Pre [Esc]Hide",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 44u16.min(area.width.saturating_sub(2));
    let overlay_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    // Dock to right side (non-invasive — leaves canvas visible on the left)
    let x = area.x + area.width.saturating_sub(overlay_width).saturating_sub(1);
    let y = area.y + 1;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" CREATION MODE ")
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(widget, overlay_area);
}

/// Draw the stem separation mode overlay with per-stem meters, controls, and spectrum.
#[allow(clippy::too_many_lines)]
fn draw_stem_overlay(frame: &mut Frame, area: Rect, stem: &StemOverlayData) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(32);

    // Separation in progress — show progress bar
    if let Some(pct) = stem.separation_progress {
        lines.push(Line::from(Span::styled(
            "  Separating stems...",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
        let bar_width = 30usize;
        let filled = ((pct * bar_width as f32) as usize).min(bar_width);
        let bar = format!(
            "  [{}{}]  {:.0}%",
            "\u{2588}".repeat(filled),
            " ".repeat(bar_width.saturating_sub(filled)),
            pct * 100.0
        );
        lines.push(Line::from(Span::styled(
            bar,
            Style::default().fg(Color::Cyan),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  [Esc] Cancel",
            Style::default().fg(Color::DarkGray),
        )));
    } else if !stem.has_stems {
        // No stems loaded — prompt user
        if stem.has_audio {
            lines.push(Line::from(Span::styled(
                "  Audio loaded. Press [Enter] to",
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(Span::styled(
                "  separate into stems (SCNet).",
                Style::default().fg(Color::Gray),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  [Enter] Separate  [Esc] Close",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  No audio loaded.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  Load audio first (O key),",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  then open Stem Mode (S key).",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  [Esc] Close",
                Style::default().fg(Color::DarkGray),
            )));
        }
    } else {
        // Header
        lines.push(Line::from(Span::styled(
            "  \u{2500}\u{2500} Stems \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::DarkGray),
        )));

        // Per-stem rows
        for (i, s) in stem.stems.iter().enumerate() {
            let selected = i == stem.selected_idx;
            let prefix = if selected { " \u{25b8} " } else { "   " };

            let (r, g, b) = s.color;
            let stem_color = Color::Rgb(r, g, b);

            // Mute/Solo indicators
            let mute_str = if s.muted { "M" } else { " " };
            let solo_str = if s.solo { "S" } else { " " };
            let mute_color = if s.muted { Color::Red } else { Color::DarkGray };
            let solo_color = if s.solo {
                Color::Yellow
            } else {
                Color::DarkGray
            };

            // Volume bar
            let vol_bars = ((s.volume * 5.0) as usize).min(10);
            let vol_bar: String =
                "\u{2588}".repeat(vol_bars) + &"\u{2591}".repeat(10usize.saturating_sub(vol_bars));

            // RMS level indicator
            let rms_bars = ((s.rms * 8.0) as usize).min(8);
            let rms_bar: String = "\u{2588}".repeat(rms_bars);

            // Beat indicator
            let beat_char = if s.onset { "\u{25cf}" } else { " " };

            let name_color = if selected { Color::White } else { stem_color };

            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:<6}", s.label), Style::default().fg(name_color)),
                Span::styled(mute_str, Style::default().fg(mute_color)),
                Span::styled(solo_str, Style::default().fg(solo_color)),
                Span::styled(" ", Style::default()),
                Span::styled(vol_bar, Style::default().fg(stem_color)),
                Span::styled(
                    format!(" {:.0}%", s.volume * 100.0),
                    Style::default().fg(Color::Gray),
                ),
                Span::styled(" ", Style::default()),
                Span::styled(rms_bar, Style::default().fg(Color::Green)),
                Span::styled(format!(" {beat_char}"), Style::default().fg(Color::Red)),
            ]));

            // Mini spectrum for selected stem (condensed 32→16 bars)
            if selected && s.visible {
                let band_chars = [
                    "\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}", "\u{2585}", "\u{2586}",
                    "\u{2587}", "\u{2588}",
                ];
                let mut spectrum_spans: Vec<Span<'_>> =
                    vec![Span::styled("     ", Style::default())];
                for chunk_idx in 0..16 {
                    let avg =
                        f32::midpoint(s.spectrum[chunk_idx * 2], s.spectrum[chunk_idx * 2 + 1]);
                    let level = (avg * 7.0).clamp(0.0, 7.0) as usize;
                    spectrum_spans.push(Span::styled(
                        band_chars[level],
                        Style::default().fg(stem_color),
                    ));
                }
                lines.push(Line::from(spectrum_spans));
            }
        }

        // Footer controls
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  \u{2500}\u{2500} Controls \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            " [\u{2191}\u{2193}]Sel [\u{2190}\u{2192}]Vol [m]Mute [s]Solo",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            " [v]Vis [c]ClearSolo [Esc]Close",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let overlay_width = 48u16.min(area.width.saturating_sub(2));
    let overlay_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(overlay_width).saturating_sub(1);
    let y = area.y + 1;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" STEM MODE ")
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(widget, overlay_area);
}

/// Draw the workflow save overlay (name + description input).
fn draw_workflow_save_overlay(frame: &mut Frame, area: Rect, data: &WorkflowSaveData<'_>) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(16);

    lines.push(Line::from(Span::styled(
        "  Save Workflow",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));

    // Name field
    let name_style = if data.active_field == 0 {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    lines.push(Line::from(Span::styled("  Name:", name_style)));
    let name_display = if data.active_field == 0 {
        let mut s = format!("  {}", data.name);
        let cursor_pos = data.cursor + 2;
        if cursor_pos <= s.len() {
            s.insert(cursor_pos, '\u{2502}');
        }
        s
    } else {
        format!("  {}", data.name)
    };
    lines.push(Line::from(Span::styled(name_display, name_style)));
    lines.push(Line::from(""));

    // Description field
    let desc_style = if data.active_field == 1 {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    lines.push(Line::from(Span::styled("  Description:", desc_style)));
    let desc_display = if data.active_field == 1 {
        let mut s = format!("  {}", data.description);
        let cursor_pos = data.cursor + 2;
        if cursor_pos <= s.len() {
            s.insert(cursor_pos, '\u{2502}');
        }
        s
    } else {
        let d = if data.description.is_empty() {
            "(optional)"
        } else {
            data.description
        };
        format!("  {d}")
    };
    lines.push(Line::from(Span::styled(desc_display, desc_style)));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "  Tab=switch  Enter=save  Esc=cancel",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 50u16.min(area.width.saturating_sub(4));
    let overlay_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" SAVE WORKFLOW ")
            .style(Style::default().bg(Color::Black).fg(Color::Cyan)),
    );

    frame.render_widget(widget, overlay_area);
}

/// Draw the workflow browse/load overlay (list of saved workflows).
fn draw_workflow_browse_overlay(frame: &mut Frame, area: Rect, data: &WorkflowBrowseData) {
    let mut lines: Vec<Line<'_>> = Vec::with_capacity(32);

    lines.push(Line::from(Span::styled(
        "  Load Workflow",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));

    if data.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No saved workflows found.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (i, entry) in data.entries.iter().enumerate() {
            let is_selected = i == data.selected_idx;
            let prefix = if is_selected { "> " } else { "  " };
            let style = if is_selected {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let mut tags = String::new();
            if entry.has_stems {
                tags.push_str(" [S]");
            }
            if entry.has_timeline {
                tags.push_str(" [T]");
            }

            lines.push(Line::from(Span::styled(
                format!("{prefix}{}{tags}", entry.name),
                style,
            )));

            if is_selected {
                let detail_style = Style::default().fg(Color::DarkGray);
                lines.push(Line::from(Span::styled(
                    format!("    {}", entry.created_at),
                    detail_style,
                )));
                if !entry.description.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("    {}", entry.description),
                        detail_style,
                    )));
                }
            }
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Up/Down=nav  Enter=load  Del=delete  Esc=close",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 55u16.min(area.width.saturating_sub(4));
    let overlay_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4));
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" BROWSE WORKFLOWS ")
            .style(Style::default().bg(Color::Black).fg(Color::Cyan)),
    );

    frame.render_widget(widget, overlay_area);
}
