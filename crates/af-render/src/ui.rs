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
    /// File/Folder selection prompt.
    FileOrFolderPrompt,
    /// Creation mode interactive overlay.
    CreationMode,
    /// Quitting (should not reach draw).
    Quitting,
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
        );
    }

    // Overlays (modal, one at a time)
    // Help overlay draws even in fullscreen if toggled
    if *ctx.state == RenderState::Help {
        draw_help_overlay(frame, area);
    } else if *ctx.state == RenderState::FileOrFolderPrompt {
        draw_prompt_overlay(frame, area);
    } else if let Some((buf, cursor)) = ctx.charset_edit {
        draw_charset_edit_overlay(frame, area, buf, cursor);
    } else if let Some(creation) = ctx.creation {
        draw_creation_overlay(frame, area, creation, ctx.audio);
    }
}

/// Draw the File/Folder prompt overlay.
fn draw_prompt_overlay(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "Select Action",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [F] ", Style::default().fg(Color::Green)),
            Span::styled("Open Single Media File", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [D] ", Style::default().fg(Color::Magenta)),
            Span::styled(
                "Select Folder for Batch Export Generation",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " Esc to cancel ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let overlay_width = 48u16.min(area.width.saturating_sub(4));
    let overlay_height = 9u16;
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Action ")
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(widget, overlay_area);
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

        RenderState::FileOrFolderPrompt => "? SELECT",
        RenderState::CreationMode => "K CREATE",
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

    // Reusable formatting buffer — single allocation reused for all numeric values
    let mut buf = String::with_capacity(16);

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
        ($fmt:literal, $val:expr) => {{
            use std::fmt::Write;
            buf.clear();
            write!(buf, $fmt, $val).ok();
            buf.clone()
        }};
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

    // ─── Camera (conditional — hidden on small terminals) ─────────
    let show_camera = area.height >= 35;
    if show_camera {
        lines.push(Line::from(Span::styled(
            "\u{2500}\u{2500}\u{2500} Camera \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(section),
        )));
        lines.push(kv_line(
            "</>",
            "Zoom",
            &fmt!("{:.2}", config.camera_zoom_amplitude),
        ));
        lines.push(kv_line(
            ",/.",
            "Rot",
            &fmt!("{:.2}", config.camera_rotation),
        ));
        lines.push(kv_line(";/'", "PanX", &fmt!("{:.2}", config.camera_pan_x)));
        lines.push(kv_line(":/\"", "PanY", &fmt!("{:.2}", config.camera_pan_y)));
    }

    // ─── Audio ──────────────
    lines.push(Line::from(Span::styled(
        "\u{2500}\u{2500}\u{2500} Audio \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(section),
    )));
    lines.push(kv_line(
        "\u{2191}/\u{2193}",
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
            Span::styled(" K\u{25cf}", Style::default().fg(Color::Cyan))
        } else {
            Span::styled(" K\u{25cb}", Style::default().fg(label))
        };
        lines.push(Line::from(vec![
            Span::styled(" o/O C A ?", Style::default().fg(label)),
            creation_indicator,
        ]));
    }

    let sidebar =
        Paragraph::new(lines).block(Block::default().borders(Borders::LEFT).title(" Params "));

    frame.render_widget(sidebar, area);
}

/// Draw a semi-transparent help overlay with all keybindings.
fn draw_help_overlay(frame: &mut Frame, area: Rect) {
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
        Line::from(" t/T      Strobe \u{00b1}"),
        Line::from(" r/R      Chromatic \u{00b1}"),
        Line::from(" w/W      Wave \u{00b1}"),
        Line::from(" h/H      Color pulse \u{00b1}"),
        Line::from(" l/L      Scan lines"),
        Line::from(" z/Z      Zalgo \u{00b1}"),
        Line::from(" y/Y      Stability \u{00b1}"),
        Line::from(" j/J      Strobe dcy \u{00b1}"),
        Line::from(" u/U      Wave speed \u{00b1}"),
        Line::from(Span::styled(
            " Color FX best in ASCII/Quadrant",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Camera \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" </>      Zoom \u{00b1}"),
        Line::from(" ,/.      Rotation \u{00b1}"),
        Line::from(" ;/'      Pan X \u{00b1}"),
        Line::from(" :/\"      Pan Y \u{00b1}"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Audio \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" \u{2191}/\u{2193}      Sensitivity \u{00b1}"),
        Line::from(" \u{2190}/\u{2192}      Seek \u{00b1}5s"),
        Line::from(" v        Spectrum"),
        Line::from(" p/P      Preset cycle"),
        Line::from(Span::styled(
            " \u{2500}\u{2500} Overlays \u{2500}\u{2500}\u{2500}\u{2500}",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" o/Ctrl+O Open visual"),
        Line::from(" O        Open audio"),
        Line::from(" C        Charset editor"),
        Line::from(" K        Creation (Esc=hide q=off)"),
        Line::from(" x        Fullscreen"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? or Esc to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help_width = 42u16.min(area.width.saturating_sub(4));
    let max_height = area.height.saturating_sub(2);
    let help_height = (help_text.len() as u16 + 2).min(max_height);
    let x = area.x + area.width.saturating_sub(help_width) / 2;
    let y = area.y + area.height.saturating_sub(help_height) / 2;
    let help_area = Rect::new(x, y, help_width, help_height);

    let help = Paragraph::new(help_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
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
        " [\u{2191}\u{2193}] Select [\u{2190}\u{2192}] Adjust [a] Auto [p] Preset [Esc] Hide [q] Off",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 52u16.min(area.width.saturating_sub(4));
    let overlay_height = lines.len() as u16 + 2;
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" CREATION MODE ")
            .style(Style::default().bg(Color::Black).fg(Color::White)),
    );

    frame.render_widget(widget, overlay_area);
}
