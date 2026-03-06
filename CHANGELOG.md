# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] — 2026-03-06

### Added
- **Workflow save/load** — Full workflow persistence with TUI overlays (`Ctrl+S` save, `Ctrl+W` browse/load). Saves config, source info, stem states, stem WAV files, and feature timeline in a structured directory (`workflows/<name>/`).
- **Stem WAV persistence** — Mono f32 IEEE float WAV encoder writes separated stem audio to workflow directory for reproducible playback.
- **Bincode feature timeline** — Serialized `FeatureTimeline` (`timeline.bin`) for deterministic batch replay without re-analysis.
- **`--workflow-list` CLI flag** — Lists all saved workflows with metadata (stems/timeline tags, date, description).
- **`--save-workflow` / `--load-workflow` CLI flags** — Batch and interactive workflow persistence.
- **Workflow description/tags** — Editable description field in TUI save overlay, displayed in browse overlay.
- **Flash notification system** — Green overlay notifications for workflow save/load/delete operations.
- **Serde derives on AudioFeatures** — `Serialize`, `Deserialize`, `Debug` for bincode/toml round-tripping.

### Changed
- **Rayon-parallelized stem analysis** — `BatchAnalyzer::analyze_stems()` now uses `par_iter()` across 4 stems for faster offline analysis.
- **Batch export workflow integration** — `--save-workflow` is now processed inside `run_batch_export()` where stem data is available, instead of post-export in main.

### Fixed
- **CI parity** — `cargo fmt`, `clippy -D warnings` with `--all-targets`, `items_after_test_module` in sextants.rs, test module lint allows.

## [1.3.0] — 2026-03-05

### Added
- **Stem separation** (`af-stems` crate) — Music source separation via SCNet (Python subprocess). 4 stems: drums, bass, other, vocals.
- **Stem overlay** (`S` key) — TUI overlay for per-stem control: mute, solo, volume, spectrum visualization, beat indicators.
- **Per-stem audio-reactive pipeline** — 4 independent FFT pipelines with BeatDetector, MFCC, FeatureSmoother. Combined features feed existing mapping pipeline.
- **Stem playback** — Lock-free cpal mixer with atomic mute/solo/volume. MediaClock sync for seek.
- **Setup scripts** — `scripts/setup_stems.bat` (Windows) and `scripts/setup_stems.sh` (Unix) for automated Python/uv/SCNet environment setup.

### Fixed
- **Audio reactivity in all render modes** — Contrast and brightness adjustments now apply to HalfBlock, Braille, Quadrant, Sextant, and Octant modes (previously ASCII-only). All audio mappings targeting contrast/brightness now affect every render mode.
- **Edge detection confined to ASCII mode** — Edge character replacement no longer overwrites Unicode characters in non-ASCII modes (Braille, Quadrant, Sextant, Octant), preventing visual corruption.
- **MediaClock recreation per separation** — Fresh clock per stem separation prevents stale sample rate from previous sessions.

### Changed
- **Linear interpolation resampling** — Playback resampling upgraded from nearest-neighbor to linear interpolation in both `af-stems` and `af-audio` cpal callbacks.
- **Detail-mode preset density** — All Octant presets bumped to 3.0–3.5, Braille to 2.5–3.0, Sextant to 2.5, Quadrant to 1.8. Matches sub-pixel resolution of each mode.
- **Preset audio enrichment** — Detail-mode presets gain additional audio mappings (beat flash, glow, chromatic). Removed ineffective ASCII-only parameters (edge_threshold, edge_mix, shape_matching) from non-ASCII presets.
- Audio file path tracking for stem separation (`loaded_audio_path` in App).
- Full stem cleanup on audio reload (prevent resource leaks).

### Quality
- 130 tests (67 unit/integration + 63 doctests), 0 clippy warnings, 0 rustdoc warnings, 3 benchmark suites.

## [1.2.1] — 2026-03-03

### Changed
- **Conditional rayon parallelism** — Grid processing (compositor, braille, quadrant, halfblock, sextant, octant) and camera transform now dispatch to sequential iteration when cell/pixel count is below threshold (4000 cells / 50K pixels). Eliminates rayon thread-pool scheduling overhead (~200-400ns) on typical 80×24 terminals. Zero-cost for large grids (batch export, fullscreen).
- **Capture ring buffer reduced** — Audio capture ring buffer from 2 seconds (88K samples) to 100ms (4.4K samples). Sufficient for 60 FPS reads with safety margin. No latency change (ring buffer is capacity, not enforced delay).

### Quality
- 118 tests, 0 clippy warnings, 0 rustdoc warnings, 3 benchmark suites.

## [1.2.0] — 2026-03-03

### Added
- **Regression test suite** — 14 unit tests covering all bugs fixed in v1.1.1–v1.1.3 (octant LUT, double-smoothing, edge gate, beat passthrough, invert semantics, camera rotation wrap).
- **Integration test suite** — 4 test files (14 tests): audio pipeline end-to-end (sine wave → FFT → features → smoother), mapping pipeline (features → parameter deltas), preset loading (22 TOML files validated), compositor (ASCII/Octant/Braille rendering verification).
- **Criterion benchmarks** — 3 benchmark files: compositor (ASCII/Octant/Braille modes), audio (FFT/features/smoother), effects (strobe/glow/chromatic/temporal stability). Run: `cargo bench`.
- **`input_gain` parameter** — Pre-FFT sample gain [0.1, 10.0], default 1.0. Multiplies audio samples before FFT, affecting all features proportionally. Keys: `N` (down) / `M` (up). TOML: `[audio] input_gain = 1.0`.
- **CI: rustdoc + bench** — `RUSTDOCFLAGS="-D warnings" cargo doc` and `cargo bench --no-run` added to CI pipeline.

### Changed
- **`edge_mix` proportional blend restored** — `mix × magnitude > 0.5` determines edge visibility. Default edge_mix raised from 0.3 to 0.5 so the bass→edge_threshold mapping produces visible edges at moderate magnitudes.
- **Rasterizer alpha clamp** — `v.clamp(0.0, 1.0)` added before u8 cast in glyph rasterization to prevent color corruption from ab_glyph overflow.
- **af-app lib.rs created** — Exposes pipeline, generative, creation, cli modules for integration testing.

### Quality
- 118 tests (43 unit + 14 integration + 61 doctests), 0 clippy warnings, 0 rustdoc warnings.
- 3 criterion benchmark suites (compositor, audio, effects).

## [1.1.3] — 2026-03-03

### Fixed
- **Edge display condition was impossible** — compositor.rs required `normalized_mag * edge_mix > 0.5`, making edges unreachable at default `edge_mix=0.3` (would need `mag > 1.67`, max is 1.0). The bass→edge_threshold default mapping produced **zero visual effect**. Removed the impossible gate; edges now display whenever magnitude exceeds threshold.
- **beat_intensity fully attenuated by FeatureSmoother** — scale=0.5 gave attack alpha=0.3, passing only 30% of signal on first frame. A perfect beat of 1.0 was seen as 0.3. beat_intensity and onset_envelope now **bypass smoothing entirely** for full transient amplitude.
- **Bass/sub-bass smoothing too sluggish** — scale reduced from 1.3 to 0.8. Release decay tau drops from ~500ms to ~340ms for punchier bass response.

### Changed
- **10 preset recalibration** — presets 01, 02, 03, 04, 05, 07, 09, 11, 13, 19 had effective deltas < 0.05 (barely perceptible). Amounts increased 1.5-2× and sensitivities raised to MODERATE tier. Presets 01 and 05 gain a second mapping for visible modulation.
- **AUDIO_GUIDE.md** — smoother band scaling table updated (bass x0.8, beat/onset bypass).

### Quality
- 84 tests, 0 clippy warnings.

## [1.1.2] — 2026-03-03

### Fixed
- **Double-smoothing eliminated** — per-mapping EMA was redundantly applied on top of FeatureSmoother output, attenuating audio signal ~3×. Per-mapping smoothing is now opt-in only (explicit `smoothing` field required). Without it, features pass through directly. ~3× more audio signal reaches visual parameters.
- **onset_envelope synchronization** — envelope now updated BEFORE `apply_audio_mappings()` in both TUI and batch mode. Mappings using `onset_envelope` as source see current-frame value (was 1-frame late).
- **Batch onset double-decay** — onset_envelope was decayed twice per frame in batch render loop. Removed duplicate update.
- **`invert` target semantics unified** — interactive mode used threshold (`delta > 0.5`), batch used toggle (`!config.invert`). Both now use threshold for deterministic behavior.
- **HSV→RGB color clamp** — added `.clamp(0.0, 1.0)` before u8 cast to prevent color corruption on floating-point overflow.
- **camera_rotation wrap in batch** — added `rem_euclid(TAU)` in generative.rs to prevent unbounded float drift during long renders. Matches interactive pipeline.
- **Per-mapping smoothing framerate-corrected** — `alpha_corrected = 1 - (1-alpha)^(60/fps)` ensures identical behavior at 30 and 60 FPS.

### Changed
- **Default audio sensitivity** — 1.5 → 2.0 for stronger baseline reactivity.
- **Default mapping amounts** — bass 0.5→0.7, spectral_flux 0.6→0.8, rms 0.25→0.4, beat_intensity 0.8→1.2, spectral_centroid 0.5→0.7.
- **Default bass→edge_threshold curve** — Exponential → Smooth (Hermite smoothstep). Less aggressive compression of moderate signals (Smooth(0.3) = 0.216 vs Exponential(0.3) = 0.09).
- **13 presets recalibrated** — 9 under-reactive presets boosted (amounts, sensitivity, smoothing), 4 over-reactive presets attenuated (×0.7 amounts) to compensate for double-smoothing elimination.
- **AUDIO_GUIDE.md** — smoothing documentation updated (opt-in per-mapping, feature-level global), BPM normalization corrected (/ 300), invert target description corrected (threshold, not toggle).

### Quality
- 84 tests (25 unit + 59 doctests), 0 clippy warnings, 0 rustdoc warnings.
- Full architecture audit: pipeline parity verified, no double-processing, no redundant allocations.

## [1.1.1] — 2026-03-03

### Fixed
- **Octant mode: real Unicode 16.0 characters** — LUT reimplemented from UnicodeData.txt: 230 true octant codepoints (U+1CD00–U+1CDE5) + 18 Block Elements + 6 Braille fallback. Replaces previous 100% Braille degradation that caused "?" on some terminals.
- **char_density() quadrants** — single quadrants now 0.25, half blocks 0.5, three-quarter blocks 0.75 (was flat 0.25 for all). Fixes temporal stability accuracy.
- **char_density() sextants** — correct reverse-mapping from codepoint offset (was using raw offset bit count, incorrect due to Unicode gaps at indices 21/42).
- **char_density() octants** — heuristic density from codepoint range position (was dead code targeting U+1CD00+ which the old LUT never produced).
- **HalfBlock invert** — `config.invert` now respected in HalfBlock mode (was ignored via `_config` prefix).
- **Batch mode: Octant in mode cycle** — `RenderMode::Octant` added to batch mutation mode rotation (was missing).
- **Batch charset_pool alignment** — pool indices 0–9 now match TUI key mapping (was completely different ordering). Indices 10–13 added for TOML/batch-only charsets (Short2, Extended, Discrete, Hires).

### Changed
- **REFERENCE.md charset table** — corrected indices 3–9 to match actual code (Blocks, Minimal, Glitch1, Glitch2, Edge, Digital, Binary). Previous table showed obsolete mapping (Short2, Binary, Extended, Discrete, Blocks, Minimal).
- **REFERENCE.md charset display order** — now lightest→densest (was reversed).
- **REFERENCE.md HalfBlock description** — corrected to "`▄` with fg (bottom) / bg (top) colors" (was "`▄` `▀`").
- **REFERENCE.md Additional Charsets** — added Short2, Extended, Discrete, Hires as TOML-only charsets with correct character counts.
- **REFERENCE.md target_fps range** — corrected to 15–120 (was "30, 60", mismatched with `clamp(15, 120)`).
- **config.rs comments** — RenderMode doc now lists all 6 variants; charset_index doc corrected from "5 presets" to "0–9".
- **color.rs rustdoc** — escaped `[0,255]` bracket notation in 4 doc comments to eliminate rustdoc warnings.

### Quality
- 84 tests (25 unit + 59 doctests), 0 clippy warnings, 0 rustdoc warnings.
- Octant LUT generator script: `scripts/gen_octant_lut.py` (Python/uv, downloads Unicode 16.0 UnicodeData.txt).

## [1.1.0] — 2026-03-03

### Added
- **Playback position display**: `▶ MM:SS` in sidebar Audio section, reads from `MediaClock` (video/audio file playback).
- **Parameter reset** (`Backspace`): Resets all `RenderConfig` to defaults, preserving loaded media, threads, and media clock.
- **Parameter change flash**: Green `●` indicator on sidebar title for 3 frames on toggle, 6 frames on full reset.
- **Scrollable Help overlay**: `Up`/`Down` scroll with `▲▼` indicators in title. Backspace hint added to Navigation section.
- **Overlay background dim**: Canvas behind Help and CharsetEdit overlays dimmed (RGB/3 foreground, dark background) for visual separation.
- **Compact sidebar mode**: Zero-value effects hidden when terminal height < 30 rows.
- **Sidebar perf warning**: `⚠` shown in sidebar title when frame budget exceeded for 10+ consecutive frames.

### Changed
- **Creation Mode overlay**: Docked to top-right (44 cols) instead of center-screen. Non-invasive — leaves canvas visible on left.
- **Help overlay**: Camera and audio sections compacted into 2-line format. Total line count reduced. Footer updated to `↑↓ scroll  ? or Esc close`.
- **Footer action hints**: `[o]Med [O]Aud K●/K○` replaces `o/O C K●` — clearer discoverability with creation mode status.
- **Help state isolation**: Help mode now intercepts all keys (Up/Down scroll, ?/Esc close only). No unintended parameter changes while reading help.
- **Sidebar formatting**: `format!()` direct replaces `buf.clone()` intermediate — eliminates unnecessary String allocation per sidebar line.

### Fixed
- **Audio playback from video**: Loading a video via UI file dialog (`o`) now correctly starts audio playback from the video's soundtrack, enabling audio-reactive features.

### Quality
- 0 clippy warnings (pedantic + deny), 0 compilation warnings.

## [1.0.4] — 2026-03-03

### Fixed
- **CRITICAL — Anti-aliasing downsampling**: 2:1 decimation in symphonia decoder now uses 4-tap FIR low-pass pre-filter (moving average), preventing spectral fold-back that corrupted high-frequency features (brilliance, presence, spectral_centroid) on 44.1kHz+ audio.
- **Spectral centroid normalization**: divided by actual Nyquist frequency (`sample_rate / 2`) instead of hardcoded 20kHz. Fixes centroid bias on 24kHz decoded audio (was capped at 0.6 instead of 1.0).
- **Silence guard parity**: batch onset detection now uses spectral energy threshold (`> 1e-6`) matching live mode, replacing inconsistent RMS-based guard (`> 1e-4`).
- **BPM normalization range**: `bpm / 200.0` → `bpm / 300.0` in both pipeline.rs and generative.rs, covering the full [30, 300] BPM clamp range for drum & bass and speedcore genres.

### Changed
- Named constants with documentation: `BAND_ENERGY_GAIN` (features.rs), `FLUX_GAIN` (beat.rs), `MFCC_BRIGHTNESS_SCALE` / `MFCC_ROUGHNESS_SCALE` (state.rs). All empirical gain/normalization values now documented.
- `AudioMapping::offset` field now `#[serde(default)]` — optional in TOML (defaults to 0.0).

### Quality
- 83 tests + 1 video-feature test, 0 clippy warnings.
- Audio pipeline audited against librosa, essentia, aubio standards.

## [1.0.3] — 2026-03-03

### Changed
- **Complete preset redesign**: 22 presets rewritten from scratch based on SOTA VJ research (Resolume, Synesthesia, IRCAM spectral mapping). Ordered from most faithful (01_pure_photo) to most chaotic (21_glitch_storm), with 22_hires_export for batch.
- New naming convention: descriptive, ordered by visual intensity.
- Audio mappings calibrated per VJ best practices: bass→spatial, mids→tonal, highs→detail, onset→impact.
- Smoothing/sensitivity tuned per preset character: 0.15 for glitch, 0.9 for contemplative.
- All 6 render modes represented, all 4 color modes used, all 8 effects showcased.

### Quality
- 83 tests, 0 clippy warnings.
- All 22 presets parse and load correctly.

## [1.0.2] — 2026-03-03

### Fixed
- **UI launch regression**: `validate_source()` no longer requires a visual source — TUI launches with empty canvas when no `--image`/`--video` is provided.
- **RenderConfig::default() sync**: 7 divergences between code defaults and `config/default.toml` resolved — code now matches TOML (Octant, BlueNoise16, Oklab, 60fps, spectrum off, wave_speed 0.0, temporal_stability 0.3).
- **Enum `#[default]` attributes**: `RenderMode`, `ColorMode`, `DitherMode` enum derive defaults now match `RenderConfig::default()` (Octant, Oklab, BlueNoise16). Doctests updated accordingly.

### Changed
- **Documentation consolidation**: 10 docs reduced to 6 (README, CHANGELOG, CLAUDE.md, USAGE.md, AUDIO_GUIDE.md, REFERENCE.md). Zero duplication. All defaults synchronized with code. Stale Audio Mixer Panel references removed (feature removed in v0.9.0).
- **Workspace version**: bumped to 1.0.2.

### Quality
- 83 tests (28 unit + 55 doctests), 0 clippy warnings.

## [1.0.1] — 2026-03-02

### Fixed
- **CRITICAL — Sextant bit ordering**: `dy*2+dx` (row-major) corrected to `dx*3+dy` (column-major) matching U+1FB00 LUT specification.
- **HIGH — scanline_darken deserialization**: field added to `RenderSection` and `load_config()` — TOML `scanline_darken` values now correctly applied.
- **aspect_ratio clamped**: `clamp_all()` now bounds aspect_ratio to [0.1, 10.0].
- **beat_intensity normalization**: added to `FeatureTimeline::normalize()` min-max pass.
- **validate_source error propagation**: `let _ =` replaced with `?` for proper CLI error reporting.
- **invert mapping**: changed from toggle (`!config.invert`) to level-set (`delta > 0.5`) preventing oscillation.
- **camera_rotation wrapping**: `rem_euclid(TAU)` prevents float precision degradation over time.
- **FFT Hann window**: NaN guard for `size==1` (division by zero in `size-1`).
- **color_pulse hue**: `% 1.0` replaced with `.rem_euclid(1.0)` for negative hue safety.
- **Batch MFCC**: timbral_brightness and timbral_roughness now computed in `BatchAnalyzer` (parity with live mode).
- **Batch min_frames**: hardcoded `60` replaced with actual `fps` parameter.

### Removed
- `luminance.rs` module (unused `process_luminance`).
- `geometry/` module (`box_drawing_char`, `PETSCII_TO_UNICODE` — unused).
- `edge_char()` and `gradient()` functions from `edge.rs` (unused).

### Quality
- 83 tests (28 unit + 55 doctests), 0 clippy warnings.

## [1.0.0] — 2026-03-01

### Fixed
- **zalgo_intensity bounds mismatch**: generative.rs clamped [0,1] instead of [0,5] — batch export zalgo now at full range parity with interactive mode.
- **Spectral flux normalization**: beat detection now volume-independent (normalized by bin count in both real-time and offline analyzers).
- **Beat detection silence guard**: explicit spectral energy / RMS threshold prevents false onset detection during silent passages.
- **onset_envelope offline computation**: FeatureTimeline now computes onset_envelope with exponential decay in BatchAnalyzer, enabling strobe/flash parity in batch exports.
- **Color quantization**: nearest-neighbor rounding replaces floor division, reducing maximum quantization error from ~20% to ~10%.
- **Empty batch folder**: explicit error message instead of silent u32::MAX fallback.

### Removed
- **FileOrFolderPrompt dead code**: unreachable AppState/RenderState variant, handler, overlay, and state mapping removed from app.rs and ui.rs.

### Added
- **af-export test coverage**: 6 unit tests (rasterizer cell dimensions, target dimensions, glyph cache, render grid, dimension mismatch safety, muxer).
- **af-source test coverage**: 3 unit tests (blend_frames endpoints, blend_frames different sizes, scan_dir extension filtering).
- **af-core test coverage**: 2 unit tests (onset_envelope normalization, energy levels classification).

### Quality
- 90 tests total (28 unit + 62 doctests) at release, all 7 crates with test coverage.
- 0 clippy warnings (pedantic + deny).
- All audit findings from 6-agent comprehensive review verified and resolved.

## [0.9.0] — 2026-03-01

### Added
- **`--preset all`**: Multi-preset batch generation — cycles through all available presets with smooth interpolated transitions. Preset changes are triggered by energy transitions or time expiry.
- **`--seed <N>`**: Reproducible batch exports — same seed produces identical mutation sequences.
- **`--preset-duration <SECS>`**: Control maximum duration per preset in `--preset all` mode (default 15s).
- **`--crossfade-ms <MS>`**: Override adaptive crossfade duration between media clips (default: energy-adaptive).
- **`--mutation-intensity <F>`**: Scale mutation probabilities (0=none, 1=default, 2=aggressive).
- **Camera burst mutations**: 4 variants (zoom pulse, rotation pulse, pan X/Y drift) triggered on strong beats with smoothstep easing.
- **Zalgo and Fade effect bursts**: 2 new burst types (was 4, now 6): `zalgo_intensity` and `fade_decay` bursts.
- **Smooth mutation transitions (SmoothOverride)**: All continuous mutations use smoothstep easing (3t²−2t³) with configurable ramp-up/hold/ramp-down phases. No more abrupt visual jumps.
- **Low-energy drift**: Subtle parameter variations (glow, saturation, brightness) during quiet sections prevent visual stasis.
- **Invert/mode/color_mode auto-revert**: Discrete mutations automatically revert after countdown (90/180/180 frames) instead of persisting indefinitely.
- **Adaptive crossfade**: Energy-based crossfade duration between clips — fast (250ms) in high-energy, slow (1000ms) in low-energy sections.
- **MacroState struct**: All mutation state grouped into a single struct with `tick()`/`apply()` pattern.
- **PresetSequencer**: Energy-driven preset rotation with interpolated transitions (`interpolate_configs` lerps numeric fields, snaps discrete fields at t=0.5).
- **17 named constants**: All mutation probabilities, cooldowns, durations, and thresholds extracted from inline magic numbers.
- **4 new tests**: `smooth_override_ramp`, `interpolate_configs_endpoints`, `preset_sequencer_cycles`, `load_all_presets` (78 total).

### Changed
- **Density pulse**: Continuous range [0.4, 2.5] (was binary 0.5/2.0).
- **Effect burst**: 6 types (was 4: +zalgo, +fade).
- **Batch export signature**: `run_batch_export()` now accepts 5 additional parameters for full customization.

### Removed
- **Audio Mixer panel (A key)**: `AudioPanelState` struct, `AppState::AudioPanel` variant, `draw_audio_panel_overlay()` (162 lines), `handle_audio_panel_key()`/`adjust_panel_value()`/`toggle_panel_cell()` (120 lines), all imports and references — complete removal with zero residual code.
- **Orphan preset `01.toml`**: Unnumbered duplicate removed from `config/presets/`.
- **`#[allow(dead_code)]`**: Vestigial attribute removed from `FolderBatchSource`.

## [0.8.0] — 2026-03-01

### Added
- **Preset 20_sextant_film**: Cinematic Sextant rendering with Oklab perceptual color, soft edges, filmic glow. First preset to use `peak` audio source and per-mapping `smoothing` override.
- **Preset 21_octant_dense**: Maximum sub-pixel density (Octant mode), spectral bar charset (CHARSET_GLITCH_2), fullscreen. First preset to use `beat_phase` audio source.
- **Preset 22_hires_export**: Ultra high-resolution batch export preset optimized for `--export-scale 24-48`. CHARSET_FULL 70-char gradient, Oklab color, subtle effects. First preset to use `offset` in audio mappings.
- **CHARSET_HIRES**: New 34-character ASCII-pure charset optimized for large character cells in batch export. Excludes lowercase letters (distractingly "readable" at large scale).
- **`--preset-list` CLI flag**: Lists all available presets sorted alphabetically and exits.
- **Octant `char_density()`**: Temporal stability now handles Octant characters (U+1CD00-U+1CDE5) natively via bit-count density instead of fallback 0.5.
- **Sextant in batch mode cycle**: Sextant added to macro-generative mode rotation (was excluded since v0.6.0). 5 modes: Ascii/HalfBlock/Braille/Quadrant/Sextant.
- **Octant rasterizer cache**: Octant codepoints (U+1CD00-U+1CDE5) pre-cached in batch export rasterizer (future-proof; .notdef guard silently skips absent glyphs).
- **AudioMapping validation**: `clamp_all()` now validates mapping fields — amount clamped [-10, 10], offset [-5, 5], per-mapping smoothing [0, 1].
- **Named constants in effects.rs**: `GLOW_BRIGHTNESS_THRESHOLD` (140), `GLOW_FACTOR_SCALE` (40.0), `STABILITY_DENSITY_SCALE` (0.3) extracted from inline magic numbers.
- **3 new tests**: Oklab roundtrip (rgb→oklab→rgb ≤1 drift), CHARSET_HIRES LUT monotonicity, char_density Octant/Braille/Sextant coverage.

### Changed
- **default.toml**: `wave_amplitude` 0.1→0.0 (neutral by default — no unexpected wave effect on plain images), `wave_speed` 0.3→2.0 (ready for immediate effect when amplitude is activated manually).
- **Batch charset_pool**: Expanded from 10 to 11 entries (added CHARSET_HIRES). Modulo index now uses `charset_pool.len()` instead of hardcoded 10.
- **Preset 13_breath**: Added explicit `strobe_decay = 0.95` (was falling back to default 0.75; slow decay matches contemplative aesthetic).

### Fixed
- **Audio sources coverage**: `peak` (0→1 preset) and `beat_phase` (0→1 preset) were extracted but never used in any preset. Now used in presets 20 and 21 respectively.
- **Mapping features unused**: `offset` and per-mapping `smoothing` were implemented but never demonstrated in any preset. Now used in presets 20 and 22.
- **CHARSET_GLITCH_2 orphaned**: Spectral bar charset (`▂▃▄▅▆▇█`) was defined but never used in any preset. Now used as initial charset for preset 21_octant_dense.

## [0.7.1] — 2026-03-01

### Removed
- **Procedural mode (`--procedural`)**: CLI argument, feature flags (`procedural` in af-source and af-app Cargo.toml), and `full` feature simplified to `["video"]` only.
- **Mandelbrot generator**: `MandelbrotSource`, `procedural.rs` factory module, and `procedural/mandelbrot.rs` (174 LOC) deleted.
- **`is_camera_baked` field**: Removed from `FrameBuffer` struct (af-core), `ImageSource` (af-source), and camera early-return check (af-render). Field was only set to `true` by Mandelbrot — now dead code.
- **`rayon` dependency** from af-source (only consumer was Mandelbrot parallel evaluation).
- All documentation references to procedural mode and Mandelbrot cleaned from README, USAGE, PRESET_GUIDE, EFFECTS_REFERENCE, TIPS_AND_TRICKS.

### Changed
- **Preset 12_deep_zoom**: Renamed from "Mandelbrot Camera-Reactive" to "Camera-Reactive Deep Zoom". Now documented as working with any image or video source.

## [0.7.0] — 2026-03-01

### Added
- **Batch BeatDetector parity**: Offline `detect_onsets()` rewritten to replicate interactive `BeatDetector` logic — bass-weighted spectral flux (bass bins ×2.0), 10-frame warmup skip, FPS-adaptive cooldown (~130ms), BPM estimation via median of 16 inter-onset intervals (clamped [30, 300]), beat_phase accumulator with onset reset.
- **Feature normalization**: Min/max scaling of 16 continuous audio features across entire track to [0, 1] range. Dead-zone protection (range < 1e-6 → 0.5).
- **Energy level classification**: Sliding window RMS average (5-second window) with 30th/70th percentile thresholds → 3 energy levels (low/medium/high) driving clip pacing and mutation frequency.
- **Source crossfade**: Linear per-pixel RGBA blend between consecutive clips over `fps/2` frames (~500ms at 30fps). Smooth transitions replace hard cuts on media file changes.
- **Mutation coordination**: Cooldown (90 frames between mutation events), max 2 mutations per event, energy-scaled probabilities (high energy ×1.5, low energy ×0.3). Priority-ordered: mode → charset → effect burst → density pulse → color mode → invert flash.
- **Effect burst intensity scaling**: Burst magnitudes scale with `beat_intensity` (minimum 0.5× floor), creating proportional visual response to onset strength.
- **ETA progress logging**: Batch export logs frame count, percentage, actual FPS, and estimated remaining time every 100 frames.
- **Preset 18_spectral_bands**: Each frequency band drives a distinct effect — sub_bass→wave, low_mid→fade, high_mid→chromatic, presence→glow, brilliance→pulse. Quadrant mode, Oklab color, shape matching.
- **Preset 19_cinematic_camera**: Camera-focused preset — bass→zoom, spectral_centroid→rotation, mid→pan_x, presence→pan_y, rms→glow. HalfBlock mode, Direct color.

### Fixed
- **Scanline darken hardcoded**: Batch used `0.3` instead of `frame_config.scanline_darken`. Now reads from config.
- **Color pulse phase drift**: Phase not reset when `color_pulse_speed` set to 0, causing offset accumulation. Now resets to 0.0.
- **charset_pool duplicate**: Index 9 was `CHARSET_FULL` (duplicate of index 0). Replaced with `CHARSET_EXTENDED`.
- **Double clip advance**: `FolderBatchSource::next_frame()` auto-advanced on clip budget AND `batch.rs` advanced on onset, causing media files to be skipped. Clip budget now managed exclusively by `batch.rs`.
- **ffmpeg stderr silenced**: Both `Mp4Muxer` and `mux_audio_video()` discarded stderr via `Stdio::null()`. Now piped and logged on error — ffmpeg failures produce actionable error messages.
- **folder_batch.rs path handling**: `path.to_str().unwrap_or("")` replaced with proper `if let Some(path_str)` pattern.
- **Preset 02 Matrix**: `spectral_flux→zalgo_intensity` replaced with `spectral_flux→fade_decay` (zalgo too aggressive for Matrix aesthetic, fade_decay enhances rain effect).
- **Preset 14 Interference**: Dead `beat_phase→color_pulse_speed` mapping (beat_phase was always 0 in batch) replaced with `spectral_flux→wave_amplitude` (Exponential).
- **Preset TOML audit**: 17 presets audited — inert `wave_speed` removed from 10 presets (where `wave_amplitude=0.0`), inert `strobe_decay` removed from 6 presets (where `beat_flash_intensity=0.0`), explicit `zalgo_intensity=0.0` added to presets 01–10.
- **Graceful pipe error**: Batch export now breaks cleanly on pipe write failure instead of panicking, handling interrupted ffmpeg processes.

### Changed
- **Clip sequencing decoupled from mutations**: Proportional clip budget with energy-based pacing — high energy sections use 50% shorter clips, low energy sections use 50% longer clips. Strong onsets (>0.9 beat_intensity) can accelerate clip change during high energy only.
- **default.toml zalgo_intensity**: Default changed from 0.5 to 0.0. Presets that use zalgo specify it explicitly.

## [0.6.1] — 2026-02-28

### Added
- **DrawContext struct**: Replaces 15 individual `draw()` parameters with a single context struct, improving readability and maintainability.
- **Layout constants**: `SIDEBAR_WIDTH`, `SPECTRUM_HEIGHT`, `MIN_TERM_WIDTH`, `MIN_TERM_HEIGHT` centralized in constants. Eliminates magic numbers.
- **Ctrl+O**: Open visual file picker (alternative to lowercase `o`).
- **Shift+Tab**: Reverse render mode cycle.
- **scanline_darken**: New config field (0.0–1.0) controlling scan line darkness. Previously hardcoded.
- **Adaptive overlays**: Help, creation, and mixer overlays adapt to terminal height.
- **Condensed sidebar**: Compact layout for terminals with fewer rows.

### Fixed
- **Sidebar layout**: 4-pad key + 6-pad label format with brackets removed from key indicators. Consistent alignment across all sections.
- **Unicode truncation**: `truncate()` now respects char boundaries, preventing panics on multi-byte characters.
- **Onset decay**: Fixed exponential decay calculation for onset envelope in UI.
- **color_pulse_phase reset**: Phase now resets properly on preset change.
- **Creation bars clamped**: Effect bar values clamped to [0, max] preventing overflow rendering.
- **Scanline gap cap**: Maximum raised to 8 (was unbounded).

### Changed
- **widgets.rs merged into ui.rs**: Single rendering module instead of two, reducing indirection.

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
