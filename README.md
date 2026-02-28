<img width="32" height="32" alt="classcii_32" src="https://github.com/user-attachments/assets/ec99168e-60d2-4818-9330-67b95ab0f674" /> 

# _classcii

 Real-time audio-reactive ASCII/Unicode rendering engine for terminal-based TUI applications — with an offline generative batch export pipeline to lossless MP4.
This engine pushes the limits of typography by integrating advanced topologies (Braille, Quadrants, Sextants, Octants), Blue Noise and Bayer dithering, perceptual Oklab color space, MFCC timbral analysis, 8 real-time post-processing effects, a Creation Mode with auto-modulation presets, and audio-reactive Zalgo glitches — all while guaranteeing zero-allocation in the hot loops and 100% lock-free Safe Rust memory management.

## Requirements

- **Terminal**: GPU-accelerated (Alacritty, Kitty, WezTerm) for real-time mode.
- **FFmpeg + FFprobe**: Required in `PATH` for video source decoding and batch export.
- **Rust 1.88+**: Edition 2024.

## Architecture

The system utilizes a three-thread topology to isolate workloads:

1. **Source Thread**: Handles I/O and media decoding.
2. **Audio Thread**: Handles microphone capture, FFT analysis, and feature extraction.
3. **Main Thread**: Handles state aggregation, rendering matrix convolution, and TUI updates.

State synchronization between threads relies on lock-free structures (`triple_buffer` and `flume` channels). The rendering pipeline is designed to operate without allocating memory in the hot loop.

A fourth execution mode — **Batch Export** — runs headless (no terminal) and drives the entire pipeline offline from a pre-analyzed audio `FeatureTimeline`.

## Crate Layout

| Crates | Function |
|--------|----------|
| `af-core` | Shared primitives, configuration matrices, `FeatureTimeline`, and lock-free topologies. |
| `af-audio` | Audio capture (CPAL), FFT analysis, feature extraction, and offline `BatchAnalyzer`. |
| `af-ascii` |  Projections: Luma-to-ASCII, spatial quantization (Braille, Quadrant, Sextant U+1FB00, Octant U+1CD00), Bayer Dithering, and edge detection. |
| `af-render` | Display backend (`ratatui`), partial redraws, and Zalgo typographical distortions. |
| `af-source` | Input stream decoding (Image, FFmpeg, `FolderBatchSource`). |
| `af-export` | Offline rasterizer (`ab_glyph`) with Alpha-blended Zalgo compositing, lossless MP4 muxer (FFmpeg subprocess). |
| `af-app` | Application entry point, thread orchestration, auto-generative mapper, and batch pipeline director. |

## Compilation

```bash
# Minimal (image-only)
cargo build

# Release with video + batch export support
cargo build --release --features video
```

## Usage

### Real-Time Mode (TUI)

```bash
# Static image
classcii --image path/to/image.png

# Image with microphone audio reactivity
classcii --image path/to/image.png --audio mic

# Video file
classcii --video path/to/video.mp4

# Video with audio file override
classcii --video path/to/video.mp4 --audio path/to/track.mp3

# Configuration overrides
classcii --image photo.jpg --mode braille --fps 60
classcii --image photo.jpg --preset 07_neon_abyss
```

### Batch Export Mode (Headless)

Scans a folder of media files (images + videos), pre-analyzes the audio, and renders a fully audio-reactive ASCII-art MP4 — frame-accurate, offline, with zero dropped frames.

```bash
# Required ONLY batch-folder. Auto-detects audio, auto-names MP4.
classcii --batch-folder ./media/

# Manual specifications
classcii --batch-folder ./media/ --audio track.mp3 --batch-out output.mp4 --fps 60
classcii --batch-folder ./media/ --preset 02_matrix
```

**Arguments** for batch mode:
- `--batch-folder`: Directory containing images (PNG, JPG) and/or videos (MP4, MKV, etc.).
- `--audio` (Optional): Path to audio file. If omitted, will search the batch folder.
- `--batch-out` (Optional): Output path for the MP4. If omitted, uses `<folder_name>_<timestamp>.mp4`.

**Pipeline**:
1. Full offline audio analysis → `FeatureTimeline`.
2. `AutoGenerativeMapper` modulates `RenderConfig` per frame.
3. `FolderBatchSource` sequences media files.
4. **Macro-generative** director logic creates structural variations (mode cycle, invert flashes, charset rotations) on strong beats.
5. `Compositor` converts source pixels to `AsciiGrid` utilizing advanced bitmasking (Sextant, Octant) and O(1) Bayer Dithering.
5. `Rasterizer` converts `AsciiGrid` to high-resolution RGBA pixels (parallel execution with zero-alloc Zalgo diacritics alpha-blending).
6. `Mp4Muxer` encodes to lossless x264 CRF 0 / YUV444p.
7. Final audio+video muxing via FFmpeg.

## CLI Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--image <PATH>` | Source: static image (PNG, JPEG, BMP, GIF) | — |
| `--video <PATH>` | Source: video file (requires `--features video`) | — |
| `--procedural <TYPE>` | Source: generator (`noise`, `plasma`, `particles`, `starfield`) | — |
| `--audio <PATH\|mic>` | Audio source: file path or `mic` for microphone | — |
| `--batch-folder <DIR>` | Batch export: media folder (images + videos) | — |
| `--batch-out <PATH>` | Batch export: output MP4 file path (opt) | — |
| `-c, --config <PATH>` | TOML configuration file | `config/default.toml` |
| `--preset <NAME>` | Load a named preset (overrides `--config`) | — |
| `--mode <MODE>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant`, `sextant`, `octant` | from config |
| `--fps <N>` | Target framerate (30 or 60) | from config |
| `--no-color` | Disable color output | `false` |
| `--log-level <LEVEL>` | Log level: `error`, `warn`, `info`, `debug`, `trace` | `warn` |

## Runtime Controls

| Keybind | Action |
|---------|--------|
| `Tab` | Cycle render mode (Ascii / HalfBlock / Braille / Quadrant / Sextant / Octant) |
| `1`–`0` | Select built-in charset |
| `c` | Toggle color output |
| `i` | Invert luminance |
| `e` / `E` | Toggle edge detection / Adjust edge mix |
| `s` | Toggle shape matching |
| `m` | Cycle color mode |
| `b` | Cycle background style |
| `d` / `D` | Adjust density scale |
| `[` / `]` | Adjust contrast |
| `{` / `}` | Adjust brightness |
| `-` / `+` | Adjust saturation |
| `f` / `F` | Adjust fade decay |
| `g` / `G` | Adjust glow intensity |
| `t` / `T` | Adjust strobe intensity |
| `r` / `R` | Adjust chromatic aberration |
| `w` / `W` | Adjust wave distortion |
| `h` / `H` | Adjust color pulse speed |
| `l` / `L` | Adjust scan line gap |
| `v` | Toggle spectrum display |
| `n` | Cycle dither mode (Bayer8x8 / BlueNoise16 / Off) |
| `↑` / `↓` | Adjust general audio sensitivity |
| `←` / `→` | Seek temporal stream |
| `Space` | Pause / Resume engine |
| `C` | Open Custom Charset Editor |
| `A` | Open Audio Reactivity Mixer Panel |
| `K` | Enter Creation Mode (auto-modulation) |
| `o` | Open visual file picker (image / video) |
| `O` | Open audio file picker |
| `p` / `P` | Cycle preset |
| `x` | Toggle fullscreen |
| `?` | Toggle help menu |
| `q` / `Esc` | Quit / Close active overlay |

## Audio Reactivity Mapping

Audio-reactive behavior is configured via `audio_mappings` in TOML config files:

```toml
[[audio.mappings]]
source = "bass"            # Audio feature: rms, peak, sub_bass, bass, low_mid, mid,
                           # high_mid, presence, brilliance, spectral_centroid,
                           # spectral_flux, spectral_flatness, onset, beat_intensity,
                           # beat_phase, bpm, timbral_brightness, timbral_roughness,
                           # onset_envelope
target = "zalgo_intensity" # Visual target: edge_threshold, edge_mix, contrast,
                           # brightness, saturation, density_scale, invert,
                           # zalgo_intensity
amount = 1.0               # Multiplier
offset = 0.0               # Additive offset after multiplication
curve = "Linear"           # Response curve: Linear, Exponential, Threshold, Smooth
enabled = true
```

Multiple mappings can be defined simultaneously. MFCC-derived timbral features (`timbral_brightness`, `timbral_roughness`) enable instrument-aware reactivity via 26 Mel-spaced triangular filters (300-8000 Hz) with DCT-II compression to 5 coefficients. Four response curves (Linear, Exponential, Threshold, Smooth) and per-mapping smoothing override the global EMA smoothing. In batch export mode, mappings are applied per-frame from the pre-analyzed `FeatureTimeline`.

## Post-Processing Effects

8 real-time composable effects applied in a fixed pipeline order:

| Effect | Key | Description |
|--------|-----|-------------|
| Temporal Stability | (auto) | Anti-flicker via char density heuristic |
| Wave Distortion | `w/W` | Sinusoidal row shift |
| Chromatic Aberration | `r/R` | R/B channel offset |
| Color Pulse | `h/H` | HSV hue rotation |
| Fade Trails | `f/F` | Temporal persistence |
| Strobe | `t/T` | Beat-synced continuous envelope flash |
| Scan Lines | `l/L` | Darken every Nth row |
| Glow | `g/G` | Brightness bloom |

## Creation Mode

Press `K` to enter Creation Mode, an auto-modulation engine that drives all visual effects from audio features. Four presets adapt to the audio content:

| Preset | Character |
|--------|-----------|
| Ambient | Slow oscillations, low intensity, drift-based |
| Percussive | Beat-locked, aggressive strobe and wave |
| Psychedelic | High chromatic aberration, fast color pulse |
| Cinematic | Smooth fade, wide glow, controlled dynamics |

Navigation: `Up/Down` select effect, `Left/Right` adjust master intensity (auto) or selected effect (manual), `a` toggle auto/manual mode, `p` cycle preset, `Esc` exit.

## R&D: Perceptual Color and Dithering

- **Oklab color space**: Perceptually uniform brightness adjustments via `rgb_to_oklab` / `oklab_to_rgb`. Selectable via `m` key (Direct / HSV / Oklab / Quantized).
- **Blue Noise 16x16 dithering**: Perceptually superior to Bayer ordered dithering. Cycle with `n` (Bayer8x8 / BlueNoise16 / Off).
- **Temporal Stability**: Anti-flicker heuristic based on character density distance, preventing rapid ASCII character oscillation.

## Presets

Available in `config/presets/`, selectable via `--preset <name>` or cycled live with `p`/`P`:

| Preset | Description |
|--------|-------------|
| `01_cyber_braille` | Braille matrix, high contrast cyberpunk |
| `02_matrix` | Classic Matrix digital rain aesthetic |
| `03_ghost_edge` | Edge detection with spectral fade trails |
| `04_pure_ascii` | Clean ASCII gradient, minimal effects |
| `05_classic_gradient` | Standard luminance gradient mapping |
| `06_vector_edges` | Edge-dominant, vector-style rendering |
| `07_neon_abyss` | Neon colors, deep glow, high saturation |
| `08_cyber_noise` | Glitch-heavy, noise-driven visuals |
| `09_brutalism_mono` | Monochrome, high contrast brutalist style |
| `10_ethereal_shape` | Shape matching, soft ethereal aesthetics |

Usage: `classcii --image photo.jpg --preset 02_matrix`

## Configuration

Configurations and presets are managed via TOML files. Audio mappings and charset edits can be done live in the TUI or persisted in `config/default.toml` and `config/presets/*.toml`.

## Quality Assurance

- Native O(1) mathematical mapping for UI bounds and typographical translation (Sextant/Octant LUTs processed at compile-time).
- Hot loops are absolutely memory-stable and do not allocate memory (R1).
- Zero `unsafe` blocks — strict immunity to segfaults enforced by `#![deny(unsafe_code)]` workspace-wide (R2).
- Zero panicking unwraps — `?` operator and graceful fallback implemented across all layers (R3).
- Zero unnecessary copies — driven by `Arc<FrameBuffer>`, `arc-swap`, and lock-free `triple_buffer` mechanics (R4).
- Compile strictness: `cargo clippy --workspace --features video -- -D warnings` passes 0 warnings with pedantic lints enabled.
- 65 tests (unit + doctests) pass. `cargo fmt --check --all` clean.
- All division operations guarded against zero. All user inputs clamped to valid ranges.
- Release profile: LTO=fat, codegen-units=1, strip=symbols, panic=abort.

## License

MIT OR Apache-2.0
