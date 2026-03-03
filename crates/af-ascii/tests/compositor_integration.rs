//! Integration test: compositor rendering pipeline.
//! Verifies: FrameBuffer → Compositor::process → AsciiGrid with valid characters.
#![allow(clippy::expect_used, clippy::field_reassign_with_default, clippy::needless_borrow)]

use af_ascii::compositor::Compositor;
use af_core::config::{RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, FrameBuffer};

/// Create a test frame with a gradient pattern.
fn gradient_frame(width: u32, height: u32) -> FrameBuffer {
    let mut fb = FrameBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let lum = ((x + y) * 255 / (width + height).max(1)) as u8;
            let idx = ((y * width + x) * 4) as usize;
            fb.data[idx] = lum;     // R
            fb.data[idx + 1] = lum; // G
            fb.data[idx + 2] = lum; // B
            fb.data[idx + 3] = 255; // A
        }
    }
    fb
}

#[test]
fn ascii_mode_produces_charset_chars() {
    let frame = gradient_frame(160, 48);
    let mut grid = AsciiGrid::new(80, 24);
    let mut config = RenderConfig::default();
    config.render_mode = RenderMode::Ascii;

    let mut comp = Compositor::new(&config.charset);
    comp.process(&frame, None, &config, &mut grid);

    // Grid should have non-space characters (gradient produces varied luminance)
    let non_space = grid.cells.iter().filter(|c| c.ch != ' ').count();
    assert!(
        non_space > 0,
        "ASCII mode should produce non-space characters on gradient"
    );
}

#[test]
fn octant_mode_produces_unicode_chars() {
    let frame = gradient_frame(160, 96);
    let mut grid = AsciiGrid::new(80, 24);
    let mut config = RenderConfig::default();
    config.render_mode = RenderMode::Octant;

    let mut comp = Compositor::new(&config.charset);
    comp.process(&frame, None, &config, &mut grid);

    // Check that octant chars are in expected ranges
    let octant_count = grid
        .cells
        .iter()
        .filter(|c| {
            let cp = c.ch as u32;
            // Real octant, block element, braille, or space/full
            (0x1CD00..=0x1CDE5).contains(&cp)
                || (0x2580..=0x259F).contains(&cp)
                || (0x2800..=0x28FF).contains(&cp)
                || c.ch == ' '
                || c.ch == '\u{2588}'
                || cp == 0x1FB82
                || cp == 0x1FB85
                || cp == 0x2582
                || cp == 0x2586
        })
        .count();
    assert_eq!(
        octant_count,
        grid.cells.len(),
        "all cells should contain valid octant/block/braille chars"
    );
}

#[test]
fn braille_mode_produces_braille_chars() {
    let frame = gradient_frame(160, 96);
    let mut grid = AsciiGrid::new(80, 24);
    let mut config = RenderConfig::default();
    config.render_mode = RenderMode::Braille;

    let mut comp = Compositor::new(&config.charset);
    comp.process(&frame, None, &config, &mut grid);

    let braille_count = grid
        .cells
        .iter()
        .filter(|c| {
            let cp = c.ch as u32;
            (0x2800..=0x28FF).contains(&cp)
        })
        .count();
    assert!(
        braille_count > grid.cells.len() / 2,
        "braille mode should produce mostly braille chars, got {}/{}",
        braille_count,
        grid.cells.len()
    );
}

#[test]
fn empty_frame_does_not_panic() {
    let frame = FrameBuffer::new(1, 1);
    let mut grid = AsciiGrid::new(80, 24);
    let config = RenderConfig::default();
    let mut comp = Compositor::new(&config.charset);

    // Should not panic on minimal frame
    comp.process(&frame, None, &config, &mut grid);
}
