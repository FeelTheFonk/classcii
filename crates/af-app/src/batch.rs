use anyhow::Result;
use std::path::Path;

#[cfg(feature = "video")]
use af_ascii::compositor::Compositor;
#[cfg(feature = "video")]
use af_audio::batch_analyzer::BatchAnalyzer;
use af_core::config::RenderConfig;
#[cfg(feature = "video")]
use af_core::frame::{AsciiCell, AsciiGrid, FrameBuffer};
#[cfg(feature = "video")]
use af_core::traits::Source;
#[cfg(feature = "video")]
use af_export::muxer::{Mp4Muxer, mux_audio_video};
#[cfg(feature = "video")]
use af_export::rasterizer::Rasterizer;

#[cfg(feature = "video")]
use af_source::folder_batch::FolderBatchSource;

#[cfg(feature = "video")]
use crate::generative::AutoGenerativeMapper;

/// Point d'entrée pour l'export génératif par lots.
///
/// # Errors
/// Retourne une erreur si l'analyse audio, le scan du dossier, ou l'encodage échoue.
#[allow(clippy::too_many_lines)]
pub fn run_batch_export(
    folder: &Path,
    audio_path_str: Option<&String>,
    final_output: Option<&Path>,
    config: RenderConfig,
    target_fps: u32,
    export_scale: Option<f32>,
) -> Result<()> {
    #[cfg(not(feature = "video"))]
    {
        let _ = (
            folder,
            audio_path_str,
            final_output,
            config,
            target_fps,
            export_scale,
        );
        anyhow::bail!("L'export par lots requiert la feature 'video' (ffmpeg support).");
    }

    #[cfg(feature = "video")]
    {
        // === Auto-discovery Audio ===
        let resolved_audio_path = if let Some(path_str) = audio_path_str {
            std::path::PathBuf::from(path_str)
        } else {
            log::info!("Recherche d'un fichier audio dans {}...", folder.display());
            let found = std::fs::read_dir(folder)?
                .filter_map(std::result::Result::ok)
                .map(|e| e.path())
                .find(|p| {
                    if let Some(ext) = p.extension().and_then(|s| s.to_str()) {
                        let e = ext.to_lowercase();
                        e == "mp3" || e == "wav" || e == "flac" || e == "ogg" || e == "aac"
                    } else {
                        false
                    }
                });
            match found {
                Some(p) => {
                    log::info!("Audio trouvé : {}", p.display());
                    p
                }
                None => anyhow::bail!("Aucun fichier audio trouvé dans le dossier."),
            }
        };

        // === Auto-naming Output ===
        let resolved_output_path = if let Some(out_path) = final_output {
            out_path.to_path_buf()
        } else {
            let folder_name = folder
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("batch");
            let timestamp = {
                use std::time::SystemTime;
                let secs = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let s = secs % 60;
                let m = (secs / 60) % 60;
                let h = (secs / 3600) % 24;
                let days = secs / 86400;
                let mut y = 1970u64;
                let mut remaining = days;
                loop {
                    let days_in_year = if y.is_multiple_of(4)
                        && (!y.is_multiple_of(100) || y.is_multiple_of(400))
                    {
                        366
                    } else {
                        365
                    };
                    if remaining < days_in_year {
                        break;
                    }
                    remaining -= days_in_year;
                    y += 1;
                }
                let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
                let mdays: [u64; 12] = [
                    31,
                    if leap { 29 } else { 28 },
                    31,
                    30,
                    31,
                    30,
                    31,
                    31,
                    30,
                    31,
                    30,
                    31,
                ];
                let mut mo = 0u64;
                for &md in &mdays {
                    if remaining < md {
                        break;
                    }
                    remaining -= md;
                    mo += 1;
                }
                format!("{y}{:02}{:02}_{h:02}{m:02}{s:02}", mo + 1, remaining + 1)
            };
            let default_name = format!("{folder_name}_{timestamp}.mp4");
            let mut p = std::env::current_dir()?;
            p.push(default_name);
            p
        };

        let audio_path = resolved_audio_path.as_path();
        let final_output = resolved_output_path.as_path();

        // === Étape 1 : Pré-analyse audio complète (offline) ===
        log::info!("Étape 1/4 : Analyse Audio de {}", audio_path.display());
        let mut analyzer = BatchAnalyzer::new(target_fps, 44100, 2048);
        let timeline = analyzer.analyze_file(audio_path)?;

        let mut mapper = AutoGenerativeMapper::new(config, timeline);

        // === Étape 2 : Initialisation de la source dossier ===
        log::info!(
            "Étape 2/4 : Initialisation Media Folder {}",
            folder.display()
        );
        let total_frames_u32 = mapper.get_timeline().total_frames() as u32;
        let mut source = FolderBatchSource::new(folder, target_fps, total_frames_u32)?;

        let (native_w, native_h) = source.native_size();
        let target_w = native_w.max(1280);
        let target_h = native_h.max(720);

        let grid_w = (target_w / 8) as u16;
        let grid_h = (target_h / 16) as u16;

        let mut grid = AsciiGrid::new(grid_w, grid_h);

        // === Étape 3 : Pipeline de rendu (Compositor + Rasterizer + Muxer) ===
        log::info!("Étape 3/4 : Préparation de l'encodeur FFmpeg");

        let font_data = include_bytes!("../../af-export/assets/FiraCode-Regular.ttf");
        let scale_val = export_scale.unwrap_or(16.0);
        let rasterizer = Rasterizer::new(font_data, scale_val)?;

        let (raster_w, raster_h) = rasterizer.target_dimensions(grid_w, grid_h);

        let temp_video = final_output.with_extension("temp.mp4");
        let mut muxer = Mp4Muxer::new(&temp_video, raster_w, raster_h, target_fps)?;

        let mut frame_config = RenderConfig::default();
        mapper.apply_at(0.0, 0.0, &mut frame_config);
        let mut compositor = Compositor::new(&frame_config.charset);
        let mut raster_fb = FrameBuffer::new(raster_w, raster_h);

        let total_frames = mapper.get_timeline().total_frames();
        let frame_duration = 1.0 / f64::from(target_fps);

        let mut resizer = af_source::resize::Resizer::new();
        let mut resized_source = FrameBuffer::new(target_w, target_h);
        let mut transformed_source = FrameBuffer::new(1, 1);

        // === Pre-allocated effect buffers (R1 compliance) ===
        let mut prev_grid = AsciiGrid::new(grid_w, grid_h);
        let mut glow_brightness_buf: Vec<u8> =
            Vec::with_capacity(usize::from(grid_w) * usize::from(grid_h));
        let mut effect_fg_buf: Vec<(u8, u8, u8)> = Vec::new();
        let mut effect_row_buf: Vec<AsciiCell> = Vec::new();
        let mut onset_envelope: f32 = 0.0;
        let mut color_pulse_phase: f32 = 0.0;
        let mut wave_phase: f32 = 0.0;

        // === Pre-allocated charset pool (avoid per-beat .to_string()) ===
        let charset_pool: [&str; 10] = [
            af_core::charset::CHARSET_FULL,
            af_core::charset::CHARSET_DENSE,
            af_core::charset::CHARSET_SHORT_1,
            af_core::charset::CHARSET_BLOCKS,
            af_core::charset::CHARSET_MINIMAL,
            af_core::charset::CHARSET_GLITCH_1,
            af_core::charset::CHARSET_GLITCH_2,
            af_core::charset::CHARSET_DIGITAL,
            af_core::charset::CHARSET_EXTENDED,
            af_core::charset::CHARSET_BINARY,
        ];

        log::info!("Boucle de Rendu : {total_frames} frames à {target_fps}fps");

        let mut macro_mode_override: Option<af_core::config::RenderMode> = None;
        let mut macro_invert_override: Option<bool> = None;
        let mut macro_charset_override: Option<(usize, String)> = None;
        // C.3: New macro-mutations
        let mut macro_density_override: Option<f32> = None;
        let mut macro_density_countdown: u32 = 0;
        let mut macro_effect_burst: Option<(u8, f32)> = None; // (effect_id, value)
        let mut macro_effect_countdown: u32 = 0;
        let mut macro_color_mode_override: Option<af_core::config::ColorMode> = None;

        for frame_idx in 0..total_frames {
            let timestamp_secs = frame_idx as f64 * frame_duration;
            let current_features = mapper.get_timeline().get_at_time(timestamp_secs);

            // Apply audio mappings with curve + smoothing (zero-alloc: reuses frame_config)
            mapper.apply_at(timestamp_secs, onset_envelope, &mut frame_config);

            // === True Generative Clip Sequencing ===
            if current_features.onset && current_features.beat_intensity > 0.85 {
                source.next_media();

                // Mode cycle (12%)
                if fastrand::f64() < 0.12 {
                    let modes = [
                        af_core::config::RenderMode::Ascii,
                        af_core::config::RenderMode::HalfBlock,
                        af_core::config::RenderMode::Braille,
                        af_core::config::RenderMode::Quadrant,
                        af_core::config::RenderMode::Sextant,
                        af_core::config::RenderMode::Octant,
                    ];
                    let current = macro_mode_override
                        .as_ref()
                        .unwrap_or(&frame_config.render_mode);
                    let current_mode_idx = modes.iter().position(|m| m == current).unwrap_or(0);
                    macro_mode_override = Some(modes[(current_mode_idx + 1) % modes.len()].clone());
                }

                // Invert flash (10%)
                if fastrand::f64() < 0.10 {
                    let current = macro_invert_override.unwrap_or(frame_config.invert);
                    macro_invert_override = Some(!current);
                }

                // Charset rotation (15%)
                if fastrand::f64() < 0.15 {
                    let current_idx = macro_charset_override
                        .as_ref()
                        .map_or(frame_config.charset_index, |(i, _)| *i);
                    let new_idx = (current_idx + 1) % 10;
                    let mut new_charset = String::new();
                    new_charset.push_str(charset_pool[new_idx]);
                    macro_charset_override = Some((new_idx, new_charset));
                }

                // Density pulse (8%): 0.5 or 2.0 for 30 frames
                if fastrand::f64() < 0.08 {
                    macro_density_override = Some(if fastrand::bool() { 0.5 } else { 2.0 });
                    macro_density_countdown = 30;
                }

                // Effect burst (6%): random effect boost for 60 frames
                if fastrand::f64() < 0.06 {
                    let bursts: [(u8, f32); 4] = [(0, 1.5), (1, 2.5), (2, 0.4), (3, 2.0)];
                    let pick = bursts[fastrand::usize(0..bursts.len())];
                    macro_effect_burst = Some(pick);
                    macro_effect_countdown = 60;
                }

                // Color mode cycle (5%)
                if fastrand::f64() < 0.05 {
                    let modes = [
                        af_core::config::ColorMode::Direct,
                        af_core::config::ColorMode::HsvBright,
                        af_core::config::ColorMode::Oklab,
                        af_core::config::ColorMode::Quantized,
                    ];
                    let current = macro_color_mode_override
                        .as_ref()
                        .unwrap_or(&frame_config.color_mode);
                    let idx = modes.iter().position(|m| m == current).unwrap_or(0);
                    macro_color_mode_override = Some(modes[(idx + 1) % modes.len()].clone());
                }
            }

            // Decay countdowns for temporary mutations
            if macro_density_countdown > 0 {
                macro_density_countdown -= 1;
                if macro_density_countdown == 0 {
                    macro_density_override = None;
                }
            }
            if macro_effect_countdown > 0 {
                macro_effect_countdown -= 1;
                if macro_effect_countdown == 0 {
                    macro_effect_burst = None;
                }
            }

            // Apply macro overlays
            if let Some(ref m) = macro_mode_override {
                frame_config.render_mode = m.clone();
            }
            if let Some(inv) = macro_invert_override {
                frame_config.invert = inv;
            }
            if let Some((idx, ref chars)) = macro_charset_override {
                frame_config.charset_index = idx;
                frame_config.charset.clone_from(chars);
            }
            if let Some(density) = macro_density_override {
                frame_config.density_scale = density;
            }
            if let Some((effect_id, value)) = macro_effect_burst {
                match effect_id {
                    0 => frame_config.glow_intensity = value,
                    1 => frame_config.chromatic_offset = value,
                    2 => frame_config.wave_amplitude = value,
                    3 => frame_config.color_pulse_speed = value,
                    _ => {}
                }
            }
            if let Some(ref cm) = macro_color_mode_override {
                frame_config.color_mode = cm.clone();
            }

            if let Some(src_frame) = source.next_frame() {
                if transformed_source.width != src_frame.width
                    || transformed_source.height != src_frame.height
                {
                    transformed_source = FrameBuffer::new(src_frame.width, src_frame.height);
                }

                af_render::camera::VirtualCamera::apply_transform(
                    &frame_config,
                    &src_frame,
                    &mut transformed_source,
                );

                let _ = resizer.resize_into(&transformed_source, &mut resized_source);

                compositor.update_if_needed(&frame_config.charset);
                compositor.process(
                    &resized_source,
                    Some(&current_features),
                    &frame_config,
                    &mut grid,
                );

                // === Full effect pipeline (parity with interactive app.rs) ===

                // 0. Temporal stability (anti-flicker)
                if frame_config.temporal_stability > 0.0 {
                    af_render::effects::apply_temporal_stability(
                        &mut grid,
                        &prev_grid,
                        frame_config.temporal_stability,
                    );
                }

                // Onset envelope tracking
                if current_features.onset {
                    onset_envelope = 1.0;
                } else {
                    onset_envelope *= frame_config.strobe_decay;
                }

                // Color pulse phase update
                if frame_config.color_pulse_speed > 0.0 {
                    color_pulse_phase = (color_pulse_phase
                        + frame_config.color_pulse_speed / target_fps as f32)
                        % 1.0;
                }

                // 1. Wave distortion (persistent phase + beat modulator)
                if frame_config.wave_amplitude > 0.001 {
                    wave_phase = (wave_phase + frame_config.wave_speed / target_fps as f32)
                        % std::f32::consts::TAU;
                }
                let wave_phase_total =
                    wave_phase + current_features.beat_phase * std::f32::consts::TAU * 0.5;
                af_render::effects::apply_wave_distortion(
                    &mut grid,
                    frame_config.wave_amplitude,
                    frame_config.wave_speed,
                    wave_phase_total,
                    &mut effect_row_buf,
                );

                // 2. Chromatic aberration
                af_render::effects::apply_chromatic_aberration(
                    &mut grid,
                    frame_config.chromatic_offset,
                    &mut effect_fg_buf,
                );

                // 3. Color pulse (hue rotation)
                af_render::effects::apply_color_pulse(&mut grid, color_pulse_phase);

                // 4. Fade trails
                if frame_config.fade_decay > 0.0 {
                    af_render::effects::apply_fade_trails(
                        &mut grid,
                        &prev_grid,
                        frame_config.fade_decay,
                    );
                }

                // 5. Strobe
                af_render::effects::apply_strobe(
                    &mut grid,
                    onset_envelope,
                    frame_config.beat_flash_intensity,
                );

                // 6. Scan lines
                af_render::effects::apply_scan_lines(&mut grid, frame_config.scanline_gap, 0.3);

                // 7. Glow
                if frame_config.glow_intensity > 0.0 {
                    af_render::effects::apply_glow(
                        &mut grid,
                        frame_config.glow_intensity,
                        &mut glow_brightness_buf,
                    );
                }

                // Save grid for next frame's fade/temporal effects
                prev_grid.copy_from(&grid);

                raster_fb.data.fill(0);
                rasterizer.render(&grid, &mut raster_fb, frame_config.zalgo_intensity);
                muxer.write_frame(&raster_fb)?;
            }

            if frame_idx % 100 == 0 {
                log::info!(
                    "Progress: {frame_idx}/{total_frames} ({:.1}%)",
                    frame_idx as f64 / total_frames as f64 * 100.0
                );
            }
        }

        log::info!("Clôture du flux vidéo...");
        muxer.finish()?;

        // === Étape 4 : Muxage Audio + Vidéo ===
        log::info!("Étape 4/4 : Muxing Audio/Video Final");
        mux_audio_video(&temp_video, audio_path, final_output)?;

        let _ = std::fs::remove_file(temp_video);

        log::info!("Export réussi vers {}", final_output.display());
        Ok(())
    }
}
