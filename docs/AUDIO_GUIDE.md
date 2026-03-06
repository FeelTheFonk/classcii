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

### With Stem Separation

```
Audio File → SCNet (Python subprocess) → 4 WAV stems
                                              ↓
                              ┌───────────────┼───────────────┐
                              ↓               ↓               ↓
                          Drums FFT      Bass FFT       Vocals FFT  ...
                              ↓               ↓               ↓
                       AudioFeatures   AudioFeatures   AudioFeatures
                              ↓               ↓               ↓
                         stem_source     stem_source     stem_source
                              └───────────────┼───────────────┘
                                              ↓
                                 combine_stem_features()
                                              ↓
                                    apply_audio_mappings()
```

Each stem gets its own FFT pipeline, BeatDetector, MFCC, and FeatureSmoother. Mappings with `stem_source` read from the specific stem; mappings without it read from the weighted combination.

---

## 21 Audio Sources

### Amplitude

| Source | Range | Description |
|--------|-------|-------------|
| `rms` | 0.0–1.0 | Root Mean Square — perceived overall loudness. Smooth, good for continuous modulation. |
| `peak` | 0.0–1.0 | Peak sample amplitude — spiky, reacts to transients faster than RMS. |

### Frequency Bands

9 bands from FFT magnitude spectrum, gain-boosted with sqrt compression for perceptible reactivity.

| Source | Frequency Range | Musical Content |
|--------|----------------|-----------------|
| `sub_bass` | 20–60 Hz | Sub-bass rumble, kick fundamentals |
| `bass` | 60–250 Hz | Bass guitar, kick body, bass synths |
| `low_mid` | 250–500 Hz | Warmth, body of instruments |
| `mid` | 500–2000 Hz | Vocal fundamentals, guitar, piano |
| `high_mid` | 2000–4000 Hz | Vocal presence, attack transients |
| `presence` | 4000–6000 Hz | Clarity, definition, consonants |
| `brilliance` | 6000–20000 Hz | Air, shimmer, hi-hats, cymbals |

### Spectral Descriptors

| Source | Range | Description |
|--------|-------|-------------|
| `spectral_centroid` | 0.0–1.0 | Frequency center of mass. High = bright/trebly, Low = dark/bassy. |
| `spectral_flux` | 0.0–1.0 | Frame-to-frame spectral change. High during transients and attacks. Bass-weighted, sqrt-compressed. |
| `spectral_flatness` | 0.0–1.0 | Noise vs tonal ratio. 1.0 = white noise, 0.0 = pure tone. |
| `spectral_rolloff` | 0.0–1.0 | Frequency below which 85% of spectral energy is concentrated. |

### Beat & Rhythm

| Source | Range | Description |
|--------|-------|-------------|
| `onset` | 0 or 1 | Binary trigger — fires on detected beat/transient. |
| `beat_intensity` | 0.0–1.0 | Onset strength — how strong the detected beat is. |
| `beat_phase` | 0.0–1.0 | Position within current beat cycle (0.0 = on beat, 0.5 = off-beat). |
| `bpm` | normalized | Estimated BPM / 300. Slow-moving, useful for macro modulation. |
| `onset_envelope` | 0.0–1.0 | Exponential decay envelope from last onset. Ideal for strobe/flash. |

### MFCC Timbral Features

Derived from 26 Mel-spaced triangular filters (300–8000 Hz), compressed via DCT-II to 5 coefficients.

| Source | Range | Description |
|--------|-------|-------------|
| `timbral_brightness` | 0.0–1.0 | High-frequency energy ratio in Mel spectrum. Reacts to instrument brightness. |
| `timbral_roughness` | 0.0–1.0 | Spectral irregularity across Mel bands. High for harsh/distorted sounds. |

### Signal Analysis

| Source | Range | Description |
|--------|-------|-------------|
| `zero_crossing_rate` | 0.0–1.0 | Normalized sign-change count. High for percussive/noise, low for tonal content. |

---

## 19 Mapping Targets

Each target is a visual parameter in `RenderConfig`. Mappings are additive — delta is added to the current value.

### Render Parameters

| Target | Range | Default | Effect |
|--------|-------|---------|--------|
| `edge_threshold` | 0.0–1.0 | 0.0 | Edge detection sensitivity |
| `edge_mix` | 0.0–1.0 | 0.5 | Edge vs fill blend (mix×mag > 0.5 shows edge) |
| `contrast` | 0.1–3.0 | 1.0 | Luminance contrast multiplier |
| `brightness` | -1.0–1.0 | 0.0 | Luminance offset |
| `saturation` | 0.0–3.0 | 1.0 | Color saturation multiplier |
| `density_scale` | 0.25–4.0 | 1.0 | Character density multiplier |
| `invert` | threshold | false | Sets invert = true when delta > 0.5, false otherwise |

### Effect Parameters

| Target | Range | Default | Effect |
|--------|-------|---------|--------|
| `beat_flash_intensity` | 0.0–2.0 | 0.0 | Strobe envelope amplitude on beats |
| `chromatic_offset` | 0.0–5.0 | 0.0 | R/B channel displacement |
| `wave_amplitude` | 0.0–1.0 | 0.0 | Sinusoidal row shift strength |
| `color_pulse_speed` | 0.0–5.0 | 0.0 | HSV hue rotation speed |
| `fade_decay` | 0.0–1.0 | 0.0 | Temporal persistence |
| `glow_intensity` | 0.0–2.0 | 0.0 | Brightness bloom |
| `zalgo_intensity` | 0.0–5.0 | 0.0 | Zalgo combining diacritics density |

### Camera Parameters

| Target | Range | Default | Effect |
|--------|-------|---------|--------|
| `camera_zoom_amplitude` | 0.1–10.0 | 1.0 | Virtual camera zoom multiplier |
| `camera_rotation` | any | 0.0 | Virtual camera rotation (radians, wrapped at 2PI) |
| `camera_pan_x` | -2.0–2.0 | 0.0 | Virtual camera horizontal pan |
| `camera_pan_y` | -2.0–2.0 | 0.0 | Virtual camera vertical pan |
| `camera_tilt_x` | -1.0–1.0 | 0.0 | Perspective tilt via projective division |

---

## 4 Mapping Curves

Curves shape the source signal before multiplication by `amount` and `sensitivity`.

### Linear (default)
```
y = x

Output │        /
       │      /
       │    /
       │  /
       │/
       └──────── Input
```
Direct proportional mapping. Best for smooth continuous modulation.

### Exponential
```
y = x²

Output │          /
       │        /
       │      /
       │    /
       │___/
       └──────── Input
```
Suppresses low values, amplifies high values. Quiet passages produce almost no effect; loud passages produce strong response. Good for `bass → wave_amplitude`.

### Threshold
```
y = 0 if x < 0.3, else (x - 0.3) / 0.7

Output │        /
       │      /
       │    /
       │___/
       │
       └──────── Input
         ↑ 0.3
```
Hard gate at 0.3. Nothing below threshold passes. Ideal for `onset → invert` or `onset → zalgo_intensity`.

### Smooth (Smoothstep)
```
y = 3x² - 2x³

Output │      ___
       │    /
       │   |
       │  /
       │__/
       └──────── Input
```
S-curve with gentle transitions at both ends. Best for `beat_intensity → beat_flash_intensity`.

---

## Mapping Configuration

```toml
[[audio.mappings]]
enabled = true
source = "bass"                # One of 21 audio sources
target = "wave_amplitude"      # One of 19 visual targets
amount = 0.4                   # Multiplier
offset = 0.0                   # Additive offset after multiplication
curve = "Smooth"               # Linear, Exponential, Threshold, Smooth
smoothing = 0.3                # Per-mapping EMA override (optional)
stem_source = "drums"          # Route to a specific stem (optional, requires stem separation)
```

### Stem-Routed Mappings

When stem separation is active, mappings can target a specific stem's audio features via the `stem_source` field. Valid values: `"drums"`, `"bass"`, `"other"`, `"vocals"`. Without `stem_source`, mappings use the combined (mixed) features.

```toml
# Drums drive the strobe
[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.8
stem_source = "drums"

# Vocals drive glow
[[audio.mappings]]
source = "rms"
target = "glow_intensity"
amount = 0.6
stem_source = "vocals"
```

When stems are active but no stem-specific mappings exist in the config, default stem mappings are auto-injected (drums→strobe/wave, bass→contrast, vocals→glow, other→chromatic).

Multiple mappings can be active simultaneously. Per-mapping smoothing is opt-in. Without explicit `smoothing` field, features pass through directly (already smoothed by the feature-level EMA).

---

## Smoothing

### Feature-Level Smoothing (Global)

`smoothing` in `[audio]` controls the `FeatureSmoother` EMA applied to all audio features before they reach mappings:

```
smoothed = previous × (1 - alpha) + current × alpha
```

- `0.1` = minimal smoothing (responsive, slight jitter)
- `0.3` = balanced (responsive, default)
- `0.6` = moderate (smooth, slight lag)
- `0.9` = heavy (very smooth, significant lag)

This smoothing is attack/release asymmetric: fast response to increases, slow decay.

### Per-Mapping Smoothing (Opt-In)

Add a **second** EMA stage to individual mappings. Without this field, features pass through directly — no additional filtering:

```toml
[[audio.mappings]]
source = "onset_envelope"
target = "beat_flash_intensity"
amount = 0.5
smoothing = 0.3    # Optional — adds per-mapping EMA (framerate-corrected)
```

Per-mapping smoothing is framerate-independent (calibrated for 60 FPS, corrected via `1 - (1-alpha)^(60/fps)`). Use sparingly — double-smoothing (feature + per-mapping) can over-dampen transient signals.

### Adaptive Per-Band Smoothing

The internal feature smoother applies frequency-aware multipliers automatically:

| Band Category | Multiplier | Rationale |
|---------------|------------|-----------|
| Sub-bass, Bass | x0.8 | Punchy — fast attack for kick/bass hits |
| Mid, Low-mid | x1.0 | Neutral — standard smoothing |
| High-mid, Presence, Brilliance | x0.7 | Fast — high frequencies need quick tracking |
| Beat intensity, Onset envelope | bypass | **No smoothing** — transient events pass through at full amplitude |
| Onset (bool), Beat phase | bypass | Instantaneous — no smoothing applied |

This is automatic and requires no configuration. Per-mapping `smoothing` overrides take priority.

---

## Genre Strategies

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
`Exponential` on bass prevents constant modulation. `onset_envelope` with `Smooth` gives clean strobe hits.

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
Low-energy music needs `Linear` curves and moderate amounts. MFCC features react to timbral shifts.

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
High RMS floor — use `Exponential` to differentiate quiet verses from loud choruses.

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
Sub-bass dominates in trap — map it to wave for physical feel. `mid` captures vocal energy for glow.
