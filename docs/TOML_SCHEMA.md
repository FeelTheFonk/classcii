# TOML Configuration Schema

Complete annotated schema for classcii configuration files. All fields are optional — unspecified fields use their defaults.

## File Structure

```toml
[render]
# All render and effect parameters

[audio]
# Global audio settings

[[audio.mappings]]
# Audio-to-visual mappings (repeatable)
```

Configuration is loaded from `config/default.toml` by default. Presets in `config/presets/` override the default. CLI flags (`--mode`, `--fps`, `--preset`) override config files.

---

## `[render]` Section

### Render Mode

| Field | Type | Values | Default |
|-------|------|--------|---------|
| `render_mode` | String | `"Ascii"`, `"Braille"`, `"HalfBlock"`, `"Quadrant"`, `"Sextant"`, `"Octant"` | `"Ascii"` |

Sub-pixel resolution per cell: Ascii (1×1), HalfBlock (1×2), Braille (2×4), Quadrant (2×2), Sextant (2×3), Octant (2×4).

### Charset

| Field | Type | Range | Default |
|-------|------|-------|---------|
| `charset` | String | Any string, min 2 chars | `" .:-=+*#%@"` |
| `charset_index` | Integer | 0–9 | `0` |

`charset` defines the luminance ramp from lightest to densest. Only used in Ascii render mode. `charset_index` selects a built-in charset (see [CHARSET_REFERENCE.md](CHARSET_REFERENCE.md)). If both are specified, `charset` takes precedence.

### Dithering

| Field | Type | Values | Default |
|-------|------|--------|---------|
| `dither_mode` | String | `"Bayer8x8"`, `"BlueNoise16"`, `"None"` | `"Bayer8x8"` |

Legacy field `dither_enabled` (boolean) is supported: `true` → Bayer8x8, `false` → None.

Serde alias: `"BlueNoise64"` maps to `BlueNoise16` for backward compatibility.

### Display

| Field | Type | Range | Default |
|-------|------|-------|---------|
| `invert` | Boolean | — | `false` |
| `color_enabled` | Boolean | — | `true` |
| `fullscreen` | Boolean | — | `false` |
| `show_spectrum` | Boolean | — | `true` |
| `target_fps` | Integer | 30, 60 | `30` |

`fullscreen` hides the sidebar and spectrum panel. `show_spectrum` controls the audio spectrum display below the main viewport.

### Image Processing

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `edge_threshold` | Float | 0.0–1.0 | `0.3` | Edge detection sensitivity (0.0 = disabled) |
| `edge_mix` | Float | 0.0–1.0 | `0.5` | Edge vs fill blend (1.0 = edges only) |
| `shape_matching` | Boolean | — | `false` | Use shape-aware character matching (~3× slower) |
| `aspect_ratio` | Float | 0.5–4.0 | `2.0` | Terminal character aspect ratio correction |
| `density_scale` | Float | 0.25–4.0 | `1.0` | Character resolution multiplier |

### Color

| Field | Type | Values | Default |
|-------|------|--------|---------|
| `color_mode` | String | `"Direct"`, `"HsvBright"`, `"Oklab"`, `"Quantized"` | `"HsvBright"` |
| `saturation` | Float | 0.0–3.0 | `1.2` |
| `contrast` | Float | 0.1–3.0 | `1.0` |
| `brightness` | Float | −1.0–1.0 | `0.0` |
| `bg_style` | String | `"Black"`, `"SourceDim"`, `"Transparent"` | `"Black"` |

Color modes:
- **Direct**: RGB from source pixel, unmodified.
- **HsvBright**: HSV with V forced to 1.0 — character encodes luminance, color is pure hue+saturation.
- **Oklab**: Perceptually uniform — L forced to 1.0 for consistent brightness perception.
- **Quantized**: Reduced palette for retro/posterized aesthetic.

### Post-Processing Effects

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `fade_decay` | Float | 0.0–1.0 | `0.3` | Temporal persistence (0 = disabled) |
| `glow_intensity` | Float | 0.0–2.0 | `0.5` | Brightness bloom |
| `zalgo_intensity` | Float | 0.0–5.0 | `0.0` | Zalgo diacritics density |
| `beat_flash_intensity` | Float | 0.0–2.0 | `0.3` | Strobe envelope amplitude |
| `chromatic_offset` | Float | 0.0–5.0 | `0.0` | R/B channel displacement |
| `wave_amplitude` | Float | 0.0–1.0 | `0.0` | Sinusoidal row shift |
| `wave_speed` | Float | 0.0–10.0 | `2.0` | Wave oscillation speed |
| `color_pulse_speed` | Float | 0.0–5.0 | `0.0` | HSV hue rotation speed |
| `scanline_gap` | Integer | 0–8 | `0` | Scan line spacing (0 = off) |
| `strobe_decay` | Float | 0.5–0.99 | `0.75` | Strobe envelope decay rate |
| `temporal_stability` | Float | 0.0–1.0 | `0.0` | Anti-flicker strength (0 = off) |

---

## `[audio]` Section

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `smoothing` | Float | 0.0–1.0 | `0.7` | Global EMA smoothing for all mappings |
| `sensitivity` | Float | 0.0–5.0 | `1.0` | Global multiplier for all mapping outputs |

### `[[audio.mappings]]`

Repeatable section. Each entry defines one audio-to-visual parameter mapping.

| Field | Type | Range | Default | Description |
|-------|------|-------|---------|-------------|
| `enabled` | Boolean | — | `true` | Activate/deactivate this mapping |
| `source` | String | see below | — | Audio feature source (required) |
| `target` | String | see below | — | Visual parameter target (required) |
| `amount` | Float | any | — | Multiplier applied to shaped source value (required) |
| `offset` | Float | any | `0.0` | Additive offset after multiplication |
| `curve` | String | `"Linear"`, `"Exponential"`, `"Threshold"`, `"Smooth"` | `"Linear"` | Response curve |
| `smoothing` | Float | 0.0–1.0 | global value | Per-mapping EMA override |

#### Valid Sources (19)

`rms`, `peak`, `sub_bass`, `bass`, `low_mid`, `mid`, `high_mid`, `presence`, `brilliance`, `spectral_centroid`, `spectral_flux`, `spectral_flatness`, `beat_intensity`, `onset`, `beat_phase`, `bpm`, `timbral_brightness`, `timbral_roughness`, `onset_envelope`

#### Valid Targets (14)

`edge_threshold`, `edge_mix`, `contrast`, `brightness`, `saturation`, `density_scale`, `invert`, `beat_flash_intensity`, `chromatic_offset`, `wave_amplitude`, `color_pulse_speed`, `fade_decay`, `glow_intensity`, `zalgo_intensity`

---

## Examples

### Minimal Configuration

```toml
[render]
render_mode = "Ascii"
color_enabled = true
target_fps = 30
```

Everything else uses defaults. No audio mappings — static rendering only.

### Photo Viewing

```toml
[render]
render_mode = "Ascii"
charset = "$@B%8&WM#*oahkbdpqwmZO0QLCJUYXzcvunxrjft/|()1{}?-_+~<>i!lI;:,\"^`'. "
color_enabled = true
color_mode = "HsvBright"
dither_mode = "BlueNoise16"
contrast = 1.2
saturation = 1.2
edge_threshold = 0.3
edge_mix = 0.5
glow_intensity = 0.3
target_fps = 30
```

### Audio-Reactive Cyberpunk

```toml
[render]
render_mode = "Braille"
color_enabled = true
color_mode = "Direct"
dither_mode = "BlueNoise16"
contrast = 1.6
saturation = 1.8
glow_intensity = 1.5
chromatic_offset = 1.5
scanline_gap = 3
fade_decay = 0.6
target_fps = 60

[audio]
sensitivity = 1.5
smoothing = 0.3

[[audio.mappings]]
source = "bass"
target = "brightness"
amount = 0.4
curve = "Exponential"

[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.6
curve = "Smooth"

[[audio.mappings]]
source = "spectral_flux"
target = "chromatic_offset"
amount = 0.4
curve = "Smooth"
```

### Batch Export — Generative Clip

```toml
[render]
render_mode = "Ascii"
charset = " .:-=+*#%@"
color_enabled = true
color_mode = "HsvBright"
contrast = 1.3
saturation = 1.2
fade_decay = 0.4
glow_intensity = 0.5
beat_flash_intensity = 0.3
target_fps = 60

[audio]
sensitivity = 1.2
smoothing = 0.5

[[audio.mappings]]
source = "bass"
target = "wave_amplitude"
amount = 0.4
curve = "Smooth"

[[audio.mappings]]
source = "spectral_centroid"
target = "glow_intensity"
amount = 0.5
curve = "Linear"

[[audio.mappings]]
source = "rms"
target = "brightness"
amount = 0.3
curve = "Linear"

[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.5
curve = "Smooth"
```

---

## Default Values Summary

All defaults as defined in `RenderConfig::default()`:

```toml
[render]
render_mode = "Ascii"
charset = " .:-=+*#%@"      # CHARSET_SHORT_1
charset_index = 0
dither_mode = "Bayer8x8"
invert = false
color_enabled = true
edge_threshold = 0.3
edge_mix = 0.5
shape_matching = false
aspect_ratio = 2.0
density_scale = 1.0
color_mode = "HsvBright"
saturation = 1.2
contrast = 1.0
brightness = 0.0
bg_style = "Black"
fade_decay = 0.3
glow_intensity = 0.5
zalgo_intensity = 0.0
beat_flash_intensity = 0.3
chromatic_offset = 0.0
wave_amplitude = 0.0
wave_speed = 2.0
color_pulse_speed = 0.0
scanline_gap = 0
strobe_decay = 0.75
temporal_stability = 0.0
target_fps = 30
fullscreen = false
show_spectrum = true

[audio]
smoothing = 0.7
sensitivity = 1.0
```

Note: `config/default.toml` may differ from `RenderConfig::default()` — the TOML file can override any default. The values above are the programmatic defaults used when no config file is loaded or when a field is omitted.
