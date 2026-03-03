#![allow(clippy::field_reassign_with_default, clippy::needless_borrow)]
use criterion::{Criterion, black_box, criterion_group, criterion_main};

use af_core::frame::{AsciiCell, AsciiGrid};
use af_render::effects;

fn filled_grid(w: u16, h: u16) -> AsciiGrid {
    let mut grid = AsciiGrid::new(w, h);
    for cell in &mut grid.cells {
        *cell = AsciiCell {
            ch: '#',
            fg: (180, 120, 60),
            bg: (0, 0, 0),
        };
    }
    grid
}

fn bench_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("effects");

    // Strobe
    {
        let mut grid = filled_grid(80, 24);
        group.bench_function("strobe_80x24", |b| {
            b.iter(|| {
                effects::apply_strobe(black_box(&mut grid), 0.8, 1.5);
            });
        });
    }

    // Glow
    {
        let mut grid = filled_grid(80, 24);
        let mut brightness_buf = Vec::new();
        group.bench_function("glow_80x24", |b| {
            b.iter(|| {
                effects::apply_glow(black_box(&mut grid), 1.0, &mut brightness_buf);
            });
        });
    }

    // Chromatic aberration
    {
        let mut grid = filled_grid(80, 24);
        let mut fg_buf = Vec::new();
        group.bench_function("chromatic_80x24", |b| {
            b.iter(|| {
                effects::apply_chromatic_aberration(black_box(&mut grid), 2.0, &mut fg_buf);
            });
        });
    }

    // Temporal stability
    {
        let mut current = filled_grid(80, 24);
        let previous = filled_grid(80, 24);
        group.bench_function("temporal_stability_80x24", |b| {
            b.iter(|| {
                effects::apply_temporal_stability(black_box(&mut current), &previous, 0.3);
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_effects);
criterion_main!(benches);
