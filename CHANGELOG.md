# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
