# Preset Guide

Complete reference for classcii's 22 built-in presets, with creation tutorials and TOML structure.

## Using Presets

### CLI

```bash
classcii --image photo.jpg --preset 07_neon_abyss
classcii --video clip.mp4 --preset 02_matrix --audio mic
```

### Live Cycling

Press `p` / `P` to cycle forward / backward through all presets in `config/presets/`. The current preset name is shown in the sidebar.

---

## Built-in Presets

### 01_cyber_braille — Cyberpunk Braille Matrix

```
Mode: Braille | Color: HsvBright | Dither: BlueNoise16 | FPS: 60
```

High-contrast cyberpunk aesthetic using Braille characters. Strong glow (1.5), chromatic aberration (1.5), scan lines (gap 3), heavy saturation (1.8). Audio maps bass to brightness (Exponential), onset to zalgo (Threshold), spectral_flux to contrast (Smooth).

### 02_matrix — Digital Rain

```
Mode: Ascii (Edge charset) | Color: HsvBright | Dither: Bayer8x8 | FPS: 60
```

Classic Matrix aesthetic. Heavy fade trails (0.85) create the rain effect. Low brightness (−0.1), high saturation (2.0). Audio maps rms to density_scale, beat_intensity to brightness, spectral_flux to fade_decay.

### 03_ghost_edge — Spectral Edges

```
Mode: Quadrant | Color: Oklab | Dither: BlueNoise16 | FPS: 60 | Inverted
```

Edge detection at full mix (1.0) with Oklab perceptual color. Heavy fade (0.92) creates ghost trails. Inverted luminance for spectral aesthetic. Low saturation (0.8), subtle wave (0.05), slow color pulse (0.3). Audio maps spectral_centroid to edge_mix, timbral_brightness to saturation.

### 04_pure_ascii — Minimal Clean

```
Mode: Ascii (Dense charset) | Color: disabled | Dither: Bayer8x8 | FPS: 30
```

Pure ASCII rendering — no color, no effects, no post-processing. Monochrome gradient only. Single audio mapping: rms to brightness. The baseline reference preset.

### 05_classic_gradient — Standard Rendering

```
Mode: Ascii (Blocks charset) | Color: HsvBright | Dither: BlueNoise16 | FPS: 60
```

Balanced default-like rendering with SourceDim background. Moderate contrast (1.3), standard effects (fade 0.4, glow 0.5, flash 0.5). Audio maps bass to edge, spectral_flux to contrast, rms to brightness (Smooth).

### 06_vector_edges — Wireframe

```
Mode: Ascii (Blocks charset) | Color: disabled | Dither: Bayer8x8 | FPS: 60
```

Monochrome edge-dominant rendering. Edge mix at 1.0, threshold at 0.5. Very heavy fade (0.95) + temporal stability (0.6) = persistent wireframe. High audio sensitivity (4.0). Maps spectral_flux to edge, onset to invert, rms to contrast.

### 07_neon_abyss — Neon Glow

```
Mode: Ascii (custom charset) | Color: Direct | Dither: BlueNoise16 | FPS: 60
```

Full color with edge detection at 1.0 mix. Maximum contrast (2.0), strong glow (0.8), chromatic aberration (1.0), subtle wave (0.1), slow pulse (0.5). Low smoothing (0.2) for responsive reactivity. Uses timbral_roughness to drive contrast — instrument-aware.

### 08_cyber_noise — Glitch Art

```
Mode: Braille (inverted) | Color: Quantized | Dither: BlueNoise16 | FPS: 60
```

Maximum visual chaos. Heavy chromatic (2.5), strong wave (0.3, speed 4.0), fast color pulse (2.0), scan lines (4), high flash (1.5), high density (1.5), extreme saturation (2.5). Quantized color palette. Very low smoothing (0.1), high sensitivity (3.0). Maps spectral_flux to zalgo at 1.5× with Exponential curve.

### 09_brutalism_mono — Monochrome Brutalism

```
Mode: HalfBlock (Unicode blocks charset) | Color: disabled | Dither: Bayer8x8 | FPS: 60
```

Monochrome brutalist aesthetic. High density (2.0), high contrast (1.5), brightened (0.2). Strong glow (1.5), strong flash (1.2). Fullscreen mode (no sidebar). Maps beat_intensity to density with Threshold, onset to invert.

### 10_ethereal_shape — Soft Ethereal

```
Mode: Ascii (custom extended) | Color: Oklab | Dither: BlueNoise16 | FPS: 60
```

Shape matching enabled with Oklab perceptual color. Low density (0.8), brightened (0.3), higher saturation (1.5). Temporal stability (0.6) for smooth output. Transparent background. Maps spectral_centroid to brightness, timbral_brightness to saturation — instrument-responsive ethereal rendering.

### 11_reactive — Effects Showcase

```
Mode: Ascii (Full charset) | Color: Direct | Dither: BlueNoise16 | FPS: 60
```

Demonstrates all 8 visual effects simultaneously at moderate, non-fatiguing levels. Chromatic (1.5), wave (0.25), glow (0.8), color pulse (1.0), scan lines (3), fade (0.4), temporal stability (0.3), zalgo (0.5). Four audio mappings: bass→wave, spectral_centroid→glow, spectral_flux→chromatic, rms→brightness.

### 12_deep_zoom — Camera-Reactive Deep Zoom

```
Mode: Braille | Color: Direct | Dither: BlueNoise16 | FPS: 60
```

Audio-reactive camera preset. Bass drives camera zoom depth (Smooth curve), spectral centroid steers rotation. Glow (0.6), fade (0.5), color pulse (0.5). Four audio mappings targeting camera and visual effects. Works with any image or video source.

### 13_breath — Ultra-Minimalist Contemplative

```
Mode: Ascii (charset " .:░") | Color: disabled | FPS: 30 | Fullscreen
```

Extreme minimalism. A single RMS→brightness mapping (Smooth, 0.4) creates a slow organic breathing effect. High temporal stability (0.7), very high smoothing (0.9). Zero aggressive effects. Silence is space, sound is light.

### 14_interference — Wave Interference Patterns

```
Mode: Braille | Color: Oklab | Dither: BlueNoise16 | FPS: 60
```

High wave speed (7.0) and amplitude (0.4) create moiré-like distortions. Scan lines (3), chromatic (1.0). Bass→wave (Exponential), spectral_flux→wave (Exponential), spectral_flux→chromatic (Smooth). Low smoothing (0.4) for responsive patterns.

### 15_noir — Cinematic Film Noir

```
Mode: HalfBlock | Color: disabled | Shape matching | FPS: 30
```

Monochrome, high contrast (2.0), strong edges (threshold 0.5, mix 0.8). Subtle glow (0.3), fade (0.85), temporal stability (0.4). Three audio mappings: rms→glow, centroid→edge_mix, onset_envelope→brightness. Pure light and shadow drama.

### 16_aurora — Aurora Borealis

```
Mode: Quadrant | Color: Direct | Dither: BlueNoise16 | FPS: 60
```

Saturated colors (1.8), strong glow (0.8), chromatic drift (0.5), wave (0.15), color pulse (0.8). Exploits spectral_rolloff→color_pulse_speed and zero_crossing_rate→wave_amplitude. Audio-driven camera pan via rms→camera_pan_x.

### 17_static — Broken TV / White Noise

```
Mode: Ascii (charset " 01") | Color: Quantized | FPS: 60
```

Binary charset, no fade (0.0), high density (1.5), high contrast (1.8), scan lines (2). Spectral_flatness→density_scale (Linear, 0.8), zero_crossing_rate→zalgo (Threshold, 0.6), onset_envelope→beat_flash (Smooth, 0.4). Very low smoothing (0.2) for intentional instability.

### 18_spectral_bands — Per-Band Frequency Mapping

```
Mode: Quadrant | Color: Oklab | Dither: Bayer8x8 | FPS: 60
```

Each frequency band drives a distinct visual effect, showcasing the full range of audio sources. Sub_bass→wave_amplitude (Smooth), low_mid→fade_decay (Linear), high_mid→chromatic_offset (Smooth), presence→glow_intensity (Linear), brilliance→color_pulse_speed (Smooth). Shape matching enabled, moderate edge detection (0.3), sensitivity 1.3. Every part of the spectrum has a visible, distinct visual consequence.

### 19_cinematic_camera — Audio-Reactive Camera

```
Mode: HalfBlock | Color: Direct | Dither: BlueNoise16 | FPS: 60
```

Camera-focused preset with smooth audio-driven motion. Bass→camera_zoom_amplitude (Smooth, 0.3), spectral_centroid→camera_rotation (Smooth, 0.2), mid→camera_pan_x (Linear, 0.2), presence→camera_pan_y (Linear, 0.15), rms→glow_intensity (Smooth, 0.4). High smoothing (0.7) for cinematic fluidity. Best with large images or video sources.

### 20_sextant_film — Cinematic Sextant

```
Mode: Sextant | Color: Oklab | Dither: BlueNoise16 | FPS: 60
```

Sextant mode (2×3 sub-pixels per cell) delivers high spatial resolution with a filmic, organic aesthetic. Oklab color for perceptual accuracy. Soft edges (threshold 0.2, mix 0.3), cinematic glow (0.4), fade (0.6). Four audio mappings: rms→brightness (Smooth, per-mapping smoothing 0.8 for ultra-smooth response), spectral_centroid→saturation, bass→glow, peak→edge_mix (Threshold). First preset to use `peak` audio source and per-mapping `smoothing` override.

### 21_octant_dense — Maximum Sub-Pixel Density

```
Mode: Octant | Color: Direct | Dither: BlueNoise16 | FPS: 60 | Fullscreen
```

Octant mode (2×4 sub-pixels per cell) pushes spatial density to the limit. CHARSET_GLITCH_2 (spectral bars `▂▃▄▅▆▇█`) complements the dense block aesthetic. High density (2.0), sharp contrast (1.5), temporal stability (0.5) crucial for Octant anti-flicker. Four audio mappings: rms→brightness, spectral_flux→contrast, beat_intensity→beat_flash (Threshold), beat_phase→wave_amplitude. First preset to use `beat_phase` audio source.

### 22_hires_export — Ultra High-Resolution Batch Export

```
Mode: Ascii (CHARSET_FULL 70 chars) | Color: Oklab | Dither: BlueNoise16 | FPS: 30
```

Optimized for batch export with `--export-scale 24-48`. CHARSET_FULL provides the finest tonal gradient (70 characters). Oklab for maximum perceptual fidelity. Subtle effects only — no saturation at high resolution. Four audio mappings: rms→brightness (Smooth), spectral_centroid→saturation (Smooth), bass→contrast (Linear, offset 0.8 — always-elevated baseline), spectral_flux→glow (Smooth). First preset to use `offset` in audio mappings.

---

## Creating a Custom Preset

### TOML Structure

Every preset is a TOML file with two sections: `[render]` and `[audio]`.

```toml
# config/presets/my_preset.toml

[render]
# Render mode (required)
render_mode = "Ascii"          # Ascii, Braille, HalfBlock, Quadrant, Sextant, Octant

# Charset (Ascii mode only)
charset_index = 0              # 0-9 built-in index
charset = " .:-=+*#%@"        # Custom charset string (overrides index)

# Color
color_enabled = true
color_mode = "HsvBright"       # Direct, HsvBright, Oklab, Quantized
dither_mode = "BlueNoise16"    # Bayer8x8, BlueNoise16, None

# Image processing
density_scale = 1.0            # 0.25–4.0
invert = false
contrast = 1.0                 # 0.1–3.0
brightness = 0.0               # -1.0–1.0
saturation = 1.2               # 0.0–3.0
edge_threshold = 0.3           # 0.0–1.0 (0 = off)
edge_mix = 0.5                 # 0.0–1.0
shape_matching = false
aspect_ratio = 2.0
bg_style = "Black"             # Black, SourceDim, Transparent

# Effects
fade_decay = 0.3               # 0.0–1.0
glow_intensity = 0.5           # 0.0–2.0
beat_flash_intensity = 0.3     # 0.0–2.0
chromatic_offset = 0.0         # 0.0–5.0
wave_amplitude = 0.0           # 0.0–1.0
wave_speed = 2.0               # 0.0–10.0
color_pulse_speed = 0.0        # 0.0–5.0
scanline_gap = 0               # 0–8
strobe_decay = 0.75            # 0.5–0.99
temporal_stability = 0.0       # 0.0–1.0
zalgo_intensity = 0.0          # 0.0–5.0

# Virtual Camera
camera_zoom_amplitude = 1.0    # 0.1–10.0
camera_rotation = 0.0          # any (radians)
camera_pan_x = 0.0             # -2.0–2.0
camera_pan_y = 0.0             # -2.0–2.0

# Display
target_fps = 60
fullscreen = false
show_spectrum = true

[audio]
sensitivity = 1.0              # 0.0–5.0
smoothing = 0.7                # 0.0–1.0

[[audio.mappings]]
enabled = true
source = "bass"
target = "wave_amplitude"
amount = 0.4
offset = 0.0
curve = "Smooth"
# smoothing = 0.3              # Optional per-mapping override
```

### Step-by-Step Tutorial

1. **Start from a base**: Copy an existing preset or `config/default.toml`
2. **Choose your render mode**: Braille for detail, Ascii for character art, HalfBlock for color density
3. **Set your color palette**: Direct for faithful colors, HsvBright for vibrant, Oklab for perceptual accuracy, Quantized for retro
4. **Tune contrast/brightness/saturation**: These are your primary image controls
5. **Add effects gradually**: Start with one effect, tune it, then add the next. Combining too many at once makes it hard to evaluate each one.
6. **Define audio mappings**: Match source energy type to the visual parameter you want affected. Use appropriate curves (see [AUDIO_GUIDE.md](AUDIO_GUIDE.md)).
7. **Test with `p`/`P`**: Place your preset in `config/presets/` and cycle to it live
8. **Iterate**: Adjust values while running — the sidebar shows all current parameter values

### Naming Convention

Presets are auto-discovered by alphabetical order. Use the format `NN_name.toml` (e.g., `12_my_preset.toml`) for consistent cycling order.

### Performance Considerations

| Parameter | Impact |
|-----------|--------|
| `target_fps` | 60 = smooth but CPU-intensive, 30 = lower load |
| `density_scale` | > 1.0 increases character count quadratically |
| `shape_matching` | ~3× slower than simple luminance mapping. Auto-disabled on grids >10k cells (300×100+). |
| Render mode | Ascii is fastest; Octant is most complex (2×4 sub-pixels per cell) |
| `dither_mode` | Minimal performance difference between modes |
| Number of mappings | Linear cost — 10+ mappings negligible |
