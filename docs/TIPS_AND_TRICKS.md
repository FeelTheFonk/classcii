# Tips and Tricks

Practical advice for getting the best results with classcii.

## Terminal Selection

The terminal emulator directly affects rendering quality and performance.

| Terminal | GPU Accel | Unicode | Zalgo | Notes |
|----------|-----------|---------|-------|-------|
| **WezTerm** | Yes | Excellent | Good | Best overall: fast, correct Unicode, good Zalgo stacking. Cross-platform. |
| **Alacritty** | Yes | Excellent | Fair | Fastest raw rendering. Limited combining character stacking. |
| **Kitty** | Yes | Excellent | Good | Image protocol support, good Unicode. Linux/macOS. |
| **Windows Terminal** | Yes | Good | Fair | Default on Windows 11. Adequate for most use cases. |
| **iTerm2** | Partial | Good | Fair | macOS only. Acceptable performance. |
| **xterm / cmd.exe** | No | Poor | No | Not recommended. Missing Unicode blocks, slow rendering. |

For Sextant (U+1FB00) and Octant (U+1CD00) render modes, the terminal must support these Unicode blocks and have a font that includes them. FiraCode, JetBrains Mono, and Cascadia Code have good coverage.

## FPS Optimization

If framerate is low:

1. **Reduce terminal size** — fewer cells = fewer pixels to process. Half the width = roughly 4× fewer cells.
2. **Use `--fps 30`** — halves the rendering workload.
3. **Lower `density_scale`** — `0.5` renders at half resolution. Start here before reducing terminal size.
4. **Disable shape matching** — press `s` to toggle off. Shape matching is ~3× slower than simple luminance mapping.
5. **Use Ascii mode** — simpler than Braille/Quadrant/Sextant/Octant which compute sub-pixel patterns.
6. **Disable dithering** — press `n` to cycle to Off. Minor improvement.
7. **Reduce effects** — chromatic aberration and wave distortion scan neighboring cells. Disable them if needed.

## Content-Specific Configurations

### Portraits and Photos

```bash
classcii --image portrait.jpg --mode ascii
```

Best settings:
- **Charset**: Full (key `1`) — 70 levels of gray = smooth gradients
- **Color mode**: HsvBright (key `m`) — preserves skin tones while encoding luminance in characters
- **Edge**: threshold 0.3, mix 0.5 — reveals facial features without overwhelming
- **Dither**: BlueNoise16 (key `n`) — reduces banding in gradients
- **Effects**: minimal. Glow 0.3 for warmth. No wave, no chromatic.

### Music Videos

```bash
classcii --video clip.mp4 --audio mic --preset 01_cyber_braille
```

Best settings:
- **Mode**: Braille — highest spatial resolution
- **Dither**: BlueNoise16
- **Creation Mode**: Press `K`, select Psychedelic or Percussive
- **Effects**: chromatic 1.5, wave 0.2, color pulse 1.0, fade 0.4
- **Audio**: High sensitivity (1.5+), low smoothing (0.2–0.3) for responsive visuals

### Films and Slow Content

```bash
classcii --video film.mp4 --preset 03_ghost_edge
```

Best settings:
- **Mode**: Quadrant or Sextant — good detail without character noise
- **Temporal stability**: 0.3–0.5 — eliminates per-frame flicker
- **Creation Mode**: Cinematic preset
- **Effects**: fade 0.5, glow 0.7, subtle chromatic 0.5
- **Color**: Oklab for perceptual accuracy

### Abstract / Generative Art

```bash
classcii --procedural plasma --audio mic --mode octant
```

Best settings:
- **Mode**: Octant — maximum sub-pixel resolution (2×4 per cell)
- **Shape matching**: on (`s`)
- **Color mode**: Oklab — perceptually uniform
- **Effects**: all active at moderate levels (use preset `11_reactive`)
- **Background**: Transparent (`b`) for compositing

### High-Contrast Graphic Art

```bash
classcii --image logo.png --mode ascii
```

Best settings:
- **Charset**: Binary (key `5`) — 2-level quantization
- **Color**: disabled (`c`)
- **Contrast**: high (1.5–2.0, keys `]` repeatedly)
- **Edge**: threshold 0.5, mix 1.0
- **Effects**: none — pure structural rendering

## Dithering Guide

### When to Use Each Mode

| Mode | Character | Best For |
|------|-----------|----------|
| **Bayer8x8** (default) | Ordered pattern, visible grid structure | Retro/CRT aesthetic, consistent results |
| **BlueNoise16** | Perceptually random, no visible pattern | Photos, video, anything requiring natural gradients |
| **None** | Hard quantization, visible banding | Posterized style, high-contrast art, performance |

### Bayer vs Blue Noise

Bayer dithering uses a repeating 8×8 matrix — the pattern is visible on close inspection but creates a consistent texture. Blue Noise uses a 16×16 matrix with perceptually randomized thresholds — no visible pattern, but slightly more visual "noise" in uniform areas.

For most photographic content, BlueNoise16 produces better results. For stylistic or retro rendering, Bayer8x8 has a more intentional, artificial look.

## Background Styles

| Style | Effect | Use Case |
|-------|--------|----------|
| **Black** | Pure black behind all characters | Maximum contrast, clean look, default |
| **SourceDim** | Source pixel color at reduced brightness | Immersive — fills gaps with ambient color from the image/video |
| **Transparent** | Terminal default background | Compositing, screenshots, blending with terminal background image |

Press `b` to cycle between them. SourceDim works particularly well with sparse charsets (Binary, Edge) where gaps between characters would otherwise be black.

## Audio Workflow

### Quick Setup

1. Start with `--audio mic` for live reactivity
2. Press `A` to open the Audio Mixer
3. Add/modify mappings interactively
4. Press `v` to show the spectrum analyzer — verify audio is being captured
5. Use `Up`/`Down` to adjust global sensitivity until effects are visible but not overwhelming

### Dial-In Strategy

1. **Start with one mapping**: `rms → brightness` (amount 0.3) — the most universal mapping
2. **Add frequency-specific mappings**: `bass → wave_amplitude`, `spectral_centroid → glow_intensity`
3. **Add beat-driven effects**: `onset_envelope → beat_flash_intensity` with `Smooth` curve
4. **Tune smoothing**: Lower smoothing (0.2–0.3) for beat-driven content, higher (0.7–0.9) for ambient
5. **Save to TOML**: Copy working mappings to a preset file for persistence

### Microphone vs File

- **Microphone** (`--audio mic`): Captures system audio (or mic input). Good for live DJ sets, ambient listening. Latency depends on system audio pipeline.
- **Audio file** (`--audio track.mp3`): Decoded offline with Symphonia. Frame-accurate sync. Supports MP3, FLAC, WAV, OGG, AAC. Best for controlled renders and batch export.

## Batch Export Workflow

### File Organization

```
my_project/
├── photos/
│   ├── img_001.jpg
│   ├── img_002.png
│   └── img_003.bmp
├── videos/
│   └── clip.mp4
└── audio.mp3
```

```bash
# Discovers all media recursively, finds audio.mp3 automatically
classcii --batch-folder ./my_project/
```

### Auto-Naming

When `--batch-out` is omitted, output is named `<folder>_<timestamp>.mp4`.

### Preset for Batch

The preset applies to the entire export. Choose presets with audio mappings for the most dynamic results:

```bash
classcii --batch-folder ./media/ --preset 11_reactive --audio track.mp3 --fps 60
```

### Output Format

- Codec: x264 lossless (CRF 0)
- Color: YUV444p (no chroma subsampling)
- Audio: 320 kbps AAC
- Resolution: determined by font metrics and terminal-equivalent grid size

## Keyboard Cheat Sheet

```
RENDER          EFFECTS              AUDIO/PANELS
Tab  mode       f/F  fade            Up/Dn  sensitivity
1-0  charset    g/G  glow            v      spectrum
c    color      r/R  chromatic       A      mixer panel
i    invert     w/W  wave            K      creation mode
m    color mode h/H  color pulse     C      charset editor
b    bg style   l/L  scan lines      o/O    file picker
n    dither     t/T  strobe
x    fullscreen z/Z  zalgo           NAV
d/D  density    y/Y  stability       Space  pause
e/E  edge       j/J  strobe decay    ←/→    seek
s    shapes     u/U  wave speed      ?      help
[/]  contrast                        q/Esc  quit
{/}  brightness p/P  preset cycle
-/+  saturation a    aspect ratio
```
