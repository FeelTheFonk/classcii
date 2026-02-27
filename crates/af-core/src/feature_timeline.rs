use crate::frame::AudioFeatures;

/// Une timeline complète pré-calculée des features audio d'un morceau.
/// Utilisée pour le rendu offline (batch export).
#[derive(Clone)]
pub struct FeatureTimeline {
    /// Les features extraites pour chaque frame.
    pub frames: Vec<AudioFeatures>,
    /// Durée de chaque frame en secondes (typiquement 1.0 / fps).
    pub frame_duration: f32,
    /// Le taux d'échantillonnage de l'audio source.
    pub sample_rate: u32,
}

impl FeatureTimeline {
    /// Obtenir les features à un temps `t` (en secondes).
    ///
    /// # Example
    /// ```
    /// use af_core::feature_timeline::FeatureTimeline;
    /// let timeline = FeatureTimeline { frames: vec![], frame_duration: 0.016, sample_rate: 44100 };
    /// let features = timeline.get_at_time(1.0);
    /// ```
    #[must_use]
    pub fn get_at_time(&self, time: f64) -> AudioFeatures {
        if self.frames.is_empty() {
            return AudioFeatures::default();
        }

        let index = (time as f32 / self.frame_duration) as usize;
        let clamped_index = index.min(self.frames.len().saturating_sub(1));
        self.frames[clamped_index]
    }

    /// Nombre total de frames pré-analysées.
    #[must_use]
    pub fn total_frames(&self) -> usize {
        self.frames.len()
    }
}
