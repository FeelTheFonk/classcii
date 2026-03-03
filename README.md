https://github.com/user-attachments/assets/7bd9091a-01ea-4cdf-a842-218c25aa388b

# _classcii

Real-time audio-reactive ASCII/Unicode rendering engine for terminal-based TUI applications — with an offline generative batch export pipeline to mathematically lossless RGB MP4.

Integrates advanced topologies (Braille, Quadrants, Sextants, Octants), Blue Noise and Bayer dithering, perceptual Oklab color space, MFCC timbral analysis, a zero-alloc virtual camera (zoom, pan, rotation), 8 real-time post-processing effects, and audio-reactive Zalgo glitches — all with zero-allocation hot loops and 100% lock-free safe Rust memory management.

## Requirements

- **Terminal**: GPU-accelerated (Alacritty, Kitty, WezTerm) for real-time mode.
- **FFmpeg + FFprobe**: Required in `PATH` for video source decoding and lossless RGB batch exports (`libx264rgb`).
- **Rust 1.88+**: Edition 2024.

## Architecture

Three-thread topology:

1. **Source Thread**: I/O and media decoding.
2. **Audio Thread**: Microphone capture, FFT analysis, feature extraction.
3. **Main Thread**: State aggregation, rendering, TUI updates.

Lock-free synchronization via `triple_buffer` and `flume` channels. A fourth mode — **Batch Export** — runs headless from a pre-analyzed audio `FeatureTimeline`.

## Crate Layout

| Crate | Function |
|-------|----------|
| `af-core` | Shared primitives, configuration, `FeatureTimeline`, lock-free topologies |
| `af-audio` | Audio capture (CPAL), FFT, feature extraction, offline `BatchAnalyzer` |
| `af-ascii` | Luma-to-ASCII, spatial quantization (Braille, Quadrant, Sextant U+1FB00, Octant U+1CD00), dithering, edge detection |
| `af-render` | Display backend (`ratatui`), partial redraws, Zalgo distortions |
| `af-source` | Input stream decoding (Image, FFmpeg, `FolderBatchSource`) |
| `af-export` | Offline rasterizer (`ab_glyph`) with alpha-blended Zalgo, lossless MP4 muxer |
| `af-app` | Entry point, thread orchestration, generative mapper, batch pipeline |

## Compilation

```bash
# Minimal (image-only)
cargo build

# Release with video + batch export support
cargo build --release --features full
```

## Quick Start

```bash
# Launch TUI (no source — empty canvas)
classcii

# Static image
classcii --image photo.jpg

# Image + microphone audio reactivity
classcii --image photo.jpg --audio mic

# Video file
classcii --video clip.mp4

# Preset
classcii --image photo.jpg --preset 07_neon_abyss

# Batch export
classcii --batch-folder ./media/ --audio track.mp3 --preset all
```

## Documentation

| Document | Content |
|----------|---------|
| [Usage Guide](docs/USAGE.md) | CLI reference, keyboard controls, configuration, batch export, troubleshooting |
| [Audio Guide](docs/AUDIO_GUIDE.md) | Audio pipeline, 21 sources, 18 targets, 4 curves, smoothing, genre strategies |
| [Reference](docs/REFERENCE.md) | TOML schema, 8 effects, 22 presets, 10 charsets, default values |

## Quality Assurance

- O(1) mathematical mapping for UI bounds and typographical translation (Sextant/Octant LUTs).
- Hot loops are memory-stable — zero allocation (R1).
- Zero `unsafe` blocks — `#![deny(unsafe_code)]` workspace-wide (R2).
- Zero panicking unwraps — `?` operator and graceful fallback (R3).
- Zero unnecessary copies — `Arc<FrameBuffer>`, `arc-swap`, lock-free `triple_buffer` (R4).
- `cargo clippy --workspace --features full -- -D warnings` passes 0 warnings.
- 83 tests (unit + doctests). `cargo fmt --check --all` clean.
- All division operations guarded. All inputs clamped to valid ranges.
- Release profile: LTO=fat, codegen-units=1, strip=symbols, panic=abort.

## License

MIT OR Apache-2.0
