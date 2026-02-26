# clasSCII

Audio-reactive ASCII art engine. Transforms images into animated ASCII/Unicode art, modulated by real-time audio analysis.

## Features

- **4 render modes**: ASCII, HalfBlock (▄), Braille (⠿), Quadrant (▞)
- **5 built-in charsets**: Compact, Standard (Paul Bourke), Full, Blocks, Minimal
- **Audio-reactive**: 7-band spectral analysis, beat detection, BPM estimation
- **Audio playback**: Decoded audio files play through speakers while driving visuals
- **Post-processing**: Glow, fade trails, beat flash effects
- **20+ keyboard controls**: Density, contrast, brightness, saturation, color mode, aspect ratio, edge detection, shape matching, audio sensitivity/smoothing
- **Hot-reloadable config**: TOML-based, file-watched
- **Lock-free architecture**: Triple-buffer audio → render, zero-allocation hot paths

## Architecture

```
clasSCII workspace (6 crates)
├── af-core     — Types, traits, config, charset LUT, color
├── af-audio    — cpal capture, symphonia decode, FFT, features, beat, smoothing
├── af-source   — Image loading, fast_image_resize
├── af-ascii    — Luminance, color map, compositor, edge, shape, braille, quadrant, halfblock
├── af-render   — Canvas, UI layout, FPS, effects
└── af-app      — CLI, event loop, pipeline, hot-reload
```

## Usage

```bash
# Image only
cargo run --release -- --image path/to/image.png

# Image + audio file (plays audio, visuals react)
cargo run --release -- --audio path/to/track.wav --image path/to/image.png

# Image + microphone
cargo run --release -- --audio default --image path/to/image.png
```

## Controls

| Key | Action |
|---|---|
| `q` / `Esc` | Quit |
| `Space` | Pause/Resume |
| `Tab` | Cycle render mode |
| `1-5` | Select charset |
| `d` / `D` | Density −/+ |
| `i` | Toggle invert |
| `c` | Toggle color |
| `m` | Cycle color mode |
| `b` | Cycle BG style |
| `[` / `]` | Contrast −/+ |
| `{` / `}` | Brightness −/+ |
| `-` / `+` | Saturation −/+ |
| `a` | Cycle aspect ratio |
| `e` | Toggle edges |
| `s` | Toggle shape matching |
| `↑` / `↓` | Audio sensitivity |
| `←` / `→` | Audio smoothing |
| `?` | Help overlay |

## Build

Requires Rust 1.85+ (Edition 2024).

```bash
cargo build --release
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

## License

MIT OR Apache-2.0
