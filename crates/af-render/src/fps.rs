use std::collections::VecDeque;
use std::time::Instant;

/// Compteur FPS par fenêtre glissante. Zéro allocation après init.
///
/// # Example
/// ```
/// use af_render::fps::FpsCounter;
/// let mut counter = FpsCounter::new(60);
/// counter.tick();
/// let fps = counter.fps();
/// assert!(fps >= 0.0);
/// ```
pub struct FpsCounter {
    /// Timestamps des dernières N frames.
    timestamps: VecDeque<Instant>,
    /// Taille de la fenêtre (nombre de frames à moyenner).
    window: usize,
    /// FPS calculé, mis à jour à chaque tick.
    fps: f64,
    /// Temps de la dernière frame en ms (pour debug).
    pub frame_time_ms: f64,
}

impl FpsCounter {
    /// Create a new FPS counter with the given averaging window size.
    ///
    /// # Example
    /// ```
    /// use af_render::fps::FpsCounter;
    /// let counter = FpsCounter::new(60);
    /// assert!(counter.fps().abs() < f64::EPSILON);
    /// ```
    #[must_use]
    pub fn new(window: usize) -> Self {
        Self {
            timestamps: VecDeque::with_capacity(window + 1),
            window,
            fps: 0.0,
            frame_time_ms: 0.0,
        }
    }

    /// Appeler une fois par frame, APRÈS le rendu.
    ///
    /// # Example
    /// ```
    /// use af_render::fps::FpsCounter;
    /// let mut counter = FpsCounter::new(60);
    /// counter.tick();
    /// counter.tick();
    /// ```
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(&last) = self.timestamps.back() {
            self.frame_time_ms = now.duration_since(last).as_secs_f64() * 1000.0;
        }
        self.timestamps.push_back(now);
        if self.timestamps.len() > self.window {
            self.timestamps.pop_front();
        }
        if self.timestamps.len() >= 2 {
            let first = self.timestamps.front().copied().unwrap_or(now);
            let duration = now.duration_since(first);
            let secs = duration.as_secs_f64();
            if secs > 0.0 {
                self.fps = (self.timestamps.len() - 1) as f64 / secs;
            }
        }
    }

    /// FPS moyen sur la fenêtre.
    ///
    /// # Example
    /// ```
    /// use af_render::fps::FpsCounter;
    /// let counter = FpsCounter::new(60);
    /// assert!(counter.fps().abs() < f64::EPSILON);
    /// ```
    #[must_use]
    pub fn fps(&self) -> f64 {
        self.fps
    }
}
