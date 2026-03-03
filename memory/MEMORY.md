# classcii Project Memory

## Architecture
- 7 crates: af-core, af-audio, af-ascii, af-render, af-app, af-export, af-source
- ~11k LOC Rust, edition 2024, MSRV 1.88
- Zero-unsafe, rayon parallel, triple buffer audio, ArcSwap config

## Version History (This Session)
- v1.1.1: Octant Unicode 16.0 LUT (230 real codepoints), char_density fix, HalfBlock invert, REFERENCE.md charset table
- v1.1.2: Double-smoothing eliminated, onset_envelope sync, invert unified, HSV clamp, camera_rotation wrap, 13 presets recalibrated
- v1.1.3: Edge gate impossible fixed (compositor.rs), beat_intensity/onset_envelope bypass smoother, bass scale 1.3→0.8, 10 presets boosted

## Key Design Decisions
- Per-mapping smoothing is OPT-IN (smoothing: field in TOML). Without it, features pass through directly from FeatureSmoother.
- beat_intensity and onset_envelope bypass FeatureSmoother entirely (transient passthrough)
- Bass/sub-bass smoother scale = 0.8 (was 1.3)
- Default audio_sensitivity = 2.0 (was 1.5)
- Default bass curve = Smooth (was Exponential)
- edge_mix proportional blend: mix*mag > 0.5 threshold (restored in v1.2.0 plan)

## v1.2.0 Plan (IN PROGRESS)
- AXE 1: Regression tests — DONE (12 new tests, 96 total)
- AXE 2: Integration tests — TODO (4 new test files in tests/)
- AXE 3: Benchmarks criterion — TODO
- AXE 4: edge_mix proportional restore — TODO
- AXE 5: input_gain parameter — TODO
- AXE 6: CI hardening (rustdoc, bench) — TODO
- AXE 7: Documentation + release — TODO

## Critical Bugs Found & Fixed
1. Octant LUT produced 0 real octant chars (100% Braille fallback)
2. Double-smoothing: per-mapping EMA redundant with FeatureSmoother
3. Edge gate impossible: mix*mag>0.5 with mix=0.3 requires mag>1.67
4. beat_intensity attenuated to 30% by FeatureSmoother (scale=0.5)
5. onset_envelope 1-frame delay (updated after mapping instead of before)
6. invert semantics diverged (pipeline=threshold, generative=toggle)
7. HSV→RGB missing clamp (potential color corruption)
8. camera_rotation not wrapped in generative (unbounded drift)
9. Batch onset double-decay (updated twice per frame)

## CI
- .github/workflows/ci.yml: fmt, clippy, test, MSRV 1.88, cross-build (Win+Linux)
- .github/workflows/release.yml: tag-triggered, binary packaging, GitHub Release
