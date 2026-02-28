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
    pub effects: Vec<(&'a str, f32, f32)>, // (name, value, max)
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
) {
    let area = frame.area();

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
            config,
            audio,
            fps_counter,
            preset_name,
            loaded_visual,
            loaded_audio,
            state,
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
        "Compact", "Standard", "Full", "Blocks", "Minimal", "Glitch 1", "Glitch 2", "Digital",
        "Classic", "Extended",
    ];
    let charset_name = charset_names.get(config.charset_index).unwrap_or(&"Custom");

    let fps_str = format!("{:.0} FPS", fps_counter.fps());
    let ft_str = format!("{:.1}ms", fps_counter.frame_time_ms);

    // Onset indicator
    let onset_str = if audio.is_some_and(|a| a.onset) {
        "● BEAT"
    } else {
        "○"
    };

    // Typographic  closure
    let kv = |k: &str, lbl: &str, val: String| -> Line {
        Line::from(vec![
            Span::styled(format!(" {k:<5} "), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{lbl:<5}: "), Style::default().fg(Color::DarkGray)),
            Span::styled(val, Style::default().fg(Color::White)),
        ])
    };

    let mut lines = vec![
        Line::from(Span::styled(state_str, Style::default().fg(Color::Green))),
        Line::from(""),
        Line::from(Span::styled(
            "─ Render ──",
            Style::default().fg(Color::Yellow),
        )),
        kv("[Tab]", "Mode", mode_str.to_string()),
        kv("[p/P]", "Prst", preset_name.unwrap_or("Custom").to_string()),
        kv("[1-0]", "Char", charset_name.to_string()),
        kv("[d/D]", "Dens", format!("{:.2}", config.density_scale)),
        kv(
            "[c]",
            "Colr",
            (if config.color_enabled { "ON" } else { "OFF" }).to_string(),
        ),
        kv("[m]", "CMod", color_mode_str.to_string()),
        kv(
            "[i]",
            "Inv",
            (if config.invert { "ON" } else { "OFF" }).to_string(),
        ),
        kv("[/]", "Cont", format!("{:.1}", config.contrast)),
        kv("{ }", "Brgt", format!("{:.2}", config.brightness)),
        kv("-/+", "Sat", format!("{:.1}", config.saturation)),
        kv("[e]", "Edge", format!("{:.1}", config.edge_threshold)),
        kv("[E]", "EMix", format!("{:.2}", config.edge_mix)),
        kv(
            "[s]",
            "Shap",
            (if config.shape_matching { "ON" } else { "OFF" }).to_string(),
        ),
        kv("[a]", "Aspc", format!("{:.1}", config.aspect_ratio)),
        kv("[b]", "BG", bg_str.to_string()),
        kv(
            "[n]",
            "Dthr",
            match config.dither_mode {
                af_core::config::DitherMode::Bayer8x8 => "Bayer8",
                af_core::config::DitherMode::BlueNoise16 => "BNoise",
                af_core::config::DitherMode::None => "OFF",
            }
            .to_string(),
        ),
        kv("[f/F]", "Fade", format!("{:.1}", config.fade_decay)),
        kv("[g/G]", "Glow", format!("{:.1}", config.glow_intensity)),
        kv(
            "[t/T]",
            "Strob",
            format!("{:.1}", config.beat_flash_intensity),
        ),
        kv("[r/R]", "Chrom", format!("{:.1}", config.chromatic_offset)),
        kv("[w/W]", "Wave", format!("{:.2}", config.wave_amplitude)),
        kv("[h/H]", "Pulse", format!("{:.1}", config.color_pulse_speed)),
        kv(
            "[l/L]",
            "Scan",
            if config.scanline_gap == 0 {
                "OFF".to_string()
            } else {
                format!("{}", config.scanline_gap)
            },
        ),
        Line::from(""),
        Line::from(Span::styled(
            "─ Audio ───",
            Style::default().fg(Color::Yellow),
        )),
        kv("↑/↓", "Sens", format!("{:.1}", config.audio_sensitivity)),
        kv("", "Smth", format!("{:.2}", config.audio_smoothing)),
    ];

    if let Some(features) = audio {
        lines.push(Line::from(format!(" RMS: {:.2}", features.rms)));
        lines.push(Line::from(format!(" BPM: {:.0}", features.bpm)));
        lines.push(Line::from(Span::styled(
            format!(" {onset_str}"),
            Style::default().fg(if features.onset {
                Color::Red
            } else {
                Color::DarkGray
            }),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "─ Info ────",
        Style::default().fg(Color::Yellow),
    )));
    lines.push(Line::from(format!(" {fps_str}")));
    lines.push(Line::from(format!(" {ft_str}")));
    // Fichiers chargés
    let truncate = |name: &str, max: usize| -> String {
        if name.len() > max {
            format!("..{}", &name[name.len().saturating_sub(max - 2)..])
        } else {
            name.to_string()
        }
    };
    if let Some(name) = loaded_visual {
        lines.push(Line::from(vec![
            Span::styled(" V ", Style::default().fg(Color::DarkGray)),
            Span::styled(truncate(name, 18), Style::default().fg(Color::Cyan)),
        ]));
    }
    if let Some(name) = loaded_audio {
        lines.push(Line::from(vec![
            Span::styled(" A ", Style::default().fg(Color::DarkGray)),
            Span::styled(truncate(name, 18), Style::default().fg(Color::Magenta)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " o/O=open C=char A=mix",
        Style::default().fg(Color::DarkGray),
    )));

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
        Line::from(" q/Esc    Quit"),
        Line::from(" Space    Play/Pause"),
        Line::from(" Tab      Cycle mode"),
        Line::from(" 1-0      Select charset"),
        Line::from(" d/D      Density ±"),
        Line::from(" i        Toggle invert"),
        Line::from(" c        Toggle color"),
        Line::from(" m        Cycle color mode"),
        Line::from(" b        Cycle BG style"),
        Line::from(" [ ]      Contrast ±"),
        Line::from(" { }      Brightness ±"),
        Line::from(" -/+      Saturation ±"),
        Line::from(" f/F      Fade decay ±"),
        Line::from(" g/G      Glow intens ±"),
        Line::from(" t/T      Strobe intens ±"),
        Line::from(" r/R      Chromatic ±"),
        Line::from(" w/W      Wave amplit ±"),
        Line::from(" h/H      Color pulse ±"),
        Line::from(" l/L      Scan lines"),
        Line::from(" a        Cycle aspect"),
        Line::from(" e/E      Edge toggl/mix"),
        Line::from(" s        Toggle shapes"),
        Line::from(" ↑/↓      Audio sensitivity"),
        Line::from(" ←/→      Seek ±5s"),
        Line::from(" v        Toggle spectrum"),
        Line::from(" p/P      Cycle preset"),
        Line::from(" o        Open visual"),
        Line::from(" O        Open audio"),
        Line::from(" C        Edit charset"),
        Line::from(" A        Audio mixer"),
        Line::from(" K        Creation mode"),
        Line::from(" n        Cycle dither mode"),
        Line::from(" x        Toggle fullscreen"),
        Line::from(" ?        Toggle help"),
        Line::from(""),
        Line::from(Span::styled(
            " Press ? or Esc to close ",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help_width = 38u16;
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

    // Header
    let auto_str = if creation.auto_mode { "ON" } else { "OFF" };
    let auto_color = if creation.auto_mode {
        Color::Green
    } else {
        Color::Red
    };
    let bar_len = (creation.master_intensity / 2.0 * 10.0) as usize;
    let master_bar: String =
        "\u{2588}".repeat(bar_len) + &"\u{2591}".repeat(10_usize.saturating_sub(bar_len));

    lines.push(Line::from(vec![
        Span::styled("  [a] AUTO: ", Style::default().fg(Color::Gray)),
        Span::styled(auto_str, Style::default().fg(auto_color)),
        Span::styled("    Master: ", Style::default().fg(Color::Gray)),
        Span::styled(master_bar, Style::default().fg(Color::Cyan)),
        Span::styled(
            format!(" [{:.1}]", creation.master_intensity),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  [p] Preset: ", Style::default().fg(Color::Gray)),
        Span::styled(creation.preset_name, Style::default().fg(Color::Yellow)),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  \u{2500}\u{2500} Effects \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
        Style::default().fg(Color::DarkGray),
    )));

    // Effects list
    for (i, (name, value, max)) in creation.effects.iter().enumerate() {
        let selected = i == creation.selected_effect;
        let prefix = if selected { " \u{25b8} " } else { "   " };
        let bar_len = if *max > 0.0 {
            (value / max * 7.0) as usize
        } else {
            0
        };
        let bar: String =
            "\u{2588}".repeat(bar_len) + &"\u{2591}".repeat(7_usize.saturating_sub(bar_len));
        let name_color = if selected { Color::White } else { Color::Gray };

        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(format!("{name:<12}"), Style::default().fg(name_color)),
            Span::styled(bar, Style::default().fg(Color::Green)),
            Span::styled(format!("  [{value:.1}]"), Style::default().fg(Color::White)),
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
        "  [\u{2191}\u{2193}] Select  [\u{2190}\u{2192}] Master  [a] Auto  [p] Preset  [Esc] Close",
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
