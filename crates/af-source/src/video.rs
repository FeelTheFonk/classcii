// DEVIATION: R9 — Ce module utilise ffmpeg via subprocess (std::process::Command)
// au lieu de ffmpeg-the-third, car vcpkg/pkg-config sont absents sur Windows MSVC.
// Prérequis : `ffmpeg` et `ffprobe` accessibles dans PATH (WinGet v8.0.1+).
//
// Architecture :
//   - `probe_video`       : interroge ffprobe pour obtenir width/height/fps
//   - `spawn_ffmpeg_pipe` : lance ffmpeg → flux raw RGBA sur stdout
//   - `spawn_video_thread`: thread dédié, lit les frames, gère les commandes
//   - `process_commands`  : dispatche les commandes dans la boucle principale
//   - `find_or_create_slot`: gère le pool Arc<FrameBuffer> zero-alloc
//   - Sync A/V : timing esclave via MediaClock (audio = maître)

use anyhow::{Context, Result};
use flume::{Receiver, Sender};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use af_core::clock::MediaClock;
use af_core::frame::FrameBuffer;

/// Taille du pool de frames pré-allouées.
/// Doit être > capacité du canal (3) pour garantir un slot libre sans allocation.
const POOL_SIZE: usize = 6;

/// Tolérance de synchronisation A/V en secondes (~1 frame à 24fps).
const SYNC_TOLERANCE_SECS: f64 = 0.04;

/// Commandes interactives pour le thread vidéo.
///
/// # Example
/// ```
/// use af_source::video::VideoCommand;
/// let cmd = VideoCommand::Seek(5.0);
/// assert!(matches!(cmd, VideoCommand::Seek(_)));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoCommand {
    /// Reprendre la lecture.
    Play,
    /// Mettre en pause.
    Pause,
    /// Sauter de `delta` secondes (positif = avance, négatif = recul).
    Seek(f64),
    /// Redimensionner le canvas cible (redémarre ffmpeg avec nouveaux `-vf scale`).
    Resize(u32, u32),
    /// Arrêter le thread proprement.
    Quit,
}

/// Métadonnées extraites via ffprobe.
#[derive(Clone, Copy)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    /// Images par seconde (ex: 23.976, 24.0, 30.0, 60.0).
    pub fps: f64,
}

/// État mutable centralisé du thread vidéo.
struct VideoState {
    /// Largeur actuelle du pipe ffmpeg.
    w: u32,
    /// Hauteur actuelle du pipe ffmpeg.
    h: u32,
    /// True si la lecture est en pause (commande locale).
    is_paused: bool,
    /// FPS cible envoyé à ffmpeg.
    target_fps: u32,
    /// Pool pré-alloué de frames réutilisables (zero-alloc en hot path).
    pool: Vec<Arc<FrameBuffer>>,
    /// Nombre de frames lues depuis le pipe ffmpeg courant.
    frames_read: u64,
    /// Position en secondes à laquelle le pipe ffmpeg courant a été démarré (-ss).
    pipe_start_secs: f64,
}

impl VideoState {
    fn new(info: &VideoInfo) -> Self {
        // Cap initial pour limiter la bande passante du pipe :
        // 1920×800@24fps ≈ 142 MB/s → 640×360@24fps ≈ 21 MB/s
        let w = info.width.min(640);
        let h = info.height.min(360);
        let target_fps = info.fps.clamp(1.0, 60.0).round() as u32;
        let pool = (0..POOL_SIZE)
            .map(|_| Arc::new(FrameBuffer::new(w, h)))
            .collect();
        Self {
            w,
            h,
            is_paused: false,
            target_fps,
            pool,
            frames_read: 0,
            pipe_start_secs: 0.0,
        }
    }

    /// Position vidéo courante dérivée du pipe ffmpeg.
    fn current_pos_secs(&self, fps: f64) -> f64 {
        self.pipe_start_secs + self.frames_read as f64 / fps.max(1.0)
    }
}

/// Interroge `ffprobe` pour obtenir les métadonnées du flux vidéo principal.
///
/// # Errors
/// Retourne une erreur si `ffprobe` est introuvable ou si le fichier
/// ne contient aucun flux vidéo décodable.
///
/// # Example
/// ```no_run
/// // Nécessite ffprobe en PATH
/// // let info = probe_video(Path::new("video.mkv"));
/// ```
pub fn probe_video(path: &Path) -> Result<VideoInfo> {
    let path_str = path.to_str().context("Chemin vidéo invalide (non-UTF8)")?;

    let output = Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,r_frame_rate",
            "-of",
            "default=noprint_wrappers=1",
            "-i",
            path_str,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .context(
            "Impossible de lancer ffprobe. Vérifiez que ffprobe est installé et dans le PATH.",
        )?;

    let text = String::from_utf8_lossy(&output.stdout);

    let mut width: u32 = 1920;
    let mut height: u32 = 1080;
    let mut fps: f64 = 30.0;
    let mut found_any = false;

    for line in text.lines() {
        if let Some(val) = line.strip_prefix("width=") {
            width = val.trim().parse().unwrap_or(1920);
            found_any = true;
        } else if let Some(val) = line.strip_prefix("height=") {
            height = val.trim().parse().unwrap_or(1080);
            found_any = true;
        } else if let Some(val) = line.strip_prefix("r_frame_rate=") {
            found_any = true;
            // Format: "24/1" ou "30000/1001" ou "24000/1001"
            let val = val.trim();
            let mut parts = val.splitn(2, '/');
            let num: f64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(30.0);
            let den: f64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
            if den > 0.0 {
                fps = num / den;
            }
        }
    }

    if !found_any {
        log::warn!(
            "ffprobe n'a retourné aucune métadonnée pour {}, utilisation des défauts ({}x{}@{:.0}fps)",
            path.display(),
            width,
            height,
            fps
        );
    }

    if width == 0 || height == 0 {
        anyhow::bail!(
            "ffprobe n'a trouvé aucun flux vidéo dans {}",
            path.display()
        );
    }

    log::info!(
        "probe_video: {width}x{height} @ {fps:.3}fps — {}",
        path.display()
    );

    Ok(VideoInfo { width, height, fps })
}

/// Lance un processus `ffmpeg` qui écrit des frames RGBA brutes sur stdout.
///
/// Chaque frame = `w × h × 4` bytes (RGBA row-major, sans padding).
/// `-ss` avant `-i` = seek rapide keyframe-based.
/// `-an` supprime l'audio (géré séparément par symphonia).
///
/// Retourne `None` si le spawn échoue (log::warn émis).
#[must_use]
pub fn spawn_ffmpeg_pipe(
    path: &Path,
    w: u32,
    h: u32,
    pos_secs: f64,
    target_fps: u32,
) -> Option<Child> {
    let Some(path_str) = path.to_str() else {
        log::warn!("spawn_ffmpeg_pipe: chemin non-UTF8");
        return None;
    };

    let scale_filter = format!("scale={w}:{h}:flags=lanczos");
    let fps_str = target_fps.to_string();
    let pos_str = format!("{pos_secs:.3}");

    match Command::new("ffmpeg")
        .args([
            "-ss",
            &pos_str, // seek rapide avant -i (keyframe-based)
            "-i",
            path_str, // fichier source
            "-vf",
            &scale_filter, // scale + filter
            "-f",
            "rawvideo", // format raw
            "-pix_fmt",
            "rgba", // RGBA 4 bytes/pixel
            "-r",
            &fps_str, // fps output
            "-an",    // pas d'audio dans ce pipe
            "-hide_banner",
            "-loglevel",
            "error",
            "pipe:1", // stdout
        ])
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            log::debug!("ffmpeg spawné: {w}x{h} @ {target_fps}fps depuis {pos_secs:.1}s");
            Some(child)
        }
        Err(e) => {
            log::warn!("spawn_ffmpeg_pipe: impossible de lancer ffmpeg: {e}");
            None
        }
    }
}

/// Lit exactement `buf.len()` bytes depuis `reader`.
///
/// # Errors
/// Retourne `Ok(true)` si lu avec succès, `Ok(false)` sur EOF avant complétion,
/// `Err` sur erreur I/O fatale.
pub fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> Result<bool> {
    let mut total = 0usize;
    while total < buf.len() {
        match reader.read(&mut buf[total..]) {
            Ok(0) => return Ok(false), // EOF
            Ok(n) => total += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(true)
}

/// Retourne `true` si le thread doit quitter (Quit reçu ou canal déconnecté).
/// Modifie `state` et `maybe_child` en conséquence.
/// Redémarre ffmpeg en interne si Seek ou Resize est reçu.
fn process_commands(
    cmd_rx: &Receiver<VideoCommand>,
    state: &mut VideoState,
    maybe_child: &mut Option<Child>,
    path: &Path,
    clock: Option<&MediaClock>,
) -> bool {
    let mut need_restart = false;
    loop {
        match cmd_rx.try_recv() {
            Ok(VideoCommand::Quit) => {
                // .as_mut() car maybe_child: &mut Option<Child> (pas de move possible)
                if let Some(c) = maybe_child.as_mut() {
                    let _ = c.kill();
                }
                log::info!("Thread vidéo: Quit reçu, arrêt propre.");
                return true;
            }
            Ok(VideoCommand::Pause) => {
                state.is_paused = true;
                log::debug!("Thread vidéo: Pause");
            }
            Ok(VideoCommand::Play) => {
                state.is_paused = false;
                log::debug!("Thread vidéo: Play");
            }
            Ok(VideoCommand::Seek(delta)) => {
                // Lire la position depuis le clock audio si disponible
                let current = clock.map_or(
                    state.current_pos_secs(f64::from(state.target_fps)),
                    MediaClock::pos_secs,
                );
                let new_secs = (current + delta).max(0.0);
                state.pipe_start_secs = new_secs;
                state.frames_read = 0;
                need_restart = true;
                log::debug!("Thread vidéo: Seek -> {new_secs:.1}s");
            }
            Ok(VideoCommand::Resize(nw, nh)) => {
                if nw > 0 && nh > 0 && (nw != state.w || nh != state.h) {
                    state.w = nw;
                    state.h = nh;
                    state.pool.clear();
                    for _ in 0..POOL_SIZE {
                        state.pool.push(Arc::new(FrameBuffer::new(nw, nh)));
                    }
                    need_restart = true;
                    log::debug!("Thread vidéo: Resize -> {nw}x{nh}");
                }
            }
            Err(flume::TryRecvError::Empty) => return false,
            Err(flume::TryRecvError::Disconnected) => {
                if let Some(c) = maybe_child.as_mut() {
                    let _ = c.kill();
                }
                return true;
            }
        }
        // Restart ffmpeg si Seek ou Resize reçu
        if need_restart {
            if let Some(c) = maybe_child.as_mut() {
                let _ = c.kill();
            }
            *maybe_child = spawn_ffmpeg_pipe(
                path,
                state.w,
                state.h,
                state.pipe_start_secs,
                state.target_fps,
            );
            need_restart = false;
        }
    }
}

/// Trouve ou crée un slot libre dans le pool.
///
/// Invariant : retourne un index `i` tel que `Arc::strong_count(&pool[i]) == 1`.
/// Si tous les slots sont pris, alloue un nouveau slot (cas exceptionnel).
fn find_or_create_slot(pool: &mut Vec<Arc<FrameBuffer>>, w: u32, h: u32) -> usize {
    let free_idx = pool.iter().position(|a| Arc::strong_count(a) == 1);
    if let Some(i) = free_idx {
        // Vérifier taille correcte (peut différer après Resize)
        if pool[i].data.len() != (w * h * 4) as usize {
            pool[i] = Arc::new(FrameBuffer::new(w, h));
        }
        i
    } else {
        // Pool saturé: allouer un nouveau slot (cas rare avec POOL_SIZE > channel cap)
        // DECISION: allouer plutôt que bloquer pour respecter R1 (jamais bloquer).
        pool.push(Arc::new(FrameBuffer::new(w, h)));
        pool.len() - 1
    }
}

/// Spawne le thread de décodage vidéo via `ffmpeg` subprocess.
///
/// Le thread lit les frames RGBA depuis stdout de ffmpeg et les envoie
/// via `frame_tx`. Les commandes (Play/Pause/Seek/Resize/Quit) sont
/// reçues depuis `cmd_rx`.
///
/// Si `clock` est `Some`, le thread se synchronise sur l'horloge audio.
/// Sinon, il utilise un pacing mur indépendant (wall-clock).
///
/// Retourne le handle du thread + les dimensions natives du flux vidéo.
///
/// # Errors
/// Retourne une erreur si `ffprobe` est introuvable ou si le fichier est invalide.
pub fn spawn_video_thread(
    path: PathBuf,
    frame_tx: Sender<Arc<FrameBuffer>>,
    cmd_rx: Receiver<VideoCommand>,
    clock: Option<Arc<MediaClock>>,
) -> Result<(thread::JoinHandle<()>, (u32, u32))> {
    let info = probe_video(&path)?;
    let native_w = info.width;
    let native_h = info.height;

    let handle = thread::Builder::new()
        .name("af-video".to_string())
        .spawn(move || {
            video_loop(&path, &frame_tx, &cmd_rx, info, clock.as_deref());
        })
        .context("Impossible de spawner le thread vidéo")?;

    Ok((handle, (native_w, native_h)))
}

/// Boucle principale du thread vidéo.
///
/// Deux modes de timing :
/// - Avec `clock` (audio présent) : esclave du clock audio, skip/wait si drift
/// - Sans `clock` (vidéo seule) : pacing mur indépendant (wall-clock)
fn video_loop(
    path: &Path,
    frame_tx: &Sender<Arc<FrameBuffer>>,
    cmd_rx: &Receiver<VideoCommand>,
    info: VideoInfo,
    clock: Option<&MediaClock>,
) {
    const CLOCK_TIMEOUT: Duration = Duration::from_secs(5);

    let mut state = VideoState::new(&info);
    let frame_period = Duration::from_secs_f64(1.0 / info.fps.clamp(1.0, 120.0));
    // DECISION: Pas de spawn immédiat — on attend le premier VideoCommand::Resize
    // envoyé par check_resize() du thread principal (~1 frame après démarrage).
    // Évite un double-spawn inutile (min(640,360) puis taille réelle du canvas).
    let mut maybe_child: Option<Child> = None;
    let mut last_frame = Instant::now();
    let clock_wait_start = Instant::now();

    loop {
        // === Commandes (non-bloquant) ===
        if process_commands(cmd_rx, &mut state, &mut maybe_child, path, clock) {
            return;
        }

        // === Guard : pas de ffmpeg encore → attendre le premier Resize ===
        if maybe_child.is_none() {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // === Pause (locale ou via clock audio) ===
        let clock_paused = clock.is_some_and(MediaClock::is_paused);
        if state.is_paused || clock_paused {
            thread::sleep(Duration::from_millis(10));
            continue;
        }

        // === Timing : mode synchronisé ou indépendant ===
        let target_secs = if let Some(c) = clock {
            // Mode synchronisé : attendre que l'audio démarre (avec timeout)
            if c.is_started() {
                c.pos_secs()
            } else if clock_wait_start.elapsed() < CLOCK_TIMEOUT {
                thread::sleep(Duration::from_millis(10));
                continue;
            } else {
                // Timeout : fallback wall-clock indépendant
                log::warn!("Audio non démarré après 5s, vidéo en mode indépendant");
                state.current_pos_secs(info.fps)
            }
        } else {
            // Mode indépendant : pacing mur (wall-clock)
            let elapsed = last_frame.elapsed();
            if let Some(remaining) = frame_period.checked_sub(elapsed) {
                thread::sleep(remaining);
                continue;
            }
            last_frame = Instant::now();
            state.current_pos_secs(info.fps)
        };

        let video_pos = state.current_pos_secs(info.fps);
        let drift = video_pos - target_secs;

        // Vidéo en avance sur l'audio → attendre
        if clock.is_some() && drift > SYNC_TOLERANCE_SECS {
            let wait = (drift - SYNC_TOLERANCE_SECS).min(0.05);
            thread::sleep(Duration::from_secs_f64(wait));
            continue;
        }

        // === Obtenir un slot libre dans le pool (zero-alloc si possible) ===
        let frame_bytes = (state.w * state.h * 4) as usize;
        let idx = find_or_create_slot(&mut state.pool, state.w, state.h);

        // Arc::get_mut réussit ssi strong_count == 1 (garanti par find_or_create_slot)
        let Some(fb) = Arc::get_mut(&mut state.pool[idx]) else {
            continue; // Sécurité (ne devrait pas arriver)
        };

        // === Lire une frame depuis le pipe ffmpeg ===
        let read_result = maybe_child
            .as_mut()
            .and_then(|c| c.stdout.as_mut())
            .map_or(Ok(false), |stdout| {
                read_exact_or_eof(stdout, &mut fb.data[..frame_bytes])
            });

        match read_result {
            Ok(true) => {
                state.frames_read += 1;

                // Vidéo en retard sur l'audio → lire la frame mais ne pas l'envoyer (skip)
                if clock.is_some() && drift < -SYNC_TOLERANCE_SECS {
                    continue;
                }

                // Frame lue et en sync : envoyer un clone (pool garde sa référence)
                if frame_tx.send(Arc::clone(&state.pool[idx])).is_err() {
                    if let Some(mut c) = maybe_child {
                        let _ = c.kill();
                    }
                    return;
                }
            }
            Ok(false) => {
                // EOF : fin de la vidéo, dernière frame reste affichée
                let pos = state.current_pos_secs(info.fps);
                log::info!("Thread vidéo: EOF à {pos:.1}s, arrêt.");
                break;
            }
            Err(e) => {
                log::warn!("Thread vidéo: erreur lecture pipe: {e}");
                if let Some(mut c) = maybe_child {
                    let _ = c.kill();
                }
                maybe_child = None;
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    // Cleanup final
    if let Some(mut c) = maybe_child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }
    log::info!("Thread vidéo terminé proprement.");
}
