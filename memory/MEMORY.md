# classcii Project Memory

## Architecture
- 8 crates: af-core, af-audio, af-ascii, af-render, af-app, af-export, af-source, af-stems
- ~12k+ LOC Rust, edition 2024, MSRV 1.88
- Zero-unsafe, rayon parallel, triple buffer audio, ArcSwap config
- Three-thread topology: Source, Audio, Main (lock-free)

## Current State
- v1.4.0 — stable, all CI passes (clippy, fmt, test, rustdoc, bench)
- 25 presets (including 3 stem-aware: 23, 24, 25)
- Workflow persistence (save/load with bincode timeline + stem WAVs)
- 6 render modes: ASCII, HalfBlock, Braille, Quadrant, Sextant, Octant

## CI
- .github/workflows/ci.yml: fmt, clippy, test, rustdoc, bench, MSRV 1.88, cross-build (Win+Linux)
- .github/workflows/release.yml: tag-triggered, binary packaging, GitHub Release
