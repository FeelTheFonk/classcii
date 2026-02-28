# Effects Reference

Complete reference for classcii's 8 post-processing effects — pipeline order, parameters, interactions, and combining strategies.

## Pipeline Order

Effects are applied in a fixed order each frame. The order is architecturally significant:

```
ASCII Grid (raw)
    │
    ├── 1. Temporal Stability   ← reduces flicker before effects add visual noise
    ├── 2. Wave Distortion      ← spatial displacement (must precede color effects)
    ├── 3. Chromatic Aberration  ← channel offset on displaced grid
    ├── 4. Color Pulse           ← hue rotation on current colors
    ├── 5. Fade Trails           ← temporal blending with previous frame
    ├── 6. Strobe                ← beat-synced brightness flash
    ├── 7. Scan Lines            ← row darkening pattern
    └── 8. Glow                  ← brightness bloom (must be last — operates on final colors)
    │
    ▼
Final Frame
```

Temporal Stability is first because it compares the current frame to the previous and decides whether to keep the old character — this must happen before effects modify the grid. Glow is last because it reads final pixel brightness to determine bloom.

---

## Effect Details

### 1. Temporal Stability

Reduces ASCII character flickering by comparing the visual density of the current and previous character for each cell. If the density distance is below a threshold, the previous character is kept.

| Property | Value |
|----------|-------|
| Parameter | `temporal_stability` |
| Range | 0.0–1.0 |
| Default | 0.0 (disabled) |
| Keybind | `y` (−0.1) / `Y` (+0.1) |
| Sidebar | TStab |

- `0.0` = disabled — characters update every frame
- `0.3` = moderate — reduces noise in low-contrast areas
- `0.7` = heavy — significant stabilization, some detail loss
- `1.0` = maximum — only high-contrast changes pass through

Use with video sources or high-FPS rendering where per-frame character changes create visual noise.

### 2. Wave Distortion

Applies a sinusoidal horizontal shift to each row of the ASCII grid. Creates a "wavy" or "underwater" visual distortion.

| Property | Value |
|----------|-------|
| Parameters | `wave_amplitude`, `wave_speed` |
| Ranges | 0.0–1.0 (amplitude), 0.0–10.0 (speed) |
| Defaults | 0.0, 2.0 |
| Keybinds | `w`/`W` (amplitude ±0.05), `u`/`U` (speed ±0.5) |
| Sidebar | Wave, WSpd |

Row shift is calculated as:

```
offset = amplitude × sin(phase + row × frequency) × max_shift
```

Maximum shift is capped at 8 cells. Rows wrap horizontally — displaced characters reappear on the opposite side (no blank gaps).

`beat_phase` modulates the phase at 50% strength for audio-synced undulation.

### 3. Chromatic Aberration

Offsets the red and blue color channels horizontally in opposite directions, simulating lens dispersion.

| Property | Value |
|----------|-------|
| Parameter | `chromatic_offset` |
| Range | 0.0–5.0 |
| Default | 0.0 (disabled) |
| Keybind | `r` (−0.5) / `R` (+0.5) |
| Sidebar | Chrm |

- `0.5–1.0` = subtle color fringing at edges
- `1.5–2.5` = visible RGB separation — cyberpunk aesthetic
- `3.0–5.0` = heavy separation — glitch art

Requires color to be enabled (`c` key). No visual effect in monochrome mode.

### 4. Color Pulse

Rotates the hue of all colored cells over time using HSV color space manipulation.

| Property | Value |
|----------|-------|
| Parameter | `color_pulse_speed` |
| Range | 0.0–5.0 |
| Default | 0.0 (disabled) |
| Keybind | `h` (−0.5) / `H` (+0.5) |
| Sidebar | Puls |

- `0.5–1.0` = slow rainbow drift
- `2.0–3.0` = fast hue cycling — psychedelic
- `5.0` = very rapid rotation

Requires color to be enabled. Works independently of `color_mode` — applied after color mapping. Black cells `(0,0,0)` are skipped (no HSV conversion needed), saving 30–60% of conversions on dark presets.

### 5. Fade Trails

Blends the current frame with the previous frame, creating a temporal persistence or "motion blur" effect.

| Property | Value |
|----------|-------|
| Parameter | `fade_decay` |
| Range | 0.0–1.0 |
| Default | 0.3 |
| Keybind | `f` (−0.01) / `F` (+0.01) |
| Sidebar | Fade |

- `0.0` = disabled — no temporal blending
- `0.2–0.4` = subtle trails — smooths motion
- `0.6–0.8` = heavy trails — ghosting effect
- `0.95+` = extreme persistence — near-static overlay

Higher values mean previous frames persist longer. Good for creating an ethereal, dreamlike quality.

### 6. Strobe

Beat-synced brightness flash using a continuous envelope driven by `onset_envelope`.

| Property | Value |
|----------|-------|
| Parameters | `beat_flash_intensity`, `strobe_decay` |
| Ranges | 0.0–2.0 (intensity), 0.5–0.99 (decay) |
| Defaults | 0.3, 0.75 |
| Keybinds | `t`/`T` (intensity ±0.1), `j`/`J` (decay ±0.05) |
| Sidebar | Strb, SDcy |

The strobe applies a brightness boost on beat detection that decays exponentially:

```
flash = onset_envelope × beat_flash_intensity
brightness += flash
```

`strobe_decay` controls how quickly the flash fades between beats:
- `0.5` = very fast decay (sharp, punchy)
- `0.75` = moderate decay (balanced)
- `0.95` = slow decay (lingering glow, overlapping flashes)

### 7. Scan Lines

Darkens every Nth row, simulating CRT scan line artifacts.

| Property | Value |
|----------|-------|
| Parameter | `scanline_gap` |
| Range | 0–8 |
| Default | 0 (disabled) |
| Keybind | `l` (−1) / `L` (+1) |
| Sidebar | Scan |

- `0` = disabled
- `2` = very dense scan lines
- `3–4` = visible but not overwhelming
- `6–8` = sparse, wide-gap scan lines

The darkened rows are set to 30% of their original brightness. Works in all render modes.

### 8. Glow

Brightness bloom effect — bright cells bleed light into their neighbors.

| Property | Value |
|----------|-------|
| Parameter | `glow_intensity` |
| Range | 0.0–2.0 |
| Default | 0.5 |
| Keybind | `g` (−0.1) / `G` (+0.1) |
| Sidebar | Glow |

A cell is considered "bright" if its maximum RGB component exceeds 140 (threshold lowered from 200 in v0.5.1 for wider visibility). The 4 cardinal neighbors (up/down/left/right) receive a brightness boost proportional to `glow_intensity`.

- `0.0` = disabled
- `0.3–0.7` = subtle bloom — adds warmth
- `1.0–1.5` = strong bloom — neon halo effect
- `2.0` = maximum — heavy light bleed

Requires color to be enabled. Applied last in the pipeline so it operates on final composited colors.

---

## Virtual Camera

Affine transformation applied to the source frame *before* ASCII conversion. Operates on raw pixels, not on the ASCII grid.

| Property | Value |
|----------|-------|
| Parameters | `camera_zoom_amplitude`, `camera_rotation`, `camera_pan_x`, `camera_pan_y` |
| Ranges | 0.1–10.0 (zoom), any (rotation), −2.0–2.0 (pan) |
| Defaults | 1.0, 0.0, 0.0, 0.0 |
| Keybinds | `<`/`>` (zoom ±0.1), `,`/`.` (rotation ±0.05), `;`/`'` (pan X ±0.05) |
| Sidebar | Zoom, Rot, PanX, PanY |

The camera applies a 2D affine transform (zoom, rotation, translation) to every pixel of the source frame using bilinear interpolation. This runs before any ASCII rasterization, so the effect is sub-pixel smooth.

For procedural sources with native camera integration (e.g. Mandelbrot), the camera parameters are applied analytically by the generator itself (`is_camera_baked = true`), bypassing the pixel-level transform for mathematical precision.

All 4 camera parameters are valid audio mapping targets — they can be driven by any of the 21 audio sources for reactive zoom, rotation, and panning.

---

## Combining Effects

### Ethereal / Dreamy
```toml
fade_decay = 0.6
glow_intensity = 1.0
temporal_stability = 0.3
color_pulse_speed = 0.5
```
Heavy fade + strong glow + slow pulse = floating, dreamlike atmosphere. Temporal stability prevents the fade from creating noise.

### Cyberpunk / Glitch
```toml
chromatic_offset = 2.0
scanline_gap = 3
beat_flash_intensity = 0.5
wave_amplitude = 0.15
```
Chromatic + scan lines = CRT monitor aesthetic. Add wave for screen corruption. Strobe for beat-synced flashes.

### Film / Cinematic
```toml
fade_decay = 0.5
glow_intensity = 0.8
chromatic_offset = 0.5
scanline_gap = 4
temporal_stability = 0.3
```
Moderate everything, no extremes. Fade + glow for warmth, subtle chromatic for lens character, scan lines for film grain feel.

### Psychedelic
```toml
color_pulse_speed = 3.0
wave_amplitude = 0.4
chromatic_offset = 2.5
glow_intensity = 1.2
zalgo_intensity = 1.0
```
Everything high. Fast color rotation + heavy wave + strong chromatic = full visual overload. Zalgo adds text corruption.

---

## Zalgo Effect

Zalgo inserts Unicode combining diacritical marks above and below ASCII characters, creating a "corrupted text" or "eldritch" visual effect.

### Technical Details

Combining diacritical marks (U+0300–U+036F) are zero-width characters that stack vertically on the base character. classcii randomly selects from above-marks and below-marks, with the count controlled by `zalgo_intensity`.

- `0.0` = disabled
- `0.5` = moderate corruption — 1–2 diacritics per cell
- `1.0–2.0` = heavy corruption — 3+ diacritics, significant visual distortion
- `5.0` = extreme — heavy stacking, text nearly illegible

### Rendering

In the batch export rasterizer (`af-export`), Zalgo diacritics are rendered with alpha-blended compositing — each diacritical mark is individually rasterized and composited onto the base glyph. This preserves visual fidelity in the output MP4.

In TUI mode, Zalgo rendering depends on the terminal's Unicode combining character support. GPU-accelerated terminals (WezTerm, Kitty) handle this well. Some terminals may not render stacked diacritics correctly.

### Keybind

| Key | Action |
|-----|--------|
| `z` | Zalgo intensity −0.5 |
| `Z` | Zalgo intensity +0.5 |
