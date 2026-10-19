#[cfg(any(target_os = "macos", target_os = "linux", test))]
mod pcm {
    include!("system_audio_pcm.rs");
}
use crate::visualizer::Capture;
use anyhow::{Context, Result};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    Started,
    Error(String),
}

pub struct SystemAudio {
    stop: Option<Sender<()>>,
    pub events: Receiver<Event>,
    pub capture: Capture,
}
impl SystemAudio {
    /// Starts only after the user selects the system-audio capture control.
    pub fn start(ffmpeg: String) -> Self {
        let (stop, stopped) = mpsc::channel();
        let (events_tx, events) = mpsc::sync_channel(8);
        let capture = Capture::default();
        capture.enable(true);
        let worker_capture = capture.clone();
        std::thread::spawn(move || {
            if let Err(error) = run(stopped, &events_tx, worker_capture, ffmpeg) {
                log::warn!("System-audio capture failed: {error:#}");
                let _ = events_tx.try_send(Event::Error(error.to_string()));
            }
        });
        Self {
            stop: Some(stop),
            events,
            capture,
        }
    }
}
impl Drop for SystemAudio {
    fn drop(&mut self) {
        self.capture.enable(false);
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

// Keep capture at the same 48 kHz stereo rate as the native playback tap.
struct Samples {
    capture: Capture,
    rate: f64,
    phase: f64,
    previous: Option<[f32; 2]>,
}
impl Samples {
    fn new(capture: Capture, rate: u32) -> Self {
        Self {
            capture,
            rate: rate as f64 / 48000.,
            phase: 1.,
            previous: None,
        }
    }
    fn push(&mut self, data: &[f32], channels: usize) {
        if channels == 0 || self.rate <= 0. {
            return;
        }
        let mut output = Vec::with_capacity(2048);
        for frame in data.chunks_exact(channels) {
            let sanitize = |n: f32| if n.is_finite() { n.clamp(-1., 1.) } else { 0. };
            let current = [
                sanitize(frame[0]),
                sanitize(*frame.get(1).unwrap_or(&frame[0])),
            ];
            if let Some(previous) = self.previous {
                while self.phase <= 1. {
                    let t = self.phase as f32;
                    output.extend([
                        previous[0] + (current[0] - previous[0]) * t,
                        previous[1] + (current[1] - previous[1]) * t,
                    ]);
                    self.phase += self.rate;
                    if output.len() >= 2048 {
                        self.capture.push(&output, 2);
                        output.clear();
                    }
                }
                self.phase -= 1.;
            }
            self.previous = Some(current);
        }
        self.capture.push(&output, 2);
    }
}

#[cfg(target_os = "macos")]
fn run(stop: Receiver<()>, events: &SyncSender<Event>, capture: Capture, _: String) -> Result<()> {
    use std::process::{Command, Stdio};
    let version = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()?;
    anyhow::ensure!(
        String::from_utf8_lossy(&version.stdout)
            .split('.')
            .next()
            .and_then(|n| n.parse::<u32>().ok())
            .is_some_and(|n| n >= 13),
        "System-audio capture requires macOS 13 or later"
    );
    let helper = std::env::current_exe()?
        .parent()
        .context("Missing application directory")?
        .join("feishin-audio-capture");
    anyhow::ensure!(
        helper.is_file(),
        "System-audio capture is unavailable in this build"
    );
    let child = Command::new(helper)
        .arg("--capture")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    capture_process(child, stop, events, capture)
}

#[cfg(target_os = "windows")]
fn run(stop: Receiver<()>, events: &SyncSender<Event>, capture: Capture, _: String) -> Result<()> {
    use rodio::cpal::{
        self, FromSample, Sample, SampleFormat, SizedSample,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    };
    fn input<T: Sample + SizedSample>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        capture: Capture,
        events: SyncSender<Event>,
    ) -> Result<cpal::Stream>
    where
        f32: FromSample<T>,
    {
        let mut samples = Samples::new(capture, config.sample_rate.0);
        let channels = config.channels as usize;
        Ok(device.build_input_stream(
            config,
            move |data: &[T], _| {
                let data = data
                    .iter()
                    .map(|n| n.to_sample::<f32>())
                    .collect::<Vec<_>>();
                samples.push(&data, channels);
            },
            move |error| {
                let _ = events.try_send(Event::Error(error.to_string()));
            },
            None,
        )?)
    }
    // CPAL's WASAPI output devices become loopback devices when used as inputs.
    let device = cpal::default_host()
        .default_output_device()
        .context("No default audio output device")?;
    let supported = device.default_output_config()?;
    let config = supported.config();
    let stream = match supported.sample_format() {
        SampleFormat::F32 => input::<f32>(&device, &config, capture, events.clone())?,
        SampleFormat::I16 => input::<i16>(&device, &config, capture, events.clone())?,
        SampleFormat::I32 => input::<i32>(&device, &config, capture, events.clone())?,
        SampleFormat::U16 => input::<u16>(&device, &config, capture, events.clone())?,
        other => anyhow::bail!("Unsupported system-audio sample format: {other}"),
    };
    stream.play()?;
    let _ = events.try_send(Event::Started);
    let _ = stop.recv();
    drop(stream);
    Ok(())
}

#[cfg(target_os = "linux")]
fn run(
    stop: Receiver<()>,
    events: &SyncSender<Event>,
    capture: Capture,
    ffmpeg: String,
) -> Result<()> {
    use anyhow::ensure;
    use std::process::{Command, Stdio};
    let sink = Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .context("System audio requires pactl and a PulseAudio or PipeWire audio server")?;
    ensure!(
        sink.status.success(),
        "Could not find the default system-audio output"
    );
    let sink = String::from_utf8(sink.stdout)?;
    let sink = sink.trim();
    ensure!(
        !sink.is_empty() && sink.len() <= 1024 && !sink.chars().any(char::is_control),
        "Invalid default audio output"
    );
    let child = Command::new(if ffmpeg.is_empty() { "ffmpeg" } else { &ffmpeg })
        .args([
            "-nostdin",
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "pulse",
            "-i",
            &format!("{sink}.monitor"),
            "-vn",
            "-ar",
            "48000",
            "-ac",
            "2",
            "-f",
            "f32le",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Could not start FFmpeg system-audio capture")?;
    capture_process(child, stop, events, capture)
}

#[cfg(any(target_os = "macos", target_os = "linux", test))]
fn capture_process(
    mut child: std::process::Child,
    stop: Receiver<()>,
    events: &SyncSender<Event>,
    capture: Capture,
) -> Result<()> {
    use std::{io::Read, time::Duration};
    let mut stdout = child.stdout.take().unwrap();
    let error_tx = events.clone();
    let stderr = child.stderr.take().map(|mut stderr| {
        std::thread::spawn(move || {
            let mut result = Vec::new();
            let mut bytes = [0_u8; 1024];
            while let Ok(count) = stderr.read(&mut bytes) {
                if count == 0 {
                    break;
                }
                let retain = count.min(8192_usize.saturating_sub(result.len()));
                result.extend_from_slice(&bytes[..retain]);
            }
            String::from_utf8_lossy(&result).trim().to_owned()
        })
    });
    let reader = std::thread::spawn(move || {
        let mut samples = Samples::new(capture, 48000);
        let mut bytes = [0_u8; 4096];
        let mut pending = Vec::new();
        let mut started = false;
        loop {
            match stdout.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    pending.extend_from_slice(&bytes[..count]);
                    let complete = pending.len() / 8 * 8;
                    let floats = pcm::stereo_f32(&[&pending[..complete]], false)
                        .expect("Bounded complete PCM frames");
                    samples.push(&floats, 2);
                    if complete > 0 && !started {
                        let _ = error_tx.try_send(Event::Started);
                        started = true;
                    }
                    pending.drain(..complete);
                }
                Err(error) => {
                    let _ = error_tx.try_send(Event::Error(error.to_string()));
                    break;
                }
            }
        }
    });
    let result = loop {
        if stop.recv_timeout(Duration::from_millis(100)) != Err(mpsc::RecvTimeoutError::Timeout) {
            break Ok(());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break Err(anyhow::anyhow!("System-audio capture exited: {status}"));
            }
            Ok(None) => (),
            Err(error) => break Err(error.into()),
        }
    };
    drop(child.stdin.take());
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    let detail = stderr
        .and_then(|thread| thread.join().ok())
        .unwrap_or_default();
    if result.is_err() && !detail.is_empty() {
        anyhow::bail!("{detail}");
    }
    result
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn run(_: Receiver<()>, _: &SyncSender<Event>, _: Capture, _: String) -> Result<()> {
    anyhow::bail!("System-audio capture is unavailable on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synthetic_pcm_process_reports_ready_and_is_reaped_when_capture_closes() {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        if Command::new("ffmpeg")
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            eprintln!("FFmpeg is unavailable; synthetic capture process check skipped");
            return;
        }
        let child = Command::new("ffmpeg")
            .args([
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-re",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:sample_rate=48000",
                "-t",
                "10",
                "-ac",
                "2",
                "-f",
                "f32le",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let capture = Capture::default();
        capture.enable(true);
        let (stop, stopped) = mpsc::channel();
        let (events_tx, events) = mpsc::sync_channel(8);
        let worker_capture = capture.clone();
        let worker =
            std::thread::spawn(move || capture_process(child, stopped, &events_tx, worker_capture));
        let audio = SystemAudio {
            stop: Some(stop),
            events,
            capture: capture.clone(),
        };
        assert_eq!(
            audio.events.recv_timeout(Duration::from_secs(3)).unwrap(),
            Event::Started
        );
        let frames = capture.snapshot(4096);
        assert!(!frames.is_empty() && frames.iter().any(|n| n[0].abs() > 0.01));
        assert!(frames.iter().all(|n| (n[0] - n[1]).abs() < 0.001));
        let closed = Instant::now();
        drop(audio);
        worker.join().unwrap().unwrap();
        assert!(closed.elapsed() < Duration::from_secs(2));
        assert!(capture.snapshot(10).is_empty());
    }
    #[test]
    fn streaming_resample_preserves_stereo_rate_and_bounds() {
        let capture = Capture::default();
        capture.enable(true);
        let mut samples = Samples::new(capture.clone(), 44100);
        let frames = [0.25, -0.75].repeat(4411);
        for chunk in frames.chunks(86) {
            samples.push(chunk, 2);
        }
        let converted = capture.take_frames();
        assert!((4799..=4801).contains(&converted.len()));
        assert!(converted.iter().all(|frame| *frame == [0.25, -0.75]));
        samples.push(&[f32::NAN, f32::INFINITY, 5., -5.], 2);
        assert!(
            capture
                .snapshot(10)
                .iter()
                .flatten()
                .all(|n| n.is_finite() && n.abs() <= 1.)
        );
        capture.enable(false);
        samples.push(&frames, 2);
        assert!(capture.snapshot(10).is_empty());
    }
}
