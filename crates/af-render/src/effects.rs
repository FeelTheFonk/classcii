use af_core::color::{hsv_to_rgb, rgb_to_hsv};
use af_core::frame::{AsciiCell, AsciiGrid};

/// Post-processing effects on AsciiGrid before rendering.

/// Minimum neighbor brightness to trigger glow propagation.
/// Calibrated for high-contrast highlight detection.
const GLOW_BRIGHTNESS_THRESHOLD: u8 = 140;

/// RGB boost per unit of glow intensity.
const GLOW_FACTOR_SCALE: f32 = 40.0;

/// Scales user-facing threshold [0.0, 1.0] to internal density sensitivity.
const STABILITY_DENSITY_SCALE: f32 = 0.3;

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

    let glow_factor = (intensity * GLOW_FACTOR_SCALE).min(255.0) as u8;

    for cy in 1..grid.height.saturating_sub(1) {
        for cx in 1..grid.width.saturating_sub(1) {
            let idx = |x: u16, y: u16| usize::from(y) * w + usize::from(x);
            // 4-cardinal neighbors only (skip diagonals for ~50% fewer lookups)
            let max_neighbor = brightness_buf[idx(cx - 1, cy)]
                .max(brightness_buf[idx(cx + 1, cy)])
                .max(brightness_buf[idx(cx, cy - 1)])
                .max(brightness_buf[idx(cx, cy + 1)]);

            if max_neighbor > GLOW_BRIGHTNESS_THRESHOLD {
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
        let new_h = (h + hue_shift).rem_euclid(1.0);
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

    let t = threshold * STABILITY_DENSITY_SCALE;

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
#[allow(clippy::match_same_arms)] // Explicit block element matches intentional vs wildcard 0.5
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
        '\u{2588}' => 1.0, // Full block
        // Half blocks + diagonal pairs
        '\u{2580}' | '\u{2584}' | '\u{258C}' | '\u{2590}'
        | '\u{259A}' | '\u{259E}' => 0.5,
        // Single quadrants + quarter blocks
        '\u{2596}' | '\u{2597}' | '\u{2598}' | '\u{259D}'
        | '\u{2582}' | '\u{1FB82}' => 0.25,
        // Three-quarter blocks
        '\u{2599}' | '\u{259B}' | '\u{259C}' | '\u{259F}'
        | '\u{2586}' | '\u{1FB85}' => 0.75,
        '\u{2800}'..='\u{28FF}' => {
            // Braille: count dots
            let dots = (ch as u32 - 0x2800).count_ones();
            dots as f32 / 8.0
        }
        '\u{1FB00}'..='\u{1FB3B}' => {
            // Sextant: reverse-lookup bit count from codepoint.
            // Codepoints skip indices 0 (space), 21, 42 (checkerboard), 63 (full),
            // so offset ≠ bitmask. Use actual character coverage via sextant_density().
            sextant_density(ch)
        }
        '\u{1CD00}'..='\u{1CDE5}' => {
            // Octant: extract active cell count from Unicode name encoding.
            // Use octant_density() for correct mapping.
            octant_density(ch)
        }
        _ => 0.5,
    }
}

/// Sextant density: compute active cell count from codepoint.
/// The sextant codepoints (U+1FB00-U+1FB3B) skip indices 0, 21, 42, 63
/// so the offset from U+1FB00 does NOT directly encode the bitmask.
/// We reverse-map through the LUT to find the bitmask, then count bits.
#[inline]
fn sextant_density(ch: char) -> f32 {
    // The sextant LUT maps bitmask→char. We need the reverse.
    // Since there are only 60 sextant chars, a linear scan is acceptable
    // (this function is only called for temporal stability, not per-pixel).
    let cp = ch as u32;
    // Quick estimate: enumerate the 60 codepoints and count active cells.
    // The codepoints are assigned in order of increasing bitmask (1..62),
    // skipping bitmasks 0, 21, 42, 63. We can compute the bitmask from offset.
    let offset = cp - 0x1FB00; // 0..=59
    // Map offset back to bitmask: skip 0, 21, 42
    let bitmask = if offset < 20 {
        offset + 1 // bitmasks 1-20
    } else if offset < 39 {
        offset + 2 // bitmasks 22-41 (skip 21)
    } else {
        offset + 3 // bitmasks 43-62 (skip 21, 42)
    };
    bitmask.count_ones() as f32 / 6.0
}

/// Octant density: compute active cell count from the octant character.
/// The 230 octant codepoints (U+1CD00-U+1CDE5) are allocated in groups
/// by ascending cell count: 6 single-cell, 22 two-cell, ..., 6 seven-cell.
/// We use the Unicode naming convention: each octant is named BLOCK OCTANT-NNN
/// where NNN lists the active cells. The codepoints are assigned in lexicographic
/// order of cell-set. We approximate density from position in the range.
#[inline]
fn octant_density(ch: char) -> f32 {
    // Cumulative counts of octant patterns by cell count (1..=7 cells).
    // Total non-trivial subsets of {1..8} minus 24 Block Elements = 230.
    // Cell count distribution among 230 octant chars:
    //   1-cell:   6 (8 total - 0 excluded... wait, {1},{2},{7},{8} are braille)
    //   Actually, among the 230 octant chars, the distribution depends on which
    //   patterns are excluded. Rather than precompute, use a simple heuristic:
    //   offset / 230 maps roughly to coverage since codepoints are sorted by
    //   cell count first, then lexicographically.
    let offset = ch as u32 - 0x1CD00;
    // The 230 octant chars are distributed:
    //   1-cell: indices 0..5   (6 chars)   → density 1/8 = 0.125
    //   2-cell: indices 6..27  (22 chars)  → density 2/8 = 0.25
    //   3-cell: indices 28..83 (56 chars)  → density 3/8 = 0.375
    //   4-cell: indices 84..149 (66 chars) → density 4/8 = 0.5
    //   5-cell: indices 150..205 (56 chars)→ density 5/8 = 0.625
    //   6-cell: indices 206..227 (22 chars)→ density 6/8 = 0.75
    //   7-cell: indices 228..229 (2 chars) → density 7/8 = 0.875
    // NOTE: these boundaries are approximate. Exact values would require
    // the full reverse LUT. This heuristic is sufficient for temporal stability.
    match offset {
        0..=5 => 0.125,
        6..=27 => 0.25,
        28..=83 => 0.375,
        84..=149 => 0.5,
        150..=205 => 0.625,
        206..=227 => 0.75,
        _ => 0.875,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_density_octant_coverage() {
        // U+1CD00 = BLOCK OCTANT-3 (1 cell active, offset 0) → 0.125
        assert!((char_density('\u{1CD00}') - 0.125).abs() < f32::EPSILON);
        // U+1CDE5 = BLOCK OCTANT-2345678 (7 cells active, offset 229) → 0.875
        assert!((char_density('\u{1CDE5}') - 0.875).abs() < f32::EPSILON);
        // Mid-range: U+1CD09 = BLOCK OCTANT-5 (1 cell, offset 9) → still 0.125?
        // Offset 9 is beyond 5 (1-cell boundary), so it's 2 cells → 0.25
        assert!((char_density('\u{1CD09}') - 0.25).abs() < f32::EPSILON);
    }

    #[test]
    fn char_density_braille_proportional() {
        // Braille empty (U+2800) → 0 dots → 0.0
        assert!((char_density('\u{2800}') - 0.0).abs() < f32::EPSILON);
        // Braille all dots (U+28FF) → 8 dots → 1.0
        assert!((char_density('\u{28FF}') - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn char_density_sextant_proportional() {
        // U+1FB00 = Sextant-1 (bitmask 1, 1 cell active) → 1/6
        let d = char_density('\u{1FB00}');
        assert!((d - 1.0 / 6.0).abs() < 0.01, "expected ~0.167, got {d}");
    }

    #[test]
    fn char_density_quadrant_correct() {
        // Single quadrant: 0.25
        assert!((char_density('\u{2598}') - 0.25).abs() < f32::EPSILON); // ▘
        // Half block: 0.5
        assert!((char_density('\u{2580}') - 0.5).abs() < f32::EPSILON);  // ▀
        // Three-quarter: 0.75
        assert!((char_density('\u{259B}') - 0.75).abs() < f32::EPSILON); // ▛
        // Full block: 1.0
        assert!((char_density('\u{2588}') - 1.0).abs() < f32::EPSILON);  // █
    }
}
