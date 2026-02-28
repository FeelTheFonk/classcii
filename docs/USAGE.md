# Usage Guide

Complete reference for classcii v0.5.3 — real-time audio-reactive ASCII/Unicode rendering engine.

## Quick Start

### Prerequisites

- **Rust 1.88+** (Edition 2024)
- **FFmpeg + FFprobe** in `PATH` (required for video sources and batch export)
  - Windows: `winget install ffmpeg`
  - Linux: `sudo apt install ffmpeg`
  - macOS: `brew install ffmpeg`
- **GPU-accelerated terminal** recommended for real-time mode (Alacritty, WezTerm, Kitty)

### Installation

```bash
git clone https://github.com/FeelTheFonk/classcii.git
cd classcii
cargo build --release --features video
```

### First Run

```bash
# Static image as ASCII art
classcii --image photo.jpg

# Image with live microphone audio reactivity
classcii --image photo.jpg --audio mic

# Video file with its audio track
classcii --video movie.mp4

# Video with external audio
classcii --video clip.mp4 --audio track.mp3
```

---

## CLI Reference

| Flag | Description | Default |
|------|-------------|---------|
| `--image <PATH>` | Source: static image (PNG, JPEG, BMP, GIF) | — |
| `--video <PATH>` | Source: video file (requires `--features video`) | — |
| `--procedural <TYPE>` | Source: generator (`noise`, `plasma`, `particles`, `starfield`) | — |
| `--audio <PATH\|mic>` | Audio source: file path or `mic` for microphone | — |
| `--batch-folder <DIR>` | Batch export: media folder (images + videos) | — |
| `--batch-out <PATH>` | Batch export: output MP4 file path | auto-named |
| `-c, --config <PATH>` | TOML configuration file | `config/default.toml` |
| `--preset <NAME>` | Load a named preset (overrides `--config`) | — |
| `--mode <MODE>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant`, `sextant`, `octant` | from config |
| `--fps <N>` | Target framerate (30 or 60) | from config |
| `--no-color` | Disable color output | `false` |
| `--log-level <LEVEL>` | Logging: `error`, `warn`, `info`, `debug`, `trace` | `warn` |

### Examples

```bash
# Braille mode at 60fps with neon preset and microphone
classcii --image sunset.png --audio mic --mode braille --fps 60 --preset 07_neon_abyss

# Procedural animation with live audio
classcii --procedural plasma --audio mic

# Batch export with preset
classcii --batch-folder ./media/ --audio track.mp3 --fps 60 --preset 02_matrix

# Auto-discovery batch (finds audio in folder, auto-names output)
classcii --batch-folder ./my_media/
```

---

## Keyboard Controls

All controls are available in real-time TUI mode. Press `?` to show the in-app help overlay.

### Navigation & Playback

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit application or close active overlay |
| `?` | Toggle help overlay |
| `Space` | Pause / Resume |
| `Left` / `Right` | Seek video/audio stream |

### Render Mode & Display

| Key | Action |
|-----|--------|
| `Tab` | Cycle render mode: Ascii → HalfBlock → Braille → Quadrant → Sextant → Octant |
| `1`–`0` | Select built-in charset (1 = Full, 2 = Dense, ..., 0 = Digital) |
| `c` | Toggle color output |
| `i` | Invert luminance |
| `m` | Cycle color mode: Direct → HsvBright → Oklab → Quantized |
| `b` | Cycle background style: Black → SourceDim → Transparent |
| `n` | Cycle dither mode: Bayer8x8 → BlueNoise16 → Off |
| `a` | Toggle aspect ratio correction |
| `x` | Toggle fullscreen (hide sidebar and spectrum) |
| `p` / `P` | Cycle preset (forward / backward) |

### Render Parameters

| Key | Action | Range |
|-----|--------|-------|
| `d` / `D` | Density scale −/+ 0.25 | 0.25 – 4.0 |
| `e` / `E` | Toggle edge detection / adjust edge mix | 0.0 – 1.0 |
| `s` | Toggle shape matching | on/off |
| `[` / `]` | Contrast −/+ 0.05 | 0.1 – 3.0 |
| `{` / `}` | Brightness −/+ 0.05 | −1.0 – 1.0 |
| `-` / `+` | Saturation −/+ 0.1 | 0.0 – 3.0 |

### Effect Parameters

| Key | Action | Range |
|-----|--------|-------|
| `f` / `F` | Fade decay −/+ 0.01 | 0.0 – 1.0 |
| `g` / `G` | Glow intensity −/+ 0.1 | 0.0 – 2.0 |
| `r` / `R` | Chromatic aberration −/+ 0.5 | 0.0 – 5.0 |
| `w` / `W` | Wave amplitude −/+ 0.05 | 0.0 – 1.0 |
| `h` / `H` | Color pulse speed −/+ 0.5 | 0.0 – 5.0 |
| `l` / `L` | Scan line gap −/+ 1 | 0 – 8 |
| `t` / `T` | Beat flash intensity −/+ 0.1 | 0.0 – 2.0 |
| `z` / `Z` | Zalgo intensity −/+ 0.5 | 0.0 – 5.0 |
| `y` / `Y` | Temporal stability −/+ 0.1 | 0.0 – 1.0 |
| `j` / `J` | Strobe decay −/+ 0.05 | 0.5 – 0.99 |
| `u` / `U` | Wave speed −/+ 0.5 | 0.0 – 10.0 |

### Audio

| Key | Action |
|-----|--------|
| `Up` / `Down` | Audio sensitivity ± 0.1 |
| `v` | Toggle spectrum display |

### Panels & Overlays

| Key | Action |
|-----|--------|
| `C` | Open custom charset editor |
| `A` | Open audio reactivity mixer panel |
| `K` | Toggle Creation Mode (auto-modulation overlay) |
| `o` | Open visual file picker (image / video) |
| `O` | Open audio file picker |

---

## Audio Reactivity

classcii maps audio features to visual parameters in real-time. Mappings are defined in TOML config files under `[[audio.mappings]]`.

### 19 Audio Sources

| Source | Range | Description |
|--------|-------|-------------|
| `rms` | 0.0–1.0 | Root Mean Square amplitude (overall loudness) |
| `peak` | 0.0–1.0 | Peak amplitude |
| `sub_bass` | 0.0–1.0 | 20–60 Hz energy |
| `bass` | 0.0–1.0 | 60–250 Hz energy |
| `low_mid` | 0.0–1.0 | 250–500 Hz energy |
| `mid` | 0.0–1.0 | 500–2000 Hz energy |
| `high_mid` | 0.0–1.0 | 2000–4000 Hz energy |
| `presence` | 0.0–1.0 | 4000–6000 Hz energy |
| `brilliance` | 0.0–1.0 | 6000–20000 Hz energy |
| `spectral_centroid` | 0.0–1.0 | Frequency center of mass (timbral brightness) |
| `spectral_flux` | 0.0–1.0 | Frame-to-frame spectral change (transient energy) |
| `spectral_flatness` | 0.0–1.0 | Noise vs tonal content (1.0 = white noise) |
| `beat_intensity` | 0.0–1.0 | Onset/beat strength |
| `onset` | 0 or 1 | Beat/transient detected (binary trigger) |
| `beat_phase` | 0.0–1.0 | Position within current beat cycle |
| `bpm` | normalized | Estimated BPM / 200 |
| `timbral_brightness` | 0.0–1.0 | MFCC-derived brightness (high-frequency timbre) |
| `timbral_roughness` | 0.0–1.0 | MFCC-derived roughness (spectral irregularity) |
| `onset_envelope` | 0.0–1.0 | Exponential decay envelope from last onset |

### 14 Mapping Targets

| Target | Range | Description |
|--------|-------|-------------|
| `edge_threshold` | 0.0–1.0 | Edge detection sensitivity |
| `edge_mix` | 0.0–1.0 | Edge vs fill blend |
| `contrast` | 0.1–3.0 | Luminance contrast |
| `brightness` | −1.0–1.0 | Luminance offset |
| `saturation` | 0.0–3.0 | Color saturation |
| `density_scale` | 0.25–4.0 | Character density multiplier |
| `invert` | toggle | Flip luminance when delta > 0.5 |
| `beat_flash_intensity` | 0.0–2.0 | Strobe envelope intensity |
| `chromatic_offset` | 0.0–5.0 | R/B channel displacement |
| `wave_amplitude` | 0.0–1.0 | Sinusoidal row shift strength |
| `color_pulse_speed` | 0.0–5.0 | HSV hue rotation speed |
| `fade_decay` | 0.0–1.0 | Temporal persistence |
| `glow_intensity` | 0.0–2.0 | Brightness bloom |
| `zalgo_intensity` | 0.0–1.0 | Zalgo diacritics distortion |

### 4 Mapping Curves

| Curve | Formula | Use Case |
|-------|---------|----------|
| `Linear` | `y = x` | Direct proportional response |
| `Exponential` | `y = x²` | Suppress low values, amplify peaks |
| `Threshold` | `y = 0 if x < 0.3, else (x−0.3)/0.7` | Gate — only react above threshold |
| `Smooth` | `y = 3x² − 2x³` | Smoothstep — gentle transitions |

### Mapping Configuration

```toml
[[audio.mappings]]
enabled = true
source = "bass"                # Audio source (one of 19)
target = "wave_amplitude"      # Visual target (one of 14)
amount = 0.4                   # Multiplier
offset = 0.0                   # Additive offset after multiplication
curve = "Smooth"               # Response curve (Linear/Exponential/Threshold/Smooth)
smoothing = 0.3                # Per-mapping EMA override (optional, defaults to global)
```

Multiple mappings can be active simultaneously. The global `audio_smoothing` applies to all mappings unless overridden by per-mapping `smoothing`.

---

## Post-Processing Effects

8 composable effects applied in a fixed pipeline order each frame:

| # | Effect | Key | Parameter | Range | Default |
|---|--------|-----|-----------|-------|---------|
| 1 | Temporal Stability | `y/Y` | `temporal_stability` | 0.0–1.0 | 0.0 |
| 2 | Wave Distortion | `w/W` (amplitude), `u/U` (speed) | `wave_amplitude`, `wave_speed` | 0.0–1.0, 0.0–10.0 | 0.0, 2.0 |
| 3 | Chromatic Aberration | `r/R` | `chromatic_offset` | 0.0–5.0 | 0.0 |
| 4 | Color Pulse | `h/H` | `color_pulse_speed` | 0.0–5.0 | 0.0 |
| 5 | Fade Trails | `f/F` | `fade_decay` | 0.0–1.0 | 0.3 |
| 6 | Strobe | `t/T` (intensity), `j/J` (decay) | `beat_flash_intensity`, `strobe_decay` | 0.0–2.0, 0.5–0.99 | 0.3, 0.75 |
| 7 | Scan Lines | `l/L` | `scanline_gap` | 0–8 | 0 |
| 8 | Glow | `g/G` | `glow_intensity` | 0.0–2.0 | 0.5 |

Pipeline order matters: Temporal Stability reduces flicker first, then effects layer progressively, with Glow applied last as a brightness bloom.

---

## Creation Mode

Press `K` to enter Creation Mode — an auto-modulation engine that drives all visual effects from audio features in real-time.

### 5 Presets

| Preset | Character |
|--------|-----------|
| **Ambient** | Smooth breath-like oscillations driven by RMS and spectral centroid. Low intensity. |
| **Percussive** | Beat-locked strobe, chromatic aberration, wave on hits. Onset-driven. |
| **Psychedelic** | Fast color pulse from RMS, heavy chromatic from spectral flux, all effects active. |
| **Cinematic** | Fade/glow dominant, subtle scan lines, controlled dynamics. Film-like. |
| **Custom** | Auto-modulation disabled — full manual control over all effects. |

### Controls (while Creation Mode overlay is visible)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Select effect |
| `Left` / `Right` | Adjust master intensity (auto mode) or selected effect value (manual mode) |
| `a` | Toggle auto/manual mode |
| `p` | Cycle preset |
| `Esc` | Hide overlay (modulation continues) |
| `q` | Fully deactivate Creation Mode |

The sidebar shows `K●` when Creation Mode is active (modulating) and `K○` when inactive.

---

## Presets

11 presets in `config/presets/`, selectable via `--preset <name>` or cycled live with `p`/`P`:

| Preset | Render Mode | Style |
|--------|-------------|-------|
| `01_cyber_braille` | Braille | High contrast cyberpunk, neon glow, chromatic aberration, scan lines |
| `02_matrix` | Ascii | Classic Matrix digital rain aesthetic |
| `03_ghost_edge` | Ascii | Edge detection with spectral fade trails |
| `04_pure_ascii` | Ascii | Clean ASCII gradient, minimal effects |
| `05_classic_gradient` | Ascii | Standard luminance gradient mapping |
| `06_vector_edges` | Ascii | Edge-dominant, vector-style rendering |
| `07_neon_abyss` | Ascii | Neon colors, deep glow, high saturation, timbral mapping |
| `08_cyber_noise` | Ascii | Glitch-heavy, noise-driven visuals |
| `09_brutalism_mono` | Ascii | Monochrome, high contrast brutalist style |
| `10_ethereal_shape` | Ascii | Shape matching, soft ethereal aesthetics |
| `11_reactive` | Ascii | All effects showcase at moderate levels, 4 audio mappings |

```bash
classcii --image photo.jpg --preset 07_neon_abyss
classcii --video clip.mp4 --preset 02_matrix --audio mic
```

---

## Configuration (TOML)

Configuration is loaded from `config/default.toml` by default, or from a custom path via `--config`. Presets in `config/presets/` override the default config. All fields are optional — unspecified fields keep their defaults.

### Minimal Config

```toml
[render]
render_mode = "Ascii"
charset = " .:-=+*#%@"
color_enabled = true
target_fps = 30

[audio]
smoothing = 0.3
sensitivity = 1.0
```

### Full Schema

See [TOML_SCHEMA.md](TOML_SCHEMA.md) for the complete annotated schema with all 32+ parameters, types, ranges, and defaults.

---

## Batch Export

Headless mode that scans a media folder, pre-analyzes audio, and renders a fully audio-reactive ASCII-art MP4 — frame-accurate, offline, with zero dropped frames.

```bash
# Minimal — auto-discovers audio, auto-names output
classcii --batch-folder ./media/

# Full control
classcii --batch-folder ./media/ --audio track.mp3 --batch-out output.mp4 --fps 60 --preset 02_matrix
```

### Pipeline

1. **Discovery**: Scans folder for images (PNG, JPG) and videos (MP4, MKV, etc.). Audio auto-discovered if not specified.
2. **Audio Analysis**: Full offline FFT, spectral features, onset detection → `FeatureTimeline`.
3. **Generative Mapping**: `AutoGenerativeMapper` modulates `RenderConfig` per frame from the timeline.
4. **Macro Director**: On strong beats, triggers structural variations — mode cycling, invert flashes, charset rotations.
5. **Compositing**: Source pixels → `AsciiGrid` via advanced bitmasking and dithering.
6. **Rasterization**: `AsciiGrid` → high-resolution RGBA pixels (parallel, zero-alloc, alpha-blended Zalgo).
7. **Encoding**: Lossless x264 CRF 0 / YUV444p.
8. **Muxing**: Final audio+video mux via FFmpeg.

All 8 post-processing effects and all 19 audio source mappings are applied in the batch pipeline, achieving full parity with interactive mode.

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "ffmpeg not found" | Install FFmpeg and ensure it's in your `PATH` |
| Video stuttering | Use a GPU-accelerated terminal (Alacritty, WezTerm) |
| No audio reactivity | Check `--audio mic` or provide a valid audio file path |
| Colors look wrong | Try cycling color mode with `m` or toggle color with `c` |
| Low framerate | Reduce terminal size, use `--fps 30`, or lower `density_scale` |
| "Terminal too small" | Resize terminal to at least 80x20 |
| Batch export fails | Ensure source folder contains media files; check FFmpeg is in PATH |
| Effects not visible | Color must be enabled (`c`) for chromatic, pulse, glow effects |
| Keys not responding | Close any active overlay first (`Esc`) — overlays capture input |
| Creation Mode not modulating | Ensure preset is not Custom; check `K●` indicator in sidebar |
