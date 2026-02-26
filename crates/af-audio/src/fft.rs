use realfft::RealFftPlanner;

/// FFT pipeline: windowed real FFT using realfft.
///
/// Pre-allocates the FFT plan and scratch buffers for zero-allocation hot path.
///
/// # Example
/// ```
/// use af_audio::fft::FftPipeline;
/// let fft = FftPipeline::new(2048);
/// ```
pub struct FftPipeline {
    fft_size: usize,
    input_buf: Vec<f32>,
    spectrum_buf: Vec<realfft::num_complex::Complex<f32>>,
    scratch: Vec<realfft::num_complex::Complex<f32>>,
    plan: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    /// Hann window coefficients.
    window: Vec<f32>,
}

impl FftPipeline {
    /// Create a new FFT pipeline with the given window size.
    ///
    /// # Panics
    /// Panics if `size` is 0.
    #[must_use]
    pub fn new(size: usize) -> Self {
        assert!(size > 0, "FFT size must be > 0");

        let mut planner = RealFftPlanner::<f32>::new();
        let plan = planner.plan_fft_forward(size);

        let input_buf = plan.make_input_vec();
        let spectrum_buf = plan.make_output_vec();
        let scratch = plan.make_scratch_vec();

        // Hann window
        let window: Vec<f32> = (0..size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (size as f32 - 1.0)).cos())
            })
            .collect();

        Self {
            fft_size: size,
            input_buf,
            spectrum_buf,
            scratch,
            plan,
            window,
        }
    }

    /// Process `samples` through windowed FFT.
    ///
    /// Returns the spectrum magnitude (N/2+1 bins).
    ///
    /// # Example
    /// ```
    /// use af_audio::fft::FftPipeline;
    /// let mut fft = FftPipeline::new(256);
    /// let samples = vec![0.0f32; 256];
    /// let spectrum = fft.process(&samples);
    /// assert_eq!(spectrum.len(), 129); // N/2 + 1
    /// ```
    pub fn process(&mut self, samples: &[f32]) -> Vec<f32> {
        let n = self.fft_size.min(samples.len());

        // Copy and window
        for (i, slot) in self.input_buf.iter_mut().enumerate() {
            *slot = if i < n {
                samples[i] * self.window[i]
            } else {
                0.0
            };
        }

        // Forward FFT
        if self.plan.process_with_scratch(
            &mut self.input_buf,
            &mut self.spectrum_buf,
            &mut self.scratch,
        ).is_err() {
            return vec![0.0; self.spectrum_buf.len()];
        }

        // Magnitude
        self.spectrum_buf
            .iter()
            .map(|c| (c.re * c.re + c.im * c.im).sqrt() / self.fft_size as f32)
            .collect()
    }

    /// FFT window size.
    #[must_use]
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }
}
