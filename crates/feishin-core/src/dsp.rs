use crate::{model::Item, storage::Settings};
use rodio::{Source, source::SeekError};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

pub const FREQUENCIES: [f32; 12] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 3000.0, 4000.0, 6300.0, 10000.0, 16000.0,
];
pub const WIDTHS: [f32; 12] = [1.9, 1.3, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.2, 1.2, 1.5];
#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(default)]
pub struct Compressor {
    pub enabled: bool,
    pub threshold: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
    pub makeup: f32,
    pub knee: f32,
}
impl Default for Compressor {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -24.0,
            ratio: 4.0,
            attack: 20.0,
            release: 250.0,
            makeup: 6.0,
            knee: 2.83,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct Config {
    pub equalizer: [f32; 12],
    pub gain: f32,
    pub preamp: f32,
    pub compressor: Compressor,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            equalizer: [0.0; 12],
            gain: 1.0,
            preamp: 0.0,
            compressor: Compressor::default(),
        }
    }
}
impl Config {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            equalizer: if settings.equalizer_enabled {
                settings.equalizer
            } else {
                [0.0; 12]
            },
            gain: 1.0,
            preamp: if settings.equalizer_enabled {
                settings.equalizer_preamp
            } else {
                0.0
            },
            compressor: settings.compressor,
        }
    }
}

pub fn replay_gain(item: &Item, settings: &Settings) -> f32 {
    let (db, peak) = match settings.replay_gain.as_str() {
        "track" => (
            item.gain_track.or(item.gain_album),
            item.peak_track.or(item.peak_album),
        ),
        "album" => (
            item.gain_album.or(item.gain_track),
            item.peak_album.or(item.peak_track),
        ),
        _ => return 1.0,
    };
    let gain = db
        .filter(|db| db.is_finite())
        .map_or(1.0, |db| 10.0f32.powf(db.clamp(-60.0, 30.0) / 20.0));
    peak.filter(|peak| peak.is_finite() && *peak > 0.0)
        .map_or(gain, |peak| gain.min(1.0 / peak))
}

struct Band {
    coefficients: [f64; 5],
    state: Vec<[f64; 2]>,
}
impl Band {
    fn new(frequency: f64, width: f64, gain: f64, sample_rate: f64, channels: usize) -> Self {
        let a = 10.0f64.powf(gain / 40.0);
        let omega = 2.0 * std::f64::consts::PI * frequency / sample_rate;
        let alpha =
            omega.sin() * (std::f64::consts::LN_2 / 2.0 * width * omega / omega.sin()).sinh();
        let a0 = 1.0 + alpha / a;
        Self {
            coefficients: [
                (1.0 + alpha * a) / a0,
                -2.0 * omega.cos() / a0,
                (1.0 - alpha * a) / a0,
                -2.0 * omega.cos() / a0,
                (1.0 - alpha / a) / a0,
            ],
            state: vec![[0.0; 2]; channels],
        }
    }
    fn filter(&mut self, sample: f64, channel: usize) -> f64 {
        let [b0, b1, b2, a1, a2] = self.coefficients;
        let state = &mut self.state[channel];
        let output = b0 * sample + state[0];
        state[0] = b1 * sample - a1 * output + state[1];
        state[1] = b2 * sample - a2 * output;
        output
    }
}

pub struct Equalized<S> {
    inner: S,
    config: Arc<RwLock<Config>>,
    current: Config,
    bands: Vec<Band>,
    channel: usize,
    samples: usize,
    sample_rate: u32,
    channels: u16,
    envelope: f64,
}
impl<S: Source> Equalized<S> {
    pub fn new(inner: S, config: Arc<RwLock<Config>>) -> Self {
        let mut source = Self {
            sample_rate: inner.sample_rate(),
            channels: inner.channels(),
            inner,
            config,
            current: Config::default(),
            bands: vec![],
            channel: 0,
            samples: 0,
            envelope: 0.0,
        };
        source.refresh();
        source
    }
    fn refresh(&mut self) {
        let config = self.config.read().map(|config| *config).unwrap_or_default();
        if config != self.current
            || self.inner.sample_rate() != self.sample_rate
            || self.inner.channels() != self.channels
        {
            self.current = config;
            self.sample_rate = self.inner.sample_rate();
            self.channels = self.inner.channels();
            self.bands = FREQUENCIES
                .iter()
                .zip(WIDTHS)
                .zip(config.equalizer)
                .filter(|((f, _), g)| {
                    (**f as f64) < self.sample_rate as f64 * 0.49
                        && g.is_finite()
                        && g.abs() > 0.001
                })
                .map(|((f, width), g)| {
                    Band::new(
                        *f as f64,
                        width as f64,
                        g.clamp(-24.0, 24.0) as f64,
                        self.sample_rate as f64,
                        self.channels as usize,
                    )
                })
                .collect();
            self.channel %= self.channels.max(1) as usize;
        }
    }
}
impl<S: Source> Iterator for Equalized<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.inner.next()?;
        if self.samples.is_multiple_of(1024) {
            self.refresh();
        }
        self.samples = self.samples.wrapping_add(1);
        let mut sample =
            sample as f64 * 10.0f64.powf(self.current.preamp.clamp(-12.0, 12.0) as f64 / 20.0);
        for band in &mut self.bands {
            sample = band.filter(sample, self.channel);
        }
        self.channel = (self.channel + 1) % self.channels.max(1) as usize;
        let compressor = self.current.compressor;
        if compressor.enabled {
            let timing = if sample.abs() > self.envelope {
                compressor.attack.clamp(0.01, 2000.0)
            } else {
                compressor.release.clamp(1.0, 9000.0)
            };
            let coefficient = (-1.0
                / (self.sample_rate as f64 * self.channels.max(1) as f64 * timing as f64 / 1000.0))
                .exp();
            self.envelope = coefficient * self.envelope + (1.0 - coefficient) * sample.abs();
            let over = 20.0 * self.envelope.max(1e-12).log10()
                - compressor.threshold.clamp(-60.0, 0.0) as f64;
            let slope = 1.0 / compressor.ratio.clamp(1.0, 20.0) as f64 - 1.0;
            let knee = 20.0 * (compressor.knee.clamp(1.0, 10.0) as f64).log10();
            let reduction = if over < -knee / 2.0 {
                0.0
            } else if knee > 0.0 && over.abs() < knee / 2.0 {
                slope * (over + knee / 2.0).powi(2) / (2.0 * knee)
            } else {
                slope * over.max(0.0)
            };
            sample *= 10.0f64.powf((reduction + compressor.makeup.clamp(0.0, 30.0) as f64) / 20.0);
        }
        Some(sample as f32 * self.current.gain)
    }
}
impl<S: Source> Source for Equalized<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }
    fn channels(&self) -> u16 {
        self.inner.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }
    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(position)?;
        for band in &mut self.bands {
            band.state.fill([0.0; 2]);
        }
        self.channel = 0;
        self.envelope = 0.0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preamp_and_compressor_reduce_a_signal_without_device_or_premature_clipping() {
        let source = || rodio::buffer::SamplesBuffer::new(1, 48000, vec![0.5; 48000]);
        let config = Arc::new(RwLock::new(Config {
            preamp: -6.0206,
            ..Default::default()
        }));
        assert!((Equalized::new(source(), config.clone()).last().unwrap() - 0.25).abs() < 1e-5);
        *config.write().unwrap() = Config {
            compressor: Compressor {
                enabled: true,
                threshold: -20.0,
                ratio: 4.0,
                attack: 0.01,
                release: 1.0,
                makeup: 0.0,
                knee: 1.0,
            },
            ..Default::default()
        };
        let compressed = Equalized::new(source(), config.clone()).last().unwrap();
        assert!(
            (0.148..0.151).contains(&compressed),
            "compressed level {compressed}"
        );
        config.write().unwrap().compressor.ratio = 1.0;
        assert!((Equalized::new(source(), config.clone()).last().unwrap() - 0.5).abs() < 1e-5);
        config.write().unwrap().compressor.enabled = false;
        assert_eq!(Equalized::new(source(), config.clone()).last(), Some(0.5));
        config.write().unwrap().gain = 3.0;
        // Sink volume is applied later, so DSP must preserve headroom until then.
        assert_eq!(Equalized::new(source(), config).last(), Some(1.5));
        let settings = Settings {
            equalizer_preamp: 6.0,
            ..Default::default()
        };
        assert_eq!(Config::from_settings(&settings).preamp, 0.0);
        assert_eq!(FREQUENCIES.len(), 12);
    }
    #[test]
    fn original_compressor_ranges_import_and_reach_native_and_ffmpeg_processing() {
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../../feishin-gpui/assets/compressor-settings-reference.json"
        ))
        .unwrap();
        for case in cases.iter().filter(|case| {
            [
                "enabled",
                "makeup-commit",
                "knee-commit",
                "attack-blur",
                "ratio-enter",
                "threshold-clamp",
                "reset",
                "disabled-again",
            ]
            .contains(&case["name"].as_str().unwrap())
        }) {
            let imported = crate::migration::preview(
                &Settings::default(),
                &serde_json::to_vec(&serde_json::json!({"playback":{"compressor":case["stored"]}}))
                    .unwrap(),
            )
            .unwrap()
            .settings;
            let expected: Compressor = serde_json::from_value(case["stored"].clone()).unwrap();
            assert_eq!(imported.compressor, expected, "{}", case["name"]);
            let filter = crate::proxy::dsp_filters(&imported, 1.);
            let filter = if filter.is_empty() {
                filter
            } else {
                format!("lavfi=[{filter}]")
            };
            // Preserve the editor value; FFmpeg's documented hard knee limit is 8.
            assert_eq!(
                filter,
                case["filter"]
                    .as_str()
                    .unwrap()
                    .replace(":knee=10", ":knee=8"),
                "{}",
                case["name"]
            );
        }
        let source = rodio::buffer::SamplesBuffer::new(1, 48000, vec![0.01; 480]);
        let config = Arc::new(RwLock::new(Config {
            compressor: Compressor {
                enabled: true,
                ratio: 1.,
                makeup: 30.,
                ..Default::default()
            },
            ..Default::default()
        }));
        let sample = Equalized::new(source, config).last().unwrap();
        assert!((sample - 0.31622776).abs() < 1e-6, "30 dB makeup: {sample}");
        for (key, value) in [("makeup", 30.1), ("knee", 10.1)] {
            assert!(
                crate::migration::preview(
                    &Settings::default(),
                    &serde_json::to_vec(
                        &serde_json::json!({"playback":{"compressor":{key:value}}})
                    )
                    .unwrap()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn equalizer_gain_flat_response_live_changes_and_replaygain_peak_limit() {
        let samples = (0..44100)
            .map(|n| (2.0 * std::f32::consts::PI * 1000.0 * n as f32 / 44100.0).sin() * 0.1)
            .collect::<Vec<_>>();
        let config = Arc::new(RwLock::new(Config::default()));
        let source = rodio::buffer::SamplesBuffer::new(1, 44100, samples.clone());
        let flat = Equalized::new(source, config.clone()).collect::<Vec<_>>();
        assert_eq!(flat, samples);
        config.write().unwrap().equalizer[5] = 6.0;
        let source = rodio::buffer::SamplesBuffer::new(1, 44100, samples.clone());
        let boosted = Equalized::new(source, config.clone())
            .skip(5000)
            .collect::<Vec<_>>();
        let peak = boosted.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        assert!((0.198..0.201).contains(&peak));
        let mut source = Equalized::new(
            rodio::buffer::SamplesBuffer::new(1, 44100, samples),
            config.clone(),
        );
        config.write().unwrap().gain = 0.0;
        assert_eq!(source.next(), Some(0.0));
        let item = Item {
            gain_track: Some(6.0),
            peak_track: Some(1.0),
            ..Default::default()
        };
        let settings = Settings {
            replay_gain: "track".into(),
            ..Default::default()
        };
        assert_eq!(replay_gain(&item, &settings), 1.0);
        assert_eq!(replay_gain(&item, &Settings::default()), 1.0);
    }
}
