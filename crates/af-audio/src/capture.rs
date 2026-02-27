use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, RingBuffer};

/// Audio capture via cpal.
///
/// Writes mono f32 samples into a lock-free ring buffer.
///
/// # Example
/// ```no_run
/// use af_audio::capture::AudioCapture;
/// let capture = AudioCapture::start_default().unwrap();
/// ```
pub struct AudioCapture {
    stream: cpal::Stream,
    consumer: Consumer<f32>,
    sample_rate: u32,
}

impl AudioCapture {
    /// Reference to the underlying cpal stream (kept alive for capture).
    pub fn stream(&self) -> &cpal::Stream {
        &self.stream
    }

    /// Start capturing from the default input device.
    ///
    /// # Errors
    /// Returns an error if the audio device is unavailable.
    pub fn start_default() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("Pas de périphérique audio trouvé"))?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        // Ring buffer: 2 seconds of audio @ sample_rate
        let buf_size = sample_rate as usize * 2;
        let (mut producer, consumer) = RingBuffer::new(buf_size);

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Downmix to mono and push into ring buffer
                for chunk in data.chunks(channels) {
                    let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                    let _ = producer.push(mono);
                }
            },
            |err| {
                log::error!("Audio stream error: {err}");
            },
            None,
        )?;

        stream.play()?;

        Ok(Self {
            stream,
            consumer,
            sample_rate,
        })
    }

    /// Read available samples from the ring buffer into `out`.
    ///
    /// Returns how many samples were read.
    pub fn read_samples(&mut self, out: &mut Vec<f32>) -> usize {
        let available = self.consumer.slots();
        out.clear();
        out.reserve(available);
        let mut count = 0;
        while let Ok(sample) = self.consumer.pop() {
            out.push(sample);
            count += 1;
        }
        count
    }

    /// The sample rate of the capture stream.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
