//! Integration test: preset TOML loading and validation.
//! Verifies: all presets parse, have valid sources/targets, and produce valid configs.
#![allow(
    clippy::expect_used,
    clippy::float_cmp,
    clippy::unwrap_used,
    clippy::needless_borrow
)]

use af_core::config::{AUDIO_SOURCES, AUDIO_TARGETS, load_config};
use std::path::Path;

#[test]
fn all_presets_parse_successfully() {
    let preset_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/presets");
    assert!(preset_dir.exists(), "config/presets/ directory must exist");

    let mut count = 0;
    for entry in std::fs::read_dir(preset_dir).expect("cannot read presets dir") {
        let entry = entry.expect("cannot read entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            let result = load_config(&path);
            assert!(
                result.is_ok(),
                "preset {} failed to parse: {:?}",
                path.display(),
                result.err()
            );
            count += 1;
        }
    }
    assert!(count > 0, "expected at least 1 preset on disk");
}

#[test]
fn all_presets_have_valid_sources_and_targets() {
    let preset_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/presets");

    for entry in std::fs::read_dir(preset_dir).expect("cannot read presets dir") {
        let entry = entry.expect("cannot read entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let config = load_config(&path).expect("preset should parse");
        let name = path.file_name().unwrap().to_string_lossy();

        assert!(
            !config.audio_mappings.is_empty(),
            "preset {name} has zero audio mappings"
        );

        for (i, mapping) in config.audio_mappings.iter().enumerate() {
            assert!(
                AUDIO_SOURCES.contains(&mapping.source.as_str()),
                "preset {name} mapping #{i}: unknown source '{}'",
                mapping.source
            );
            assert!(
                AUDIO_TARGETS.contains(&mapping.target.as_str()),
                "preset {name} mapping #{i}: unknown target '{}'",
                mapping.target
            );
            // Validate stem_source if present
            if let Some(ref stem) = mapping.stem_source {
                let valid_stems = ["drums", "bass", "other", "vocals"];
                assert!(
                    valid_stems.contains(&stem.as_str()),
                    "preset {name} mapping #{i}: unknown stem_source '{stem}'"
                );
            }
        }
    }
}

#[test]
fn default_config_loads_successfully() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../config/default.toml");
    let config = load_config(&path).expect("default.toml should parse");

    assert!(
        config.audio_mappings.len() >= 4,
        "default should have at least 4 mappings, got {}",
        config.audio_mappings.len()
    );
    // Verify TOML values override code defaults
    assert!(
        (config.audio_sensitivity - 1.2).abs() < f32::EPSILON,
        "sensitivity should match TOML value 1.2, got {}",
        config.audio_sensitivity
    );
    assert!(
        (config.audio_smoothing - 0.5).abs() < f32::EPSILON,
        "smoothing should match TOML value 0.5, got {}",
        config.audio_smoothing
    );
}

