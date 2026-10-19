use anyhow::{Result, ensure};
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "type")]
    pub kind: String,
    pub bar_align: String,
    pub bar_gap: f32,
    pub bar_radius: f32,
    pub bar_width: f32,
    pub loading_delay: f32,
    pub stretched: bool,
    pub skip_enabled: bool,
    pub skip_backward_seconds: f64,
    pub skip_forward_seconds: f64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            kind: "slider".into(),
            bar_align: "center".into(),
            bar_gap: 1.,
            bar_radius: 4.,
            bar_width: 2.,
            loading_delay: 2.,
            stretched: false,
            skip_enabled: false,
            skip_backward_seconds: 5.,
            skip_forward_seconds: 10.,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.kind.as_str(), "slider" | "waveform")
                && matches!(self.bar_align.as_str(), "top" | "center" | "bottom"),
            "Invalid seek bar mode"
        );
        ensure!(
            (0. ..=30.).contains(&self.bar_width)
                && (0. ..=30.).contains(&self.bar_gap)
                && (0. ..=30.).contains(&self.bar_radius)
                && (0. ..=60.).contains(&self.loading_delay),
            "Invalid waveform drawing settings"
        );
        ensure!(
            self.skip_backward_seconds.is_finite()
                && self.skip_backward_seconds >= 0.
                && self.skip_forward_seconds.is_finite()
                && self.skip_forward_seconds >= 0.,
            "Invalid skip duration"
        );
        Ok(())
    }
    pub fn load_delay(&self) -> f32 {
        if self.loading_delay == 0. {
            2.
        } else {
            self.loading_delay
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Data {
    pub peaks: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub right_peaks: Vec<f32>,
    pub duration: f64,
}
impl Data {
    pub fn validate(&self) -> bool {
        !self.peaks.is_empty()
            && self.peaks.len() <= 4096
            && self.duration.is_finite()
            && self.duration > 0.
            && (self.right_peaks.is_empty() || self.right_peaks.len() == self.peaks.len())
            && self
                .peaks
                .iter()
                .chain(&self.right_peaks)
                .all(|peak| (0. ..=1.).contains(peak))
    }

    // Match the original wavesurfer.js 7.12.8 renderer, including its zero-value
    // defaults, channel alignment, device-pixel rounding and canvas slicing.
    pub fn drawing(&self, config: &Config, width: f64, height: f64, scale: f64) -> Vec<Canvas> {
        if !self.validate()
            || config.validate().is_err()
            || !width.is_finite()
            || !height.is_finite()
            || !scale.is_finite()
            || width <= 0.
            || height <= 0.
            || scale <= 0.
        {
            return Vec::new();
        }
        let scale = scale.max(1.);
        let bars = config.bar_width != 0. || config.bar_gap != 0. || config.bar_align != "center";
        let grid_width = if config.bar_width == 0. {
            0.5
        } else {
            f64::from(config.bar_width)
        };
        let grid_gap = if config.bar_gap == 0. {
            grid_width / 2.
        } else {
            f64::from(config.bar_gap)
        };
        let clamp_width = |width: f64| {
            if bars {
                (width / (grid_width + grid_gap)).floor() * (grid_width + grid_gap)
            } else {
                width
            }
        };
        let single_width = clamp_width(width.min(8000.));
        if single_width == 0. {
            return Vec::new();
        }
        let right = if self.right_peaks.is_empty() {
            &self.peaks
        } else {
            &self.right_peaks
        };
        (0..(width / single_width).ceil() as usize)
            .filter_map(|index| {
                let offset = index as f64 * single_width;
                let canvas_width = clamp_width((width - offset).min(single_width));
                if canvas_width <= 0. {
                    return None;
                }
                let start = ((offset / width) * self.peaks.len() as f64).floor() as usize;
                let end =
                    (((offset + canvas_width) / width) * self.peaks.len() as f64).floor() as usize;
                let channels = [&self.peaks[start..end], &right[start..end]];
                let max = channels[0].iter().copied().fold(0., f32::max);
                let vertical_scale = if config.stretched && max > 0. {
                    1. / f64::from(max)
                } else {
                    1.
                };
                let pixel_width = (canvas_width * scale).round();
                let pixel_height = (height * scale).round();
                let half_height = pixel_height / 2.;
                let mut canvas = Canvas {
                    offset: offset.round(),
                    width: canvas_width,
                    bars: Vec::new(),
                    paths: Vec::new(),
                };
                if bars {
                    let bar_width = if config.bar_width == 0. {
                        1.
                    } else {
                        f64::from(config.bar_width) * scale
                    };
                    let gap = if config.bar_gap != 0. {
                        f64::from(config.bar_gap) * scale
                    } else if config.bar_width != 0. {
                        bar_width / 2.
                    } else {
                        0.
                    };
                    let spacing = bar_width + gap;
                    let index_scale = if channels[0].is_empty() {
                        0.
                    } else {
                        pixel_width / spacing / channels[0].len() as f64
                    };
                    let mut previous_x = 0.;
                    let mut maxima = [0f64; 2];
                    for i in 0..=channels[0].len() {
                        let x = (i as f64 * index_scale).round();
                        if x > previous_x {
                            let top = (maxima[0] * half_height * vertical_scale).round();
                            let bottom = (maxima[1] * half_height * vertical_scale).round();
                            let total = (top + bottom).max(1.);
                            let y = match config.bar_align.as_str() {
                                "top" => 0.,
                                "bottom" => pixel_height - total,
                                _ => half_height - top,
                            };
                            canvas
                                .bars
                                .push([previous_x * spacing, y, bar_width, total]);
                            previous_x = x;
                            maxima = [0.; 2];
                        }
                        for channel in 0..2 {
                            maxima[channel] = maxima[channel].max(f64::from(
                                channels[channel].get(i).copied().unwrap_or_default(),
                            ));
                        }
                    }
                } else {
                    for (index, channel) in channels.iter().enumerate() {
                        let horizontal_scale = if channel.is_empty() {
                            0.
                        } else {
                            pixel_width / channel.len() as f64
                        };
                        let mut path = vec![[0., half_height]];
                        let mut previous_x = 0.;
                        let mut maximum = 0f64;
                        for i in 0..=channel.len() {
                            let x = (i as f64 * horizontal_scale).round();
                            if x > previous_x {
                                let delta =
                                    (maximum * half_height * vertical_scale).round().max(1.);
                                path.push([
                                    previous_x,
                                    half_height + if index == 0 { -delta } else { delta },
                                ]);
                                previous_x = x;
                                maximum = 0.;
                            }
                            maximum =
                                maximum.max(f64::from(channel.get(i).copied().unwrap_or_default()));
                        }
                        path.push([previous_x, half_height]);
                        canvas.paths.push(path);
                    }
                }
                Some(canvas)
            })
            .collect()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Canvas {
    pub offset: f64,
    pub width: f64,
    pub bars: Vec<[f64; 4]>,
    pub paths: Vec<Vec<[f64; 2]>>,
}
pub fn analyze<S: Source>(mut source: S, cancelled: &AtomicBool) -> Result<Data> {
    let channels = source.channels() as usize;
    let rate = source.sample_rate() as u64;
    ensure!(channels > 0 && rate > 0, "Invalid waveform audio format");
    let mut peaks = Vec::<f32>::new();
    let mut right_peaks = Vec::<f32>::new();
    let mut block = 1024u64;
    let mut count = 0;
    let mut total = 0u64;
    let mut peak = [0f32; 2];
    while let Some(sample) = source.next() {
        ensure!(!cancelled.load(Ordering::Relaxed), "Waveform cancelled");
        if sample.is_finite() {
            peak[0] = peak[0].max(sample.abs().min(1.));
        }
        for channel in 1..channels {
            if let Some(sample) = source.next()
                && channel == 1
                && sample.is_finite()
            {
                peak[1] = peak[1].max(sample.abs().min(1.));
            }
        }
        count += 1;
        total += 1;
        ensure!(total <= rate * 6 * 3600, "Waveform exceeds six hours");
        if count == block {
            peaks.push(peak[0]);
            if channels > 1 {
                right_peaks.push(peak[1]);
            }
            peak = [0.; 2];
            count = 0;
            if peaks.len() == 4096 {
                for channel in [&mut peaks, &mut right_peaks] {
                    for i in 0..channel.len() / 2 {
                        channel[i] = channel[i * 2].max(channel[i * 2 + 1]);
                    }
                    channel.truncate(channel.len() / 2);
                }
                block *= 2;
            }
        }
    }
    if count > 0 {
        peaks.push(peak[0]);
        if channels > 1 {
            right_peaks.push(peak[1]);
        }
    }
    let data = Data {
        peaks,
        right_peaks,
        duration: total as f64 / rate as f64,
    };
    ensure!(data.validate(), "No waveform audio data");
    Ok(data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waveform_geometry_matches_original_renderer_at_multiple_device_scales() {
        #[derive(Deserialize)]
        struct Case {
            config: Config,
            data: Data,
            width: f64,
            height: f64,
            scale: f64,
            canvases: Vec<Canvas>,
        }
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/waveform-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let case: Case = serde_json::from_value(case.clone()).unwrap();
            assert_eq!(
                case.data
                    .drawing(&case.config, case.width, case.height, case.scale),
                case.canvases,
                "{:?} width={} scale={}",
                case.config,
                case.width,
                case.scale
            );
        }
        for case in reference["loadDelays"].as_array().unwrap() {
            let config = Config {
                loading_delay: case["value"].as_f64().unwrap() as f32,
                ..Config::default()
            };
            assert_eq!(
                f64::from(config.load_delay()) * 1000.,
                case["milliseconds"].as_f64().unwrap()
            );
        }
    }
    #[test]
    fn stereo_peaks_preserve_duration_transients_normalization_and_bounded_memory() {
        let mut samples = vec![0.; 1024 * 2 * 5000];
        samples[2048 * 100 + 1] = 0.8;
        samples[2048 * 2500] = 0.4;
        let data = analyze(
            rodio::buffer::SamplesBuffer::new(2, 48000, samples),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(data.duration, 1024. * 5000. / 48000.);
        assert!(data.peaks.len() <= 4096);
        assert_eq!(data.peaks.iter().copied().fold(0., f32::max), 0.4);
        assert_eq!(data.right_peaks.iter().copied().fold(0., f32::max), 0.8);
        assert_eq!(data.right_peaks.len(), data.peaks.len());
        assert_eq!(data.peaks[50], 0.);
        assert_eq!(data.right_peaks[1250], 0.);
        assert!(
            analyze(
                rodio::buffer::SamplesBuffer::new(1, 48000, vec![0.; 16]),
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert!(
            analyze(
                rodio::buffer::SamplesBuffer::new(1, 48000, vec![f32::NAN; 16]),
                &AtomicBool::new(false)
            )
            .unwrap()
            .peaks
            .iter()
            .all(|peak| *peak == 0.)
        );
    }
}
