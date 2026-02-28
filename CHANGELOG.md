# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.3] — 2026-02-28

### Fixed
- **Creation Mode Ambient preset**: `color_pulse_speed` and `wave_amplitude` were driven by internal timer (`color_pulse_phase`), not by audio. Now driven by `spectral_centroid` and `rms` respectively — truly audio-reactive.
- **Creation Mode Psychedelic preset**: `color_pulse_speed` was hard-coded constant (`3.0 * mi`). Now modulated by `rms` — rotation speed responds to music volume.

## [0.5.2] — 2026-02-28

### Fixed
- **Flash/strobe too aggressive**: `beat_flash_intensity` default 0.8→0.3, `strobe_decay` 0.85→0.75 (faster decay, less overlap). Removed `onset→invert` default mapping (main fatigue source). Reduced `beat_intensity→beat_flash_intensity` mapping amount 0.5→0.3.
- **Batch export macro fire too frequent**: Probabilities reduced from 25%/20%/33% to 8%/6%/12%. Simultaneous multi-change probability drops from ~23% to ~3%.
- **Creation Percussive too intense**: `beat_flash_intensity` multiplier 1.8→0.8, `zalgo_intensity` multiplier 2.5→1.2.
- **Key routing for y/Y, j/J, u/U**: New effect keys were missing from main dispatch match — keys were dead. Added to effect key routing.
- **Clippy `needless_pass_by_value`**: Allow lint on `start_source` (Arc consumed under `#[cfg(feature = "video")]`).

### Added
- **Keybind `y/Y`**: Temporal stability control (±0.1, range 0.0–1.0). Previously config-only.
- **Keybind `j/J`**: Strobe decay control (±0.05, range 0.5–0.99). Previously hidden.
- **Keybind `u/U`**: Wave speed control (±0.5, range 0.0–10.0). Previously config-only.
- **Preset "Reactive"** (`11_reactive.toml`): Showcases all visual effects (chromatic, wave, glow, pulse, scan, zalgo, fade, stability) at moderate levels with audio-reactive mappings.
- Sidebar: TStab, SDcy, WSpd indicators in Effects section.
- Help overlay: stability, strobe decay, wave speed entries.

## [0.5.1] — 2026-02-28

### Fixed
- **Video playback rollback**: Preset changes no longer reset video to beginning. Resize handler now preserves playback position (mirrors Seek handler). Preset change only triggers resize when render mode, density, or aspect ratio actually change.
- **Wave effect too brutal**: Capped max row shift to 8 cells (was fraction of grid width). Rows now wrap instead of showing blank gaps. Persistent phase with beat_phase as 50% additive modulator for smooth audio sync.
- **Creation Mode decoupled from overlay**: Modulation continues when overlay is hidden (Esc). K toggles overlay, q fully deactivates. Sidebar shows K● (active) / K○ (inactive).
- **Glow too subtle**: Brightness threshold lowered from 200 to 140, making glow visible on more cells.

### Added
- **Zalgo effect** exposed in Creation Mode (index 7) with audio modulation in Percussive/Psychedelic presets.
- **Z/z keybinding** for manual zalgo intensity control (±0.5).
- **2 new default audio mappings**: beat_intensity → beat_flash_intensity (Smooth curve), spectral_centroid → glow_intensity.
- Help overlay: zalgo keybind, color FX visibility note, creation mode q/Esc hints.

## [0.5.0] — 2026-02-28

### Added
- **Full batch effect pipeline**: All 8 post-processing effects now applied in batch export (temporal stability, wave distortion, chromatic aberration, color pulse, fade trails, strobe, scan lines, glow), achieving full parity with interactive renderer.
- **Generative mapper completion**: All 19 audio sources, 14 mapping targets, MappingCurve application (Linear/Exponential/Threshold/Smooth), and per-mapping EMA smoothing in offline batch pipeline.
- **Categorized help overlay**: 5 sections (Navigation, Render, Effects, Audio, Overlays) with visual headers.
- **Terminal size guard**: Graceful "Terminal too small" message when below 80x20.
- **Sidebar section separators**: Visual grouping (Render, Effects, Audio, Info) with improved contrast (Gray labels).

### Changed
- `AutoGenerativeMapper::apply_at()` now writes into caller-provided `&mut RenderConfig` instead of returning `Arc<RenderConfig>` (zero-alloc).
- `draw_sidebar` refactored: shared `String` buffer with `write!()` replaces ~100 `format!()` allocations per frame.
- Interactive render loop uses persistent `render_config_scratch` with `clone_from` instead of per-frame `clone()`.
- Batch charset pool: pre-allocated `[&str; 10]` array eliminates per-beat `.to_string()`.
- Creation overlay effect bars unified to 10 chars with value/max display.

### Removed
- **Webcam support**: Removed `webcam.rs`, `nokhwa` dependency, `--webcam` CLI flag, and all associated feature gates. Feature was never implemented.

## [0.4.0] — 2026-02-28

### Added
- **Creation Mode** (`K`): Auto-modulation engine with 4 presets (Ambient, Percussive, Psychedelic, Cinematic). Image-adaptive parameter adjustment based on luminance, contrast, edge density, and dominant hue.
- **MFCC timbral analysis**: 26 Mel-spaced triangular filters (300–8000 Hz), DCT-II to 5 coefficients. New audio sources: `timbral_brightness`, `timbral_roughness`, `onset_envelope`.
- **MappingCurve**: 4 response curves for audio mappings (Linear, Exponential, Threshold, Smooth).
- **Per-mapping smoothing**: Optional per-mapping EMA smoothing override via `AudioMapping.smoothing`.
- **Curve column** in Audio Mixer panel: 6th editable column for response curve selection.
- **Dither mode toggle** (`n`): Cycle between Bayer8x8, BlueNoise16, and Off.

### Changed
- `apply_audio_mappings` signature extended with `onset_envelope` and `smooth_state` parameters.
- `AudioMapping` struct extended with `curve` and `smoothing` fields.
- Help overlay updated with all new keybindings. Charset range corrected to `1-0`.
- Creation Mode modulation rewritten: proportional per-frame set (no accumulation bug). Manual mode allows direct effect adjustment via Left/Right on selected effect.

### Fixed
- **Key routing**: `K` (Creation Mode) and `n` (dither toggle) now correctly dispatched in main event loop.
- `DitherMode::BlueNoise64` renamed to `BlueNoise16` to match actual 16x16 matrix. Serde alias preserves backward compatibility with existing TOML configs.

## [0.3.0] — 2026-02-28

### Added
- **8 post-processing effects**: Chromatic Aberration (`r/R`), Wave Distortion (`w/W`), Color Pulse (`h/H`), Scan Lines (`l/L`), Strobe (`t/T`), Fade Trails (`f/F`), Glow (`g/G`), Temporal Stability (auto).
- **Blue Noise 16x16 dithering**: Perceptually superior ordered dithering via `DitherMode` enum.
- **Oklab color space**: Perceptually uniform color processing. New `ColorMode::Oklab` variant.
- **Temporal Stability**: Anti-flicker heuristic based on character density distance.
- `ColorMode` extended with `Oklab` and `Quantized` variants.

## [0.2.0] — 2026-02-28

### Added
- **Video support**: FFmpeg subprocess decoding (DEVIATION R9). Frame pool with `Arc`, `POOL_SIZE=6`, flume channels.
- **A/V synchronization**: Clock timeout 5s + fallback wall-clock.
- **File picker**: `rfd` integration, `o/O` keys, TUI suspension, `MediaType` auto-detect.
- **Adaptive thresholds** and area sampling for video rendering.

### Fixed
- `maybe_child=None` guard (no EOF when no subprocess).
- `ffprobe` validation `found_any` flag.

## [0.1.0] — 2026-01-15

### Added
- Initial release: real-time ASCII/Unicode rendering engine.
- 6 render modes: ASCII, Braille, HalfBlock, Quadrant, Sextant, Octant.
- Audio capture (CPAL), FFT analysis, beat detection, 16 audio sources.
- Batch export pipeline with offline audio analysis, `ab_glyph` rasterizer, lossless MP4 muxer.
- 10 built-in charsets, 10 presets, TOML configuration.
- Lock-free triple-buffer + flume architecture. Zero-alloc hot paths, zero unsafe.
- CI/CD pipeline with auto-release.
