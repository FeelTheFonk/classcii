# clasSCII

A real-time, audio-reactive ASCII/Unicode rendering engine for terminal-based TUI applications — with offline generative batch export to lossless MP4.

## Requirements

- **Terminal**: GPU-accelerated (Alacritty, Kitty, WezTerm) for real-time mode.
- **FFmpeg + FFprobe**: Required in `PATH` for video source decoding and batch export.
- **Rust 1.85+**: Edition 2024.

## Architecture

The system utilizes a three-thread topology to isolate workloads:

1. **Source Thread**: Handles I/O and media decoding.
2. **Audio Thread**: Handles microphone capture, FFT analysis, and feature extraction.
3. **Main Thread**: Handles state aggregation, rendering matrix convolution, and TUI updates.

State synchronization between threads relies on lock-free structures (`triple_buffer` and `flume` channels). The rendering pipeline is designed to operate without allocating memory in the hot loop.

A fourth execution mode — **Batch Export** — runs headless (no terminal) and drives the entire pipeline offline from a pre-analyzed audio `FeatureTimeline`.

## Crate Layout

| Crate | Function |
|-------|----------|
| `af-core` | Shared primitives, configuration matrices, `FeatureTimeline`, and lock-free topologies. |
| `af-audio` | Audio capture (CPAL), FFT analysis, feature extraction, and offline `BatchAnalyzer`. |
| `af-ascii` | Luma-to-ASCII projection, and spatial quantization (Braille, Quadrant). |
| `af-render` | Display backend (`ratatui`), and partial redraws. |
| `af-source` | Input stream decoding (Image, FFmpeg, Webcam, `FolderBatchSource`). |
| `af-export` | Offline rasterizer (`ab_glyph`), lossless MP4 muxer (FFmpeg subprocess). |
| `af-app` | Application entry point, thread orchestration, generative mapper, and batch pipeline. |

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
classcii --image photo.jpg --preset psychedelic
```

### Batch Export Mode (Headless)

Scans a folder of media files (images + videos), pre-analyzes the audio, and renders a fully audio-reactive ASCII-art MP4 — frame-accurate, offline, with zero dropped frames.

```bash
# Required ONLY batch-folder. Auto-detects audio, auto-names MP4.
classcii --batch-folder ./media/

# Manual specifications
classcii --batch-folder ./media/ --audio track.mp3 --batch-out output.mp4 --fps 60
classcii --batch-folder ./media/ --preset aggressive
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
5. `Compositor` converts source pixels to `AsciiGrid`.
5. `Rasterizer` converts `AsciiGrid` to high-resolution RGBA pixels (FiraCode font, parallel).
6. `Mp4Muxer` encodes to lossless x264 CRF 0 / YUV444p.
7. Final audio+video muxing via FFmpeg.

## CLI Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--image <PATH>` | Source: static image (PNG, JPEG, BMP, GIF) | — |
| `--video <PATH>` | Source: video file (requires `--features video`) | — |
| `--webcam` | Source: webcam feed (requires `--features webcam`) | `false` |
| `--procedural <TYPE>` | Source: generator (`noise`, `plasma`, `particles`, `starfield`) | — |
| `--audio <PATH\|mic>` | Audio source: file path or `mic` for microphone | — |
| `--batch-folder <DIR>` | Batch export: media folder (images + videos) | — |
| `--batch-out <PATH>` | Batch export: output MP4 file path (opt) | — |
| `-c, --config <PATH>` | TOML configuration file | `config/default.toml` |
| `--preset <NAME>` | Load a named preset (overrides `--config`) | — |
| `--mode <MODE>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant` | from config |
| `--fps <N>` | Target framerate (30 or 60) | from config |
| `--no-color` | Disable color output | `false` |
| `--log-level <LEVEL>` | Log level: `error`, `warn`, `info`, `debug`, `trace` | `warn` |

## Runtime Controls

| Keybind | Action |
|---------|--------|
| `Tab` | Cycle render mode (Ascii / HalfBlock / Braille / Quadrant) |
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
| `g` / `G` | Adjust glow amplitude |
| `↑` / `↓` | Adjust general audio sensitivity |
| `←` / `→` | Seek temporal stream |
| `Space` | Pause / Resume engine |
| `C` | Open Custom Charset Editor |
| `A` | Open Audio Reactivity Mixer Panel |
| `o` / `O` | Open popup menu: [F]ile or [D]irectory (Batch Export) |
| `p` / `P` | Cycle preset |
| `x` | Toggle fullscreen |
| `?` | Toggle help menu |
| `q` / `Esc` | Quit / Close active overlay |

## Audio Reactivity Mapping

Audio-reactive behavior is configured via `audio_mappings` in TOML config files:

```toml
[[audio_mappings]]
source = "bass"           # Audio feature: rms, peak, sub_bass, bass, low_mid, mid,
                          # high_mid, presence, brilliance, spectral_centroid,
                          # spectral_flux, spectral_flatness, onset, beat_intensity,
                          # beat_phase, bpm
target = "edge_threshold" # Visual target: edge_threshold, edge_mix, contrast,
                          # brightness, saturation, density_scale, invert
amount = 1.0              # Multiplier
offset = 0.0              # Additive offset after multiplication
enabled = true
```

Multiple mappings can be defined simultaneously. In batch export mode, mappings are applied per-frame from the pre-analyzed `FeatureTimeline`.

## Presets

Available in `config/presets/`:

| Preset | Description |
|--------|-------------|
| `ambient` | Soft, smooth, low reactivity |
| `aggressive` | High contrast, strong onset response |
| `minimal` | Simple ASCII, minimal effects |
| `retro` | Classic terminal aesthetics |
| `psychedelic` | Maximum saturation, wild color modes |

Usage: `classcii --image photo.jpg --preset psychedelic`

## Configuration

Configurations and presets are managed via TOML files. Audio mappings and charset edits can be done live in the TUI or persisted in `config/default.toml` and `config/presets/*.toml`.

## Quality Assurance

- Hot loops do not allocate memory (R1).
- No `unsafe` blocks (R2).
- No panicking unwraps — `?` operator and `anyhow::Result` throughout (R3).
- Zero unnecessary copies — `Arc<FrameBuffer>`, `arc-swap`, `triple_buffer` (R4).
- `cargo clippy --workspace --features video -- -D warnings` passes clean (R7).
- `cargo fmt --check --all` passes clean (R7).

## License

MIT
