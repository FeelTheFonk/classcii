# Reference

Exhaustive technical reference for classcii — TOML schema, post-processing effects, charsets, and presets.

All default values are synchronized with `RenderConfig::default()` v1.5.0.

---

## TOML Schema

Configuration files have two sections: `[render]` and `[audio]`. All fields are optional — unspecified fields use program defaults. Loaded from `config/default.toml` by default. CLI flags override config files.

### `[render]` — Render Mode & Display

| Field | Type | Values / Range | Default |
|-------|------|----------------|---------|
| `render_mode` | String | `"Ascii"`, `"Braille"`, `"HalfBlock"`, `"Quadrant"`, `"Sextant"`, `"Octant"` | `"Octant"` |
| `charset` | String | Any string, min 2 chars | CHARSET_FULL (70 chars) |
| `charset_index` | Integer | 0–9 | `0` |
| `dither_mode` | String | `"Bayer8x8"`, `"BlueNoise16"`, `"None"` | `"BlueNoise16"` |
| `invert` | Boolean | — | `false` |
| `color_enabled` | Boolean | — | `true` |
| `color_mode` | String | `"Direct"`, `"HsvBright"`, `"Oklab"`, `"Quantized"` | `"Oklab"` |
| `fullscreen` | Boolean | — | `false` |
| `show_spectrum` | Boolean | — | `false` |
| `target_fps` | Integer | 15–120 | `60` |

Sub-pixel resolution per cell: Ascii (1x1), HalfBlock (1x2), Braille (2x4), Quadrant (2x2), Sextant (2x3), Octant (2x4).

`charset` defines the luminance ramp (lightest to densest). Only used in Ascii mode. `charset_index` selects a built-in charset. If both specified, `charset` takes precedence.

Legacy: `dither_enabled` (boolean) supported — `true` maps to Bayer8x8, `false` to None. `"BlueNoise64"` alias maps to BlueNoise16.

Color modes:
- **Direct**: RGB from source pixel, unmodified.
- **HsvBright**: HSV with V forced to 1.0 — character encodes luminance, color is pure hue+saturation.
- **Oklab**: Perceptually uniform — L forced to 1.0 for consistent brightness perception.
- **Quantized**: Reduced palette for retro/posterized aesthetic.

### `[render]` — Image Processing

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `edge_threshold` | Float | 0.0–1.0 | `0.0` | Edge detection sensitivity (0 = disabled) |
| `edge_mix` | Float | 0.0–1.0 | `0.5` | Edge vs fill blend (mix×mag > 0.5 shows edge) |
| `shape_matching` | Boolean | — | `false` | Shape-aware character matching (~3x slower) |
| `aspect_ratio` | Float | 0.1–10.0 | `2.0` | Terminal character aspect ratio correction |
| `density_scale` | Float | 0.25–4.0 | `1.0` | Character resolution multiplier |
| `saturation` | Float | 0.0–3.0 | `1.0` | Color saturation multiplier |
| `contrast` | Float | 0.1–3.0 | `1.0` | Luminance contrast |
| `brightness` | Float | -1.0–1.0 | `0.0` | Luminance offset |
| `bg_style` | String | `"Black"`, `"SourceDim"`, `"Transparent"` | `"Black"` |

### `[render]` — Post-Processing Effects

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `fade_decay` | Float | 0.0–1.0 | `0.0` | Temporal persistence (0 = disabled) |
| `glow_intensity` | Float | 0.0–2.0 | `0.0` | Brightness bloom (0 = disabled) |
| `zalgo_intensity` | Float | 0.0–5.0 | `0.0` | Zalgo diacritics density |
| `beat_flash_intensity` | Float | 0.0–2.0 | `0.0` | Strobe envelope amplitude |
| `chromatic_offset` | Float | 0.0–5.0 | `0.0` | R/B channel displacement |
| `wave_amplitude` | Float | 0.0–1.0 | `0.0` | Sinusoidal row shift |
| `wave_speed` | Float | 0.0–10.0 | `0.0` | Wave oscillation speed |
| `color_pulse_speed` | Float | 0.0–5.0 | `0.0` | HSV hue rotation speed |
| `scanline_gap` | Integer | 0–8 | `0` | Scan line spacing (0 = off) |
| `scanline_darken` | Float | 0.0–1.0 | `0.3` | Scan line darkening factor |
| `strobe_decay` | Float | 0.5–0.99 | `0.85` | Strobe envelope decay rate |
| `temporal_stability` | Float | 0.0–1.0 | `0.3` | Anti-flicker strength (0 = off) |

### `[render]` — Virtual Camera

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `camera_zoom_amplitude` | Float | 0.1–10.0 | `1.0` | Affine zoom multiplier |
| `camera_rotation` | Float | any | `0.0` | Affine rotation (radians, wrapped at 2PI) |
| `camera_pan_x` | Float | -2.0–2.0 | `0.0` | Horizontal panning |
| `camera_pan_y` | Float | -2.0–2.0 | `0.0` | Vertical panning |
| `camera_tilt_x` | Float | -1.0–1.0 | `0.0` | Perspective tilt (projective division) |

### `[audio]` — Global Settings

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `smoothing` | Float | 0.0–1.0 | `0.3` | Global EMA smoothing for all mappings |
| `sensitivity` | Float | 0.0–5.0 | `2.0` | Global multiplier for all mapping outputs |
| `input_gain` | Float | 0.1–10.0 | `1.0` | Pre-FFT sample gain (increase for quiet mic) |

### `[[audio.mappings]]` — Audio-to-Visual Mappings

Repeatable section. Each entry defines one mapping.

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `enabled` | Boolean | — | `true` | Activate/deactivate |
| `source` | String | 21 values | — | Audio feature source (required) |
| `target` | String | 19 values | — | Visual parameter target (required) |
| `amount` | Float | any | — | Multiplier (required) |
| `offset` | Float | any | `0.0` | Additive offset after multiplication |
| `curve` | String | `"Linear"`, `"Exponential"`, `"Threshold"`, `"Smooth"` | `"Linear"` | Response curve |
| `smoothing` | Float | 0.0–1.0 | global value | Per-mapping EMA override |
| `stem_source` | String | `"drums"`, `"bass"`, `"other"`, `"vocals"` | — | Route mapping to a specific stem's features (requires stem separation) |

For the full list of valid sources and targets, see [Audio Guide](AUDIO_GUIDE.md).

---

## Effects Pipeline

8 composable effects applied in a fixed order each frame:

```
ASCII Grid (raw)
    │
    ├── 1. Temporal Stability   ← reduces flicker before effects add noise
    ├── 2. Wave Distortion      ← spatial displacement (precedes color effects)
    ├── 3. Chromatic Aberration  ← channel offset on displaced grid
    ├── 4. Color Pulse           ← hue rotation on current colors
    ├── 5. Fade Trails           ← temporal blending with previous frame
    ├── 6. Strobe                ← beat-synced brightness flash
    ├── 7. Scan Lines            ← row darkening pattern
    └── 8. Glow                  ← brightness bloom (last — operates on final colors)
    │
    ▼
Final Frame
```

### 1. Temporal Stability

Reduces ASCII character flickering by comparing visual density of current and previous character per cell. If density distance is below threshold, previous character is kept.

- `0.0` = disabled — characters update every frame
- `0.3` = moderate — reduces noise in low-contrast areas
- `0.7` = heavy — significant stabilization, some detail loss
- `1.0` = maximum — only high-contrast changes pass through

The user-facing threshold [0.0, 1.0] is scaled by `STABILITY_DENSITY_SCALE` (0.3).

### 2. Wave Distortion

Sinusoidal horizontal shift per row: `offset = amplitude × sin(phase + row × frequency) × max_shift`. Maximum shift capped at 8 cells. Rows wrap horizontally. `beat_phase` modulates phase at 50% strength.

### 3. Chromatic Aberration

Offsets red and blue channels horizontally in opposite directions, simulating lens dispersion. Requires color enabled.

- `0.5–1.0` = subtle fringing
- `1.5–2.5` = visible RGB separation — cyberpunk
- `3.0–5.0` = heavy — glitch art

### 4. Color Pulse

Rotates hue of all colored cells via HSV manipulation. Black cells `(0,0,0)` are skipped. Requires color enabled.

- `0.5–1.0` = slow rainbow drift
- `2.0–3.0` = fast cycling — psychedelic
- `5.0` = very rapid

### 5. Fade Trails

Blends current frame with previous frame for temporal persistence.

- `0.0` = disabled
- `0.2–0.4` = subtle trails
- `0.6–0.8` = heavy ghosting
- `0.95–0.99` = extreme persistence

### 6. Strobe

Beat-synced brightness flash: `flash = onset_envelope × beat_flash_intensity`. `strobe_decay` controls how quickly the flash fades:

- `0.5` = fast decay (sharp, punchy)
- `0.75` = moderate
- `0.95` = slow decay (lingering, overlapping)

### 7. Scan Lines

Darkens every Nth row at `scanline_darken` factor (default 30%). Works in all render modes.

- `0` = disabled
- `2` = dense
- `3–4` = visible but balanced
- `6–8` = sparse, wide-gap

### 8. Glow

Brightness bloom — bright cells (max RGB > 140) bleed light to 4 cardinal neighbors. Intensity scaled by `GLOW_FACTOR_SCALE` (40.0 RGB units). Requires color enabled. Applied last.

- `0.3–0.7` = subtle bloom
- `1.0–1.5` = strong neon halo
- `2.0` = maximum light bleed

### Virtual Camera

2D affine transform (zoom, rotation, translation) with optional perspective tilt via projective division, applied to source frame *before* ASCII conversion. Bilinear interpolation. Sub-pixel smooth. All 5 camera parameters are valid audio mapping targets.

---

## Combining Strategies

### Ethereal / Dreamy
```toml
fade_decay = 0.6
glow_intensity = 1.0
temporal_stability = 0.3
color_pulse_speed = 0.5
```

### Cyberpunk / Glitch
```toml
chromatic_offset = 2.0
scanline_gap = 3
beat_flash_intensity = 0.5
wave_amplitude = 0.15
```

### Film / Cinematic
```toml
fade_decay = 0.5
glow_intensity = 0.8
chromatic_offset = 0.5
scanline_gap = 4
temporal_stability = 0.3
```

### Psychedelic
```toml
color_pulse_speed = 3.0
wave_amplitude = 0.4
chromatic_offset = 2.5
glow_intensity = 1.2
zalgo_intensity = 1.0
```

---

## Zalgo Effect

Inserts Unicode combining diacritical marks (U+0300–U+036F) above and below characters for "corrupted text" effect.

- `0.0` = disabled
- `0.5` = moderate — 1–2 diacritics per cell
- `1.0–2.0` = heavy — 3+ diacritics
- `5.0` = extreme stacking

In batch export, Zalgo diacritics are alpha-blended composited per glyph. In TUI mode, rendering depends on terminal Unicode support — GPU-accelerated terminals (WezTerm, Kitty) handle this best.

---

## Charsets

### 10 Built-in Charsets

Selected with keys `1`–`0`. Characters ordered lightest (space) to densest.

| Key | Index | Name | Characters | Len | Best For |
|-----|-------|------|------------|-----|----------|
| `1` | 0 | Full | `` .'`^",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$ `` | 70 | Photos — maximum tonal range |
| `2` | 1 | Dense | ` _.,=-+:;cba!?0123456789$W#@Ñ` | 29 | Dense imagery |
| `3` | 2 | Short 1 | `.:-=+*#%@` | 9 | Quick rendering |
| `4` | 3 | Blocks | ` ░▒▓█` | 5 | Pseudo-pixel, retro |
| `5` | 4 | Minimal | ` .:░▒▓█` | 7 | High contrast + Unicode |
| `6` | 5 | Glitch 1 | ` .°*O0@#&%` | 10 | Organic contrast |
| `7` | 6 | Glitch 2 | ` ▂▃▄▅▆▇█` | 8 | Spectrum bars |
| `8` | 7 | Edge | `.,*+#@` | 6 | Edge emphasis |
| `9` | 8 | Digital | ` 01` | 3 | Binary/cryptographic |
| `0` | 9 | Binary | ` #` | 2 | 1-bit high contrast |

### Additional Charsets (TOML-only)

Available via `charset = "..."` in TOML config or batch mode. Not mapped to TUI keys.

| Name | Characters | Len | Best For |
|------|------------|-----|----------|
| Short 2 | ` .:-=+*#%@` | 10 | Inverted gradient |
| Extended | ` .·:;+xX#%@` | 11 | Unicode dots + ASCII |
| Discrete | ` 1234` | 5 | Matrix/digital |
| Hires | `` .'`:,;_-~"!\|/(){}[]<>+*=?^#%&@$ `` | 34 | Batch export, large cells |

### Charset Mechanics

Each charset defines a luminance ramp. At startup, a 256-entry LUT maps luminance [0–255] to a character:

```
char_index = round(luminance / 255 × (charset_length - 1))
```

O(1) per pixel, zero allocation. Charsets only apply in **Ascii** render mode. Other modes use fixed Unicode block characters:

| Mode | Characters | Charset? |
|------|-----------|----------|
| Ascii | Charset chars | Yes |
| HalfBlock | `▄` with fg (bottom) / bg (top) colors | No |
| Braille | U+2800–U+28FF | No |
| Quadrant | 2x2 block elements | No |
| Sextant | U+1FB00 (2x3) | No |
| Octant | U+1CD00 (2x4) | No |

### Custom Charset Editor

Press `C` in TUI. Type characters from lightest to densest, `Enter` to apply, `Esc` to cancel. Minimum 2 characters. Any Unicode supported by your terminal font can be used.

---

## 25 Presets

In `config/presets/`, selectable via `--preset <name>` or cycled live with `p`/`P`. Auto-discovered alphabetically.

Ordered from most faithful to input (01) to most chaotic (21), with 22 as export-optimized and 23–25 as stem-aware presets.

| Preset | Mode | Fidelity | Style |
|--------|------|----------|-------|
| `01_pure_photo` | Octant | ★★★★★ | Photo-faithful, Oklab, zero effects, 1 subtle mapping |
| `02_film_grain` | Sextant | ★★★★★ | Cinematic film grain, Oklab, soft fade+temporal |
| `03_soft_focus` | Octant | ★★★★☆ | Photographic bloom, shape matching, HsvBright |
| `04_noir` | HalfBlock | ★★★★☆ | Film noir monochrome, high contrast edges |
| `05_breath` | Ascii | ★★★★☆ | Ultra-minimalist contemplative, single RMS mapping |
| `06_clean_gradient` | Ascii | ★★★☆☆ | Versatile reference, SourceDim, balanced reactivity |
| `07_braille_cinema` | Braille | ★★★☆☆ | Cinematic Braille, Oklab, bass-reactive camera |
| `08_braille_hd` | Braille | ★★★☆☆ | High-density pointillism, Direct color, balanced |
| `09_spectral_bands` | Quadrant | ★★★☆☆ | Per-band frequency mapping, 5 distinct effects |
| `10_vector_wire` | Ascii | ★★☆☆☆ | Monochrome wireframe, onset→invert, edge-dominant |
| `11_deep_zoom` | Braille | ★★☆☆☆ | Audio-reactive camera zoom+rotation, spatial |
| `12_aurora` | Quadrant | ★★☆☆☆ | Aurora borealis, saturated glow, color pulse |
| `13_reactive_showcase` | Ascii | ★★☆☆☆ | All 8 effects at moderate levels, demonstration |
| `14_brutalism` | HalfBlock | ★★☆☆☆ | Monochrome brutal, density 2.0, glow, fullscreen |
| `15_neon_edge` | Ascii | ★☆☆☆☆ | Neon urban, edge-dominant, chromatic+glow |
| `16_cyber_braille` | Braille | ★☆☆☆☆ | Cyberpunk saturated, glow+chromatic+scanlines |
| `17_matrix_rain` | Ascii | ★☆☆☆☆ | Matrix digital rain, long fade, binary charset |
| `18_interference` | Braille | ★☆☆☆☆ | Moiré wave patterns, speed 6.0, chromatic |
| `19_ghost_trail` | Quadrant | ★☆☆☆☆ | Inverted spectral ghost, extreme persistence |
| `20_tv_static` | Ascii | ☆☆☆☆☆ | Broken TV, binary charset, flatness→density, zalgo |
| `21_glitch_storm` | Braille | ☆☆☆☆☆ | Controlled chaos, all extreme effects, musically driven |
| `22_hires_export` | Ascii | ★★★★☆ | Batch export optimized, CHARSET_FULL, Oklab, subtle |
| `23_stem_drums_pulse` | Braille | ★★★☆☆ | Stem-aware: drums→strobe/wave, bass→contrast, vocals→glow |
| `24_stem_vocal_glow` | Sextant | ★★★★☆ | Stem-aware: vocals→glow/brightness, drums→flash, bass→wave |
| `25_stem_full_spectrum` | Octant | ★★★☆☆ | Stem-aware: all 4 stems mapped to distinct effects, full spectrum |

### Creating a Custom Preset

1. Copy an existing preset or `config/default.toml`
2. Choose render mode, color palette, charset
3. Tune contrast/brightness/saturation
4. Add effects one at a time
5. Define audio mappings with appropriate curves (see [Audio Guide](AUDIO_GUIDE.md))
6. Place in `config/presets/` — auto-discovered on next `p`/`P` cycle

Naming convention: `NN_name.toml` for consistent alphabetical cycling order.

### Performance Considerations

| Parameter | Impact |
|-----------|--------|
| `target_fps` | 60 = smooth but CPU-intensive, 30 = lower load |
| `density_scale` | > 1.0 increases cell count quadratically |
| `shape_matching` | ~3x slower. Auto-disabled on grids >10k cells. |
| Render mode | Ascii fastest; Octant most complex (2x4 sub-pixels) |
| Number of mappings | Linear cost — 10+ negligible |

---

## Default Values Summary

All values as defined in `RenderConfig::default()` v1.5.0, synchronized with `config/default.toml`:

```toml
[render]
render_mode = "Octant"
charset_index = 0
charset = " .'`^\",:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$"
dither_mode = "BlueNoise16"
invert = false
color_enabled = true
color_mode = "Oklab"
edge_threshold = 0.0
edge_mix = 0.5
shape_matching = false
aspect_ratio = 2.0
density_scale = 1.0
saturation = 1.0
contrast = 1.0
brightness = 0.0
bg_style = "Black"
fade_decay = 0.0
glow_intensity = 0.0
zalgo_intensity = 0.0
beat_flash_intensity = 0.0
chromatic_offset = 0.0
wave_amplitude = 0.0
wave_speed = 0.0
color_pulse_speed = 0.0
scanline_gap = 0
scanline_darken = 0.3
strobe_decay = 0.85
temporal_stability = 0.3
camera_zoom_amplitude = 1.0
camera_rotation = 0.0
camera_pan_x = 0.0
camera_pan_y = 0.0
camera_tilt_x = 0.0
target_fps = 60
fullscreen = false
show_spectrum = false

[audio]
smoothing = 0.3
sensitivity = 2.0
input_gain = 1.0
```

### Default Audio Mappings (5)

```toml
[[audio.mappings]]
source = "bass"
target = "edge_threshold"
amount = 0.7
curve = "Smooth"

[[audio.mappings]]
source = "spectral_flux"
target = "contrast"
amount = 0.8
curve = "Linear"

[[audio.mappings]]
source = "rms"
target = "brightness"
amount = 0.4
curve = "Linear"

[[audio.mappings]]
source = "beat_intensity"
target = "beat_flash_intensity"
amount = 1.2
curve = "Smooth"

[[audio.mappings]]
source = "spectral_centroid"
target = "glow_intensity"
amount = 0.7
curve = "Linear"
```
