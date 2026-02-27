# AsciiForge

A real-time, audio-reactive ASCII/Unicode rendering engine for terminal-based TUI applications.

## Requirements

The application requires a GPU-accelerated terminal emulator (e.g., Alacritty, Kitty, WezTerm) to prevent computational bottlenecks during rendering.

## Architecture

The system utilizes a three-thread topology to isolate workloads:
1. **Source Thread**: Handles I/O and media decoding.
2. **Audio Thread**: Handles microphone capture, FFT analysis, and feature extraction.
3. **Main Thread**: Handles state aggregation, rendering matrix convolution, and TUI updates.

State synchronization between threads relies on lock-free structures (`triple_buffer` and `flume` channels). The rendering pipeline is designed to operate without allocating memory (`Vec`, `String`) in the hot loop.

## Crate Layout

| Crate | Function |
|-------|----------|
| `af-core` | Shared primitives, configuration matrices, and lock-free topologies. |
| `af-audio` | Audio capture (CPAL), FFT analysis, and feature extraction. |
| `af-ascii` | Luma-to-ASCII projection, and spatial quantization (Braille, Quadrant). |
| `af-render` | Display backend (`ratatui`), and partial redraws. |
| `af-source` | Input stream decoding (Image, FFmpeg, Webcam). |
| `af-app` | Application entry point, thread orchestration, and pipeline state. |

## Dependencies

Compiling with the `video` feature requires C-ABI linkage libraries. Ensure the following are installed in the host environment:
`libavformat-dev`, `libavutil-dev`, `libavcodec-dev`, `libswscale-dev`.

## Compilation

```bash
# Development
cargo build

# Release (LTO enabled, video feature active)
cargo build --release --features video
```

## Usage

```bash
# Static image
classcii --image path/to/image.png

# Image with microphone audio reactivity
classcii --image path/to/image.png --audio mic

# Video file
classcii --video path/to/video.mp4

# Specific configuration overrides
classcii --image photo.jpg --mode braille --fps 60
classcii --image photo.jpg --preset psychedelic
```

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
| `o` / `O` | Open visual / audio file |
| `p` / `P` | Cycle preset |
| `x` | Toggle fullscreen |
| `?` | Toggle help menu |
| `q` / `Esc` | Quit / Close active overlay |

## Configuration

Configurations and presets are managed via TOML files. Audio mappings and charset edits can be done live in the TUI or persisted in `config/default.toml` and `config/presets/*.toml`.

## Quality Assurance

- Hot loops do not allocate memory.
- No `unsafe` blocks are utilized.
- Operations bypass panicking unwraps (`unwrap()`/`expect()`) in favor of deterministic bounds checking and safe error handling.

## License

MIT
