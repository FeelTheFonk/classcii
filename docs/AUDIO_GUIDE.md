# Audio Reactivity Guide

Deep reference for classcii's audio analysis pipeline, mapping system, and timbral features.

## Architecture

```
Microphone/File → CPAL/Symphonia → Ring Buffer → FFT (2048 samples)
                                                      ↓
                                              Feature Extraction
                                                      ↓
                                   ┌─────────────────────────────────────┐
                                   │  9 frequency bands (sub_bass→brill) │
                                   │  Spectral: centroid, flux, flatness │
                                   │  Beat: onset, intensity, phase, bpm │
                                   │  MFCC: brightness, roughness        │
                                   └─────────────────────────────────────┘
                                                      ↓
                                            Triple Buffer (lock-free)
                                                      ↓
                                            AudioFeatures struct → Main Thread
                                                      ↓
                                            apply_audio_mappings() → RenderConfig
```

Audio runs on a dedicated thread. Features are published via a lock-free `triple_buffer` — the main thread always reads the latest features without blocking.

---

## 19 Audio Sources

### Amplitude

| Source | Range | Description |
|--------|-------|-------------|
| `rms` | 0.0–1.0 | Root Mean Square — perceived overall loudness. Smooth, good for continuous modulation. |
| `peak` | 0.0–1.0 | Peak sample amplitude — spiky, reacts to transients faster than RMS. |

### Frequency Bands

9 bands derived from FFT magnitude spectrum. Each represents the normalized energy in a frequency range.

| Source | Frequency Range | Musical Content |
|--------|----------------|-----------------|
| `sub_bass` | 20–60 Hz | Sub-bass rumble, kick drum fundamentals |
| `bass` | 60–250 Hz | Bass guitar, kick body, bass synths |
| `low_mid` | 250–500 Hz | Warmth, body of instruments |
| `mid` | 500–2000 Hz | Vocal fundamentals, guitar, piano midrange |
| `high_mid` | 2000–4000 Hz | Vocal presence, attack transients |
| `presence` | 4000–6000 Hz | Clarity, definition, consonants |
| `brilliance` | 6000–20000 Hz | Air, shimmer, hi-hats, cymbals |

### Spectral Descriptors

| Source | Range | Description |
|--------|-------|-------------|
| `spectral_centroid` | 0.0–1.0 | Frequency center of mass. High = bright/trebly sound, Low = dark/bassy. Perceptual timbral brightness. |
| `spectral_flux` | 0.0–1.0 | Frame-to-frame spectral change. High during transients, attacks, genre transitions. Good for triggering dynamic effects. |
| `spectral_flatness` | 0.0–1.0 | Noise vs tonal ratio. 1.0 = white noise, 0.0 = pure tone. Distinguishes noise from pitched content. |

### Beat & Rhythm

| Source | Range | Description |
|--------|-------|-------------|
| `onset` | 0 or 1 | Binary trigger — fires on detected beat/transient. Use with `Threshold` curve for toggle effects. |
| `beat_intensity` | 0.0–1.0 | Onset strength — how strong the detected beat is. Continuous, good with `Smooth` curve. |
| `beat_phase` | 0.0–1.0 | Position within current beat cycle (0.0 = on beat, 0.5 = off-beat, 1.0 = next beat). Use for rhythmic oscillation. |
| `bpm` | normalized | Estimated BPM divided by 200. Slow-moving, useful for macro-level modulation. |
| `onset_envelope` | 0.0–1.0 | Exponential decay envelope from last onset. Smooth attack-release curve — ideal for strobe/flash effects. |

### MFCC Timbral Features

Derived from 26 Mel-spaced triangular filters (300–8000 Hz), compressed via DCT-II to 5 coefficients.

| Source | Range | Description |
|--------|-------|-------------|
| `timbral_brightness` | 0.0–1.0 | High-frequency energy ratio in the Mel spectrum. Reacts to instrument brightness — a guitar vs a flute, clean vs distorted. |
| `timbral_roughness` | 0.0–1.0 | Spectral irregularity across Mel bands. High for harsh/distorted sounds, low for smooth/clean tones. |

---

## 14 Mapping Targets

Each target is a visual parameter in `RenderConfig`. Mappings add to the current value (additive modulation).

### Render Parameters

| Target | Range | Default | Effect |
|--------|-------|---------|--------|
| `edge_threshold` | 0.0–1.0 | 0.3 | Edge detection sensitivity — higher reveals more edges |
| `edge_mix` | 0.0–1.0 | 0.5 | Blend between edge overlay and fill — 1.0 = edges only |
| `contrast` | 0.1–3.0 | 1.0 | Luminance contrast multiplier |
| `brightness` | −1.0–1.0 | 0.0 | Luminance offset — positive brightens, negative darkens |
| `saturation` | 0.0–3.0 | 1.2 | Color saturation multiplier — 0.0 = grayscale |
| `density_scale` | 0.25–4.0 | 1.0 | Character density — higher = more detail, lower = coarser |
| `invert` | toggle | false | Flips luminance when accumulated delta exceeds 0.5 |

### Effect Parameters

| Target | Range | Default | Effect |
|--------|-------|---------|--------|
| `beat_flash_intensity` | 0.0–2.0 | 0.3 | Strobe envelope amplitude on beats |
| `chromatic_offset` | 0.0–5.0 | 0.0 | Red/Blue channel displacement (pixels) |
| `wave_amplitude` | 0.0–1.0 | 0.0 | Sinusoidal row shift strength |
| `color_pulse_speed` | 0.0–5.0 | 0.0 | HSV hue rotation speed |
| `fade_decay` | 0.0–1.0 | 0.3 | Temporal persistence — higher = longer trails |
| `glow_intensity` | 0.0–2.0 | 0.5 | Brightness bloom around bright cells |
| `zalgo_intensity` | 0.0–1.0 | 0.0 | Zalgo combining diacritics density |

---

## 4 Mapping Curves

Curves shape the source signal before it reaches the target. Applied before `amount` and `sensitivity` multiplication.

### Linear (default)
```
y = x

Output │        ╱
       │      ╱
       │    ╱
       │  ╱
       │╱
       └──────── Input
```
Direct proportional mapping. What goes in comes out. Best for smooth continuous modulation.

### Exponential
```
y = x²

Output │          ╱
       │        ╱
       │      ╱
       │    ╱
       │___╱
       └──────── Input
```
Suppresses low values, amplifies high values. Quiet passages produce almost no effect; loud passages produce strong response. Good for bass → wave where you want silence to be truly silent.

### Threshold
```
y = 0 if x < 0.3, else (x − 0.3) / 0.7

Output │        ╱
       │      ╱
       │    ╱
       │___╱
       │
       └──────── Input
         ↑ 0.3
```
Hard gate at 0.3. Nothing below threshold passes through. Ideal for `onset → invert` or `onset → zalgo_intensity` where you want a clean on/off trigger.

### Smooth (Smoothstep)
```
y = 3x² − 2x³

Output │      ___
       │    ╱
       │   │
       │  ╱
       │__╱
       └──────── Input
```
S-curve with gentle transitions at both ends. Best for `beat_intensity → beat_flash_intensity` where you want gradual attack and natural decay.

---

## Genre-Specific Strategies

### EDM / Techno
```toml
[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.6
curve = "Smooth"

[[audio.mappings]]
source = "bass"
target = "wave_amplitude"
amount = 0.5
curve = "Exponential"

[[audio.mappings]]
source = "spectral_flux"
target = "chromatic_offset"
amount = 0.4
curve = "Smooth"
```
Bass-heavy genres benefit from `Exponential` on bass to prevent constant modulation. `onset_envelope` with `Smooth` gives clean strobe hits.

### Ambient / Classical
```toml
[[audio.mappings]]
source = "rms"
target = "brightness"
amount = 0.3
curve = "Linear"

[[audio.mappings]]
source = "spectral_centroid"
target = "glow_intensity"
amount = 0.5
curve = "Linear"

[[audio.mappings]]
source = "timbral_brightness"
target = "saturation"
amount = 0.4
curve = "Smooth"
```
Low-energy music needs `Linear` curves and moderate amounts. MFCC features react to timbral shifts — instrument changes, bowing techniques.

### Rock / Metal
```toml
[[audio.mappings]]
source = "bass"
target = "edge_threshold"
amount = 0.4
curve = "Exponential"

[[audio.mappings]]
source = "onset_envelope"
target = "chromatic_offset"
amount = 0.5
curve = "Threshold"

[[audio.mappings]]
source = "rms"
target = "contrast"
amount = 0.6
curve = "Linear"
```
High RMS floor in rock/metal — use `Exponential` to differentiate quiet verses from loud choruses. `Threshold` on onset prevents constant chromatic during sustained distortion.

### Hip-Hop / Trap
```toml
[[audio.mappings]]
source = "sub_bass"
target = "wave_amplitude"
amount = 0.5
curve = "Exponential"

[[audio.mappings]]
source = "beat_intensity"
target = "beat_flash_intensity"
amount = 0.4
curve = "Smooth"

[[audio.mappings]]
source = "mid"
target = "glow_intensity"
amount = 0.5
curve = "Linear"
```
Sub-bass dominates in trap — map it to wave for physical feel. `mid` captures vocal energy for glow modulation.

---

## Smoothing

### Global Smoothing

`audio_smoothing` in `[audio]` section applies an Exponential Moving Average (EMA) to all mapping outputs:

```
smoothed = previous × (1 − alpha) + current × alpha
```

- `0.0` = no smoothing (raw signal, jittery)
- `0.3` = light smoothing (responsive, some jitter)
- `0.7` = moderate smoothing (smooth, slight lag)
- `1.0` = maximum smoothing (very smooth, noticeable lag)

### Per-Mapping Smoothing

Override global smoothing for individual mappings:

```toml
[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.5
smoothing = 0.2    # Faster response than global
```

Use lower smoothing for beat-driven mappings (fast attack needed) and higher smoothing for ambient modulation (prevent jitter).

---

## Audio Mixer Panel

Press `A` to open the Audio Mixer panel — a TUI editor for audio mappings.

### Navigation

| Key | Action |
|-----|--------|
| `Up` / `Down` | Select mapping row |
| `Left` / `Right` | Select column |
| `Enter` | Edit selected cell (cycle values for source/target/curve, toggle for enabled) |
| `+` | Add new mapping |
| `-` | Remove selected mapping |
| `Esc` | Close panel |

### Columns

| Column | Content | Edit Action |
|--------|---------|------------|
| Enabled | `[x]` / `[ ]` | Toggle on/off |
| Source | Audio source name | Cycle through 19 sources |
| Target | Visual target name | Cycle through 14 targets |
| Amount | Multiplier value | Increment/decrement |
| Offset | Additive offset | Increment/decrement |
| Curve | Response curve | Cycle through 4 curves |

Changes take effect immediately. Mappings persist only for the current session — save to TOML for permanence.
