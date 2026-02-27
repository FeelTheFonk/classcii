use anyhow::{Context, Result};
use ffmpeg_the_third as ffmpeg;
use flume::{Receiver, Sender};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use af_core::frame::FrameBuffer;

/// Commandes interactives pour le thread vidéo SOTA.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoCommand {
    Play,
    Pause,
    Seek(f64),
    Resize(u32, u32),
    Quit,
}

/// Spawns the video decoding thread using ffmpeg-the-third.
///
/// Features SOTA zero-allocation frame pooling and PTS-based lip-sync.
pub fn spawn_video_thread(
    path: PathBuf,
    frame_tx: Sender<Arc<FrameBuffer>>,
    cmd_rx: Receiver<VideoCommand>,
) -> Result<(thread::JoinHandle<()>, (u32, u32))> {
    ffmpeg::init()?;

    let mut ictx = ffmpeg::format::input(&path)
        .with_context(|| format!("Impossible d'ouvrir le fichier vidéo: {:?}", path))?;

    let stream = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .context("Aucun flux vidéo trouvé")?;

    let stream_index = stream.index();
    let time_base = stream.time_base();
    let tb_f64 = f64::from(time_base.numerator()) / f64::from(time_base.denominator());

    let mut context_decoder =
        ffmpeg::codec::context::Context::from_parameters(stream.parameters())?;
    context_decoder.set_threading(ffmpeg::threading::Config {
        kind: ffmpeg::threading::Type::Frame,
        count: 0,
        safe: true,
    });
    let mut decoder = context_decoder.decoder().video()?;

    let width = decoder.width();
    let height = decoder.height();

    let mut sws_width = width;
    let mut sws_height = height;

    let mut sws = ffmpeg::software::scaling::context::Context::get(
        decoder.format(),
        width,
        height,
        ffmpeg::format::Pixel::RGBA,
        sws_width,
        sws_height,
        ffmpeg::software::scaling::flag::Flags::BILINEAR,
    )?;

    log::info!(
        "Video started, native {}x{}, target {}x{}, timebase: {}",
        width,
        height,
        sws_width,
        sws_height,
        tb_f64
    );

    let handle = thread::spawn(move || {
        let mut pool: Vec<Arc<FrameBuffer>> = Vec::new();

        let mut current_pts_sec = 0.0;
        let mut start_time = Instant::now();
        let mut start_pts_sec = 0.0;
        let mut is_paused = false;
        let mut eof = false;

        let mut output_frame = ffmpeg::frame::Video::empty();

        loop {
            // Check commands
            loop {
                // If EOF or paused, perform a blocking recv unless there's a frame to push
                // To avoid 100% CPU on EOF/Pause, block:
                let cmd = if is_paused || eof {
                    cmd_rx.recv().ok()
                } else {
                    cmd_rx.try_recv().ok()
                };

                match cmd {
                    Some(VideoCommand::Play) => {
                        is_paused = false;
                        start_time = Instant::now();
                        start_pts_sec = current_pts_sec;
                    }
                    Some(VideoCommand::Pause) => {
                        is_paused = true;
                    }
                    Some(VideoCommand::Seek(delta)) => {
                        let target_sec = (current_pts_sec + delta).max(0.0);
                        let target_pts = (target_sec / tb_f64) as i64;
                        if let Err(e) = ictx.seek(target_pts, ..target_pts) {
                            log::warn!("Seek failed: {}", e);
                        }
                        decoder.flush();

                        start_time = Instant::now();
                        start_pts_sec = target_sec;
                        current_pts_sec = target_sec;
                        eof = false; // Reset EOF condition on seek
                    }
                    Some(VideoCommand::Resize(w, h)) => {
                        if w > 0 && h > 0 && (w != sws_width || h != sws_height) {
                            sws_width = w;
                            sws_height = h;
                            if let Ok(new_sws) = ffmpeg::software::scaling::context::Context::get(
                                decoder.format(),
                                width,
                                height,
                                ffmpeg::format::Pixel::RGBA,
                                sws_width,
                                sws_height,
                                ffmpeg::software::scaling::flag::Flags::BILINEAR,
                            ) {
                                sws = new_sws;
                                // Invalidate pool because format size has changed
                                pool.clear();
                                output_frame = ffmpeg::frame::Video::empty();
                            }
                        }
                    }
                    Some(VideoCommand::Quit) => return,
                    None => break, // No more commands
                }
            }

            if eof || is_paused {
                continue;
            }

            match ictx.packets().next() {
                Some((stream, packet)) => {
                    if stream.index() == stream_index {
                        if decoder.send_packet(&packet).is_ok() {
                            let mut decoded = ffmpeg::frame::Video::empty();
                            while decoder.receive_frame(&mut decoded).is_ok() {
                                let pts = decoded.pts().unwrap_or(0);
                                let pts_sec = pts as f64 * tb_f64;
                                current_pts_sec = pts_sec;

                                // Fast-Drop (Gap 3): Si on est en train de seek, on skip le scaler SWS
                                // tant qu'on a pas rattrapé le target_pts_sec
                                if pts_sec < start_pts_sec {
                                    continue;
                                }

                                let target_elapsed = pts_sec - start_pts_sec;
                                if target_elapsed > 0.0 && !is_paused {
                                    let target_duration = Duration::from_secs_f64(target_elapsed);
                                    let now_elapsed = start_time.elapsed();
                                    if target_duration > now_elapsed {
                                        thread::sleep(target_duration - now_elapsed);
                                    }
                                }

                                if sws.run(&decoded, &mut output_frame).is_ok() {
                                    // SOTA Zero-Allocation Frame Pool
                                    let mut frame_arc: Option<Arc<FrameBuffer>> = None;
                                    for arc in &mut pool {
                                        if Arc::get_mut(arc).is_some() {
                                            frame_arc = Some(arc.clone());
                                            break;
                                        }
                                    }

                                    if frame_arc.is_none() {
                                        let new_f =
                                            Arc::new(FrameBuffer::new(sws_width, sws_height));
                                        pool.push(new_f.clone());
                                        frame_arc = Some(new_f);
                                    }

                                    // frame_arc is guaranteed Some by the block above.
                                    let Some(mut arc_to_send) = frame_arc else {
                                        continue;
                                    };
                                    let Some(fb_mut) = Arc::get_mut(&mut arc_to_send) else {
                                        continue;
                                    };

                                    let stride = output_frame.stride(0);
                                    let width_bytes = sws_width as usize * 4;
                                    let in_data = output_frame.data(0);
                                    for y in 0..sws_height as usize {
                                        let in_row = &in_data[y * stride..y * stride + width_bytes];
                                        let out_row = &mut fb_mut.data
                                            [y * width_bytes..(y + 1) * width_bytes];
                                        out_row.copy_from_slice(in_row);
                                    }

                                    if frame_tx.send(arc_to_send).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                None => {
                    eof = true;
                }
            }
        }
    });

    Ok((handle, (width, height)))
}
