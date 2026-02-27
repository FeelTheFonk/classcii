# clasSCII

Real-time, audio-reactive ASCII/Unicode art engine for terminal-based TUI applications.

## Architecture

clasSCII uses a three-thread model (Main, Audio, Source) with lock-free communication via `triple_buffer` and `flume` channels.

```
┌──────────┐    ┌──────────┐    ┌──────────┐
│  Source   │───▶│   Main   │◀───│  Audio   │
│  Thread   │    │  Thread  │    │  Thread  │
│ (frames)  │    │ (render) │    │  (FFT)   │
└──────────┘    └──────────┘    └──────────┘
```

## Workspace

| Crate | Role |
|-------|------|
| `af-core` | Shared types, config, traits, frame buffers |
| `af-audio` | Audio capture, FFT, beat detection, feature extraction |
| `af-ascii` | Pixel→ASCII conversion (luminance, edge, braille, halfblock, quadrant, shape matching) |
| `af-render` | Terminal rendering via ratatui (canvas, UI, effects, FPS) |
| `af-source` | Visual input sources (image, video, webcam, procedural) |
| `af-app` | Application entry point, event loop, pipeline orchestration |

## Build

```bash
# Development
cargo build --workspace

# Release with video support
cargo build --release --features video
```

## Usage

```bash
# Image source
classcii --image path/to/image.png

# Image + audio from microphone
classcii --image path/to/image.png --audio mic

# Video with embedded audio
classcii --video path/to/video.mp4

# Override render mode and FPS
classcii --image photo.jpg --mode braille --fps 60

# Load a preset
classcii --image photo.jpg --preset psychedelic
```

## Controls

| Key | Action |
|-----|--------|
| `Tab` | Cycle render mode (Ascii → HalfBlock → Braille → Quadrant) |
| `1`–`5` | Select charset |
| `c` | Toggle color |
| `i` | Invert |
| `e` | Toggle edge detection |
| `s` | Toggle shape matching |
| `m` | Cycle color mode |
| `b` | Cycle background style |
| `d`/`D` | Density scale ±0.25 |
| `[`/`]` | Contrast ±0.1 |
| `{`/`}` | Brightness ±0.05 |
| `-`/`+` | Saturation ±0.1 |
| `f`/`F` | Fade decay ±0.1 |
| `g`/`G` | Glow intensity ±0.1 |
| `↑`/`↓` | Audio sensitivity ±0.1 |
| `←`/`→` | Seek ±5s (video/audio file) |
| `Space` | Pause/Resume |
| `?` | Help overlay |
| `q`/`Esc` | Quit |

## Configuration

Default config: `config/default.toml`. Presets in `config/presets/`.

```bash
classcii --image photo.jpg --config my_config.toml
classcii --image photo.jpg --preset ambient
```

Hot-reload: edit the config file while the app is running.

## Design Principles

- **Zero allocation in hot paths** — pre-allocated buffers, no per-frame Vec creation
- **No `unwrap()` outside tests** — all errors handled gracefully
- **Clean clippy** — `cargo clippy --workspace -- -D warnings` = 0
- **Three-thread model** — lock-free, no mutex contention

## License

MIT
