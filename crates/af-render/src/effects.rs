use af_core::color::{hsv_to_rgb, rgb_to_hsv};
use af_core::frame::{AsciiCell, AsciiGrid};

/// Post-processing effects on AsciiGrid before rendering.

/// Apply strobe: boost fg brightness proportional to onset envelope.
///
/// Replaces the old `apply_beat_flash` with a continuous envelope-driven effect.
/// `envelope` [0.0, 1.0] — onset envelope (1.0 on beat, decays via strobe_decay).
/// `intensity` [0.0, 2.0] — strength multiplier.
pub fn apply_strobe(grid: &mut AsciiGrid, envelope: f32, intensity: f32) {
    if envelope < 0.001 || intensity < 0.001 {
        return;
    }

    let boost = (envelope * intensity * 128.0).min(255.0) as u8;
    if boost == 0 {
        return;
    }

    for cell in &mut grid.cells {
        cell.fg.0 = cell.fg.0.saturating_add(boost);
        cell.fg.1 = cell.fg.1.saturating_add(boost);
        cell.fg.2 = cell.fg.2.saturating_add(boost);
    }
}

/// Apply fade trails: blend current grid with previous grid.
///
/// `decay` [0.0, 1.0]: 0 = no trail, 0.99 = near-full persistence.
pub fn apply_fade_trails(current: &mut AsciiGrid, previous: &AsciiGrid, decay: f32) {
    if decay < 0.01 || current.width != previous.width || current.height != previous.height {
        return;
    }

    let d = decay.clamp(0.0, 0.99);
    let keep = 1.0 - d;

    for (cur, prev) in current.cells.iter_mut().zip(previous.cells.iter()) {
        if cur.ch == ' ' && prev.ch != ' ' {
            cur.ch = prev.ch;
            cur.fg = (
                (f32::from(prev.fg.0) * d) as u8,
                (f32::from(prev.fg.1) * d) as u8,
                (f32::from(prev.fg.2) * d) as u8,
            );
        } else if cur.ch != ' ' {
            cur.fg = (
                (f32::from(cur.fg.0) * keep + f32::from(prev.fg.0) * d) as u8,
                (f32::from(cur.fg.1) * keep + f32::from(prev.fg.1) * d) as u8,
                (f32::from(cur.fg.2) * keep + f32::from(prev.fg.2) * d) as u8,
            );
        }
    }
}

/// Apply glow: brighten fg of cells adjacent to bright cells.
///
/// `brightness_buf` must be a pre-allocated buffer of at least `width * height` elements.
/// The caller is responsible for ensuring correct size; this function will resize if needed.
pub fn apply_glow(grid: &mut AsciiGrid, intensity: f32, brightness_buf: &mut Vec<u8>) {
    if intensity < 0.01 {
        return;
    }

    let w = usize::from(grid.width);
    let h = usize::from(grid.height);
    let needed = w * h;

    // Resize only if dimensions changed (rare — terminal resize only)
    brightness_buf.resize(needed, 0);

    // Read-only pass: fill brightness map from flat cells array
    for (i, cell) in grid.cells.iter().enumerate() {
        brightness_buf[i] = cell.fg.0.max(cell.fg.1).max(cell.fg.2);
    }

    let glow_factor = (intensity * 40.0).min(255.0) as u8;

    for cy in 1..grid.height.saturating_sub(1) {
        for cx in 1..grid.width.saturating_sub(1) {
            let idx = |x: u16, y: u16| usize::from(y) * w + usize::from(x);
            // 4-cardinal neighbors only (skip diagonals for ~50% fewer lookups)
            let max_neighbor = brightness_buf[idx(cx - 1, cy)]
                .max(brightness_buf[idx(cx + 1, cy)])
                .max(brightness_buf[idx(cx, cy - 1)])
                .max(brightness_buf[idx(cx, cy + 1)]);

            if max_neighbor > 140 {
                let cell = &mut grid.cells[idx(cx, cy)];
                cell.fg.0 = cell.fg.0.saturating_add(glow_factor);
                cell.fg.1 = cell.fg.1.saturating_add(glow_factor);
                cell.fg.2 = cell.fg.2.saturating_add(glow_factor);
            }
        }
    }
}

/// Apply chromatic aberration: shift R channel left, B channel right.
///
/// `offset` [0.0, 5.0] — pixel offset for R/B channels.
/// `fg_buf` — pre-allocated buffer, resized internally if needed.
pub fn apply_chromatic_aberration(
    grid: &mut AsciiGrid,
    offset: f32,
    fg_buf: &mut Vec<(u8, u8, u8)>,
) {
    if offset < 0.01 {
        return;
    }

    let w = usize::from(grid.width);
    let h = usize::from(grid.height);
    let needed = w * h;

    fg_buf.resize(needed, (0, 0, 0));

    // Read pass: copy all fg colors from flat array
    for (i, cell) in grid.cells.iter().enumerate() {
        fg_buf[i] = cell.fg;
    }

    let shift = offset.ceil() as i32;
    #[allow(clippy::cast_possible_wrap)] // w,x derived from u16, always fits i32
    let w_i32 = w as i32;

    // Write pass: shift R left, B right, G stays centered
    for y in 0..h {
        #[allow(clippy::cast_possible_wrap)]
        for x in 0..w {
            let xi = x as i32;
            let r_x = (xi - shift).clamp(0, w_i32 - 1) as usize;
            let b_x = (xi + shift).clamp(0, w_i32 - 1) as usize;

            let r = fg_buf[y * w + r_x].0;
            let g = fg_buf[y * w + x].1;
            let b = fg_buf[y * w + b_x].2;

            grid.cells[y * w + x].fg = (r, g, b);
        }
    }
}

/// Apply wave distortion: horizontally shift rows with a smooth sinusoidal pattern.
///
/// `amplitude` [0.0, 1.0] — wave strength (max shift = amplitude * 8 cells).
/// `speed` — spatial frequency multiplier (waves per grid height).
/// `phase` — temporal phase offset (driven by persistent wave_phase + audio beat_phase).
/// `row_buf` — pre-allocated buffer, resized internally if needed.
pub fn apply_wave_distortion(
    grid: &mut AsciiGrid,
    amplitude: f32,
    speed: f32,
    phase: f32,
    row_buf: &mut Vec<AsciiCell>,
) {
    // Cap max shift to 8 cells (not grid width) for smooth, non-jarring motion
    const MAX_WAVE_SHIFT: f32 = 8.0;

    if amplitude < 0.001 {
        return;
    }

    let w = usize::from(grid.width);
    let h = grid.height;
    let hf = f32::from(h);

    row_buf.resize(w, AsciiCell::default());

    for y in 0..h {
        let yf = f32::from(y);
        let shift = (amplitude
            * MAX_WAVE_SHIFT
            * (std::f32::consts::TAU * speed * yf / hf + phase).sin()) as i16;

        // Copy row to buffer
        let row_start = usize::from(y) * w;
        row_buf[..w].copy_from_slice(&grid.cells[row_start..row_start + w]);

        // Write shifted with wrapping (no blank gaps)
        #[allow(clippy::cast_possible_wrap)]
        let w_i32 = w as i32;
        for x in 0..w {
            #[allow(clippy::cast_possible_wrap)]
            let src_x = ((x as i32 - i32::from(shift)) % w_i32 + w_i32) % w_i32;
            grid.cells[row_start + x] = row_buf[src_x as usize];
        }
    }
}

/// Apply color pulse: rotate hue of all fg colors.
///
/// `hue_shift` [0.0, 1.0) — amount to rotate (wraps).
pub fn apply_color_pulse(grid: &mut AsciiGrid, hue_shift: f32) {
    if hue_shift.abs() < 0.001 {
        return;
    }

    for cell in &mut grid.cells {
        if cell.ch == ' ' {
            continue;
        }
        // Skip black cells — no hue to rotate, saves HSV conversion
        if cell.fg.0 == 0 && cell.fg.1 == 0 && cell.fg.2 == 0 {
            continue;
        }

        let (h, s, v) = rgb_to_hsv(cell.fg.0, cell.fg.1, cell.fg.2);
        let new_h = (h + hue_shift) % 1.0;
        cell.fg = hsv_to_rgb(new_h, s, v);
    }
}

/// Apply temporal stability: suppress minor character flickering between frames.
///
/// Compares current and previous grid. If a character change is "minor"
/// (both chars have similar visual density), keep the previous character
/// to reduce perceived flicker.
///
/// `threshold` [0.0, 1.0] — 0 = off, higher = more aggressive stabilization.
pub fn apply_temporal_stability(current: &mut AsciiGrid, previous: &AsciiGrid, threshold: f32) {
    if threshold < 0.001 || current.width != previous.width || current.height != previous.height {
        return;
    }

    let t = threshold * 0.3;

    for (cur, prev) in current.cells.iter_mut().zip(previous.cells.iter()) {
        if cur.ch == ' ' || prev.ch == ' ' {
            continue;
        }

        let cur_density = char_density(cur.ch);
        let prev_density = char_density(prev.ch);

        if (cur_density - prev_density).abs() < t {
            cur.ch = prev.ch;
        }
    }
}

/// Estimate character visual density [0.0, 1.0].
/// Uses Unicode block coverage heuristic.
#[inline]
fn char_density(ch: char) -> f32 {
    match ch {
        ' ' => 0.0,
        '.' | ',' | '\'' | '`' | ':' => 0.1,
        '-' | '_' | '~' => 0.15,
        ';' | '!' | '|' | '/' | '\\' => 0.2,
        '+' | '*' | '^' | '"' | 'i' | 'l' | 't' | 'r' | 'c' => 0.3,
        '=' | '(' | ')' | '{' | '}' | '[' | ']' => 0.35,
        'v' | 'x' | 'z' | 'n' | 'u' | 'o' | 'a' | 'e' | 's' => 0.4,
        'A'..='Z' => 0.55,
        '#' | '@' | '%' | '&' | '$' => 0.7,
        '\u{2588}' => 1.0,               // Full block
        '\u{2596}'..='\u{259F}' => 0.25, // Quadrants
        '\u{2800}'..='\u{28FF}' => {
            // Braille: count dots
            let dots = (ch as u32 - 0x2800).count_ones();
            dots as f32 / 8.0
        }
        '\u{1FB00}'..='\u{1FB3B}' => {
            // Sextant: coverage from LUT index bit count
            let idx = ch as u32 - 0x1FB00;
            idx.count_ones() as f32 / 6.0
        }
        _ => 0.5,
    }
}

/// Apply scan lines: darken every Nth row.
///
/// `gap` — line spacing (0 = disabled, 2-8 typical).
/// `darken_factor` — brightness multiplier for affected lines [0.0, 1.0].
pub fn apply_scan_lines(grid: &mut AsciiGrid, gap: u8, darken_factor: f32) {
    if gap == 0 {
        return;
    }

    let factor = darken_factor.clamp(0.0, 1.0);
    let w = usize::from(grid.width);

    for (cy, row) in grid.cells.chunks_mut(w).enumerate() {
        if cy % usize::from(gap) != 0 {
            continue;
        }
        for cell in row {
            cell.fg.0 = (f32::from(cell.fg.0) * factor) as u8;
            cell.fg.1 = (f32::from(cell.fg.1) * factor) as u8;
            cell.fg.2 = (f32::from(cell.fg.2) * factor) as u8;
        }
    }
}
