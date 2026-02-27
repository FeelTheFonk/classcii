use af_core::config::{BgStyle, ColorMode, RenderConfig};
use af_core::frame::{AsciiGrid, AudioFeatures};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};

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
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    grid: &AsciiGrid,
    config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    preset_name: Option<&str>,
    _sidebar_dirty: bool,
    state: &RenderState,
) {
    let area = frame.area();

    // Si le mode plein écran exclusif est activé, ne rendre que le canevas ASCII
    if config.fullscreen {
        canvas::render_grid(frame.buffer_mut(), area, grid);
    } else {
        // === Sidebar & Layout setup ===
        let sidebar_width = 24u16; // Slight expansion for keybind hints
        let h_chunks =
            Layout::horizontal([Constraint::Min(40), Constraint::Length(sidebar_width)])
                .split(area);

        // === Canvas & Spectrum splits ===
        let canvas_area = if config.show_spectrum {
            let v_chunks = Layout::vertical([Constraint::Min(10), Constraint::Length(3)]).split(h_chunks[0]);
            let spectrum_area = v_chunks[1];
            draw_spectrum(frame, spectrum_area, audio);
            v_chunks[0]
        } else {
            h_chunks[0]
        };

        canvas::render_grid(frame.buffer_mut(), canvas_area, grid);

        // === Sidebar ===
        let sidebar_area = h_chunks[1];
        draw_sidebar(frame, sidebar_area, config, audio, fps_counter, preset_name, state);
    }

    // === Help overlay ===
    // Force draw help even in fullscreen if toggled
    if *state == RenderState::Help {
        draw_help_overlay(frame, area);
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
fn draw_sidebar(
    frame: &mut Frame,
    area: Rect,
    config: &RenderConfig,
    audio: Option<&AudioFeatures>,
    fps_counter: &FpsCounter,
    preset_name: Option<&str>,
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

    // Typographic SOTA closure
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
        kv("[c]", "Colr", (if config.color_enabled { "ON" } else { "OFF" }).to_string()),
        kv("[m]", "CMod", color_mode_str.to_string()),
        kv("[i]", "Inv", (if config.invert { "ON" } else { "OFF" }).to_string()),
        kv("[/]", "Cont", format!("{:.1}", config.contrast)),
        kv("{ }", "Brgt", format!("{:.2}", config.brightness)),
        kv("-/+", "Sat", format!("{:.1}", config.saturation)),
        kv("[e]", "Edge", format!("{:.1}", config.edge_threshold)),
        kv("[E]", "EMix", format!("{:.2}", config.edge_mix)),
        kv("[s]", "Shap", (if config.shape_matching { "ON" } else { "OFF" }).to_string()),
        kv("[a]", "Aspc", format!("{:.1}", config.aspect_ratio)),
        kv("[b]", "BG", bg_str.to_string()),
        kv("[f/F]", "Fade", format!("{:.1}", config.fade_decay)),
        kv("[g/G]", "Glow", format!("{:.1}", config.glow_intensity)),
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
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " ? = help",
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
        Line::from(" 1-5      Select charset"),
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
        Line::from(" a        Cycle aspect"),
        Line::from(" e/E      Edge toggl/mix"),
        Line::from(" s        Toggle shapes"),
        Line::from(" ↑/↓      Audio sensitivity"),
        Line::from(" ←/→      Seek ±5s"),
        Line::from(" v        Toggle spectrum"),
        Line::from(" p/P      Cycle preset"),
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
