# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] — 2026-02-28

### Added
- **Color mode parity**: `ColorMode` (HsvBright, Oklab, Quantized) now applied to ALL 6 render modes. Previously only worked in ASCII mode — Braille, HalfBlock, Quadrant, Sextant, Octant now receive full color processing.
- **Mandelbrot color palette**: Smooth HSV cyclic coloring replaces grayscale output. 3 hue cycles across iteration range with fade-in near set boundary.
- **Mandelbrot adaptive max_iter**: Iteration limit scales with zoom depth (100→1000), preserving fractal detail at deep zoom levels.
- **Config validation**: All TOML numeric values clamped to valid ranges on load via `RenderConfig::clamp_all()`. Prevents undefined behavior from out-of-range config values.
- **Temporal stability Sextant coverage**: Sextant characters (U+1FB00–U+1FB3B) now have proper density heuristics based on bit count instead of fallback 0.5.
- **default.toml**: Added missing `beat_intensity→beat_flash_intensity` and `spectral_centroid→glow_intensity` audio mappings.

### Fixed
- **Charset ordering**: 4 charsets (FULL, DENSE, SHORT_2, DISCRETE) were reversed (densest→lightest), causing inverted luminance mapping. Corrected to lightest→densest.
- **CHARSET_EXTENDED broken**: Non-monotonic repeating pattern replaced with clean ASCII+Unicode gradient `" .·:;+xX#%@"`.
- **Sidebar charset names**: Names array was mismatched with actual key→charset mapping. Corrected to match indices 0–9.
- **UI startup canvas offset**: `canvas_height` always subtracted 3 for spectrum bar even when `show_spectrum=false`. Now spectrum-aware.
- **Sidebar shows base config**: Sidebar was displaying audio-modulated `render_config`, making keybind changes appear ineffective when presets with audio mappings were active. Now displays stored base config.
- **Resize trigger for render params**: Tab (render_mode), d/D (density_scale), a (aspect_ratio) now force pixel dimension recalculation. Previously only terminal size changes triggered resize, causing degraded sub-pixel resolution when switching modes.
- **Rasterizer .notdef glyphs**: Characters absent from the export font (FiraCode) were cached as .notdef placeholder boxes ("?" in rectangles). Now skipped — missing glyphs render as transparent instead of artifacts.
- **Batch mode rotation**: Sextant and Octant removed from macro mode cycle (glyphs absent from FiraCode). Batch charset pool limited to font-safe charsets only.
- **Rasterizer R1 violation**: `empty_glyph` Vec moved from per-frame allocation to struct field. Zero-alloc in render hot path restored.
- **Rasterizer release safety**: `debug_assert` dimension check replaced with runtime early-return + `log::error` for release builds.
- **Audio decimation quality**: Nearest-neighbor sample skipping replaced with 2-tap averaging filter for anti-aliased 48kHz→24kHz downsampling.
- **classify_media false acceptance**: Removed TIFF/WEBP from accepted image extensions (image crate lacks those features, would fail silently).
- **Onset detection FPS-adaptive**: Cooldown now computed from actual FPS (~130ms) instead of fixed 4 frames. Consistent behavior at 30 and 60 FPS.
- **Onset false positives**: Added 10-frame warmup period to prevent spurious onset detection during `flux_avg` initialization.
- **fade_decay cap**: Raised effective maximum from 0.95 to 0.99 for extreme temporal persistence.
- **zalgo_intensity audio mapping clamp**: Harmonized from 0.0–1.0 to 0.0–5.0, matching TOML schema and preset values.
- **camera_rotation precision**: Normalized to [0, TAU) at usage point to prevent floating-point accumulation drift from continuous audio modulation.
- **Preset audit**: 13 of 17 presets corrected — wrong charset_index references, shape_matching on non-ASCII modes, color_mode set with color_enabled=false, overly aggressive audio mapping amounts.
- **README.md**: Removed TIFF/WebP from supported image formats list (matches implementation).
- **TIPS_AND_TRICKS.md**: Corrected `--procedural plasma` → `--procedural mandelbrot`.

### Changed
- **Effects performance**: Strobe, color pulse, scan lines, glow, fade trails, and temporal stability rewritten to use direct `cells` iteration (`iter_mut`/`zip`) instead of `get()`+`set()` double-lookup. ~50% fewer index calculations per effect per frame.
- **ShapeMatcher**: Heap-allocated `Vec<(char, u32)>` replaced with `const &'static [(char, u32)]` slice. Eliminates runtime allocation.
- **Chromatic aberration**: Read/write passes use direct indexed access on `grid.cells`.
- **Wave distortion**: Uses `copy_from_slice` + direct indexed write instead of per-cell `get`/`set`.
- **Key→charset mapping**: Key 8 remapped from CHARSET_DIGITAL to CHARSET_EDGE. Charset order rationalized for intuitive keyboard access.

## [0.5.6] — 2026-02-28

### Added
- **Animated GIF support**: GIFs loaded via `--image` or file picker now play as looping animations with frame-accurate timing. Single-frame GIFs remain static. Batch export (`--batch-folder`) supports animated GIFs.
- **Camera Pan Y keybind**: `:`/`"` control camera vertical panning (±0.05, range [-2.0, 2.0]).
- **Camera bilinear interpolation**: Virtual camera now uses bilinear interpolation instead of nearest-neighbor, eliminating aliasing artifacts during zoom and rotation. Border pixels fall back to nearest-neighbor.

### Fixed
- **Sidebar width mismatch**: `app.rs` used width 20 while `ui.rs` used 24, wasting 4 columns of render computation every frame. Synchronized to 24.
- **default.toml onset→invert ghost mapping**: Removed mapping that was documented as removed in v0.5.2 but still present in config file.
- **default.toml missing camera fields**: Added `camera_zoom_amplitude`, `camera_rotation`, `camera_pan_x`, `camera_pan_y` to `[render]` section.
- **Help overlay layout**: "Color FX" note moved to correct position (after Effects, before Camera). Pan Y entry added to Camera section.
- **Creation overlay allocation**: Replaced per-frame `Vec` allocation with fixed-size `[10]` array.

### Removed
- **Dead dependencies**: Removed unused `noise` and `glam` from `af-source` and workspace `Cargo.toml`.

## [0.5.5] — 2026-02-28

### Added
- **Lossless MP4 Export**: Muxer FFmpeg arguments updated from `-c:v libx264 -pix_fmt yuv444p` to `-c:v libx264rgb -pix_fmt rgb24` for mathematically perfect, sub-sampling free RGB rendering text output.
- **Batch Export Scaling**: Exposed new `--export-scale <FLOAT>` CLI argument to override the `Rasterizer` default (16.0) for high-resolution 4K/8K offline rendering.
- **Mandelbrot Continuous Math Field**: Implemented a zero-allocation `MandelbrotSource` procedural generator via `rayon` parallelism. Accessible via `--procedural mandelbrot`, exposing true infinite zoom.
- **Virtual Reactive Camera**: Deeply integrated zero-allocation affine transform system in real-time (`af-app`) and offline (`batch.rs`). Added `camera_zoom_amplitude`, `camera_rotation`, `camera_pan_x`, `camera_pan_y` to `RenderConfig`.
- **6 artistic presets**: deep_zoom (12), breath (13), interference (14), noir (15), aurora (16), static (17). Total: 17 presets.
- **Camera keybinds**: `</>` zoom, `,/.` rotation, `;/'` pan X. Sidebar Camera section. Help overlay Camera section.
- **Camera in Creation Mode**: Psychedelic (rotation+zoom), Abstract (pan+rotation), Spectral (zoom+pan).
- **Camera targets in generative.rs**: 4 camera targets for batch export audio-reactive camera.
- **Rasterizer cache**: Sextant U+1FB00-U+1FB3B and Latin-1 Supplement ranges cached for batch export.

### Fixed
- **Sextant LUT**: Complete rewrite of `SEXTANT_LUT[64]` with correct Unicode mappings (U+1FB00-U+1FB3B). Indices 21/42 (absent damier patterns) mapped to U+2592 (▒).
- **CLI docstring**: Procedural types corrected to "mandelbrot" only (was listing 4 non-existent types).
- **Documentation harmonization**: Targets 14→18 across all docs, codec `libx264rgb`, procedural `mandelbrot` only.

## [0.5.4] — 2026-02-28

### Fixed
- **Creation Mode Left/Right UX**: Left/Right now always adjusts the selected element (Master or effect), regardless of auto/manual mode. Previously, Left/Right only adjusted master intensity in auto mode.
- **Batch export video restart**: Videos no longer restart from 0.0 on EOF. EOF now advances to the next media file. Added proportional clip duration (`total_frames / file_count`) to ensure all media files get screen time.
- **Performance — Glow**: Reduced neighbor lookups from 8 (including diagonals) to 4 cardinal directions. ~50% fewer reads, imperceptible visual difference.
- **Performance — Color Pulse**: Skip HSV conversion on black cells `(0,0,0)`. Saves 30-60% of conversions on dark presets.
- **Performance — Shape Matching**: Auto-disabled on grids >10k cells (300×100+) where it costs 40-60ms. Logs warning once.

### Added
- **6 new Creation Mode presets**: Minimal (single dominant effect), Photoreal (sharpest rendering), Abstract (cross-mapped non-figurative), Glitch (digital corruption), Lo-Fi (vintage degraded), Spectral (per-band effect mapping). Total: 11 presets.
- **Master as index 0 in Creation Mode**: "Master" now appears as the first item in the effect list. Up/Down navigates Master (0) through Strobe Decay (9).
- **[AUTO]/[MANUAL] indicator**: Clear mode label in Creation overlay header with color coding (green/red). Auto-modulated effects display `~` suffix.
- **Frame budget tracking**: Performance warning `!` (yellow) displayed next to FPS when render time exceeds 1.5× frame budget for 10+ consecutive frames.
- **Audio feature: spectral_rolloff** (#20): Frequency below which 85% of spectral energy is concentrated. O(n) single-pass cumsum.
- **Audio feature: zero_crossing_rate** (#21): Normalized sign-change count on raw samples. Useful for percussive/noise detection.
- **Onset envelope in AudioFeatures**: `onset_envelope` field now native in `AudioFeatures` struct (was computed locally in app/batch).
- **Adaptive smoothing**: Per-frequency-band EMA multipliers — bass ×1.3 (slower), mid ×1.0, highs ×0.7 (faster), events ×0.5 (fastest).
- **Batch macro-mutations**: 3 new mutations — density pulse (8%, 30 frames), effect burst (6%, 60 frames), color mode cycle (5%). Existing probabilities increased: mode 8%→12%, invert 6%→10%, charset 12%→15%.
- **density_scale in Creation presets**: Percussive (bass-driven), Abstract (centroid-driven), Spectral (RMS-driven) with anti-thrashing (skip if delta < 0.15).
- **ColorMode PartialEq**: `ColorMode` enum now derives `PartialEq` and `Eq`.
- **21 audio sources** (was 19): Added `spectral_rolloff`, `zero_crossing_rate`.

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
