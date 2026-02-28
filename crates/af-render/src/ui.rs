use af_core::config::{BgStyle, ColorMode, RenderConfig};
use af_core::frame::{AsciiGrid, AudioFeatures};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

use crate::canvas;
use crate::fps::FpsCounter;

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
use crate::widgets::AudioPanelState;

/// Application state enum (mirrored for rendering decisions).
///
/// # Example
/// ```
/// use af_render::ui::RenderState;
/// let state = RenderState::Running;
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderState {
    /// Normal running state.
    Running,
    /// Paused.
    Paused,
    /// Help overlay visible.
    Help,
    /// Éditeur de charset personnalisé affiché (touche C).
    CharsetEdit,
    /// Panneau de mixage audio affiché (touche A).
    AudioPanel,
    /// Invite choix Fichier/Dossier.
    FileOrFolderPrompt,
    /// Mode création interactif.
    CreationMode,
    /// Quitting (should not reach draw).
    Quitting,
}

/// Draw the full UI: canvas + sidebar + spectrum bar.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    grid: &AsciiGrid,
    config: &RenderConfig,
    base_config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    preset_name: Option<&str>,
    loaded_visual: Option<&str>,
    loaded_audio: Option<&str>,
    _sidebar_dirty: bool,
    state: &RenderState,
    layout_charset_edit: Option<(&str, usize)>,
    layout_audio_panel: Option<(&AudioPanelState, &RenderConfig)>,
    layout_creation: Option<&CreationOverlayData<'_>>,
    creation_mode_active: bool,
    perf_warning: bool,
) {
    let area = frame.area();

    // Minimum terminal size guard
    if area.width < 80 || area.height < 20 {
        let msg = format!(
            "Terminal too small ({}x{}, need 80x20)",
            area.width, area.height
        );
        let p = Paragraph::new(msg)
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Red));
        frame.render_widget(p, area);
        return;
    }

    // Si le mode plein écran exclusif est activé, ne rendre que le canevas ASCII
    if config.fullscreen {
        canvas::render_grid(frame.buffer_mut(), area, grid, config.zalgo_intensity);
    } else {
        // === Sidebar & Layout setup ===
        let sidebar_width = 24u16; // Slight expansion for keybind hints
        let h_chunks = Layout::horizontal([Constraint::Min(40), Constraint::Length(sidebar_width)])
            .split(area);

        // === Canvas & Spectrum splits ===
        let canvas_area = if config.show_spectrum {
            let v_chunks =
                Layout::vertical([Constraint::Min(10), Constraint::Length(3)]).split(h_chunks[0]);
            let spectrum_area = v_chunks[1];
            draw_spectrum(frame, spectrum_area, audio);
            v_chunks[0]
        } else {
            h_chunks[0]
        };

        canvas::render_grid(
            frame.buffer_mut(),
            canvas_area,
            grid,
            config.zalgo_intensity,
        );

        // === Sidebar ===
        let sidebar_area = h_chunks[1];
        draw_sidebar(
            frame,
            sidebar_area,
            base_config,
            audio,
            fps_counter,
            preset_name,
            loaded_visual,
            loaded_audio,
            state,
            creation_mode_active,
            perf_warning,
        );
    }

    // === Help overlay ===
    // Force draw help even in fullscreen if toggled
    if *state == RenderState::Help {
        draw_help_overlay(frame, area);
    } else if *state == RenderState::FileOrFolderPrompt {
        draw_prompt_overlay(frame, area);
    } else if let Some((buf, cursor)) = layout_charset_edit {
        draw_charset_edit_overlay(frame, area, buf, cursor);
    } else if let Some((panel_state, rcfg)) = layout_audio_panel {
        draw_audio_panel_overlay(frame, area, rcfg, panel_state, audio);
    } else if let Some(creation) = layout_creation {
        draw_creation_overlay(frame, area, creation, audio);
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

    let overlay_width = 48u16;
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

    // Color by average intensity
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
        af_core::config::RenderMode::HalfBlock => "HalfBlk",
        af_core::config::RenderMode::Quadrant => "Quadrnt",
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
        RenderState::Running => "▶ RUN",
        RenderState::Paused => "⏸ PAUSE",
        RenderState::Help => "? HELP",
        RenderState::CharsetEdit => "C EDIT",
        RenderState::AudioPanel => "A MIX",
        RenderState::FileOrFolderPrompt => "? SELECT",
        RenderState::CreationMode => "K CREATE",
        RenderState::Quitting => "⏹ QUIT",
    };

    let charset_names = [
        "Standard", "Compact", "Short", "Blocks", "Minimal", "Organic", "Bars", "Edge", "Digital",
        "Binary",
    ];
    let charset_name = charset_names.get(config.charset_index).unwrap_or(&"Custom");

    // Onset indicator
    let onset_str = if audio.is_some_and(|a| a.onset) {
        "● BEAT"
    } else {
        "○"
    };

    // Reusable formatting buffer — single allocation reused for all numeric values
    let mut buf = String::with_capacity(16);

    // Key-value line builder: labels via static &str (zero-alloc), values via shared buf
    let label = Color::Gray;
    let val_c = Color::White;
    let section = Color::Yellow;
    let kv_line = |k: &str, lbl: &str, val: &str| -> Line {
        Line::from(vec![
            Span::styled(format!(" {k:<5} "), Style::default().fg(label)),
            Span::styled(format!("{lbl:<5}: "), Style::default().fg(label)),
            Span::styled(val.to_owned(), Style::default().fg(val_c)),
        ])
    };

    // Pre-format all numeric values into owned strings via buf (reuse capacity)
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
            "─── Render ─────────",
            Style::default().fg(section),
        )),
        kv_line("[Tab]", "Mode", mode_str),
        kv_line("[p/P]", "Prst", preset_name.unwrap_or("Custom")),
        kv_line("[1-0]", "Char", charset_name),
        kv_line("[d/D]", "Dens", &fmt!("{:.2}", config.density_scale)),
        kv_line(
            "[c]",
            "Colr",
            if config.color_enabled { "ON" } else { "OFF" },
        ),
        kv_line("[m]", "CMod", color_mode_str),
        kv_line("[i]", "Inv", if config.invert { "ON" } else { "OFF" }),
        kv_line("[/]", "Cont", &fmt!("{:.1}", config.contrast)),
        kv_line("{ }", "Brgt", &fmt!("{:.2}", config.brightness)),
        kv_line("-/+", "Sat", &fmt!("{:.1}", config.saturation)),
        kv_line("[e]", "Edge", &fmt!("{:.1}", config.edge_threshold)),
        kv_line("[E]", "EMix", &fmt!("{:.2}", config.edge_mix)),
        kv_line(
            "[s]",
            "Shap",
            if config.shape_matching { "ON" } else { "OFF" },
        ),
        kv_line("[a]", "Aspc", &fmt!("{:.1}", config.aspect_ratio)),
        kv_line("[b]", "BG", bg_str),
        kv_line("[n]", "Dthr", dither_str),
        // ─── Effects ────────────
        Line::from(Span::styled(
            "─── Effects ────────",
            Style::default().fg(section),
        )),
        kv_line("[f/F]", "Fade", &fmt!("{:.1}", config.fade_decay)),
        kv_line("[g/G]", "Glow", &fmt!("{:.1}", config.glow_intensity)),
        kv_line(
            "[t/T]",
            "Strob",
            &fmt!("{:.1}", config.beat_flash_intensity),
        ),
        kv_line("[r/R]", "Chrom", &fmt!("{:.1}", config.chromatic_offset)),
        kv_line("[w/W]", "Wave", &fmt!("{:.2}", config.wave_amplitude)),
        kv_line("[h/H]", "Pulse", &fmt!("{:.1}", config.color_pulse_speed)),
        kv_line("[l/L]", "Scan", &scan_str),
        kv_line("[z/Z]", "Zalgo", &fmt!("{:.1}", config.zalgo_intensity)),
        kv_line("[y/Y]", "TStab", &fmt!("{:.1}", config.temporal_stability)),
        kv_line("[j/J]", "SDcy", &fmt!("{:.2}", config.strobe_decay)),
        kv_line("[u/U]", "WSpd", &fmt!("{:.1}", config.wave_speed)),
        // ─── Camera ─────────────
        Line::from(Span::styled(
            "─── Camera ─────────",
            Style::default().fg(section),
        )),
        kv_line(
            "[</>]",
            "Zoom",
            &fmt!("{:.2}", config.camera_zoom_amplitude),
        ),
        kv_line("[,/.]", "Rot", &fmt!("{:.2}", config.camera_rotation)),
        kv_line("[;/']", "PanX", &fmt!("{:.2}", config.camera_pan_x)),
        kv_line("[:/ \"]", "PanY", &fmt!("{:.2}", config.camera_pan_y)),
        // ─── Audio ──────────────
        Line::from(Span::styled(
            "─── Audio ──────────",
            Style::default().fg(section),
        )),
        kv_line("↑/↓", "Sens", &fmt!("{:.1}", config.audio_sensitivity)),
        kv_line("", "Smth", &fmt!("{:.2}", config.audio_smoothing)),
    ];

    if let Some(features) = audio {
        lines.push(Line::from(Span::styled(
            fmt!(" RMS: {:.2}", features.rms),
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(Span::styled(
            fmt!(" BPM: {:.0}", features.bpm),
            Style::default().fg(Color::White),
        )));
        lines.push(Line::from(Span::styled(
            format!(" {onset_str}"),
            Style::default().fg(if features.onset {
                Color::Red
            } else {
                Color::DarkGray
            }),
        )));
    }

    // ─── Info ───────────────
    lines.push(Line::from(Span::styled(
        "─── Info ───────────",
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

    let truncate = |name: &str, max: usize| -> String {
        if name.len() > max {
            format!("..{}", &name[name.len().saturating_sub(max - 2)..])
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
        Span::styled(" o/O=open C=char A=mix", Style::default().fg(label)),
        creation_indicator,
    ]));

    let sidebar =
        Paragraph::new(lines).block(Block::default().borders(Borders::LEFT).title(" Params "));

    frame.render_widget(sidebar, area);
}

/// Draw a semi-transparent help overlay with all keybindings.
fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            " clasSCII — Controls ",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            " ── Navigation ──",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" q/Esc    Quit"),
        Line::from(" Space    Play/Pause"),
        Line::from(" ?        Toggle help"),
        Line::from(Span::styled(
            " ── Render ──────",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" Tab      Cycle mode"),
        Line::from(" 1-0      Select charset"),
        Line::from(" d/D      Density ±"),
        Line::from(" c        Toggle color"),
        Line::from(" m        Color mode"),
        Line::from(" i        Invert"),
        Line::from(" [ ]      Contrast ±"),
        Line::from(" { }      Brightness ±"),
        Line::from(" -/+      Saturation ±"),
        Line::from(" e/E      Edge toggl/mix"),
        Line::from(" s        Shapes"),
        Line::from(" a        Aspect ratio"),
        Line::from(" b        BG style"),
        Line::from(" n        Dither mode"),
        Line::from(Span::styled(
            " ── Effects ─────",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" f/F      Fade ±"),
        Line::from(" g/G      Glow ±"),
        Line::from(" t/T      Strobe ±"),
        Line::from(" r/R      Chromatic ±"),
        Line::from(" w/W      Wave ±"),
        Line::from(" h/H      Color pulse ±"),
        Line::from(" l/L      Scan lines"),
        Line::from(" z/Z      Zalgo ±"),
        Line::from(" y/Y      Stability ±"),
        Line::from(" j/J      Strobe dcy ±"),
        Line::from(" u/U      Wave speed ±"),
        Line::from(Span::styled(
            " Color FX best in ASCII/Quadrant",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            " ── Camera ──────",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" </>      Zoom ±"),
        Line::from(" ,/.      Rotation ±"),
        Line::from(" ;/'      Pan X ±"),
        Line::from(" :/\"      Pan Y ±"),
        Line::from(Span::styled(
            " ── Audio ───────",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" ↑/↓      Sensitivity ±"),
        Line::from(" ←/→      Seek ±5s"),
        Line::from(" v        Spectrum"),
        Line::from(" p/P      Preset cycle"),
        Line::from(Span::styled(
            " ── Overlays ────",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(" o        Open visual"),
        Line::from(" O        Open audio"),
        Line::from(" C        Charset editor"),
        Line::from(" A        Audio mixer"),
        Line::from(" K        Creation (Esc=hide q=off)"),
        Line::from(" x        Fullscreen"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? or Esc to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help_width = 42u16;
    let help_height = help_text.len() as u16 + 2;
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
            format!("Length: {len} | OK — live preview"),
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

    let overlay_width = 44u16;
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

/// Draw the Audio Reactivity Mixer overlay.
#[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
fn draw_audio_panel_overlay(
    frame: &mut Frame,
    area: Rect,
    config: &RenderConfig,
    panel: &AudioPanelState,
    audio: Option<&AudioFeatures>,
) {
    let mut lines = Vec::new();

    // Helper: Build a mini progress bar
    let build_bar = |val: f32, max: f32, width: usize| -> String {
        let filled_chars = (val / max * width as f32).clamp(0.0, width as f32) as usize;
        let p1 = "=".repeat(filled_chars);
        let p2 = " ".repeat(width.saturating_sub(filled_chars));
        format!("[{p1}{p2}]")
    };

    let cell_style = |r: usize, c: usize| -> Style {
        if panel.selected_row == r && panel.selected_col == c {
            Style::default().fg(Color::Black).bg(Color::White)
        } else {
            Style::default().fg(Color::White)
        }
    };

    // Row 0: Sensitivity
    let r0_bg = if panel.selected_row == 0 {
        Color::Cyan
    } else {
        Color::Black
    };
    let r0_fg = if panel.selected_row == 0 {
        Color::Black
    } else {
        Color::White
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!(" Sensitivity  {:<4.1} ", config.audio_sensitivity),
            Style::default().fg(r0_fg).bg(r0_bg),
        ),
        Span::styled(
            build_bar(config.audio_sensitivity, 5.0, 15),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    // Row 1: Smoothing
    let r1_bg = if panel.selected_row == 1 {
        Color::Cyan
    } else {
        Color::Black
    };
    let r1_fg = if panel.selected_row == 1 {
        Color::Black
    } else {
        Color::White
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!(" Smoothing    {:<4.2} ", config.audio_smoothing),
            Style::default().fg(r1_fg).bg(r1_bg),
        ),
        Span::styled(
            build_bar(config.audio_smoothing, 1.0, 15),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "-- Mappings ---",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " #  ON  Source         Target         Amt   Off   Crv",
        Style::default().fg(Color::Yellow),
    )));

    for (i, m) in config.audio_mappings.iter().enumerate() {
        let r = i + 2;
        let is_row_sel = panel.selected_row == r;
        let base_fg = if is_row_sel {
            Color::Cyan
        } else if !m.enabled {
            Color::DarkGray
        } else {
            Color::White
        };

        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!(" {:<2} ", i + 1),
            Style::default().fg(base_fg),
        ));
        spans.push(Span::styled(
            if m.enabled { "[*] " } else { "[ ] " },
            cell_style(r, 0),
        ));
        spans.push(Span::styled(format!("{:<14} ", m.source), cell_style(r, 1)));
        spans.push(Span::styled(format!("{:<14} ", m.target), cell_style(r, 2)));
        spans.push(Span::styled(
            format!("{:<5.2} ", m.amount),
            cell_style(r, 3),
        ));
        spans.push(Span::styled(
            format!("{:<5.2} ", m.offset),
            cell_style(r, 4),
        ));
        let curve_str = match &m.curve {
            af_core::config::MappingCurve::Linear => "Lin",
            af_core::config::MappingCurve::Exponential => "Exp",
            af_core::config::MappingCurve::Threshold => "Thr",
            af_core::config::MappingCurve::Smooth => "Smo",
        };
        spans.push(Span::styled(format!("{curve_str:<3}"), cell_style(r, 5)));

        lines.push(Line::from(spans));
    }

    lines.push(Line::from(""));
    if let Some(features) = audio {
        lines.push(Line::from(Span::styled(
            format!(
                " Live: RMS={:.2} Bass={:.2} Flux={:.2} Onset={}",
                features.rms, features.bass, features.spectral_flux, features.onset
            ),
            Style::default().fg(Color::Green),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            " Live: [Audio inactive]",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Up/Dn=row  Lt/Rt=col  Enter=cycle  +/-=adj",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        " n=new  x=delete  Esc=close",
        Style::default().fg(Color::DarkGray),
    )));

    let overlay_width = 60u16;
    let overlay_height = lines.len() as u16 + 2;
    let x = area.x + area.width.saturating_sub(overlay_width) / 2;
    let y = area.y + area.height.saturating_sub(overlay_height) / 2;
    let overlay_area = Rect::new(x, y, overlay_width, overlay_height);

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Audio Reactivity Mixer ")
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

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{name:<12}"), Style::default().fg(name_color)),
            Span::styled(bar, Style::default().fg(bar_color)),
            Span::styled(
                format!(" {value:.1}/{max:.1}"),
                Style::default().fg(Color::White),
            ),
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
        let rms_bar = "\u{2588}".repeat((af.rms * 4.0) as usize);
        let bass_bar = "\u{2588}".repeat((af.bass * 4.0) as usize);
        let mid_bar = "\u{2588}".repeat((af.mid * 4.0) as usize);
        let high_bar = "\u{2588}".repeat((af.brilliance * 4.0) as usize);
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

    let overlay_width = 52u16;
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
