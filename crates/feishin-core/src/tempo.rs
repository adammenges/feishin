//! Streaming speed changes, with optional pitch preservation. SoundTouch is
//! bundled into the binary; native playback does not require FFmpeg or MPV.
use rodio::{Source, source::SeekError};
use soundtouch::SoundTouch;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

#[derive(Clone, Copy, PartialEq)]
pub struct Options {
    pub speed: f32,
    pub preserve_pitch: bool,
}
impl From<&crate::storage::Settings> for Options {
    fn from(settings: &crate::storage::Settings) -> Self {
        Self {
            speed: if settings.speed.is_finite() {
                settings.speed.clamp(0.25, 4.0)
            } else {
                1.0
            },
            preserve_pitch: settings.preserve_pitch,
        }
    }
}

pub struct Tempo<S> {
    source: S,
    shared: Arc<RwLock<Options>>,
    options: Options,
    engine: SoundTouch,
    input: [f32; 2048],
    output: [f32; 4096],
    offset: usize,
    available: usize,
    processed: bool,
    ended: bool,
}
impl<S: Source> Tempo<S> {
    pub fn new(source: S, shared: Arc<RwLock<Options>>) -> Self {
        let options = *shared.read().unwrap();
        let mut engine = SoundTouch::new();
        engine
            .set_channels(source.channels() as u32)
            .set_sample_rate(source.sample_rate());
        let mut tempo = Self {
            source,
            shared,
            options,
            engine,
            input: [0.0; 2048],
            output: [0.0; 4096],
            offset: 0,
            available: 0,
            processed: false,
            ended: false,
        };
        tempo.configure();
        tempo
    }
    fn configure(&mut self) {
        self.engine.set_tempo(if self.options.preserve_pitch {
            self.options.speed as f64
        } else {
            1.0
        });
        self.engine.set_rate(if self.options.preserve_pitch {
            1.0
        } else {
            self.options.speed as f64
        });
    }
}
impl<S: Source> Iterator for Tempo<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.offset < self.available {
            let value = self.output[self.offset];
            self.offset += 1;
            return Some(value);
        }
        let options = self
            .shared
            .read()
            .map(|value| *value)
            .unwrap_or(self.options);
        if options != self.options {
            self.options = options;
            self.configure();
        }
        // At normal speed, preserve exact PCM and gapless boundaries without
        // introducing the time-stretcher's analysis latency.
        if self.options.speed == 1.0 && !self.processed {
            return self.source.next();
        }
        self.processed = true;
        let channels = self.source.channels() as usize;
        loop {
            self.available = self
                .engine
                .receive_samples(&mut self.output, 4096 / channels)
                * channels;
            if self.available > 0 {
                self.offset = 1;
                return Some(self.output[0]);
            }
            if self.ended {
                return None;
            }
            let mut count = 0;
            for sample in &mut self.input[..2048 / channels * channels] {
                let Some(value) = self.source.next() else {
                    break;
                };
                *sample = value;
                count += 1;
            }
            if count > 0 {
                self.engine
                    .put_samples(&self.input[..count], count / channels);
            } else {
                self.ended = true;
                self.engine.flush();
            }
        }
    }
}
impl<S: Source> Source for Tempo<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source
            .total_duration()
            .map(|duration| duration.div_f32(self.options.speed))
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.source.try_seek(position)?;
        self.engine.clear();
        self.offset = 0;
        self.available = 0;
        self.ended = false;
        self.processed = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    fn sine() -> SamplesBuffer {
        SamplesBuffer::new(
            2,
            48_000,
            (0..96_000)
                .flat_map(|frame| {
                    let value =
                        (frame as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5;
                    [value, value]
                })
                .collect::<Vec<_>>(),
        )
    }
    fn frequency(samples: &[f32]) -> f64 {
        let mono = samples.iter().step_by(2).copied().collect::<Vec<_>>();
        let mono = &mono[mono.len() / 4..mono.len() * 3 / 4];
        let crossings = mono
            .windows(2)
            .filter(|values| values[0] <= 0.0 && values[1] > 0.0)
            .count();
        crossings as f64 / (mono.len() as f64 / 48_000.0)
    }
    #[test]
    fn streaming_speed_preserves_pitch_duration_stereo_and_seeking() {
        for speed in [0.5, 2.0] {
            for preserve_pitch in [true, false] {
                let options = Arc::new(RwLock::new(Options {
                    speed,
                    preserve_pitch,
                }));
                let mut source = Tempo::new(sine(), options);
                let output = source.by_ref().collect::<Vec<_>>();
                let expected = 192_000.0 / speed as f64;
                assert!(
                    (output.len() as f64 - expected).abs() < 2000.0,
                    "duration: {} vs {expected}",
                    output.len()
                );
                let expected = if preserve_pitch {
                    440.0
                } else {
                    440.0 * speed as f64
                };
                assert!(
                    (frequency(&output) - expected).abs() < 8.0,
                    "pitch: {} vs {expected}",
                    frequency(&output)
                );
                assert!(
                    output
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .all(|frame| frame[0] == frame[1])
                );
                source.try_seek(Duration::from_millis(500)).unwrap();
                assert!(source.next().is_some());
            }
        }
        let options = Arc::new(RwLock::new(Options {
            speed: 1.0,
            preserve_pitch: true,
        }));
        let mut source = Tempo::new(sine(), options.clone());
        assert_eq!(
            source.by_ref().take(8192).collect::<Vec<_>>(),
            sine().take(8192).collect::<Vec<_>>()
        );
        options.write().unwrap().speed = 2.0;
        let output = source.collect::<Vec<_>>();
        assert!(output.len() < 96_000);
        assert!((frequency(&output) - 440.0).abs() < 8.0);
    }
}
