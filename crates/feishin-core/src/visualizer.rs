use anyhow::{Result, ensure};
use rodio::{Source, source::SeekError};
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub const PRESETS: &str = include_str!("../assets/spectrum-presets.json");
pub fn spectrum_gradients() -> &'static Value {
    static GRADIENTS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    GRADIENTS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/spectrum-gradients.json"))
            .expect("Bundled spectrum gradients")
    })
}
pub struct MilkdropPresets {
    pub names: Vec<String>,
    pub files: std::collections::BTreeMap<String, String>,
}
pub fn milkdrop_presets() -> &'static MilkdropPresets {
    static PRESETS: std::sync::OnceLock<MilkdropPresets> = std::sync::OnceLock::new();
    PRESETS.get_or_init(|| {
        use std::io::Read;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
            include_bytes!("../assets/milkdrop-presets.zip").as_slice(),
        ))
        .expect("Bundled MilkDrop archive");
        let mut names = vec![];
        let mut files = std::collections::BTreeMap::new();
        for index in 0..archive.len() {
            let mut file = archive.by_index(index).expect("Bundled MilkDrop preset");
            let Some(name) = file.name().strip_suffix(".milk").map(str::to_owned) else {
                continue;
            };
            let mut data = String::new();
            file.read_to_string(&mut data)
                .expect("Bundled MilkDrop preset text");
            names.push(name.clone());
            files.insert(name, data);
        }
        MilkdropPresets { names, files }
    })
}
pub fn install_milkdrop_textures(root: &std::path::Path) -> Result<std::path::PathBuf> {
    let folder = root.join("milkdrop-textures-v1");
    std::fs::create_dir_all(&folder)?;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(
        include_bytes!("../assets/milkdrop-textures.zip").as_slice(),
    ))?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let name = file
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid bundled texture path"))?;
        ensure!(
            name.components().count() == 1,
            "Invalid bundled texture path"
        );
        let path = folder.join(name);
        if std::fs::metadata(&path).is_ok_and(|metadata| metadata.len() == file.size()) {
            continue;
        }
        let mut data = vec![];
        std::io::Read::read_to_end(&mut file, &mut data)?;
        crate::storage::atomic_write(&path, &data)?;
    }
    Ok(folder)
}
pub fn milkdrop_names(options: &Value) -> Vec<String> {
    let mut names = milkdrop_presets().names.clone();
    if let Some(custom) = options["customPresets"].as_object() {
        names.extend(
            custom
                .keys()
                .filter(|name| !milkdrop_presets().files.contains_key(*name))
                .cloned(),
        );
    }
    names
}
pub fn milkdrop_data<'a>(options: &'a Value, name: &str) -> Option<&'a str> {
    options["customPresets"][name]
        .as_str()
        .or_else(|| milkdrop_presets().files.get(name).map(String::as_str))
}
pub fn milkdrop_pool(options: &Value) -> Vec<String> {
    let names = milkdrop_names(options);
    let selected = options["selectedPresets"].as_array();
    let names = if !enabled(options, "includeAllPresets") && selected.is_some_and(|p| !p.is_empty())
    {
        selected
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|name| names.iter().any(|n| n == name))
            .map(str::to_owned)
            .collect()
    } else {
        names
    };
    names
        .into_iter()
        .filter(|name| {
            !options["ignoredPresets"]
                .as_array()
                .is_some_and(|ignored| ignored.iter().any(|value| value.as_str() == Some(name)))
        })
        .collect()
}
pub fn milkdrop_next(options: &Value, reverse: bool, random_index: usize) -> Option<String> {
    let mut pool = milkdrop_pool(options);
    if pool.is_empty() {
        return None;
    }
    let current = options["currentPreset"].as_str();
    if enabled(options, "randomizeNextPreset") {
        if pool.len() > 1 {
            pool.retain(|name| Some(name.as_str()) != current);
        }
        return Some(pool[random_index % pool.len()].clone());
    }
    let index = pool.iter().position(|name| Some(name.as_str()) == current);
    Some(
        pool[match index {
            Some(index) if reverse => (index + pool.len() - 1) % pool.len(),
            Some(index) => (index + 1) % pool.len(),
            None if reverse => pool.len() - 1,
            None => 0,
        }]
        .clone(),
    )
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "type")]
    pub kind: String,
    pub audiomotionanalyzer: Value,
    pub butterchurn: Value,
    pub system_audio_consent: bool,
}
impl Default for Config {
    fn default() -> Self {
        let presets: Value = serde_json::from_str(PRESETS).expect("Bundled spectrum presets");
        let mut analyzer = presets[0]["value"].clone();
        analyzer["opacity"] = json!(1);
        analyzer["maxDecibels"] = json!(-15);
        analyzer["customGradients"] = json!([]);
        analyzer["presets"] = presets;
        Self {
            kind: "audiomotionanalyzer".into(),
            audiomotionanalyzer: analyzer,
            butterchurn: json!({"blendTime":2.5,"cyclePresets":true,"cycleTime":30,"ignoredPresets":[],"includeAllPresets":true,"maxFPS":0,"opacity":1,"randomizeNextPreset":true,"selectedPresets":[]}),
            system_audio_consent: false,
        }
    }
}
pub fn number(options: &Value, key: &str, default: f32) -> f32 {
    options[key].as_f64().map_or(default, |value| value as f32)
}
pub fn enabled(options: &Value, key: &str) -> bool {
    options[key].as_bool().unwrap_or(false)
}
pub fn validate(config: &Config) -> Result<()> {
    ensure!(
        matches!(config.kind.as_str(), "audiomotionanalyzer" | "butterchurn"),
        "Invalid visualizer type"
    );
    ensure!(
        serde_json::to_vec(config)?.len() <= 1_000_000,
        "Visualizer settings are too large"
    );
    let v = &config.audiomotionanalyzer;
    ensure!(
        v.is_object() && config.butterchurn.is_object(),
        "Invalid visualizer settings"
    );
    let fft = number(v, "fftSize", 16384.) as usize;
    ensure!(
        (32..=32768).contains(&fft)
            && fft.is_power_of_two()
            && number(v, "fftSize", 16384.) == fft as f32,
        "Invalid FFT size"
    );
    for (key, min, max) in [
        ("minFreq", 1., 96000.),
        ("maxFreq", 1., 96000.),
        ("minDecibels", -200., 0.),
        ("maxDecibels", -200., 50.),
        ("smoothing", 0., 1.),
        ("opacity", 0., 1.),
        ("barSpace", 0., 100.),
        ("lineWidth", 0., 100.),
        ("radius", 0., 1.),
        ("reflexRatio", 0., 0.99),
        ("reflexAlpha", 0., 1.),
        ("reflexBright", 0., 10.),
        ("fillAlpha", 0., 1.),
        ("peakHoldTime", 0., 60000.),
        ("peakFadeTime", 0., 60000.),
        ("gravity", 0., 100.),
        ("volume", 0., 10.),
        ("linearBoost", 0., 100.),
        ("spinSpeed", -100., 100.),
        ("maxFPS", 0., 240.),
    ] {
        if let Some(value) = v.get(key) {
            ensure!(
                value
                    .as_f64()
                    .is_some_and(|value| value.is_finite() && (min..=max).contains(&value)),
                "Invalid visualizer {key}"
            );
        }
    }
    ensure!(
        number(v, "minFreq", 20.) < number(v, "maxFreq", 22050.)
            && number(v, "minDecibels", -85.) < number(v, "maxDecibels", -15.),
        "Invalid analyzer range"
    );
    ensure!(
        matches!(v["mode"].as_u64().unwrap_or(10), 0..=8 | 10),
        "Invalid spectrum mode"
    );
    for (key, values) in [
        (
            "channelLayout",
            &[
                "single",
                "dual-horizontal",
                "dual-vertical",
                "dual-combined",
            ][..],
        ),
        ("frequencyScale", &["log", "linear", "bark", "mel"][..]),
        ("colorMode", &["gradient", "bar-index", "bar-level"][..]),
        ("weightingFilter", &["", "A", "B", "C", "D", "Z", "468"][..]),
    ] {
        if let Some(value) = v.get(key) {
            ensure!(
                value.as_str().is_some_and(|value| values.contains(&value)),
                "Invalid {key}"
            );
        }
    }
    if let Some(presets) = v.get("presets") {
        ensure!(
            presets
                .as_array()
                .is_some_and(|presets| presets.len() <= 1000
                    && presets.iter().all(|preset| preset["name"]
                        .as_str()
                        .is_some_and(|name| name.len() <= 256)
                        && preset["value"].is_object())),
            "Invalid spectrum presets"
        );
    }
    if let Some(gradients) = v.get("customGradients") {
        ensure!(
            gradients
                .as_array()
                .is_some_and(|gradients| gradients.len() <= 100
                    && gradients.iter().all(|gradient| gradient["name"]
                        .as_str()
                        .is_some_and(|name| !name.is_empty() && name.len() <= 128)
                        && gradient
                            .get("dir")
                            .is_none_or(|dir| matches!(dir.as_str(), Some("h" | "v")))
                        && gradient["colorStops"].as_array().is_some_and(|stops| !stops
                            .is_empty()
                            && stops.len() <= 100
                            && stops.iter().all(|stop| {
                                stop.as_str()
                                    .or_else(|| stop["color"].as_str())
                                    .is_some_and(|color| crate::appearance::color(color).is_ok())
                                    && ["pos", "level"].iter().all(|key| {
                                        stop.get(key).is_none_or(|value| {
                                            value.as_f64().is_some_and(|n| {
                                                n.is_finite() && (0. ..=1.).contains(&n)
                                            })
                                        })
                                    })
                                    && ["positionEnabled", "levelEnabled"]
                                        .iter()
                                        .all(|key| stop.get(key).is_none_or(Value::is_boolean))
                            })))),
            "Invalid spectrum gradients"
        );
    }
    let milk = &config.butterchurn;
    for key in ["cyclePresets", "randomizeNextPreset", "includeAllPresets"] {
        ensure!(
            milk.get(key).is_none_or(Value::is_boolean),
            "Invalid MilkDrop {key}"
        );
    }
    ensure!(
        milk.get("currentPreset").is_none_or(|value| value
            .as_str()
            .is_some_and(|name| name.len() <= 512 && !name.contains('\0'))),
        "Invalid MilkDrop currentPreset"
    );
    for (key, min, max) in [
        ("blendTime", 0., 60.),
        ("cycleTime", 1., 3600.),
        ("maxFPS", 0., 240.),
        ("opacity", 0., 1.),
    ] {
        if let Some(value) = milk.get(key) {
            ensure!(
                value
                    .as_f64()
                    .is_some_and(|n| n.is_finite() && (min..=max).contains(&n)),
                "Invalid MilkDrop {key}"
            );
        }
    }
    for key in ["selectedPresets", "ignoredPresets"] {
        if let Some(value) = milk.get(key) {
            ensure!(
                value.as_array().is_some_and(|v| v.len() <= 10000
                    && v.iter().all(|n| n.as_str().is_some_and(|n| n.len() <= 512))),
                "Invalid MilkDrop {key}"
            );
        }
    }
    if let Some(custom) = milk.get("customPresets") {
        ensure!(
            custom.as_object().is_some_and(|v| v.len() <= 100
                && v.iter().all(|(name, data)| !name.is_empty()
                    && name.len() <= 512
                    && data.as_str().is_some_and(|s| s.starts_with("[preset00]")
                        || s.starts_with("MILKDROP_PRESET_VERSION="))
                    && !data.as_str().unwrap_or("").contains('\0'))),
            "Invalid custom MilkDrop presets"
        );
    }
    Ok(())
}

#[derive(Clone)]
pub struct Capture {
    enabled: Arc<AtomicBool>,
    frames: Arc<Mutex<VecDeque<[f32; 2]>>>,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(false)),
            frames: Arc::new(Mutex::new(VecDeque::with_capacity(32768))),
        }
    }
}
impl Capture {
    pub fn enable(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        if !enabled {
            self.clear();
        }
    }
    pub fn clear(&self) {
        if let Ok(mut frames) = self.frames.lock() {
            frames.clear();
        }
    }
    pub fn push(&self, samples: &[f32], channels: usize) {
        if !self.enabled.load(Ordering::Relaxed) || channels == 0 {
            return;
        }
        if let Ok(mut frames) = self.frames.try_lock() {
            if !self.enabled.load(Ordering::Relaxed) {
                return;
            }
            for sample in samples.chunks_exact(channels) {
                if frames.len() == 32768 {
                    frames.pop_front();
                }
                frames.push_back([sample[0], sample.get(1).copied().unwrap_or(sample[0])]);
            }
            while frames.len() > 32768 {
                frames.pop_front();
            }
        }
    }
    pub fn snapshot(&self, size: usize) -> Vec<[f32; 2]> {
        self.frames
            .lock()
            .map(|frames| {
                frames
                    .iter()
                    .skip(frames.len().saturating_sub(size))
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn take_frames(&self) -> Vec<[f32; 2]> {
        self.frames
            .lock()
            .map(|mut frames| frames.drain(..).collect())
            .unwrap_or_default()
    }
}
pub struct Tap<S> {
    source: S,
    capture: Capture,
    chunk: [f32; 1024],
    filled: usize,
}
impl<S: Source> Tap<S> {
    pub fn new(source: S, capture: Capture) -> Self {
        Self {
            source,
            capture,
            chunk: [0.; 1024],
            filled: 0,
        }
    }
}
impl<S: Source> Iterator for Tap<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.source.next();
        if let Some(sample) = sample {
            self.chunk[self.filled] = sample;
            self.filled += 1;
        }
        if self.filled
            == self.chunk.len() / self.source.channels() as usize * self.source.channels() as usize
            || sample.is_none()
        {
            self.capture
                .push(&self.chunk[..self.filled], self.source.channels() as usize);
            self.filled = 0;
        }
        sample
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.source.size_hint()
    }
}
impl<S: Source> Source for Tap<S> {
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.filled = 0;
        self.capture.clear();
        self.source.try_seek(pos)
    }
}
#[derive(Clone, Debug, Default)]
pub struct Band {
    pub frequency: f32,
    pub low: f32,
    pub high: f32,
    pub position: f32,
    pub width: f32,
    pub values: [f64; 2],
    pub peaks: [f32; 2],
    pub peak_alpha: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub struct ModeFlags {
    pub graph: bool,
    pub leds: bool,
    pub lumi: bool,
    pub alpha: bool,
    pub outline: bool,
    pub round: bool,
    pub peak_line: bool,
}
impl ModeFlags {
    pub fn new(options: &Value) -> Self {
        let mode = number(options, "mode", 10.) as usize;
        let graph = mode == 10;
        let bands = !mode.is_multiple_of(10);
        let radial = enabled(options, "radial");
        let leds = enabled(options, "ledBars") && bands && !radial;
        let lumi = enabled(options, "lumiBars") && bands && !radial;
        Self {
            graph,
            leds,
            lumi,
            alpha: enabled(options, "alphaBars") && !lumi && !graph,
            outline: enabled(options, "outlineBars") && bands && !lumi && !leds,
            round: enabled(options, "roundBars") && bands && !lumi && !leds,
            peak_line: graph && enabled(options, "peakLine") && enabled(options, "showPeaks"),
        }
    }
}

struct FrequencyBand {
    frequency: f64,
    low: f64,
    high: f64,
    position: f64,
    width: f64,
}
fn frequency_bands(options: &Value, rate: u32, fft_size: usize, width: f32) -> Vec<FrequencyBand> {
    frequency_layout(options, rate, fft_size, width).0
}
fn frequency_layout(
    options: &Value,
    rate: u32,
    fft_size: usize,
    width: f32,
) -> (Vec<FrequencyBand>, (f64, f64)) {
    let min = f64::from(number(options, "minFreq", 20.)).max(1.);
    let max = f64::from(number(options, "maxFreq", 22050.)).min(f64::from(rate) / 2.);
    if max <= min || fft_size < 2 || rate == 0 {
        return (vec![], (min, max));
    }
    let width = f64::from(width).max(1.);
    let mode = number(options, "mode", 10.) as usize;
    let divisions = [0, 24, 12, 8, 6, 4, 3, 2, 1]
        .get(mode)
        .copied()
        .unwrap_or(0);
    let scale = options["frequencyScale"].as_str().unwrap_or("log");
    let scaling = |frequency: f64| match scale {
        "linear" => frequency,
        "bark" => 26.81 * frequency / (1960. + frequency) - 0.53,
        "mel" => (1. + frequency / 700.).log2(),
        _ => frequency.log2(),
    };
    let inverse = |value: f64| match scale {
        "linear" => value,
        "bark" => 1960. / (26.81 / (value + 0.53) - 1.),
        "mel" => 700. * (2_f64.powf(value) - 1.),
        _ => 2_f64.powf(value),
    };
    let mut bands = vec![];
    let mut extent = (min, max);
    if divisions > 0 && scale == "log" {
        let ansi = enabled(options, "ansiBands");
        let ratio = if ansi {
            10_f64.powf(0.3 / divisions as f64)
        } else {
            2_f64.powf(1. / divisions as f64)
        };
        let half = ratio.sqrt();
        let mut frequency = if ansi {
            7.94328235 / if divisions % 2 == 0 { half } else { 1. }
        } else {
            8.17579892
        };
        while frequency <= max && bands.len() < 2048 {
            let low = round_significant(frequency / half, 4, true);
            let high = round_significant(frequency * half, 4, true);
            let center = if ansi && divisions < 4 {
                let power = frequency.log10().trunc();
                let normalized = frequency / 10_f64.powf(power);
                let preferred = [
                    1., 1.12, 1.25, 1.4, 1.6, 1.8, 2., 2.24, 2.5, 2.8, 3.15, 3.55, 4., 4.5, 5.,
                    5.6, 6.3, 7.1, 8., 9., 10.,
                ];
                let nearest = preferred
                    .into_iter()
                    .min_by(|a, b| f64::abs(a - normalized).total_cmp(&f64::abs(b - normalized)))
                    .unwrap();
                (nearest * 10_f64.powf(power + 5.)).trunc() / 100_000.
            } else if ansi {
                round_significant(
                    frequency,
                    if frequency / 10_f64.powf(frequency.log10().floor()) < 5. {
                        3
                    } else {
                        2
                    },
                    false,
                )
            } else {
                round_significant(frequency, 4, true)
            };
            if center >= min {
                bands.push(FrequencyBand {
                    frequency: center,
                    low,
                    high,
                    position: 0.,
                    width: 0.,
                });
            }
            frequency *= ratio;
        }
        if let (Some(first), Some(last)) = (bands.first(), bands.last()) {
            extent = (first.low, last.high);
        }
        if let Some(first) = bands.first_mut() {
            first.low = first.low.max(min);
        }
        if let Some(last) = bands.last_mut() {
            last.high = last.high.min(max);
        }
        let count = bands.len() as f64;
        for (index, band) in bands.iter_mut().enumerate() {
            band.position = index as f64 / count;
            band.width = 1. / count;
        }
    } else if divisions > 0 {
        let count = divisions * 10;
        let low = scaling(min);
        let range = scaling(max) - low;
        for index in 0..count {
            let position = index as f64 / count as f64;
            let step = 1. / count as f64;
            bands.push(FrequencyBand {
                frequency: inverse(low + range * (position + step / 2.)),
                low: inverse(low + range * position),
                high: inverse(low + range * (position + step)),
                position,
                width: step,
            });
        }
    } else {
        let bin = |frequency: f64, round: bool| {
            let value = frequency * fft_size as f64 / f64::from(rate);
            (if round { value.round() } else { value.floor() } as usize).min(fft_size / 2 - 1)
        };
        let low = scaling(min);
        let unit = width / (scaling(max) - low);
        for index in bin(min, false)..=bin(max, true) {
            let frequency = (index as f64 * f64::from(rate) / fft_size as f64).max(1.);
            let position = (unit * (scaling(frequency) - low)).round() / width;
            if bands
                .last()
                .is_some_and(|last: &FrequencyBand| last.position >= position)
            {
                let last = bands.last_mut().unwrap();
                last.high = frequency;
                last.frequency = (last.low * frequency).sqrt();
            } else {
                bands.push(FrequencyBand {
                    frequency,
                    low: frequency,
                    high: frequency,
                    position,
                    width: 1. / width,
                });
            }
        }
    }
    (bands, extent)
}

#[derive(Debug)]
pub struct FrequencyLabel {
    pub position: f32,
    pub text: String,
    pub highlight: bool,
    pub max_width: f32,
}
#[derive(Debug)]
pub struct AmplitudeScaleLine {
    pub value: f32,
    pub y: f32,
    pub label_y: f32,
    pub labels: Vec<f32>,
    pub start: f32,
    pub end: f32,
    pub major: bool,
}
/// Original dB/percentage scale coordinates, before channel mirroring.
pub fn amplitude_scale(
    options: &Value,
    size: [f32; 2],
    scale_width: f32,
    channel: usize,
) -> Vec<AmplitudeScaleLine> {
    let flags = ModeFlags::new(options);
    if flags.lumi
        || enabled(options, "radial")
        || channel == 1 && options["channelLayout"].as_str() == Some("dual-combined")
    {
        return vec![];
    }
    let geometry = spectrum_geometry(size, options, 1.);
    let horizontal = options["channelLayout"].as_str() == Some("dual-horizontal");
    let vertical = options["channelLayout"].as_str() == Some("dual-vertical");
    let mirror = number(options, "mirror", 0.);
    let linear = enabled(options, "linearAmplitude");
    let max = if linear {
        100.
    } else {
        number(options, "maxDecibels", -15.)
    };
    let min = if linear {
        0.
    } else {
        number(options, "minDecibels", -85.)
    };
    if !min.is_finite() || !max.is_finite() || max <= min {
        return vec![];
    }
    let top = if vertical {
        (geometry.channel_height + geometry.channel_gap) * channel as f32
    } else {
        0.
    };
    let font_size = (scale_width / 2.).floor();
    let at_start = mirror != -1. && (!horizontal || channel == 0 || mirror == 1.);
    let at_end = mirror != 1. && (!horizontal || channel as f32 != mirror);
    let mut rows = vec![];
    let mut value = max;
    while value > min && rows.len() < 1024 {
        let y = top + (max - value) * geometry.height / (max - min);
        let major = value % 2. == 0.;
        let mut labels = vec![];
        if major {
            if at_start {
                labels.push(scale_width * 0.85);
            }
            if at_end {
                labels.push(if horizontal { geometry.width } else { size[0] } - scale_width * 0.1);
            }
        }
        rows.push(AmplitudeScaleLine {
            value,
            y: y.trunc() + 0.5,
            label_y: y + font_size * if y == top { 0.8 } else { 0.35 },
            labels,
            start: geometry.initial_x + scale_width * f32::from(major && at_start),
            end: geometry.initial_x + geometry.width - scale_width * f32::from(major && at_end),
            major,
        });
        value -= if linear { 20. } else { 5. };
    }
    rows
}
/// The original X scale's note selection, frequency positions and mirror labels.
pub fn frequency_labels(
    options: &Value,
    size: [f32; 2],
    scale_height: f32,
    measure: impl Fn(&str) -> f32,
) -> Vec<FrequencyLabel> {
    let geometry = spectrum_geometry(size, options, 1.);
    let (_, (low, high)) = frequency_layout(
        options,
        48000,
        number(options, "fftSize", 16384.) as usize,
        geometry.width,
    );
    let scale = options["frequencyScale"].as_str().unwrap_or("log");
    let scaling = |frequency: f64| match scale {
        "linear" => frequency,
        "bark" => 26.81 * frequency / (1960. + frequency) - 0.53,
        "mel" => (1. + frequency / 700.).log2(),
        _ => frequency.log2(),
    };
    if low >= high {
        return vec![];
    }
    let unit = f64::from(geometry.width) / (scaling(high) - scaling(low));
    let notes = enabled(options, "noteLabels");
    let mirror = number(options, "mirror", 0.);
    let horizontal = options["channelLayout"].as_str() == Some("dual-horizontal");
    let vertical = options["channelLayout"].as_str() == Some("dual-vertical");
    let radial = enabled(options, "radial");
    let font_size = (scale_height / 2.).floor();
    let label_width = font_size
        * if radial {
            if notes { 1. } else { 2. }
        } else if notes {
            0.7
        } else {
            1.5
        };
    let mut candidates = vec![];
    if !notes && (enabled(options, "ansiBands") || scale != "log") {
        let mut frequencies = vec![16., 31.5, 63., 125., 250., 500., 1000., 2000., 4000.];
        frequencies.extend(if scale == "linear" {
            vec![
                6000., 8000., 10000., 12000., 14000., 16000., 18000., 20000., 22000.,
            ]
        } else {
            vec![8000., 16000.]
        });
        candidates.extend(frequencies.into_iter().map(|frequency| (frequency, None)));
    } else {
        let mut frequency = 8.17579892_f64;
        for octave in -1..11 {
            for pitch in [
                Some("C"),
                None,
                Some("D"),
                None,
                Some("E"),
                Some("F"),
                None,
                Some("G"),
                None,
                Some("A"),
                None,
                Some("B"),
            ] {
                if frequency >= f64::from(number(options, "minFreq", 20.))
                    && frequency <= f64::from(number(options, "maxFreq", 22050.))
                    && (pitch == Some("C")
                        || notes && mirror == 0. && !horizontal && pitch.is_some())
                {
                    candidates.push((
                        frequency,
                        if notes {
                            Some(format!(
                                "{}{}",
                                pitch.unwrap(),
                                if pitch == Some("C") {
                                    octave.to_string()
                                } else {
                                    String::new()
                                }
                            ))
                        } else {
                            None
                        },
                    ));
                }
                frequency *= 2_f64.powf(1. / 12.);
            }
        }
    }
    let mut previous = -label_width / if radial { 1. } else { 4. };
    let mut labels = vec![];
    for (frequency, note) in candidates {
        let text = note.unwrap_or_else(|| {
            if frequency < 1000. {
                (frequency as u32).to_string()
            } else {
                format!("{}k", (frequency / 100.).floor() / 10.)
            }
        });
        let c = text.starts_with('C');
        if notes {
            let log = scale == "log";
            let linear = scale == "linear";
            let outer = !radial || vertical;
            let allowed = c
                || text == "G"
                    && (log
                        || frequency > 2000.
                        || !linear && frequency > 250.
                        || outer && (!linear && frequency > 125. || frequency > 1000.))
                || text == "E"
                    && (log
                        || frequency > 4000.
                        || !linear && frequency > 500.
                        || outer && (!linear && frequency > 250. || frequency > 2000.))
                || matches!(text.as_str(), "D" | "F" | "A" | "B")
                    && (linear && frequency > 4000.
                        || outer && (log || frequency > 2000. || !linear && frequency > 500.));
            if !allowed {
                continue;
            }
        }
        let x = (unit * (scaling(frequency) - scaling(low))) as f32;
        let max_width = font_size
            * if notes && mirror == 0. && !horizontal {
                if c { 1.2 } else { 0.6 }
            } else {
                3.
            };
        if if radial {
            x >= previous + label_width && x < geometry.width - label_width
        } else {
            x >= previous + label_width / 2. && x <= geometry.width
        } {
            let highlight = c && mirror == 0. && !horizontal;
            labels.push(FrequencyLabel {
                position: if radial {
                    if horizontal && mirror == 1. {
                        geometry.width - x
                    } else {
                        x
                    }
                } else if horizontal && mirror == -1. {
                    geometry.width - x
                } else {
                    geometry.initial_x + x
                },
                text: text.clone(),
                highlight,
                max_width: if radial { f32::INFINITY } else { max_width },
            });
            if horizontal || mirror != 0. && (x > label_width || mirror == 1.) {
                labels.push(FrequencyLabel {
                    position: if radial {
                        if horizontal && mirror != -1. {
                            geometry.width + x
                        } else {
                            -x
                        }
                    } else if horizontal && mirror != 1. {
                        geometry.width + x
                    } else {
                        (if geometry.initial_x != 0. {
                            geometry.initial_x
                        } else {
                            size[0]
                        }) - x
                    },
                    text: text.clone(),
                    highlight,
                    max_width: if radial { f32::INFINITY } else { max_width },
                });
            }
            previous = x + if radial {
                0.
            } else {
                max_width.min(measure(&text)) / 2.
            };
        }
    }
    labels
}
pub fn radial_scale_dimensions(size: [f32; 2], options: &Value) -> (f32, f32) {
    let minimum = size[0].min(size[1]);
    let height = (minimum / 34.).floor();
    let diameter = (minimum * 0.15)
        .max(
            radial_dimensions(size, options, 0).0 * 2.
                + f32::from(options["channelLayout"].as_str() == Some("dual-vertical")) * height,
        )
        .floor();
    (diameter, height)
}
fn round_significant(value: f64, digits: i32, preserve_integers: bool) -> f64 {
    let digits = if preserve_integers {
        digits.max(1 + value.log10() as i32)
    } else {
        digits
    };
    let factor = 10_f64.powf(f64::from(digits - 1) - value.log10().floor());
    (value * factor).round() / factor
}

fn normalized_amplitude(decibels: f32, options: &Value) -> f64 {
    let mut min = f64::from(number(options, "minDecibels", -85.));
    let mut max = f64::from(number(options, "maxDecibels", -15.));
    let mut value = f64::from(decibels);
    let boost = if enabled(options, "linearAmplitude") {
        min = 10_f64.powf(min / 20.);
        max = 10_f64.powf(max / 20.);
        let boost = 1. / f64::from(number(options, "linearBoost", 4.).max(f32::EPSILON));
        value = 10_f64.powf(value / 20.).powf(boost);
        boost
    } else {
        1.
    };
    ((value - min) / (max - min).powf(boost)).clamp(0., 1.)
}

fn peak_height(size: [f32; 2], options: &Value) -> f32 {
    if enabled(options, "radial") {
        return radial_dimensions(size, options, 0).1.abs().max(1.);
    }
    spectrum_geometry(size, options, 1.).height.max(1.)
}

/// Analyzer dimensions in canvas pixels, including the original LED spacing.
#[derive(Debug)]
pub struct SpectrumGeometry {
    pub width: f32,
    pub channel_height: f32,
    pub channel_gap: f32,
    pub height: f32,
    pub initial_x: f32,
    /// LED count, horizontal spacing, vertical spacing and element height.
    pub leds: Option<(usize, f32, f32, f32)>,
    pub no_led_gap: bool,
}
pub fn spectrum_geometry(size: [f32; 2], options: &Value, pixel_ratio: f32) -> SpectrumGeometry {
    let flags = ModeFlags::new(options);
    let radial = enabled(options, "radial");
    let vertical = options["channelLayout"].as_str() == Some("dual-vertical") && !radial;
    let horizontal = options["channelLayout"].as_str() == Some("dual-horizontal");
    let mirror = number(options, "mirror", 0.);
    let channel_height = ((size[1] - if vertical && !flags.leds { 0.5 } else { 0. })
        / if vertical { 2. } else { 1. })
    .floor();
    let height = (channel_height
        * if flags.lumi || radial {
            1.
        } else {
            1. - number(options, "reflexRatio", 0.5)
        })
    .floor();
    let width = size[0]
        - if horizontal || mirror != 0. {
            (size[0] / 2.).floor()
        } else {
            0.
        };
    let no_led_gap = options["channelLayout"].as_str() != Some("dual-vertical")
        || number(options, "reflexRatio", 0.5) > 0. && !flags.lumi;
    let leds = if flags.leds {
        let (max_count, vertical_spacing, horizontal_spacing) =
            match number(options, "mode", 3.) as usize {
                1 => (128, 3., 0.45),
                2 => (128, 4., 0.225),
                3 => (96, 6., 0.225),
                4 => (80, 6., 0.225),
                5 => (80, 6., 0.125),
                6 => (64, 6., 0.125),
                7 => (48, 8., 0.125),
                _ => (24, 16., 0.125),
            };
        let gap = (vertical_spacing * pixel_ratio)
            .min((height / (540. / vertical_spacing) + 0.1).floor().max(2.));
        let max_height = height + if no_led_gap { gap } else { 0. };
        let count = max_count
            .min((max_height / (gap * 2.)).floor() as usize)
            .max(1);
        Some((
            count,
            horizontal_spacing,
            gap,
            max_height / count as f32 - gap,
        ))
    } else {
        None
    };
    SpectrumGeometry {
        width,
        channel_height,
        height,
        no_led_gap,
        leds,
        channel_gap: if vertical {
            size[1] - channel_height * 2.
        } else {
            0.
        },
        initial_x: if mirror == -1. && !horizontal && !radial {
            (size[0] / 2.).floor()
        } else {
            0.
        },
    }
}
impl SpectrumGeometry {
    pub fn analyzer_bottom(&self, channel: usize) -> f32 {
        (self.channel_height + self.channel_gap) * channel as f32 + self.height
            - if self.no_led_gap {
                0.
            } else {
                self.leds.map_or(0., |leds| leds.2)
            }
    }
    pub fn led_height(&self, value: f64) -> f32 {
        self.leds.map_or(0., |(count, _, gap, height)| {
            ((value * count as f64).floor() as f32 * (height + gap) - gap).max(0.)
        })
    }
    /// Apply spacing to normalized FFT bands without losing their frequencies.
    pub fn bar_bounds(&self, bands: &[Band], options: &Value) -> Vec<(f32, f32)> {
        let band_mode = !(number(options, "mode", 10.) as usize).is_multiple_of(10);
        let spacing = number(options, "barSpace", 0.1);
        let mut result = Vec::with_capacity(bands.len());
        for (index, band) in bands.iter().enumerate() {
            let step = if band_mode {
                self.width / bands.len() as f32
            } else {
                band.width * self.width
            };
            let gap = (spacing
                * if spacing > 0. && spacing < 1. {
                    step
                } else {
                    1.
                })
            .min(step - 1.)
            .max(self.leds.map_or(0., |leds| step * leds.1));
            let mut x = self.initial_x
                + if band_mode {
                    (index as f64 * f64::from(self.width) / bands.len() as f64) as f32
                } else {
                    (band.position * self.width).round()
                };
            let mut width = step;
            if band_mode {
                width -= gap;
                if spacing == 0. && self.leds.is_none() {
                    x = x.trunc();
                    width = width.trunc();
                    if let Some(&(previous_x, previous_width)) = result.last()
                        && x > previous_x + previous_width
                    {
                        x -= 1.;
                        width += 1.;
                    }
                } else {
                    x += gap / 2.;
                }
            }
            result.push((x, width));
        }
        result
    }
}

pub fn radial_dimensions(size: [f32; 2], options: &Value, channel: usize) -> (f32, f32) {
    let vertical = options["channelLayout"].as_str() == Some("dual-vertical");
    let mut inner = (size[0].min(size[1])
        * 0.375
        * if vertical {
            1.
        } else {
            number(options, "radius", 0.7)
        })
    .floor();
    let mut outer = (size[0] * 0.5).floor().min((size[1] * 0.5).floor());
    if enabled(options, "radialInvert") && !vertical {
        std::mem::swap(&mut inner, &mut outer);
    }
    (
        inner,
        (outer - inner) * if vertical && channel == 1 { -1. } else { 1. },
    )
}

fn update_peak(
    band: &mut Band,
    hold: &mut f32,
    channel: usize,
    value: f32,
    elapsed: f32,
    height: f32,
    options: &Value,
) {
    let flags = ModeFlags::new(options);
    if band.peaks[channel] > 0. && band.peak_alpha[channel] > 0. {
        *hold -= elapsed;
        if *hold < 0. {
            if enabled(options, "fadePeaks") && !flags.peak_line {
                let alpha =
                    if flags.alpha && !(flags.outline && number(options, "lineWidth", 1.9) > 0.) {
                        band.peaks[channel]
                    } else {
                        1.
                    };
                band.peak_alpha[channel] = (alpha
                    * (1. + *hold / (number(options, "peakFadeTime", 900.) / 1000.)))
                    .max(0.);
                if band.peak_alpha[channel] <= 0. {
                    band.peaks[channel] = 0.;
                }
            } else {
                band.peaks[channel] +=
                    *hold * elapsed * number(options, "gravity", 11.) * 1000. / height;
            }
        }
    }
    if value >= band.peaks[channel] {
        band.peaks[channel] = value;
        *hold = number(options, "peakHoldTime", 500.) / 1000.;
        band.peak_alpha[channel] =
            if flags.alpha && !(flags.outline && number(options, "lineWidth", 1.9) > 0.) {
                value
            } else {
                1.
            };
    }
}

/// AudioMotion rotates only while nonzero spectral energy is present.
/// Changing a nonzero speed preserves phase; zero speed resets it.
pub fn spin_angle(angle: f32, speed: f32, energy: bool, elapsed: f32) -> f32 {
    if speed == 0. {
        0.
    } else if energy {
        angle + speed * std::f32::consts::TAU * elapsed / 60.
    } else {
        angle
    }
}

pub struct Analyzer {
    fft: Arc<dyn Fft<f32>>,
    size: usize,
    previous: [Vec<f32>; 2],
    pub bands: Vec<Band>,
    pub dimensions: [f32; 2],
    pub pixel_ratio: f32,
    holds: Vec<[f32; 2]>,
}
impl Default for Analyzer {
    fn default() -> Self {
        Self {
            fft: FftPlanner::new().plan_fft_forward(16384),
            size: 16384,
            previous: [vec![], vec![]],
            bands: vec![],
            dimensions: [1280., 540.],
            pixel_ratio: 1.,
            holds: vec![],
        }
    }
}
impl Analyzer {
    pub fn update(&mut self, frames: &[[f32; 2]], rate: u32, options: &Value, elapsed: f32) {
        let size = (number(options, "fftSize", 16384.) as usize)
            .clamp(32, 32768)
            .next_power_of_two();
        if size != self.size {
            self.fft = FftPlanner::new().plan_fft_forward(size);
            self.size = size;
            self.previous = [vec![], vec![]];
        }
        let smoothing = number(options, "smoothing", 0.6).clamp(0., 1.);
        let mono = options["channelLayout"].as_str().unwrap_or("single") == "single";
        let mut db = [vec![], vec![]];
        for (channel, channel_db) in db.iter_mut().enumerate() {
            let mut samples = vec![Complex::new(0., 0.); size];
            let count = frames.len().min(size);
            for (index, frame) in frames[frames.len() - count..].iter().enumerate() {
                let i = size - count + index;
                let phase = std::f32::consts::TAU * i as f32 / size as f32;
                let window = 0.42 - 0.5 * phase.cos() + 0.08 * (2. * phase).cos();
                samples[i].re = (if mono {
                    (frame[0] + frame[1]) * 0.5
                } else {
                    frame[channel]
                }) * window
                    * number(options, "volume", 1.);
            }
            self.fft.process(&mut samples);
            self.previous[channel].resize(size / 2, 0.);
            *channel_db = samples[..size / 2]
                .iter()
                .enumerate()
                .map(|(i, value)| {
                    let magnitude = value.norm() / size as f32;
                    let smoothed =
                        self.previous[channel][i] * smoothing + magnitude * (1. - smoothing);
                    self.previous[channel][i] = smoothed;
                    20. * smoothed.max(1e-10).log10()
                        + weighting_db(
                            options["weightingFilter"].as_str().unwrap_or(""),
                            i as f32 * rate as f32 / size as f32,
                        )
                })
                .collect();
        }
        let pixel_ratio = self.pixel_ratio * if enabled(options, "loRes") { 0.5 } else { 1. };
        let canvas_size = self
            .dimensions
            .map(|dimension| (dimension * pixel_ratio).floor());
        let width = spectrum_geometry(canvas_size, options, pixel_ratio).width;
        let bounds = frequency_bands(options, rate, size, width);
        if self.bands.len() != bounds.len()
            || self
                .bands
                .iter()
                .zip(&bounds)
                .any(|(old, new)| old.low != new.low as f32 || old.high != new.high as f32)
        {
            self.bands = bounds.iter().map(|_| Band::default()).collect();
            self.holds = vec![[0.; 2]; bounds.len()];
        }
        let height = peak_height(canvas_size, options) / pixel_ratio;
        for (index, bounds) in bounds.into_iter().enumerate() {
            let band = &mut self.bands[index];
            band.frequency = bounds.frequency as f32;
            band.low = bounds.low as f32;
            band.high = bounds.high as f32;
            band.position = bounds.position as f32;
            band.width = bounds.width as f32;
            for (channel, channel_db) in db.iter().enumerate() {
                let interpolate = |frequency: f64| {
                    let bin = ((frequency * size as f64 / f64::from(rate)).floor() as usize)
                        .min(size / 2 - 1);
                    let low = (bin as f64 * f64::from(rate) / size as f64).max(1.);
                    let high = (bin + 1) as f64 * f64::from(rate) / size as f64;
                    let ratio = (frequency / low).log2() / (high / low).log2();
                    let value = channel_db[bin]
                        + if bin < channel_db.len() - 1 {
                            (channel_db[bin + 1] - channel_db[bin]) * ratio as f32
                        } else {
                            0.
                        };
                    (
                        bin,
                        if value.is_nan() {
                            f32::NEG_INFINITY
                        } else {
                            value
                        },
                    )
                };
                let (first, low) = interpolate(bounds.low);
                let (last, high) = interpolate(bounds.high);
                let decibels = channel_db[first + 1..last.max(first + 1)]
                    .iter()
                    .copied()
                    .fold(low.max(high), f32::max);
                let value = normalized_amplitude(decibels, options);
                band.values[channel] = value;
                update_peak(
                    band,
                    &mut self.holds[index][channel],
                    channel,
                    value as f32,
                    elapsed,
                    height,
                    options,
                );
            }
        }
    }
}
/// Standard A/B/C/D and ITU-R 468 frequency response, normalized around 1 kHz.
pub fn weighting_db(filter: &str, frequency: f32) -> f32 {
    let f = f64::from(frequency.max(1.));
    let x = f * f;
    let pole = (x + 20.6_f64.powi(2)) * (x + 12194_f64.powi(2));
    let (response, offset) = match filter {
        "A" => (
            12194_f64.powi(2) * x * x
                / (pole * ((x + 107.7_f64.powi(2)) * (x + 737.9_f64.powi(2))).sqrt()),
            2.,
        ),
        "B" => (
            12194_f64.powi(2) * x * f / (pole * (x + 158.5_f64.powi(2)).sqrt()),
            0.17,
        ),
        "C" => (12194_f64.powi(2) * x / pole, 0.06),
        "D" => {
            let h = ((1_037_918.48 - x).powi(2) + 1_080_768.16 * x)
                / ((9_837_328. - x).powi(2) + 11_723_776. * x);
            (
                f / 6.896_688_849_647_6e-5 * (h / ((x + 79_919.29) * (x + 1_345_600.))).sqrt(),
                0.,
            )
        }
        "468" => {
            let real = 1. - 1.363_894_795_463_638e-7 * x + 2.043_828_333_606_125e-15 * x * x
                - 4.737_338_981_378_384e-24 * x * x * x;
            let imag = f
                * (5.559_488_023_498_642e-4 - 2.118_150_887_518_656e-11 * x
                    + 1.306_612_257_412_824e-19 * x * x);
            (1.246_332_637_532_143e-4 * f / real.hypot(imag), 18.2)
        }
        _ => return 0.,
    };
    (20. * response.max(1e-20).log10() + offset) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_tracks_audio_energy_speed_changes_and_zero_resets() {
        let mut angle = 0.;
        for _ in 0..60 {
            angle = spin_angle(angle, 1., true, 1. / 60.);
        }
        assert!((angle - std::f32::consts::TAU / 60.).abs() < 1e-6);
        assert_eq!(spin_angle(angle, 2., false, 1.), angle);
        let reversed = spin_angle(angle, -1., true, 1.);
        assert!(reversed.abs() < 1e-6);
        assert_eq!(spin_angle(angle, 0., false, 1.), 0.);
        assert_eq!(spin_angle(angle, 0., true, 1.), 0.);
    }

    #[test]
    fn original_amplitude_scale_labels_and_dashed_grid() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/spectrum-reference.json")).unwrap();
        let cases = reference["scalesY"].as_array().unwrap();
        assert_eq!(cases.len(), 144);
        for case in cases {
            let options = &case["options"];
            let channels = if options["channelLayout"].as_str() == Some("single") {
                1
            } else {
                2
            };
            let rows: Vec<_> = (0..channels)
                .flat_map(|channel| amplitude_scale(options, [641., 481.], 20., channel))
                .collect();
            let labels: Vec<_> = rows
                .iter()
                .flat_map(|row| {
                    row.labels
                        .iter()
                        .map(move |x| (row.value.to_string(), *x, row.label_y))
                })
                .collect();
            let expected = case["labels"].as_array().unwrap();
            assert_eq!(labels.len(), expected.len(), "{case}");
            for ((text, x, y), expected) in labels.into_iter().zip(expected) {
                assert_eq!(text, expected["text"].as_str().unwrap(), "{case}");
                assert!(
                    (x - expected["x"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}"
                );
                assert!(
                    (y - expected["y"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}"
                );
            }
            let expected = case["lines"].as_array().unwrap();
            assert_eq!(rows.len(), expected.len(), "{case}");
            for (row, expected) in rows.into_iter().zip(expected) {
                for (actual, expected) in [row.start, row.y, row.end, row.y].into_iter().zip(
                    expected["points"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(|point| point.as_array().unwrap()),
                ) {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32).abs() < 0.0001,
                        "{case}: {row:?}"
                    );
                }
                assert_eq!(expected["dash"], json!([2, if row.major { 4 } else { 8 }]));
                assert_eq!(expected["offset"], json!(usize::from(!row.major)));
                assert_eq!(
                    expected["color"],
                    json!(if row.major { "#888" } else { "#555" })
                );
            }
        }
        assert!(
            amplitude_scale(&json!({"mode": 3, "lumiBars": true}), [641., 481.], 20., 0).is_empty()
        );
        assert!(amplitude_scale(&json!({"radial": true}), [641., 481.], 20., 0).is_empty());
    }
    #[test]
    fn original_radial_frequency_labels_and_rotations() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/spectrum-reference.json")).unwrap();
        let cases = reference["radialScales"].as_array().unwrap();
        assert_eq!(cases.len(), 768);
        for case in cases {
            let options = &case["options"];
            let (diameter, height) = radial_scale_dimensions([641., 481.], options);
            assert_eq!(
                diameter,
                case["diameter"].as_f64().unwrap() as f32,
                "{case}"
            );
            let center = (diameter / 2.).floor();
            let radius = center - height * 0.7;
            let actual = frequency_labels(options, [641., 481.], height, |_| 0.);
            let expected = case["labels"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{case}");
            for (actual, expected) in actual.into_iter().zip(expected) {
                let angle = std::f32::consts::TAU * actual.position / 641.;
                assert_eq!(actual.text, expected["text"].as_str().unwrap(), "{case}");
                assert_eq!(
                    actual.highlight,
                    expected["color"].as_str() == Some("#4f4"),
                    "{case}"
                );
                assert!(
                    (angle - expected["angle"].as_f64().unwrap() as f32).abs() < 0.00001,
                    "{case}"
                );
                for (actual, expected) in [
                    center + radius * (angle - std::f32::consts::FRAC_PI_2).cos(),
                    center + radius * (angle - std::f32::consts::FRAC_PI_2).sin(),
                ]
                .into_iter()
                .zip(expected["position"].as_array().unwrap())
                {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32).abs() < 0.0002,
                        "{case}"
                    );
                }
            }
        }
    }
    #[test]
    fn original_frequency_scale_labels_positions_notes_and_mirroring() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/spectrum-reference.json")).unwrap();
        for case in reference["scales"].as_array().unwrap() {
            let actual = frequency_labels(&case["options"], [641., 481.], 20., |_| 0.);
            let expected = case["labels"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{case}");
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert_eq!(actual.text, expected["text"].as_str().unwrap(), "{case}");
                assert!(
                    (actual.position - expected["x"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{actual:?} {expected} {case}"
                );
                assert!(
                    (actual.max_width - expected["maxWidth"].as_f64().unwrap() as f32).abs()
                        < 0.0001,
                    "{case}"
                );
                assert_eq!(actual.highlight, expected["color"] == "#4f4", "{case}");
            }
        }
    }
    #[test]
    fn original_canvas_channel_led_and_bar_geometry() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/spectrum-reference.json")).unwrap();
        for case in reference["geometry"].as_array().unwrap() {
            let options = &case["options"];
            let geometry = spectrum_geometry(
                [641., 481.],
                options,
                case["pixelRatio"].as_f64().unwrap() as f32,
            );
            for (name, value) in [
                ("analyzerWidth", geometry.width),
                ("analyzerHeight", geometry.height),
                ("channelHeight", geometry.channel_height),
                ("channelGap", geometry.channel_gap),
                ("initialX", geometry.initial_x),
            ] {
                assert!(
                    (value - case["aux"][name].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{name}: {value} {case}"
                );
            }
            if let Some((count, ratio, gap, height)) = geometry.leds {
                assert_eq!(count, case["leds"][0].as_u64().unwrap() as usize, "{case}");
                assert!(
                    (gap - case["leds"][2].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}"
                );
                assert!(
                    (height - case["leds"][3].as_f64().unwrap() as f32).abs() < 0.0001,
                    "{case}"
                );
                let count_bands = frequency_bands(options, 48000, 16384, geometry.width).len();
                assert!(
                    (geometry.width / count_bands as f32 * ratio
                        - case["leds"][1].as_f64().unwrap() as f32)
                        .abs()
                        < 0.0001,
                    "{case}"
                );
                assert_eq!(geometry.led_height(0.), 0.);
                assert!(
                    (geometry.led_height(1.)
                        - (geometry.height - if geometry.no_led_gap { 0. } else { gap }))
                    .abs()
                        < 0.0001
                );
            } else {
                assert!(case["leds"].is_null(), "{case}");
            }
            let bands: Vec<_> = frequency_bands(options, 48000, 16384, geometry.width)
                .into_iter()
                .map(|band| Band {
                    position: band.position as f32,
                    width: band.width as f32,
                    ..Default::default()
                })
                .collect();
            let actual = geometry.bar_bounds(&bands, options);
            for bar in case["bars"].as_array().unwrap() {
                let index = bar["index"].as_u64().unwrap() as usize;
                let (x, width) = actual[index];
                assert!(
                    (x - bar["posX"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "x {index}: {x} {case}"
                );
                assert!(
                    (width - bar["width"].as_f64().unwrap() as f32).abs() < 0.0001,
                    "width {index}: {width} {case}"
                );
            }
            for channel in 0..2 {
                let vertical = options["channelLayout"] == "dual-vertical";
                assert_eq!(
                    geometry.analyzer_bottom(if vertical { channel } else { 0 }),
                    case["aux"]["channelCoords"][channel]["analyzerBottom"]
                        .as_f64()
                        .unwrap() as f32,
                    "{case}"
                );
            }
        }
    }
    #[test]
    fn original_analyzer_amplitude_modes_and_frame_by_frame_peaks() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/spectrum-reference.json")).unwrap();
        assert_eq!(reference["version"], "4.5.4");
        for case in reference["radial"].as_array().unwrap() {
            let size = [
                case["size"][0].as_f64().unwrap() as f32,
                case["size"][1].as_f64().unwrap() as f32,
            ];
            let inner = case["inner"].as_f64().unwrap() as f32;
            let outer = case["outer"].as_f64().unwrap() as f32;
            for channel in 0..2 {
                let direction =
                    if channel == 1 && case["options"]["channelLayout"] == "dual-vertical" {
                        -1.
                    } else {
                        1.
                    };
                assert_eq!(
                    radial_dimensions(size, &case["options"], channel),
                    (inner, (outer - inner) * direction),
                    "{case}"
                );
            }
        }
        for case in reference["bands"].as_array().unwrap() {
            let actual = frequency_bands(
                &case["options"],
                48000,
                16384,
                case["width"].as_f64().unwrap() as f32,
            );
            assert_eq!(
                actual.len(),
                case["count"].as_u64().unwrap() as usize,
                "{case}"
            );
            for expected in case["expected"].as_array().unwrap() {
                let band = &actual[expected["index"].as_u64().unwrap() as usize];
                for (name, value) in [
                    ("frequency", band.frequency),
                    ("low", band.low),
                    ("high", band.high),
                ] {
                    let target = expected[name].as_f64().unwrap();
                    assert!(
                        (value - target).abs() < target * 0.000001,
                        "{} width {}: {name} {value} != {target}",
                        case["options"],
                        case["width"]
                    );
                }
            }
        }
        for case in reference["amplitudes"].as_array().unwrap() {
            let actual =
                normalized_amplitude(case["decibels"].as_f64().unwrap() as f32, &case["options"]);
            let expected = case["expected"].as_f64().unwrap();
            assert!((actual - expected).abs() < 0.000002, "{case}: {actual}");
        }
        for case in reference["modes"].as_array().unwrap() {
            let flags = ModeFlags::new(&case["options"]);
            for (name, actual) in [
                ("isLeds", flags.leds),
                ("isLumi", flags.lumi),
                ("isAlpha", flags.alpha),
                ("isOutline", flags.outline),
                ("isRound", flags.round),
            ] {
                assert_eq!(
                    actual,
                    case["flags"][name].as_bool().unwrap(),
                    "{case}: {name}"
                );
            }
            assert_eq!(
                peak_height([1280., 540.], &case["options"]),
                case["nominalHeight"].as_f64().unwrap() as f32
            );
        }
        for case in reference["peaks"].as_array().unwrap() {
            let mut band = Band::default();
            let mut hold = 0.;
            let size = [
                case["size"][0].as_f64().unwrap() as f32,
                case["size"][1].as_f64().unwrap() as f32,
            ];
            let elapsed = 1. / case["fps"].as_f64().unwrap() as f32;
            for (index, frame) in case["frames"].as_array().unwrap().iter().enumerate() {
                update_peak(
                    &mut band,
                    &mut hold,
                    0,
                    frame["value"].as_f64().unwrap() as f32,
                    elapsed,
                    peak_height(size, &case["options"]),
                    &case["options"],
                );
                assert!(
                    (band.peaks[0] - frame["peak"].as_f64().unwrap() as f32).abs() < 0.00002,
                    "{case}: frame {index}, peak {}",
                    band.peaks[0]
                );
                assert!(
                    (band.peak_alpha[0] - frame["alpha"].as_f64().unwrap() as f32).abs() < 0.00002,
                    "{case}: frame {index}, alpha {}",
                    band.peak_alpha[0]
                );
            }
        }
    }
    #[test]
    fn disabling_capture_waits_for_buffer_access_and_rejects_late_samples() {
        use std::{
            sync::mpsc,
            time::{Duration, Instant},
        };
        let capture = Capture::default();
        capture.enable(true);
        capture.push(&[0.25, -0.75], 2);
        let guard = capture.frames.lock().unwrap();
        let worker_capture = capture.clone();
        let (done, finished) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            worker_capture.enable(false);
            done.send(()).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while capture.enabled.load(Ordering::Relaxed) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(
            finished.recv_timeout(Duration::from_millis(30)).is_err(),
            "Disabling must wait until it can clear the buffer"
        );
        drop(guard);
        finished.recv_timeout(Duration::from_secs(3)).unwrap();
        worker.join().unwrap();
        capture.push(&[1., 1.], 2);
        assert!(capture.snapshot(10).is_empty());
        capture.enable(true);
        capture.push(&[0.5, -0.5], 2);
        assert_eq!(capture.snapshot(10), vec![[0.5, -0.5]]);
    }

    #[test]
    fn gradient_stop_and_milkdrop_config_validation() {
        let mut config = Config::default();
        config.audiomotionanalyzer["customGradients"] = json!([{
            "name": "Custom", "dir": "h", "colorStops": [
                {"color":"#ff000080", "pos":0.25, "level":1., "positionEnabled":true, "levelEnabled":false},
                "#0000ff"
            ]
        }]);
        validate(&config).unwrap();
        for (key, value) in [
            ("color", json!("broken")),
            ("pos", json!(1.1)),
            ("level", json!("0.5")),
            ("levelEnabled", json!(1)),
        ] {
            let mut invalid = config.clone();
            invalid.audiomotionanalyzer["customGradients"][0]["colorStops"][0][key] = value;
            assert!(validate(&invalid).is_err(), "{key}");
        }
        config.butterchurn["cyclePresets"] = json!("true");
        assert!(validate(&config).is_err());
    }
    #[test]
    fn weighting_octave_alignment_and_peak_fade() {
        for filter in ["A", "B", "C", "D", "468"] {
            assert!(weighting_db(filter, 1000.).abs() < 0.1, "{filter}");
            for frequency in [0., 20., 100., 1000., 6300., 22050., 96000.] {
                assert!(weighting_db(filter, frequency).is_finite());
            }
        }
        assert!((weighting_db("A", 100.) + 19.14).abs() < 0.1);
        assert!((weighting_db("468", 6300.) - 12.2).abs() < 0.2);
        assert_eq!(weighting_db("Z", 100.), 0.);
        let mut config = Config::default();
        let options = &mut config.audiomotionanalyzer;
        options["smoothing"] = json!(0);
        options["mode"] = json!(3);
        options["ansiBands"] = json!(true);
        options["weightingFilter"] = json!("Z");
        options["fadePeaks"] = json!(true);
        options["peakLine"] = json!(false);
        options["peakHoldTime"] = json!(0);
        options["peakFadeTime"] = json!(1000);
        let frames: Vec<_> = (0..16384)
            .map(|i| {
                let value = (std::f32::consts::TAU * 1000. * i as f32 / 48000.).sin() * 0.2;
                [value, value]
            })
            .collect();
        let mut analyzer = Analyzer::default();
        analyzer.update(&frames, 48000, options, 0.016);
        let index = analyzer
            .bands
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.values[0].total_cmp(&b.values[0]))
            .unwrap()
            .0;
        assert!(analyzer.bands[index].low <= 1000. && analyzer.bands[index].high >= 1000.);
        let peak = analyzer.bands[index].peaks[0];
        analyzer.update(&[], 48000, options, 0.5);
        assert!(
            analyzer.bands[index].peak_alpha[0] > 0.45
                && analyzer.bands[index].peak_alpha[0] < 0.55
        );
        assert_eq!(analyzer.bands[index].peaks[0], peak);
        analyzer.update(&[], 48000, options, 0.6);
        assert_eq!(analyzer.bands[index].peaks[0], 0.);
        validate(&config).unwrap();
    }
    #[test]
    fn milkdrop_presets_selection_cycle_and_pcm_consumption() {
        let bundled = milkdrop_presets();
        assert_eq!(bundled.names.len(), 349);
        assert_eq!(
            bundled.names[0],
            "_Aderrasi - Wanderer in Curved Space - mash0000 - faclempt kibitzing meshuggana schmaltz (Geiss color mix)"
        );
        assert_eq!(bundled.files.len(), bundled.names.len());
        assert!(
            bundled
                .files
                .values()
                .all(|data| data.contains("[preset00]"))
        );
        let first = bundled.names[0].clone();
        let second = bundled.names[1].clone();
        let mut config = Config::default();
        let options = &mut config.butterchurn;
        options["randomizeNextPreset"] = json!(false);
        options["currentPreset"] = json!(first);
        assert_eq!(milkdrop_next(options, false, 0), Some(second.clone()));
        options["includeAllPresets"] = json!(false);
        options["selectedPresets"] = json!([first.clone(), "unavailable", second.clone()]);
        assert_eq!(milkdrop_pool(options), [first.clone(), second.clone()]);
        assert_eq!(milkdrop_next(options, true, 0), Some(second.clone()));
        options["ignoredPresets"] = json!([second.clone()]);
        assert_eq!(milkdrop_next(options, false, 0), Some(first.clone()));
        options["randomizeNextPreset"] = json!(true);
        options["ignoredPresets"] = json!([]);
        assert_eq!(milkdrop_next(options, false, usize::MAX), Some(second));
        options["customPresets"] = json!({"Owned": "[preset00]\nfDecay=0.9\n"});
        options["selectedPresets"] = json!(["Owned"]);
        assert_eq!(milkdrop_next(options, false, 10), Some("Owned".into()));
        assert_eq!(
            milkdrop_data(options, "Owned"),
            Some("[preset00]\nfDecay=0.9\n")
        );
        validate(&config).unwrap();
        config.butterchurn["blendTime"] = json!(-1);
        assert!(validate(&config).is_err());
        let capture = Capture::default();
        capture.enable(true);
        capture.push(&[1., 2., 3., 4.], 2);
        assert_eq!(capture.take_frames(), [[1., 2.], [3., 4.]]);
        assert!(capture.take_frames().is_empty());
        let dir = std::env::temp_dir().join(format!("feishin-textures-{}", uuid::Uuid::new_v4()));
        let folder = install_milkdrop_textures(&dir).unwrap();
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 7);
        assert!(
            std::fs::read(folder.join("clouds2.jpg"))
                .unwrap()
                .starts_with(&[0xff, 0xd8])
        );
        let bytes = std::fs::read(folder.join("cells.jpg")).unwrap();
        assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff]);
        assert_eq!(install_milkdrop_textures(&dir).unwrap(), folder);
        assert_eq!(std::fs::read(folder.join("cells.jpg")).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn pcm_tap_preserves_samples_bounds_memory_and_fft_resolves_stereo_tones() {
        let frames = (0..48000)
            .map(|i| {
                [
                    (i as f32 * 1000. * std::f32::consts::TAU / 48000.).sin(),
                    (i as f32 * 4000. * std::f32::consts::TAU / 48000.).sin() * 0.5,
                ]
            })
            .collect::<Vec<_>>();
        let samples = frames.iter().flatten().copied().collect::<Vec<_>>();
        let capture = Capture::default();
        capture.enable(true);
        let source = rodio::buffer::SamplesBuffer::new(2, 48000, samples.clone());
        assert_eq!(
            Tap::new(source, capture.clone()).collect::<Vec<_>>(),
            samples
        );
        assert_eq!(capture.snapshot(40000).len(), 32768);
        let mut config = Config::default();
        config.audiomotionanalyzer["channelLayout"] = json!("dual-horizontal");
        config.audiomotionanalyzer["smoothing"] = json!(0);
        config.audiomotionanalyzer["linearAmplitude"] = json!(true);
        config.audiomotionanalyzer["linearBoost"] = json!(1);
        validate(&config).unwrap();
        let mut analyzer = Analyzer::default();
        analyzer.update(
            &capture.snapshot(16384),
            48000,
            &config.audiomotionanalyzer,
            0.016,
        );
        for (channel, frequency) in [(0, 1000.), (1, 4000.)] {
            let peak = analyzer
                .bands
                .iter()
                .max_by(|a, b| a.values[channel].total_cmp(&b.values[channel]))
                .unwrap();
            assert!((peak.frequency - frequency).abs() < frequency * 0.05);
        }
        config.audiomotionanalyzer["fftSize"] = json!(33);
        assert!(validate(&config).is_err());
        capture.enable(false);
        assert!(capture.snapshot(100).is_empty());
        capture.push(&[1., 1.], 2);
        assert!(capture.snapshot(100).is_empty());
    }
}
