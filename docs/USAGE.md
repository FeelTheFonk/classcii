# Usage Guide

Complete reference for classcii v1.4.0 — real-time audio-reactive ASCII/Unicode rendering engine.

## Prerequisites

- **Rust 1.88+** (Edition 2024)
- **FFmpeg + FFprobe** in `PATH` (required for video sources and batch export)
  - Windows: `winget install ffmpeg`
  - Linux: `sudo apt install ffmpeg`
  - macOS: `brew install ffmpeg`
- **GPU-accelerated terminal** recommended (Alacritty, WezTerm, Kitty)
- **Python 3.10+** + `uv` (required only for stem separation)
  - Windows: run `scripts\setup_stems.bat`
  - Linux/macOS: run `scripts/setup_stems.sh`

## Installation

```bash
git clone https://github.com/FeelTheFonk/classcii.git
cd classcii
cargo build --release --features video
```

## First Run

```bash
# Launch TUI without source (empty canvas, load files with o/O)
classcii

# Static image as ASCII art
classcii --image photo.jpg

# Image with live microphone audio reactivity
classcii --image photo.jpg --audio mic

# Video file with its audio track
classcii --video movie.mp4

# With a preset
classcii --image photo.jpg --preset 07_neon_abyss --audio mic
```

---

## CLI Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--image <PATH>` | Source: static image or animated GIF (PNG, JPEG, BMP, GIF) | — |
| `--video <PATH>` | Source: video file (requires `--features video`) | — |
| `--audio <PATH\|mic>` | Audio source: file path or `mic` for microphone | — |
| `--batch-folder <DIR>` | Batch export: media folder (images + videos) | — |
| `--batch-out <PATH>` | Batch export: output MP4 file path | auto-named |
| `-c, --config <PATH>` | TOML configuration file | `config/default.toml` |
| `--preset <NAME>` | Load a named preset (overrides `--config`) | — |
| `--mode <MODE>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant`, `sextant`, `octant` | from config |
| `--fps <N>` | Target framerate (30 or 60) | from config |
| `--no-color` | Disable color output | `false` |
| `--log-level <LEVEL>` | Logging: `error`, `warn`, `info`, `debug`, `trace` | `warn` |
| `--preset-list` | List all available presets and exit | — |
| `--seed <N>` | Reproducible batch export seed | — |
| `--preset-duration <SECS>` | Max duration per preset in `--preset all` mode | `15.0` |
| `--crossfade-ms <MS>` | Crossfade duration between media clips | adaptive |
| `--mutation-intensity <F>` | Mutation probability multiplier (0=none, 2=aggressive) | `1.0` |
| `--export-scale <F>` | Upscaling factor for batch rasterization | — |
| `--stems` | Enable stem separation in batch mode (requires `--audio`) | `false` |
| `--stem-model <NAME>` | SCNet model: `standard` (41MB) or `large` (162MB) | `standard` |
| `--save-workflow <NAME>` | Save workflow after batch export | — |
| `--load-workflow <PATH>` | Load a saved workflow (overrides --config/--preset/--audio) | — |
| `--workflow-list` | List all saved workflows and exit | — |

All flags are optional. Running `classcii` with no arguments launches the TUI with an empty canvas.

---

## Keyboard Controls

Press `?` to show the in-app help overlay. Use `Up`/`Down` to scroll when open.

### Navigation & Playback

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit application or close active overlay |
| `?` | Toggle help overlay |
| `Space` | Pause / Resume |
| `Left` / `Right` | Seek video/audio stream |
| `Backspace` | Reset all parameters to defaults |

### Render Mode & Display

| Key | Action |
|-----|--------|
| `Tab` | Cycle render mode: Ascii / HalfBlock / Braille / Quadrant / Sextant / Octant |
| `1`–`0` | Select built-in charset (see [Reference](REFERENCE.md#charsets)) |
| `c` | Toggle color output |
| `i` | Invert luminance |
| `N` / `M` | Input gain down / up (±0.5, pre-FFT) |
| `m` | Cycle color mode: Direct / HsvBright / Oklab / Quantized |
| `b` | Cycle background style: Black / SourceDim / Transparent |
| `n` | Cycle dither mode: Bayer8x8 / BlueNoise16 / Off |
| `a` | Toggle aspect ratio correction |
| `x` | Toggle fullscreen (hide sidebar and spectrum) |
| `p` / `P` | Cycle preset (forward / backward) |

### Render Parameters

| Key | Action | Range |
|-----|--------|-------|
| `d` / `D` | Density scale | 0.25 – 4.0 |
| `e` / `E` | Toggle edge detection / adjust edge mix | 0.0 – 1.0 |
| `s` | Toggle shape matching | on/off |
| `[` / `]` | Contrast | 0.1 – 3.0 |
| `{` / `}` | Brightness | -1.0 – 1.0 |
| `-` / `+` | Saturation | 0.0 – 3.0 |

### Effect Parameters

| Key | Action | Range |
|-----|--------|-------|
| `f` / `F` | Fade decay | 0.0 – 1.0 |
| `g` / `G` | Glow intensity | 0.0 – 2.0 |
| `r` / `R` | Chromatic aberration | 0.0 – 5.0 |
| `w` / `W` | Wave amplitude | 0.0 – 1.0 |
| `u` / `U` | Wave speed | 0.0 – 10.0 |
| `h` / `H` | Color pulse speed | 0.0 – 5.0 |
| `l` / `L` | Scan line gap | 0 – 8 |
| `t` / `T` | Beat flash intensity | 0.0 – 2.0 |
| `j` / `J` | Strobe decay | 0.5 – 0.99 |
| `z` / `Z` | Zalgo intensity | 0.0 – 5.0 |
| `y` / `Y` | Temporal stability | 0.0 – 1.0 |

### Camera

| Key | Action | Range |
|-----|--------|-------|
| `<` / `>` | Camera zoom | 0.1 – 10.0 |
| `,` / `.` | Camera rotation | periodic |
| `;` / `'` | Camera pan X | -2.0 – 2.0 |
| `:` / `"` | Camera pan Y | -2.0 – 2.0 |

### Audio

| Key | Action |
|-----|--------|
| `Up` / `Down` | Audio sensitivity |
| `v` | Toggle spectrum display |

### Panels & Overlays

| Key | Action |
|-----|--------|
| `C` | Open custom charset editor |
| `K` | Toggle Creation Mode (auto-modulation overlay) |
| `S` | Toggle Stem Separation overlay |
| `Ctrl+S` | Save workflow (name + description) |
| `Ctrl+W` | Browse / load saved workflows |
| `o` | Open visual file picker (image / video) |
| `O` | Open audio file picker |

---

## Configuration

Configuration is loaded from `config/default.toml` by default. Presets in `config/presets/` override the default. CLI flags override config files. All fields are optional — unspecified fields use program defaults.

### Minimal Example

```toml
[render]
render_mode = "Ascii"
charset = " .:-=+*#%@"
color_enabled = true
target_fps = 30

[audio]
smoothing = 0.3
sensitivity = 1.5
```

For the complete annotated schema with all parameters, types, ranges, and defaults, see [Reference — TOML Schema](REFERENCE.md#toml-schema).

---

## Creation Mode

Press `K` to enter Creation Mode — an auto-modulation engine that drives all visual effects from audio features in real-time. Eleven presets adapt to audio content:

| Preset | Character |
|--------|-----------|
| Ambient | Slow oscillations, low intensity, drift-based |
| Percussive | Beat-locked strobe, wave, density pulse. Onset-driven. |
| Psychedelic | Fast color pulse from RMS, heavy chromatic, all effects active. |
| Cinematic | Fade/glow dominant, subtle scan lines, controlled dynamics. |
| Minimal | Single dominant effect, clean and focused. |
| Photoreal | Sharpest rendering, subtle audio response. |
| Abstract | Non-figurative cross-mapped effects, density modulation. |
| Glitch | Digital corruption, zalgo dominant, onset invert. |
| Lo-Fi | Vintage degraded, constant scan lines. |
| Spectral | Each frequency band drives a distinct effect. |
| Custom | Auto-modulation disabled — full manual control. |

### Controls (while Creation Mode is active)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Select effect (Master at top, then 9 effects) |
| `Left` / `Right` | Adjust selected element |
| `a` | Toggle auto/manual mode |
| `p` | Cycle preset |
| `Esc` | Hide overlay (modulation continues) |
| `q` | Fully deactivate Creation Mode |

Header shows `[AUTO]` (green) or `[MANUAL]` (red). The sidebar shows `K●` when active, `K○` when inactive.

---

## Stem Separation

Press `S` to open the Stem Separation overlay — a music source separation engine that splits audio into 4 independent stems (Drums, Bass, Other, Vocals) using SCNet. Each stem has independent audio-reactive analysis (FFT, beat detection, MFCC) and per-stem playback control.

### Prerequisites

- Python 3.10+ with `uv` package manager
- SCNet model checkpoint at `ext/SCNet/models/SCNet.th`
- Run `scripts/setup_stems.bat` (Windows) or `scripts/setup_stems.sh` (Linux/macOS) to install dependencies

### Controls (while Stem overlay is active)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Select stem |
| `m` | Toggle mute on selected stem |
| `s` | Toggle solo on selected stem |
| `Left` / `Right` | Adjust volume (selected stem) |
| `v` | Toggle spectrum visibility |
| `c` | Clear all solo |
| `Enter` | Start separation (requires loaded audio) |
| `Esc` | Close overlay |

### Notes

- Separation runs on CPU — expect ~30–60s for a 3-minute track (standard model).
- Progress is displayed in the overlay during separation.
- Mute/solo/volume changes are lock-free and take effect immediately.
- Per-stem audio features feed the existing mapping pipeline, so all visual effects react to the active stems.

---

## Workflow Save / Load

Workflows capture the complete state of a session — configuration, source info, stem separation results, and audio analysis — for perfect reproducibility.

### Save (TUI)

Press `Ctrl+S` to open the save overlay. Enter a name (auto-suggested from audio/visual file), optionally add a description with `Tab`, then press `Enter` to save.

### Browse / Load (TUI)

Press `Ctrl+W` to open the browse overlay. Navigate with `Up`/`Down`, press `Enter` to load, `Delete` to remove. Tags `[S]` and `[T]` indicate stem data and timeline presence.

### CLI Usage

```bash
# Save workflow after batch export
classcii --batch-folder ./media/ --audio track.mp3 --save-workflow "my_export"

# Load a saved workflow
classcii --load-workflow workflows/my_export

# List all workflows
classcii --workflow-list
```

### Directory Layout

```
workflows/<name>/
├── manifest.toml       # Version, timestamps, flags
├── config.toml         # Full RenderConfig snapshot
├── source.toml         # Source path, media type, audio path
├── stems/              # (optional)
│   ├── drums.wav       # Mono f32 IEEE float
│   ├── bass.wav
│   ├── other.wav
│   ├── vocals.wav
│   ├── states.toml     # Mute/solo/volume per stem
│   └── metadata.toml   # Sample rate, duration, model info
└── timeline.bin        # (optional) Bincode-serialized FeatureTimeline
```

### Notes

- Stem WAVs are written as mono f32 IEEE float (zero-dep encoder).
- `timeline.bin` enables deterministic replay — same visual output without re-analyzing audio.
- Workflows are stored relative to the executable directory.
- `--load-workflow` overrides `--config`, `--preset`, and `--audio`.

---

## Batch Export

Headless mode that scans a media folder, pre-analyzes audio, and renders a fully audio-reactive ASCII-art MP4 — frame-accurate, offline, zero dropped frames.

```bash
# Minimal — auto-discovers audio, auto-names output
classcii --batch-folder ./media/

# Full control
classcii --batch-folder ./media/ --audio track.mp3 --batch-out output.mp4 --fps 60

# Multi-preset mode with transitions
classcii --batch-folder ./media/ --audio track.mp3 --preset all

# Reproducible with custom mutation
classcii --batch-folder ./media/ --preset all --seed 42 --mutation-intensity 0.5
```

### Pipeline

1. **Discovery**: Scans folder for images (PNG, JPG, GIF) and videos (MP4, MKV, etc.). Audio auto-discovered if not specified.
2. **Audio Analysis**: Full offline FFT → bass-weighted spectral flux, BeatDetector-parity onset detection (warmup skip, FPS-adaptive cooldown, BPM estimation), feature normalization → `FeatureTimeline`.
3. **Energy Classification**: Sliding-window RMS (5-second) with 30th/70th percentile thresholds → 3 levels (low/medium/high) driving clip pacing and mutation frequency.
4. **Generative Mapping**: `AutoGenerativeMapper` modulates `RenderConfig` per frame.
5. **Clip Sequencing**: Energy-based clip budget — high energy = shorter clips (50%), low energy = longer clips (150%). Crossfade transitions between clips.
6. **Macro Director**: Mutation coordination with cooldown (90 frames), max 2 per event, energy-scaled probabilities. Priority-ordered: mode cycling → charset rotation → effect burst → density pulse → color mode → invert flash.
7. **Compositing**: Source pixels → `AsciiGrid` via bitmasking and dithering.
8. **Rasterization**: `AsciiGrid` → high-resolution RGBA pixels (parallel, zero-alloc, alpha-blended Zalgo).
9. **Encoding**: Lossless `libx264rgb` CRF 0 / rgb24 — zero chroma subsampling.
10. **Muxing**: Final audio+video mux via FFmpeg.

All 8 post-processing effects and all 21 audio source mappings operate in batch mode, achieving full parity with interactive rendering.

### Output Format

- Codec: libx264rgb lossless (CRF 0)
- Pixel format: rgb24 (no chroma subsampling)
- Audio: 320 kbps AAC
- Resolution: determined by font metrics and grid size

---

## Terminal Selection

| Terminal | GPU Accel | Unicode | Zalgo | Notes |
|----------|-----------|---------|-------|-------|
| **WezTerm** | Yes | Excellent | Good | Best overall. Cross-platform. |
| **Alacritty** | Yes | Excellent | Fair | Fastest raw rendering. |
| **Kitty** | Yes | Excellent | Good | Good Unicode. Linux/macOS. |
| **Windows Terminal** | Yes | Good | Fair | Default on Windows 11. |
| **iTerm2** | Partial | Good | Fair | macOS only. |
| **xterm / cmd.exe** | No | Poor | No | Not recommended. |

Sextant (U+1FB00) and Octant (U+1CD00) require fonts with coverage: FiraCode, JetBrains Mono, Cascadia Code.

## FPS Optimization

1. **Reduce terminal size** — fewer cells = fewer pixels.
2. **Use `--fps 30`** — halves rendering workload.
3. **Lower `density_scale`** — `0.5` renders at half resolution.
4. **Disable shape matching** (`s`) — ~3x slower than luminance mapping.
5. **Use Ascii mode** — simpler than Braille/Sextant/Octant.
6. **Disable dithering** (`n` → Off) — minor improvement.
7. **Reduce effects** — chromatic and wave scan neighboring cells.

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "ffmpeg not found" | Install FFmpeg and ensure it's in your `PATH` |
| Video stuttering | Use a GPU-accelerated terminal (Alacritty, WezTerm) |
| No audio reactivity | Check `--audio mic` or provide a valid audio file path |
| Colors look wrong | Cycle color mode with `m` or toggle color with `c` |
| Low framerate | Reduce terminal size, use `--fps 30`, lower `density_scale` |
| "Terminal too small" | Resize terminal to at least 80x20 |
| Batch export fails | Ensure source folder contains media files; check FFmpeg in PATH |
| Effects not visible | Color must be enabled (`c`) for chromatic, pulse, glow |
| Keys not responding | Close any active overlay first (`Esc`) |
| Creation Mode not modulating | Ensure preset is not Custom; check `K●` in sidebar |
| Stem separation fails | Run `scripts/setup_stems.bat` (or `.sh`), check `ext/SCNet/models/SCNet.th` exists |
| "Python not found" | Install Python 3.10+ and run the setup script |
