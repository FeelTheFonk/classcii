//! Integration test: audio mapping pipeline.
//! Verifies: AudioFeatures + RenderConfig → apply_audio_mappings → correct parameter deltas.
#![allow(
    clippy::expect_used,
    clippy::field_reassign_with_default,
    clippy::needless_borrow,
    clippy::float_cmp
)]

use af_app::pipeline::apply_audio_mappings;
use af_core::config::RenderConfig;
use af_core::frame::AudioFeatures;

#[test]
fn default_mappings_produce_visible_deltas() {
    let mut config = RenderConfig::default();
    let mut features = AudioFeatures::default();
    features.bass = 0.5;
    features.spectral_flux = 0.6;
    features.rms = 0.3;
    features.beat_intensity = 0.8;
    features.spectral_centroid = 0.4;

    let mut smooth = vec![];
    apply_audio_mappings(&mut config, &features, 0.7, &mut smooth, 60);

    // bass → edge_threshold (Smooth curve, amount=0.7, sensitivity=2.0)
    assert!(
        config.edge_threshold > 0.3,
        "edge_threshold should be substantial, got {}",
        config.edge_threshold
    );

    // spectral_flux → contrast (Linear, amount=0.8)
    assert!(
        config.contrast > 1.5,
        "contrast should be boosted from 1.0, got {}",
        config.contrast
    );

    // rms → brightness (Linear, amount=0.4)
    assert!(
        config.brightness > 0.1,
        "brightness should be positive, got {}",
        config.brightness
    );

    // beat_intensity → beat_flash_intensity (Smooth, amount=1.2)
    assert!(
        config.beat_flash_intensity > 1.0,
        "beat_flash should be strong, got {}",
        config.beat_flash_intensity
    );

    // spectral_centroid → glow_intensity (Linear, amount=0.7)
    assert!(
        config.glow_intensity > 0.3,
        "glow should be visible, got {}",
        config.glow_intensity
    );
}

#[test]
fn zero_features_produce_zero_deltas() {
    let mut config = RenderConfig::default();
    let features = AudioFeatures::default(); // all zeros
    let mut smooth = vec![];

    let original_contrast = config.contrast;
    let original_brightness = config.brightness;

    apply_audio_mappings(&mut config, &features, 0.0, &mut smooth, 60);

    assert!(
        (config.contrast - original_contrast).abs() < 0.01,
        "zero features should not change contrast"
    );
    assert!(
        (config.brightness - original_brightness).abs() < 0.01,
        "zero features should not change brightness"
    );
}

#[test]
fn disabled_mapping_has_no_effect() {
    let mut config = RenderConfig::default();
    // Disable all mappings
    for mapping in &mut config.audio_mappings {
        mapping.enabled = false;
    }

    let mut features = AudioFeatures::default();
    features.bass = 1.0;
    features.rms = 1.0;
    let mut smooth = vec![];

    apply_audio_mappings(&mut config, &features, 1.0, &mut smooth, 60);

    assert!(
        config.edge_threshold < 0.01,
        "disabled mappings should not affect edge_threshold"
    );
}
