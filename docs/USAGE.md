# clasSCII — User Guide

## Quick Start

### Installation

```bash
git clone <repo>
cd asciiforge
cargo build --release --features video
```

Ensure `ffmpeg` and `ffprobe` are in your system `PATH`:
- **Windows**: `winget install ffmpeg`
- **Linux**: `sudo apt install ffmpeg`
- **macOS**: `brew install ffmpeg`

### First Run

```bash
# Display an image as ASCII art
classcii --image photo.jpg

# With microphone audio reactivity
classcii --image photo.jpg --audio mic

# Play a video as ASCII art with its audio track
classcii --video movie.mp4
```

---

## Commands Reference

### Source Selection (mutually exclusive)

| Command | Description |
|---------|-------------|
| `--image <file>` | Load a static image (PNG, JPEG, BMP, GIF) |
| `--video <file>` | Load a video file (MP4, MKV, AVI, MOV, WEBM) |
| `--webcam` | Use the default webcam |
| `--procedural <type>` | Use a procedural generator: `noise`, `plasma`, `particles`, `starfield` |
| `--batch-folder <dir>` | Batch export mode: process all media in a folder |

### Audio

| Command | Description |
|---------|-------------|
| `--audio mic` | Use the system microphone for live audio reactivity |
| `--audio <file>` | Play an audio file (MP3, FLAC, WAV, OGG, AAC) and react to it |

### Render Options

| Command | Description | Default |
|---------|-------------|---------|
| `--mode <mode>` | Render mode: `ascii`, `halfblock`, `braille`, `quadrant` | config |
| `--fps <n>` | Target framerate: `30` or `60` | config |
| `--no-color` | Disable color output | off |
| `--preset <name>` | Load a preset: `ambient`, `aggressive`, `minimal`, `retro`, `psychedelic` | — |
| `-c, --config <file>` | Custom TOML config file | `config/default.toml` |
| `--log-level <level>` | Logging: `error`, `warn`, `info`, `debug`, `trace` | `warn` |

### Batch Export

| Command | Description |
|---------|-------------|
| `--batch-folder <dir>` | Source folder with images and/or videos |
| `--batch-out <file>` | Output MP4 file path (Optional: auto-named) |
| `--audio <file>` | Audio track (Optional: auto-discovered in folder) |
| `--fps <n>` | Export framerate (default: 30) |

---

## Usage Examples

### Real-Time TUI

```bash
# Basic image display
classcii --image sunset.png

# Braille mode at 60fps with psychedelic preset and microphone
classcii --image sunset.png --audio mic --mode braille --fps 60 --preset psychedelic

# Video with external audio track
classcii --video timelapse.mp4 --audio ambient.mp3

# Procedural animation
classcii --procedural plasma --audio mic
```

### Batch Export (Headless)

Generate a fully audio-reactive ASCII-art video from a folder of media files:

```bash
# Basic batch export
classcii --batch-folder ./photos/ --audio deep_house.mp3 --batch-out render.mp4

# Fully automatic (auto-discovers audio in folder, auto-generates output name)
classcii --batch-folder ./my_media/ 

# 60fps with aggressive preset
classcii --batch-folder ./media/ --audio track.flac --fps 60 --preset aggressive
```

**How it works (Generative Clip Maker):**
1. All images (PNG, JPG) and videos (MP4, MKV, etc.) in the folder are discovered recursively. If no `--audio` is provided, a compatible audio track inside the folder is automatically detected.
2. The audio file is fully pre-analyzed: FFT, spectral features, onset detection.
3. The `AutoGenerativeMapper` modulates visual parameters per frame based on the audio analysis and your `audio_mappings` config.
4. **Macro-mutations**: On intense beats (onset + high beat intensity), the engine acts as an algorithmic director: it transitions media, cycles `RenderMode`, triggers negative flashes, or rotates character matrices to create true experimental clips.
5. Each frame is composited to ASCII, then rasterized to high-resolution pixels using FiraCode font.
6. Encoded as lossless x264 (CRF 0, YUV444p) and muxed with the audio track.

**Output:** A `.mp4` file at maximum visual quality (lossless video, 320kbps AAC audio).

---

## Keyboard Controls (TUI Mode)

### Navigation
| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit or close active overlay |
| `?` | Toggle help menu |
| `Space` | Pause / Resume |
| `←` / `→` | Seek video stream |

### Render Mode
| Key | Action |
|-----|--------|
| `Tab` | Cycle: Ascii → HalfBlock → Braille → Quadrant |
| `1`–`0` | Select built-in charset |
| `c` | Toggle color |
| `i` | Invert luminance |
| `m` | Cycle color mode |
| `b` | Cycle background style |
| `x` | Toggle fullscreen |

### Visual Parameters
| Key | Action |
|-----|--------|
| `e` / `E` | Toggle edge detection / Adjust edge mix |
| `s` | Toggle shape matching |
| `d` / `D` | Density scale ±0.25 |
| `[` / `]` | Contrast ±0.05 |
| `{` / `}` | Brightness ±0.05 |
| `-` / `+` | Saturation ±0.1 |

### Effects
| Key | Action |
|-----|--------|
| `f` / `F` | Fade decay ±0.01 |
| `g` / `G` | Glow amplitude ±0.1 |

### Audio
| Key | Action |
|-----|--------|
| `↑` / `↓` | Audio sensitivity ±0.1 |

### Panels
| Key | Action |
|-----|--------|
| `C` | Custom charset editor |
| `A` | Audio reactivity mixer |
| `o` / `O` | Open popup menu: [F]ile or [D]irectory (Batch Export) |
| `p` / `P` | Cycle preset |

---

## Configuration (TOML)

### Minimal Config

```toml
render_mode = "ascii"
charset = " .:-=+*#%@"
invert = false
color_enabled = true
edge_threshold = 0.3
edge_mix = 0.5
shape_matching = false
contrast = 1.0
brightness = 0.0
saturation = 1.0
target_fps = 30
audio_smoothing = 0.3
audio_sensitivity = 1.0
```

### Audio Mappings

Control which audio features drive which visual parameters:

```toml
[[audio_mappings]]
source = "bass"
target = "contrast"
amount = 0.5
offset = 0.0
enabled = true

[[audio_mappings]]
source = "spectral_flux"
target = "edge_threshold"
amount = 1.0
offset = 0.0
enabled = true

[[audio_mappings]]
source = "onset"
target = "invert"
amount = 1.0
offset = 0.0
enabled = true
```

### Available Sources

| Source | Range | Description |
|--------|-------|-------------|
| `rms` | 0.0–1.0 | Root Mean Square amplitude |
| `peak` | 0.0–1.0 | Peak amplitude |
| `sub_bass` | 0.0–1.0 | 20–60 Hz energy |
| `bass` | 0.0–1.0 | 60–250 Hz energy |
| `low_mid` | 0.0–1.0 | 250–500 Hz energy |
| `mid` | 0.0–1.0 | 500–2000 Hz energy |
| `high_mid` | 0.0–1.0 | 2000–4000 Hz energy |
| `presence` | 0.0–1.0 | 4000–6000 Hz energy |
| `brilliance` | 0.0–1.0 | 6000–20000 Hz energy |
| `spectral_centroid` | 0.0–1.0 | Timbral brightness |
| `spectral_flux` | 0.0–1.0 | Frame-to-frame spectral change |
| `spectral_flatness` | 0.0–1.0 | Noise vs tonal |
| `onset` | 0 or 1 | Beat/attack detected |
| `beat_intensity` | 0.0–1.0 | Onset strength |
| `beat_phase` | 0.0–1.0 | Position between beats |
| `bpm` | normalized | Estimated BPM / 200 |

### Available Targets

| Target | Range | Description |
|--------|-------|-------------|
| `edge_threshold` | 0.0–1.0 | Edge detection sensitivity |
| `edge_mix` | 0.0–1.0 | Edge vs fill blend |
| `contrast` | 0.0–2.0 | Luminance contrast |
| `brightness` | -1.0–1.0 | Luminance offset |
| `saturation` | 0.0–2.0 | Color saturation |
| `density_scale` | 0.25–4.0 | Character density |
| `invert` | toggle | Flip luminance if > 0.5 |

---

## Presets

| Preset | Style |
|--------|-------|
| `ambient` | Smooth, low reactivity, soft gradients |
| `aggressive` | High contrast, rapid onset response, sharp edges |
| `minimal` | Simple ASCII, minimal effects, clean look |
| `retro` | Classic terminal aesthetics, green-on-black feel |
| `psychedelic` | Maximum saturation, wild color cycling, intense glow |

```bash
classcii --image photo.jpg --preset psychedelic
classcii --video clip.mp4 --preset aggressive --audio mic
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "ffmpeg not found" | Install FFmpeg and ensure it's in your `PATH` |
| Video stuttering | Use a GPU-accelerated terminal (Alacritty, WezTerm) |
| No audio reactivity | Check `--audio mic` or provide a valid audio file path |
| Batch export fails | Ensure `--audio` and `--batch-out` are both specified |
| Colors look wrong | Try a different `--mode` or toggle with `c` key |
| Low framerate | Reduce terminal size or use `--fps 30` |
