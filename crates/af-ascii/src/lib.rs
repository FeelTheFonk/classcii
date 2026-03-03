/// Cell count above which rayon parallelism outperforms sequential iteration.
/// Below this threshold, thread-pool scheduling overhead (~200-400ns) exceeds
/// the per-chunk work. Typical 80x24 terminal = 1920 cells (sequential faster).
pub const RAYON_CELL_THRESHOLD: u32 = 4000;

/// Dispatch grid row iteration: parallel (rayon) for large grids, sequential for small ones.
/// Avoids rayon scheduling overhead on typical 80x24 terminals.
#[inline]
pub fn for_each_row(
    cells: &mut [af_core::frame::AsciiCell],
    row_width: usize,
    f: impl Fn(usize, &mut [af_core::frame::AsciiCell]) + Send + Sync,
) {
    let cell_count = cells.len() as u32;
    if cell_count >= RAYON_CELL_THRESHOLD {
        use rayon::prelude::*;
        cells
            .par_chunks_mut(row_width)
            .enumerate()
            .for_each(|(cy, row)| f(cy, row));
    } else {
        cells
            .chunks_mut(row_width)
            .enumerate()
            .for_each(|(cy, row)| f(cy, row));
    }
}

pub mod braille;
pub mod color_map;
pub mod compositor;
pub mod dither;
pub mod edge;
pub mod halfblock;
/// ASCII conversion engine for clasSCII.
///
/// Converts pixel frames to ASCII/Unicode character grids.
pub mod masks;
pub mod quadrant;
pub mod shape_match;
