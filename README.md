https://github.com/user-attachments/assets/7bd9091a-01ea-4cdf-a842-218c25aa388b

# _classcii

 Real-time audio-reactive ASCII/Unicode rendering engine for terminal-based TUI applications — with an offline generative batch export pipeline to mathematically lossless RGB MP4.
This engine pushes the limits of typography by integrating advanced topologies (Braille, Quadrants, Sextants, Octants), Blue Noise and Bayer dithering, perceptual Oklab color space, MFCC timbral analysis, a SOTA Native Zero-Alloc Virtual Camera (Zoom, Pan, Rotation), 8 real-time post-processing effects, and audio-reactive Zalgo glitches — all while guaranteeing zero-allocation in the hot loops and 100% lock-free Safe Rust memory management.

## Requirements

- **Terminal**: GPU-accelerated (Alacritty, Kitty, WezTerm) for real-time mode.
- **FFmpeg + FFprobe**: Required in `PATH` for video source decoding and mathematically lossless RGB batch exports (`libx264rgb`).
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

# Multi-preset mode: cycle through all presets with audio-reactive transitions
classcii --batch-folder ./media/ --audio track.mp3 --preset all

# Reproducible export with custom mutation intensity
classcii --batch-folder ./media/ --preset all --seed 42 --mutation-intensity 0.5
```

**Arguments** for batch mode:
- `--batch-folder`: Directory containing images (PNG, JPG) and/or videos (MP4, MKV, etc.).
- `--audio` (Optional): Path to audio file. If omitted, will search the batch folder.
- `--batch-out` (Optional): Output path for the MP4. If omitted, uses `<folder_name>_<timestamp>.mp4`.
- `--preset all` (Optional): Cycle through all presets with smooth interpolated transitions.
- `--seed <N>` (Optional): Seed for reproducible exports.
- `--mutation-intensity <F>` (Optional): Scale mutation aggressiveness (0=pure preset, 1=default, 2=aggressive).

**Pipeline**:
1. Full offline audio analysis → `FeatureTimeline` with bass-weighted spectral flux, BPM estimation, and feature normalization.
2. Energy level classification (low/medium/high) from sliding-window RMS for adaptive clip pacing.
3. `AutoGenerativeMapper` modulates `RenderConfig` per frame from the timeline.
4. `FolderBatchSource` sequences media files with crossfade transitions between clips.
5. **Macro-generative** director with mutation coordination (cooldown, max 2 per event, energy-scaled probabilities) creates structural variations (mode cycle, charset rotations, 6-type effect bursts with smoothstep easing, continuous density pulses, color mode cycle, invert flashes with auto-revert, camera bursts) on strong beats. Optional `--preset all` mode cycles through all presets with smooth interpolated transitions.
6. `Compositor` converts source pixels to `AsciiGrid` utilizing advanced bitmasking (Sextant, Octant) and O(1) Bayer Dithering.
7. `Rasterizer` converts `AsciiGrid` to high-resolution RGBA pixels (parallel execution with zero-alloc Zalgo diacritics alpha-blending and dynamic upscaling via `--export-scale`).
8. `Mp4Muxer` encodes to mathematically pure lossless RGB x264 CRF 0 / RGB24 (`libx264rgb`), fully preventing chroma subsampling bleed on typographical texts.
9. Final audio+video muxing via FFmpeg.

## CLI Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--image <PATH>` | Source: static image or animated GIF (PNG, JPEG, BMP, GIF) | — |
| `--video <PATH>` | Source: video file (requires `--features video`) | — |
| `--audio <PATH\|mic>` | Audio source: file path or `mic` for microphone | — |
| `--batch-folder <DIR>` | Batch export: media folder (images + videos) | — |
| `--batch-out <PATH>` | Batch export: output MP4 file path (opt) | — |
| `-c, --config <PATH>` | TOML configuration file | `config/default.toml` |
| `--preset <NAME>` | Load a named preset (overrides `--config`) | — |
| `--mode <MODE>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant`, `sextant`, `octant` | from config |
| `--fps <N>` | Target framerate (30 or 60) | from config |
| `--no-color` | Disable color output | `false` |
| `--log-level <LEVEL>` | Log level: `error`, `warn`, `info`, `debug`, `trace` | `warn` |
| `--preset-list` | List all available presets and exit | `false` |
| `--seed <N>` | Reproducible batch export (same seed = same output) | — |
| `--preset-duration <SECS>` | Max duration per preset in `--preset all` mode | `15.0` |
| `--crossfade-ms <MS>` | Crossfade duration between media clips | adaptive |
| `--mutation-intensity <F>` | Mutation probability multiplier (0=none, 2=aggressive) | `1.0` |

## Runtime Controls

| Keybind | Action |
|---------|--------|
| `Tab` | Cycle render mode (Ascii / HalfBlock / Braille / Quadrant / Sextant / Octant) |
| `1`–`0` | Select built-in charset |
| `c` | Toggle color output |
| `i` | Invert luminance |
| `e` / `E` | Toggle edge detection / Adjust edge mix |
| `s` | Toggle shape matching |
| `a` | Toggle aspect ratio correction |
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
| `z` / `Z` | Adjust zalgo intensity |
| `y` / `Y` | Adjust temporal stability |
| `j` / `J` | Adjust strobe decay |
| `u` / `U` | Adjust wave speed |
| `<` / `>` | Adjust camera zoom |
| `,` / `.` | Adjust camera rotation |
| `;` / `'` | Adjust camera pan X |
| `:` / `"` | Adjust camera pan Y |
| `v` | Toggle spectrum display |
| `n` | Cycle dither mode (Bayer8x8 / BlueNoise16 / Off) |
| `↑` / `↓` | Adjust general audio sensitivity |
| `←` / `→` | Seek temporal stream |
| `Space` | Pause / Resume engine |
| `C` | Open Custom Charset Editor |
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
                           # onset_envelope, spectral_rolloff, zero_crossing_rate
target = "zalgo_intensity" # Visual target: edge_threshold, edge_mix, contrast,
                           # brightness, saturation, density_scale, invert,
                           # beat_flash_intensity, chromatic_offset, wave_amplitude,
                           # color_pulse_speed, fade_decay, glow_intensity,
                           # zalgo_intensity, camera_zoom_amplitude,
                           # camera_rotation, camera_pan_x, camera_pan_y
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

Press `K` to enter Creation Mode, an auto-modulation engine that drives all visual effects from audio features. Eleven presets adapt to the audio content:

| Preset | Character |
|--------|-----------|
| Ambient | Slow oscillations, low intensity, drift-based |
| Percussive | Beat-locked, aggressive strobe and wave, density pulse |
| Psychedelic | High chromatic aberration, fast color pulse |
| Cinematic | Smooth fade, wide glow, controlled dynamics |
| Minimal | Single dominant effect, clean and focused |
| Photoreal | Sharpest rendering, subtle audio response |
| Abstract | Non-figurative cross-mapped effects, density modulation |
| Glitch | Digital corruption, zalgo dominant, onset invert |
| Lo-Fi | Vintage degraded, constant scan lines |
| Spectral | Each frequency band drives a distinct effect |
| Custom | Manual control only (no auto-modulation) |

Navigation: `Up/Down` select effect (Master at top, then 9 effects), `Left/Right` always adjust the selected element, `a` toggle auto/manual mode, `p` cycle preset, `Esc` exit.

## R&D: Perceptual Color and Dithering

- **Oklab color space**: Perceptually uniform brightness adjustments via `rgb_to_oklab` / `oklab_to_rgb`. Selectable via `m` key (Direct / HSV / Oklab / Quantized). Color modes apply to all 6 render modes (ASCII, Braille, HalfBlock, Quadrant, Sextant, Octant).
- **Blue Noise 16x16 dithering**: Perceptually superior to Bayer ordered dithering. Cycle with `n` (Bayer8x8 / BlueNoise16 / Off).
- **Temporal Stability**: Anti-flicker heuristic based on character density distance (with Sextant/Braille-aware coverage), preventing rapid character oscillation.

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
| `11_reactive` | All effects showcase, moderate levels, audio-reactive |
| `12_deep_zoom` | Audio-reactive camera zoom and rotation |
| `13_breath` | Ultra-minimalist contemplative, single RMS mapping |
| `14_interference` | Wave interference patterns with chromatic separation |
| `15_noir` | Cinematic film noir, monochrome, high contrast edges |
| `16_aurora` | Aurora borealis, saturated glow, camera pan |
| `17_static` | Broken TV / white noise, binary charset, zalgo on transients |
| `18_spectral_bands` | Per-band frequency mapping, each band drives a distinct effect |
| `19_cinematic_camera` | Audio-reactive virtual camera, bass→zoom, centroid→rotation |
| `20_sextant_film` | Sextant mode cinematic rendering, Oklab perceptual color |
| `21_octant_dense` | Maximum sub-pixel density, Octant mode, spectral bar charset |
| `22_hires_export` | Ultra high-resolution batch export, CHARSET_FULL, Oklab |

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
- 78+ tests (unit + doctests) pass. `cargo fmt --check --all` clean.
- All division operations guarded against zero. All user inputs clamped to valid ranges.
- Release profile: LTO=fat, codegen-units=1, strip=symbols, panic=abort.

## License

MIT OR Apache-2.0
