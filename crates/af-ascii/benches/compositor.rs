#![allow(clippy::field_reassign_with_default, clippy::needless_borrow)]
use criterion::{Criterion, black_box, criterion_group, criterion_main};

use af_ascii::compositor::Compositor;
use af_core::config::{RenderConfig, RenderMode};
use af_core::frame::{AsciiGrid, FrameBuffer};

fn gradient_frame(w: u32, h: u32) -> FrameBuffer {
    let mut fb = FrameBuffer::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let lum = ((x + y) * 255 / (w + h).max(1)) as u8;
            let idx = ((y * w + x) * 4) as usize;
            fb.data[idx] = lum;
            fb.data[idx + 1] = lum;
            fb.data[idx + 2] = lum;
            fb.data[idx + 3] = 255;
        }
    }
    fb
}

fn bench_compositor(c: &mut Criterion) {
    let frame = gradient_frame(640, 480);
    let mut grid = AsciiGrid::new(80, 24);

    let mut group = c.benchmark_group("compositor");

    // Ascii mode
    {
        let mut config = RenderConfig::default();
        config.render_mode = RenderMode::Ascii;
        let mut comp = Compositor::new(&config.charset);
        group.bench_function("ascii_80x24", |b| {
            b.iter(|| {
                comp.process(black_box(&frame), None, black_box(&config), &mut grid);
            });
        });
    }

    // Octant mode
    {
        let mut config = RenderConfig::default();
        config.render_mode = RenderMode::Octant;
        let mut comp = Compositor::new(&config.charset);
        group.bench_function("octant_80x24", |b| {
            b.iter(|| {
                comp.process(black_box(&frame), None, black_box(&config), &mut grid);
            });
        });
    }

    // Braille mode
    {
        let mut config = RenderConfig::default();
        config.render_mode = RenderMode::Braille;
        let mut comp = Compositor::new(&config.charset);
        group.bench_function("braille_80x24", |b| {
            b.iter(|| {
                comp.process(black_box(&frame), None, black_box(&config), &mut grid);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_compositor);
criterion_main!(benches);
