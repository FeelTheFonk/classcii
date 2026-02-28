use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

/// Horloge partagée pour la synchronisation A/V.
///
/// L'audio est le maître : le callback cpal écrit `sample_pos` à chaque buffer.
/// Le thread vidéo lit `pos_secs()` pour caler ses frames.
///
/// Tous les champs sont atomiques — zero-alloc, zero-lock, `Send + Sync`.
///
/// # Example
/// ```
/// use af_core::clock::MediaClock;
/// let clock = MediaClock::new(48000);
/// assert!(!clock.is_started());
/// clock.mark_started();
/// assert!(clock.is_started());
/// ```
pub struct MediaClock {
    /// Position de lecture en samples (mono, source rate).
    sample_pos: AtomicUsize,
    /// Sample rate source (immutable après init par le thread audio).
    sample_rate: AtomicU32,
    /// `true` une fois que le premier callback cpal a écrit.
    started: AtomicBool,
    /// `true` si la lecture est en pause.
    paused: AtomicBool,
}

impl MediaClock {
    /// Crée une horloge avec un sample rate initial.
    ///
    /// Le sample rate peut être mis à jour par `set_sample_rate()` une fois
    /// le décodage terminé (si initialisé à 0).
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_pos: AtomicUsize::new(0),
            sample_rate: AtomicU32::new(sample_rate),
            started: AtomicBool::new(false),
            paused: AtomicBool::new(false),
        }
    }

    /// Position courante en secondes, dérivée de `sample_pos / sample_rate`.
    #[inline]
    #[must_use]
    pub fn pos_secs(&self) -> f64 {
        let rate = self.sample_rate.load(Ordering::Relaxed);
        if rate == 0 {
            return 0.0;
        }
        self.sample_pos.load(Ordering::Relaxed) as f64 / f64::from(rate)
    }

    /// Met à jour le sample rate (appelé par le thread audio après décodage).
    #[inline]
    pub fn set_sample_rate(&self, rate: u32) {
        self.sample_rate.store(rate, Ordering::Relaxed);
    }

    /// Position courante en samples.
    #[inline]
    #[must_use]
    pub fn sample_pos(&self) -> usize {
        self.sample_pos.load(Ordering::Relaxed)
    }

    /// Met à jour la position en samples (appelé par le callback cpal).
    #[inline]
    pub fn set_sample_pos(&self, pos: usize) {
        self.sample_pos.store(pos, Ordering::Relaxed);
    }

    /// Marque l'horloge comme démarrée (premier callback cpal reçu).
    #[inline]
    pub fn mark_started(&self) {
        self.started.store(true, Ordering::Relaxed);
    }

    /// `true` si l'audio a commencé à jouer.
    #[inline]
    #[must_use]
    pub fn is_started(&self) -> bool {
        self.started.load(Ordering::Relaxed)
    }

    /// Met à jour l'état de pause.
    #[inline]
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    /// `true` si la lecture est en pause.
    #[inline]
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_basic_operations() {
        let clock = MediaClock::new(48000);
        assert!(!clock.is_started());
        assert!(!clock.is_paused());
        assert_eq!(clock.sample_pos(), 0);

        clock.set_sample_pos(48000);
        let secs = clock.pos_secs();
        assert!((secs - 1.0).abs() < 0.001);

        clock.mark_started();
        assert!(clock.is_started());

        clock.set_paused(true);
        assert!(clock.is_paused());
    }

    #[test]
    fn clock_zero_sample_rate() {
        let clock = MediaClock::new(0);
        assert_eq!(clock.pos_secs(), 0.0);
    }
}
