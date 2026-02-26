use af_core::frame::{AsciiGrid, AsciiCell, AudioFeatures};

/// Post-processing effects on AsciiGrid before rendering.

/// Apply beat flash: on onset, boost all foreground brightness.
pub fn apply_beat_flash(grid: &mut AsciiGrid, features: &AudioFeatures) {
    if !features.onset {
        return;
    }

    let boost = (features.beat_intensity * 80.0) as u8;
    if boost == 0 {
        return;
    }

    for cy in 0..grid.height {
        for cx in 0..grid.width {
            let cell = grid.get(cx, cy);
            let fg = (
                cell.fg.0.saturating_add(boost),
                cell.fg.1.saturating_add(boost),
                cell.fg.2.saturating_add(boost),
            );
            grid.set(cx, cy, AsciiCell { ch: cell.ch, fg, bg: cell.bg });
        }
    }
}

/// Apply fade trails: blend current grid with previous grid.
///
/// `decay` [0.0, 1.0]: 0 = no trail, 1 = full persistence.
pub fn apply_fade_trails(
    current: &mut AsciiGrid,
    previous: &AsciiGrid,
    decay: f32,
) {
    if decay < 0.01 || current.width != previous.width || current.height != previous.height {
        return;
    }

    let d = decay.clamp(0.0, 0.95);
    let keep = 1.0 - d;

    for cy in 0..current.height {
        for cx in 0..current.width {
            let cur = current.get(cx, cy);
            let prev = previous.get(cx, cy);

            // If current cell is blank but previous wasn't, blend
            if cur.ch == ' ' && prev.ch != ' ' {
                let fg = (
                    (f32::from(prev.fg.0) * d) as u8,
                    (f32::from(prev.fg.1) * d) as u8,
                    (f32::from(prev.fg.2) * d) as u8,
                );
                current.set(cx, cy, AsciiCell { ch: prev.ch, fg, bg: cur.bg });
            } else if cur.ch != ' ' {
                // Blend current with echo of previous
                let fg = (
                    (f32::from(cur.fg.0) * keep + f32::from(prev.fg.0) * d) as u8,
                    (f32::from(cur.fg.1) * keep + f32::from(prev.fg.1) * d) as u8,
                    (f32::from(cur.fg.2) * keep + f32::from(prev.fg.2) * d) as u8,
                );
                current.set(cx, cy, AsciiCell { ch: cur.ch, fg, bg: cur.bg });
            }
        }
    }
}

/// Apply glow: brighten fg of cells adjacent to bright cells.
pub fn apply_glow(grid: &mut AsciiGrid, intensity: f32) {
    if intensity < 0.01 {
        return;
    }

    let w = grid.width;
    let h = grid.height;

    // Collect brightness map (read-only pass)
    let mut brightness: Vec<u8> = Vec::with_capacity(usize::from(w) * usize::from(h));
    for y in 0..h {
        for x in 0..w {
            let c = grid.get(x, y);
            brightness.push(c.fg.0.max(c.fg.1).max(c.fg.2));
        }
    }

    let glow_factor = (intensity * 40.0) as u8;

    for cy in 1..h.saturating_sub(1) {
        for cx in 1..w.saturating_sub(1) {
            // Check if any neighbor is bright
            let idx = |x: u16, y: u16| (y * w + x) as usize;
            let max_neighbor = brightness[idx(cx - 1, cy)]
                .max(brightness[idx(cx + 1, cy)])
                .max(brightness[idx(cx, cy - 1)])
                .max(brightness[idx(cx, cy + 1)]);

            if max_neighbor > 200 {
                let cell = grid.get(cx, cy);
                let fg = (
                    cell.fg.0.saturating_add(glow_factor),
                    cell.fg.1.saturating_add(glow_factor),
                    cell.fg.2.saturating_add(glow_factor),
                );
                grid.set(cx, cy, AsciiCell { ch: cell.ch, fg, bg: cell.bg });
            }
        }
    }
}
