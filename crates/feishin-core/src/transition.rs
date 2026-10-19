//! A continuous PCM source. Track boundaries and fades happen on the audio
//! thread, without depending on UI timers or opening another output stream.
use rodio::{
    Source,
    source::{SeekError, UniformSourceIterator},
};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const SAMPLES_PER_SECOND: u64 = RATE as u64 * CHANNELS as u64;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Normal,
    #[default]
    Gapless,
    Crossfade,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Curve {
    Linear,
    #[default]
    EqualPower,
    Exponential,
    SCurve,
}

pub fn fade_gains(curve: Curve, progress: f64) -> (f32, f32) {
    let p = progress.clamp(0.0, 1.0);
    match curve {
        Curve::Linear => ((1.0 - p) as f32, p as f32),
        Curve::EqualPower => (
            (p * std::f64::consts::FRAC_PI_2).cos() as f32,
            (p * std::f64::consts::FRAC_PI_2).sin() as f32,
        ),
        Curve::Exponential => {
            let end = (-5.0f64).exp();
            (
                (((-5.0 * p).exp() - end) / (1.0 - end)) as f32,
                ((1.0 - (-5.0 * p).exp()) / (1.0 - end)) as f32,
            )
        }
        Curve::SCurve => {
            let gain = p * p * (3.0 - 2.0 * p);
            ((1.0 - gain) as f32, gain as f32)
        }
    }
}

pub struct Track {
    pub key: String,
    source: Box<dyn Source + Send>,
    duration: Option<u64>,
    position: u64,
    gain: f32,
}
impl Track {
    pub fn with_duration(mut self, seconds: f64) -> Self {
        if self.duration.is_none() && seconds.is_finite() && seconds > 0.0 {
            self.duration = Some(samples(Duration::from_secs_f64(seconds)));
        }
        self
    }
    pub fn new(source: impl Source + Send + 'static, key: String, gain: f32) -> Self {
        let duration = source.total_duration().map(samples);
        Self {
            key,
            source: Box::new(UniformSourceIterator::new(source, CHANNELS, RATE)),
            duration,
            position: 0,
            gain,
        }
    }
    fn next(&mut self) -> Option<f32> {
        let value = self.source.next()?;
        self.position += 1;
        Some(value)
    }
}
fn samples(duration: Duration) -> u64 {
    (duration.as_secs_f64() * SAMPLES_PER_SECOND as f64).round() as u64
}

#[derive(Clone, Copy)]
pub struct Options {
    pub mode: Mode,
    pub seconds: f64,
    pub curve: Curve,
}
impl From<&crate::storage::Settings> for Options {
    fn from(settings: &crate::storage::Settings) -> Self {
        Self {
            mode: settings.transition,
            seconds: settings.crossfade_seconds.clamp(0.0, 30.0),
            curve: settings.crossfade_style,
        }
    }
}

pub struct Handle {
    next: Mutex<Option<Track>>,
    revision: AtomicU64,
    key: Mutex<String>,
    position: AtomicU64,
    gain: AtomicU32,
    options: RwLock<Options>,
}
impl Handle {
    pub fn prepare(&self, track: Option<Track>) {
        if let Ok(mut next) = self.next.lock() {
            *next = track;
        }
        self.revision.fetch_add(1, Ordering::Release);
    }
    pub fn configure(&self, options: Options, gain: f32) {
        if let Ok(mut current) = self.options.write() {
            *current = options;
        }
        self.gain.store(gain.to_bits(), Ordering::Relaxed);
    }
    pub fn key(&self) -> String {
        self.key.lock().map(|key| key.clone()).unwrap_or_default()
    }
    pub fn position(&self) -> f64 {
        self.position.load(Ordering::Relaxed) as f64 / SAMPLES_PER_SECOND as f64
    }
}

pub struct Continuous {
    current: Track,
    incoming: Option<Track>,
    shared: Arc<Handle>,
    revision: u64,
    options: Options,
    fade_samples: u64,
    fading: bool,
    since_refresh: usize,
    advanced: Box<dyn Fn(&str) + Send>,
}
impl Continuous {
    pub fn new(
        track: Track,
        options: Options,
        advanced: impl Fn(&str) + Send + 'static,
    ) -> (Self, Arc<Handle>) {
        let shared = Arc::new(Handle {
            next: Mutex::new(None),
            revision: AtomicU64::new(0),
            key: Mutex::new(track.key.clone()),
            position: AtomicU64::new(0),
            gain: AtomicU32::new(track.gain.to_bits()),
            options: RwLock::new(options),
        });
        (
            Self {
                current: track,
                incoming: None,
                shared: shared.clone(),
                revision: 0,
                options,
                fade_samples: 0,
                fading: false,
                since_refresh: 1024,
                advanced: Box::new(advanced),
            },
            shared,
        )
    }
    fn refresh(&mut self) {
        let revision = self.shared.revision.load(Ordering::Acquire);
        if revision != self.revision {
            self.incoming = self
                .shared
                .next
                .lock()
                .ok()
                .and_then(|mut next| next.take());
            self.revision = revision;
            self.fading = false;
        }
        if let Ok(options) = self.shared.options.read() {
            self.options = *options;
        }
        self.shared
            .position
            .store(self.current.position, Ordering::Relaxed);
        self.since_refresh = 0;
    }
    fn promote(&mut self) -> bool {
        let Some(incoming) = self.incoming.take() else {
            return false;
        };
        if self.options.mode == Mode::Normal {
            return false;
        }
        self.current = incoming;
        self.fading = false;
        self.shared
            .gain
            .store(self.current.gain.to_bits(), Ordering::Relaxed);
        self.shared
            .position
            .store(self.current.position, Ordering::Relaxed);
        if let Ok(mut key) = self.shared.key.lock() {
            key.clone_from(&self.current.key);
        }
        (self.advanced)(&self.current.key);
        true
    }
}
impl Iterator for Continuous {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.since_refresh >= 1024 {
            self.refresh();
        }
        self.since_refresh += 1;
        let value = match self.current.next() {
            Some(value) => value,
            None => {
                // Pick up a decoder prepared during the final audio block.
                self.refresh();
                if !self.promote() {
                    return None;
                }
                self.current.next()?
            }
        };
        let gain = f32::from_bits(self.shared.gain.load(Ordering::Relaxed));
        if self.options.mode == Mode::Crossfade
            && let Some(incoming) = &mut self.incoming
        {
            let fade = (self.options.seconds * SAMPLES_PER_SECOND as f64) as u64;
            let fade = fade
                .min(self.current.duration.unwrap_or(0))
                .min(incoming.duration.unwrap_or(fade));
            let remaining = self
                .current
                .duration
                .unwrap_or(u64::MAX)
                .saturating_sub(self.current.position);
            if !self.fading && fade > 0 && remaining < fade {
                self.fading = true;
                self.fade_samples = remaining + 1;
            }
            if self.fading
                && let Some(next) = incoming.next()
            {
                let progress = incoming.position as f64 / self.fade_samples.max(1) as f64;
                let (out, input) = fade_gains(self.options.curve, progress);
                return Some(value * gain * out + next * incoming.gain * input);
            }
        }
        Some(value * gain)
    }
}
impl Source for Continuous {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        CHANNELS
    }
    fn sample_rate(&self) -> u32 {
        RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.current.source.try_seek(position)?;
        self.current.position = samples(position);
        if let Some(incoming) = &mut self.incoming {
            incoming.source.try_seek(Duration::ZERO)?;
            incoming.position = 0;
        }
        self.fading = false;
        self.shared
            .position
            .store(self.current.position, Ordering::Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    fn track(value: f32, length: usize, key: &str) -> Track {
        Track::new(
            SamplesBuffer::new(2, 48_000, vec![value; length]),
            key.into(),
            1.0,
        )
    }
    fn options(mode: Mode) -> Options {
        Options {
            mode,
            seconds: 0.01,
            curve: Curve::Linear,
        }
    }
    #[test]
    fn gapless_boundaries_have_no_missing_samples_and_duplicate_tracks_advance() {
        let keys = Arc::new(Mutex::new(Vec::new()));
        let captured = keys.clone();
        let (mut source, handle) = Continuous::new(
            track(0.25, 4800, "first"),
            options(Mode::Gapless),
            move |key| captured.lock().unwrap().push(key.to_owned()),
        );
        handle.prepare(Some(track(0.5, 4800, "duplicate")));
        let output: Vec<_> = source.by_ref().collect();
        assert_eq!(output.len(), 9600);
        assert!(output[..4800].iter().all(|sample| *sample == 0.25));
        assert!(output[4800..].iter().all(|sample| *sample == 0.5));
        assert_eq!(*keys.lock().unwrap(), ["duplicate"]);
        assert_eq!(handle.key(), "duplicate");
    }
    #[test]
    fn crossfade_overlaps_audio_and_curves_have_correct_endpoints() {
        let (source, handle) =
            Continuous::new(track(0.25, 4800, "first"), options(Mode::Crossfade), |_| {});
        handle.prepare(Some(track(0.75, 4800, "next")));
        let output: Vec<_> = source.collect();
        assert_eq!(output.len(), 9600 - 960);
        assert!((output[4320] - 0.5).abs() < 0.002);
        assert!((output[4799] - 0.75).abs() < 0.002);
        for curve in [
            Curve::Linear,
            Curve::EqualPower,
            Curve::Exponential,
            Curve::SCurve,
        ] {
            let (out, input) = fade_gains(curve, 0.0);
            assert!((out - 1.0).abs() < 1e-6 && input.abs() < 1e-6);
            let (out, input) = fade_gains(curve, 1.0);
            assert!(out.abs() < 1e-6 && (input - 1.0).abs() < 1e-6);
        }
        let (out, input) = fade_gains(Curve::EqualPower, 0.5);
        assert!((out * out + input * input - 1.0).abs() < 1e-6);
    }
    #[test]
    fn cancelled_next_track_never_plays_and_seek_resets_the_fade() {
        let (mut source, handle) =
            Continuous::new(track(0.25, 4800, "first"), options(Mode::Crossfade), |_| {
                panic!("cancelled track advanced")
            });
        handle.prepare(Some(track(0.75, 4800, "next")));
        assert_eq!(source.by_ref().take(4200).count(), 4200);
        source.try_seek(Duration::ZERO).unwrap();
        assert_eq!(source.next(), Some(0.25));
        handle.prepare(None);
        assert!(source.all(|sample| sample == 0.25));
    }
}
