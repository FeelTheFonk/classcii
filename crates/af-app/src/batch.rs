use anyhow::Result;
use std::path::Path;

#[cfg(feature = "video")]
use af_ascii::compositor::Compositor;
#[cfg(feature = "video")]
use af_audio::batch_analyzer::BatchAnalyzer;
use af_core::config::RenderConfig;
#[cfg(feature = "video")]
use af_core::frame::{AsciiGrid, FrameBuffer};
#[cfg(feature = "video")]
use af_core::traits::Source;
#[cfg(feature = "video")]
use af_export::muxer::{Mp4Muxer, mux_audio_video};
#[cfg(feature = "video")]
use af_export::rasterizer::Rasterizer;
#[cfg(feature = "video")]
use chrono::Local;

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
) -> Result<()> {
    #[cfg(not(feature = "video"))]
    {
        let _ = (folder, audio_path_str, final_output, config, target_fps);
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
            let timestamp = Local::now().format("%Y%m%d_%H%M%S");
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

        let mapper = AutoGenerativeMapper::new(config, timeline);

        // === Étape 2 : Initialisation de la source dossier ===
        log::info!(
            "Étape 2/4 : Initialisation Media Folder {}",
            folder.display()
        );
        let mut source = FolderBatchSource::new(folder, target_fps)?;

        let (native_w, native_h) = source.native_size();
        let target_w = native_w.max(1280);
        let target_h = native_h.max(720);

        let grid_w = (target_w / 8) as u16;
        let grid_h = (target_h / 16) as u16;

        let mut grid = AsciiGrid::new(grid_w, grid_h);

        // === Étape 3 : Pipeline de rendu (Compositor + Rasterizer + Muxer) ===
        log::info!("Étape 3/4 : Préparation de l'encodeur FFmpeg");

        let font_data = include_bytes!("../../af-export/assets/FiraCode-Regular.ttf");
        let rasterizer = Rasterizer::new(font_data, 16.0)?;

        let (raster_w, raster_h) = rasterizer.target_dimensions(grid_w, grid_h);

        let temp_video = final_output.with_extension("temp.mp4");
        let mut muxer = Mp4Muxer::new(&temp_video, raster_w, raster_h, target_fps)?;

        let initial_config = mapper.config_at(0.0);
        let mut compositor = Compositor::new(&initial_config.charset);
        let mut raster_fb = FrameBuffer::new(raster_w, raster_h);

        let total_frames = mapper.get_timeline().total_frames();
        let frame_duration = 1.0 / f64::from(target_fps);

        let mut resizer = af_source::resize::Resizer::new();
        let mut resized_source = FrameBuffer::new(target_w, target_h);

        log::info!("Boucle de Rendu : {total_frames} frames à {target_fps}fps");

        let mut macro_mode_override: Option<af_core::config::RenderMode> = None;
        let mut macro_invert_override: Option<bool> = None;
        let mut macro_charset_override: Option<(usize, String)> = None;

        for frame_idx in 0..total_frames {
            let timestamp_secs = frame_idx as f64 * frame_duration;
            let current_features = mapper.get_timeline().get_at_time(timestamp_secs);
            let mut frame_config = (*mapper.config_at(timestamp_secs)).clone();

            // === True Generative Clip Sequencing ===
            // Macro-variations on strong beats for experimental clip generation
            if current_features.onset && current_features.beat_intensity > 0.85 {
                // 1. Change media
                source.next_media();

                // 2. Cycle Render Mode (1 chance out of 4)
                if rand::random::<f64>() < 0.25 {
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

                // 3. Spontaneous Invert Flash (1 chance out of 5)
                if rand::random::<f64>() < 0.2 {
                    let current = macro_invert_override.unwrap_or(frame_config.invert);
                    macro_invert_override = Some(!current);
                }

                // 4. Randomize Charset Rotations (1 chance out of 3)
                if rand::random::<f64>() < 0.33 {
                    let current_idx = macro_charset_override
                        .as_ref()
                        .map_or(frame_config.charset_index, |(i, _)| *i);
                    let new_idx = (current_idx + 1) % 10;
                    let new_charset = match new_idx {
                        0 => af_core::charset::CHARSET_SOTA_FULL.to_string(),
                        1 => af_core::charset::CHARSET_SOTA_DENSE.to_string(),
                        2 => af_core::charset::CHARSET_SHORT_1.to_string(),
                        3 => af_core::charset::CHARSET_BLOCKS.to_string(),
                        4 => af_core::charset::CHARSET_MINIMAL.to_string(),
                        5 => af_core::charset::CHARSET_GLITCH_1.to_string(),
                        6 => af_core::charset::CHARSET_GLITCH_2.to_string(),
                        7 => af_core::charset::CHARSET_DIGITAL.to_string(),
                        8 => af_core::charset::CHARSET_EXTENDED.to_string(),
                        _ => af_core::charset::CHARSET_BINARY.to_string(),
                    };
                    macro_charset_override = Some((new_idx, new_charset));
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

            if let Some(src_frame) = source.next_frame() {
                let _ = resizer.resize_into(&src_frame, &mut resized_source);

                compositor.update_if_needed(&frame_config.charset);
                compositor.process(
                    &resized_source,
                    Some(&current_features),
                    &frame_config,
                    &mut grid,
                );

                // --- Effect Pipeline ---
                if frame_config.fade_decay > 0.0 {
                    // Requires keeping a prev grid... simplified for batch offline:
                    // To keep it 100% allocation free and simple for headless, we skip fade decay in batch
                    // or we would need a persistent prev_grid. Let's just do beat flash.
                }
                af_render::effects::apply_beat_flash(&mut grid, &current_features);

                // Note: Glow is skipped in headless unless we rasterize the glow.
                // The current rasterizer just paints the characters.

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
