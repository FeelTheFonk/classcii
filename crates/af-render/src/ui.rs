use af_core::config::{BgStyle, ColorMode, RenderConfig};
use af_core::frame::{AsciiGrid, AudioFeatures};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};
use ratatui::Frame;

use crate::canvas;
use crate::fps::FpsCounter;

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
    /// Quitting (should not reach draw).
    Quitting,
}

/// Draw the full UI: canvas + sidebar + spectrum bar.
pub fn draw(
    frame: &mut Frame,
    grid: &AsciiGrid,
    config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    _sidebar_dirty: bool,
    state: &RenderState,
) {
    let area = frame.area();

    // Horizontal split: [canvas | sidebar(20)]
    let sidebar_width = 20u16;
    let h_chunks = Layout::horizontal([
        Constraint::Min(40),
        Constraint::Length(sidebar_width),
    ])
    .split(area);

    // Vertical split of left panel: [canvas | spectrum(3)]
    let v_chunks = Layout::vertical([
        Constraint::Min(10),
        Constraint::Length(3),
    ])
    .split(h_chunks[0]);

    // === Canvas ===
    let canvas_area = v_chunks[0];
    canvas::render_grid(frame.buffer_mut(), canvas_area, grid);

    // === Spectrum bar ===
    let spectrum_area = v_chunks[1];
    draw_spectrum(frame, spectrum_area, audio);

    // === Sidebar ===
    let sidebar_area = h_chunks[1];
    draw_sidebar(frame, sidebar_area, config, audio, fps_counter, state);

    // === Help overlay ===
    if *state == RenderState::Help {
        draw_help_overlay(frame, area);
    }
}

/// Draw the spectrum sparkline bar with intensity coloring.
fn draw_spectrum(frame: &mut Frame, area: Rect, audio: Option<&AudioFeatures>) {
    let data: Vec<u64> = if let Some(features) = audio {
        features
            .spectrum_bands
            .iter()
            .map(|&v| (v * 64.0).clamp(0.0, 64.0) as u64)
            .collect()
    } else {
        vec![0u64; 32]
    };

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
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title(" Spectrum "),
        )
        .data(&data)
        .style(Style::default().fg(bar_color));

    frame.render_widget(sparkline, area);
}

/// Draw the parameter sidebar with all live values.
fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    state: &RenderState,
) {
    let mode_str = match config.render_mode {
        af_core::config::RenderMode::Ascii => "ASCII",
        af_core::config::RenderMode::Braille => "Braille",
        af_core::config::RenderMode::HalfBlock => "HalfBlk",
        af_core::config::RenderMode::Quadrant => "Quadrnt",
    };

    let color_mode_str = match config.color_mode {
        ColorMode::Direct => "Direct",
        ColorMode::HsvBright => "HSV",
        ColorMode::Quantized => "Quant",
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
        RenderState::Quitting => "⏹ QUIT",
    };

    let charset_names = ["Compact", "Standard", "Full", "Blocks", "Minimal"];
    let charset_name = charset_names
        .get(config.charset_index)
        .unwrap_or(&"Custom");

    let fps_str = format!("{:.0} FPS", fps_counter.fps());
    let ft_str = format!("{:.1}ms", fps_counter.frame_time_ms);

    // Onset indicator
    let onset_str = if audio.is_some_and(|a| a.onset) {
        "● BEAT"
    } else {
        "○"
    };

    let mut lines = vec![
        Line::from(Span::styled(state_str, Style::default().fg(Color::Green))),
        Line::from(""),
        Line::from(Span::styled("─ Render ──", Style::default().fg(Color::Yellow))),
        Line::from(format!(" Mode: {mode_str}")),
        Line::from(format!(" Chars: {charset_name}")),
        Line::from(format!(" Density: {:.2}", config.density_scale)),
        Line::from(format!(" Color: {}", if config.color_enabled { "ON" } else { "OFF" })),
        Line::from(format!(" CMode: {color_mode_str}")),
        Line::from(format!(" Invert: {}", if config.invert { "ON" } else { "OFF" })),
        Line::from(format!(" Contr: {:.1}", config.contrast)),
        Line::from(format!(" Bright: {:.2}", config.brightness)),
        Line::from(format!(" Satur: {:.1}", config.saturation)),
        Line::from(format!(" Edges: {:.1}", config.edge_threshold)),
        Line::from(format!(" Shape: {}", if config.shape_matching { "ON" } else { "OFF" })),
        Line::from(format!(" Aspect: {:.1}", config.aspect_ratio)),
        Line::from(format!(" BG: {bg_str}")),
        Line::from(format!(" Fade: {:.1}", config.fade_decay)),
        Line::from(format!(" Glow: {:.1}", config.glow_intensity)),
        Line::from(""),
        Line::from(Span::styled("─ Audio ───", Style::default().fg(Color::Yellow))),
        Line::from(format!(" Sens: {:.1}", config.audio_sensitivity)),
        Line::from(format!(" Smooth: {:.2}", config.audio_smoothing)),
    ];

    if let Some(features) = audio {
        lines.push(Line::from(format!(" RMS: {:.2}", features.rms)));
        lines.push(Line::from(format!(" BPM: {:.0}", features.bpm)));
        lines.push(Line::from(Span::styled(
            format!(" {onset_str}"),
            Style::default().fg(if features.onset { Color::Red } else { Color::DarkGray }),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("─ Info ────", Style::default().fg(Color::Yellow))));
    lines.push(Line::from(format!(" {fps_str}")));
    lines.push(Line::from(format!(" {ft_str}")));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" ? = help", Style::default().fg(Color::DarkGray))));

    let sidebar = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(" Params "),
    );

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
        Line::from(" 1-5      Select charset"),
        Line::from(" d/D      Density ±"),
        Line::from(" i        Toggle invert"),
        Line::from(" c        Toggle color"),
        Line::from(" m        Cycle color mode"),
        Line::from(" b        Cycle BG style"),
        Line::from(" [/]      Contrast ±"),
        Line::from(" {/}      Brightness ±"),
        Line::from(" -/+      Saturation ±"),
        Line::from(" f/F      Fade decay ±"),
        Line::from(" g/G      Glow intens ±"),
        Line::from(" a        Cycle aspect"),
        Line::from(" e        Toggle edges"),
        Line::from(" s        Toggle shapes"),
        Line::from(" ↑/↓      Audio sensitivity"),
        Line::from(" ←/→      Audio smoothing"),
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
