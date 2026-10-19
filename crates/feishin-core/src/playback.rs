use crate::storage::Settings;
use anyhow::{Context, Result, bail};
use interprocess::TryClone;
#[cfg(not(windows))]
use interprocess::local_socket::GenericFilePath;
#[cfg(windows)]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::{Stream, prelude::*};
use rodio::{Decoder, DeviceTrait, OutputStream, OutputStreamBuilder, Sink};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::{Duration, Instant},
};

pub enum Control {
    Load {
        url: String,
        settings: Box<Settings>,
        position: f64,
        gain: f32,
        item: Box<crate::model::Item>,
        key: String,
    },
    Configure {
        settings: Box<Settings>,
        gain: f32,
    },
    Pause(bool),
    Seek(f64),
    Volume(f32),
    Speed(f32),
    Stop,
    ResetMpv(Box<Settings>),
    Cast(Option<Box<crate::dlna::Device>>),
    AdoptCast {
        status: Box<crate::dlna::Status>,
        item: Box<crate::model::Item>,
        url: String,
        key: String,
    },
    DisconnectCast {
        passive: bool,
        position: Sender<Option<f64>>,
    },
    Jukebox(Option<Box<crate::api::Api>>),
    NextUri {
        url: String,
        item: Box<crate::model::Item>,
        key: String,
        gain: f32,
    },
}

#[derive(Debug)]
pub enum Event {
    Loading(u64),
    Started(u64),
    Paused(u64, bool),
    Position(u64, f64),
    Ended(u64),
    Stopped(u64),
    Error(u64, String),
    DeviceVolume(u64, u8),
    SpeakerTopology(u64, Vec<crate::dlna::Group>),
    RadioMetadata(u64, RadioMetadata),
    JukeboxSynced(u64, crate::jukebox::Snapshot),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RadioMetadata {
    pub artist: String,
    pub title: String,
}
impl RadioMetadata {
    pub fn from_value(value: &Value) -> Self {
        let field = |names: &[&str]| {
            names
                .iter()
                .find_map(|name| {
                    value.as_object()?.iter().find_map(|(key, value)| {
                        (key.eq_ignore_ascii_case(name))
                            .then(|| value.as_str())
                            .flatten()
                            .filter(|value| !value.trim().is_empty())
                    })
                })
                .unwrap_or_default()
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(1024)
                .collect::<String>()
        };
        let mut artist = field(&["artist", "icy-artist"]);
        let mut title = field(&["title"]);
        let combined = field(&["icy-title", "StreamTitle", "stream-title"]);
        let split = combined.split_once(['-', '\u{2013}', '\u{2014}']);
        if title.is_empty() && artist.is_empty() {
            if let Some((left, right)) = split {
                artist = left.trim().into();
                title = right.trim().into();
            } else {
                title = combined;
            }
        } else if title.is_empty() {
            title = combined;
        } else if artist.is_empty()
            && let Some((left, right)) = split
            && right.trim() == title
        {
            artist = left.trim().into();
        }
        Self { artist, title }
    }
}

type NativeStorage = stream_download::storage::adaptive::AdaptiveStorageProvider<
    stream_download::storage::memory::MemoryStorageProvider,
    stream_download::storage::temp::TempStorageProvider,
>;
type NativeDecoder =
    Decoder<icy_metadata::IcyMetadataReader<stream_download::StreamDownload<NativeStorage>>>;

async fn prepare_native(
    url: &str,
    metadata: Option<(u64, Sender<Message>)>,
    ignore_ssl: bool,
) -> Result<NativeDecoder> {
    use stream_download::storage::{
        adaptive::AdaptiveStorageProvider, memory::MemoryStorageProvider, temp::TempStorageProvider,
    };
    use stream_download::{Settings, StreamDownload};
    let storage = AdaptiveStorageProvider::with_fixed_and_variable(
        MemoryStorageProvider,
        TempStorageProvider::default(),
        std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
    );
    let client = stream_download::http::reqwest::Client::builder()
        .danger_accept_invalid_certs(ignore_ssl)
        .default_headers(if metadata.is_some() {
            stream_download::http::reqwest::header::HeaderMap::from_iter([(
                stream_download::http::reqwest::header::HeaderName::from_static("icy-metadata"),
                stream_download::http::reqwest::header::HeaderValue::from_static("1"),
            )])
        } else {
            Default::default()
        })
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(30))
        .build()?;
    let stream = stream_download::http::HttpStream::new(client, url.parse()?)
        .await
        .map_err(|_| anyhow::anyhow!("Could not open the audio stream"))?;
    let interval = stream
        .headers()
        .get("icy-metaint")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<std::num::NonZeroUsize>().ok());
    let source =
        StreamDownload::from_stream(stream, storage, Settings::default().prefetch_bytes(16384))
            .await
            .map_err(|_| anyhow::anyhow!("Could not open the audio stream"))?;
    let length = if interval.is_none() && metadata.is_none() {
        source.content_length()
    } else {
        None
    };
    let source = icy_metadata::IcyMetadataReader::new(source, interval, move |value| {
        if let (Some((id, tx)), Ok(value)) = (&metadata, value) {
            let mut fields = serde_json::to_value(value.custom_fields()).unwrap_or_default();
            if let Some(title) = value.stream_title() {
                fields["StreamTitle"] = title.into();
            }
            let _ = tx.send(Message::RadioMetadata(
                *id,
                RadioMetadata::from_value(&fields),
            ));
        }
    });
    tokio::task::spawn_blocking(move || {
        let builder = Decoder::builder()
            .with_data(source)
            .with_seekable(length.is_some());
        let builder = if let Some(length) = length {
            builder.with_byte_len(length)
        } else {
            builder
        };
        builder
            .build()
            .context("Unsupported or invalid audio stream")
    })
    .await?
}

enum Message {
    Control(u64, Control),
    Prepared(u64, u64, Result<NativeDecoder>),
    Mpv(uuid::Uuid, Value),
    CastStatus(u64, Result<crate::dlna::Status>, bool),
    CastNotification(u64, crate::dlna::events::Delta),
    PreparedNext(u64, u64, Result<crate::transition::Track>),
    NativeAdvanced(u64, String),
    RadioMetadata(u64, RadioMetadata),
    PreparedCast(
        u64,
        Option<u64>,
        Result<crate::proxy::AudioFile>,
        Box<crate::model::Item>,
        String,
    ),
    Shutdown,
}

// Jellyfin's chunked transcodes cannot be byte-seeked. Request a new server stream
// and retain the absolute song clock separately from the returned audio's clock.
fn transcode_at(url: &str, position: f64) -> Option<String> {
    if !position.is_finite() || position < 0. || position > u64::MAX as f64 / 10_000_000. {
        return None;
    }
    let mut url = url::Url::parse(url).ok()?;
    let path = url.path().to_ascii_lowercase();
    if !path.contains("/audio/") || !path.rsplit('/').next()?.starts_with("stream.") {
        return None;
    }
    let mut pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if !pairs.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("static") && value.eq_ignore_ascii_case("false")
    }) {
        return None;
    }
    pairs.retain(|(key, _)| {
        !key.eq_ignore_ascii_case("startTimeTicks") && !key.eq_ignore_ascii_case("playSessionId")
    });
    pairs.push((
        "StartTimeTicks".into(),
        ((position * 10_000_000.).round() as u64).to_string(),
    ));
    pairs.push((
        "PlaySessionId".into(),
        format!("feishin-{}", uuid::Uuid::new_v4()),
    ));
    url.query_pairs_mut().clear().extend_pairs(pairs);
    Some(url.into())
}

pub struct Player {
    tx: Sender<Message>,
    pub events: Receiver<Event>,
    generation: Arc<AtomicU64>,
    pub capture: crate::visualizer::Capture,
}

pub fn audio_devices() -> Result<Vec<String>> {
    use rodio::cpal::traits::HostTrait;
    let mut names = rodio::cpal::default_host()
        .output_devices()?
        .filter_map(|device| device.name().ok())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn output_devices(settings: &Settings) -> Result<Vec<(String, String)>> {
    if settings.mpv_path.is_empty() {
        audio_devices().map(|names| names.into_iter().map(|name| (name.clone(), name)).collect())
    } else {
        mpv_audio_devices(settings)
    }
}

/// Query an idle MPV instance without loading media or disturbing the playing instance.
pub fn mpv_audio_devices(settings: &Settings) -> Result<Vec<(String, String)>> {
    let (tx, rx) = mpsc::channel();
    let mut mpv = Mpv::start(settings, tx)?;
    let request = mpv.command(json!(["get_property", "audio-device-list"]))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let message = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("MPV audio device query timed out")?;
        if let Message::Mpv(_, value) = message
            && value["request_id"].as_u64() == Some(request)
        {
            anyhow::ensure!(
                value["error"] == "success",
                "MPV audio device query failed: {}",
                value["error"]
            );
            let devices = value["data"]
                .as_array()
                .context("Invalid MPV audio device list")?;
            let mut seen = std::collections::HashSet::new();
            return Ok(devices
                .iter()
                .filter_map(|device| {
                    let id = device["name"].as_str()?.to_owned();
                    if id.is_empty() || !seen.insert(id.clone()) {
                        return None;
                    }
                    let name = device["description"]
                        .as_str()
                        .filter(|name| !name.is_empty())
                        .unwrap_or(&id)
                        .to_owned();
                    Some((id, name))
                })
                .collect());
        }
    }
}
pub fn waveform(
    url: &str,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    ignore_ssl: bool,
) -> Result<crate::waveform::Data> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let source = prepare_native(url, None, ignore_ssl).await?;
        tokio::task::spawn_blocking(move || crate::waveform::analyze(source, &cancelled)).await?
    })
}

impl Default for Player {
    fn default() -> Self {
        Self::new()
    }
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let (event_tx, events) = mpsc::channel();
        let generation = Arc::new(AtomicU64::new(0));
        let worker_tx = tx.clone();
        let worker_generation = generation.clone();
        let capture = crate::visualizer::Capture::default();
        let worker_capture = capture.clone();
        thread::spawn(move || run(rx, worker_tx, event_tx, worker_generation, worker_capture));
        Self {
            tx,
            events,
            generation,
            capture,
        }
    }

    pub fn send(&self, command: Control) -> u64 {
        let generation = if matches!(
            command,
            Control::Load { .. }
                | Control::Stop
                | Control::Cast(_)
                | Control::DisconnectCast { .. }
                | Control::Jukebox(_)
        ) {
            self.generation.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            self.generation.load(Ordering::SeqCst)
        };
        let _ = self.tx.send(Message::Control(generation, command));
        generation
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Message::Shutdown);
    }
}

struct Mpv {
    child: Child,
    stream: Stream,
    #[cfg(not(windows))]
    socket: String,
    next_key: String,
    advanced: bool,
    instance: uuid::Uuid,
    request_id: u64,
    pending: std::collections::HashMap<u64, MpvReply>,
    generation: u64,
    loaded_url: String,
    arguments: Vec<String>,
    properties: std::collections::BTreeMap<String, Value>,
    needs_restart: bool,
}

enum MpvReply {
    Command { generation: u64, operation: String },
    Loaded { generation: u64, url: String },
}

impl Mpv {
    fn configure(&mut self, settings: &Settings) -> Result<()> {
        self.command(json!(["set_property", "tls-verify", !settings.ignore_ssl]))?;
        self.needs_restart |= self
            .properties
            .keys()
            .any(|key| !settings.mpv_properties.contains_key(key));
        self.properties.clone_from(&settings.mpv_properties);
        self.command(json!([
            "set_property",
            "audio-device",
            settings.mpv_audio_device
        ]))?;
        self.command(json!([
            "set_property",
            "volume",
            if settings.muted { 0.0 } else { settings.volume }
        ]))?;
        self.command(json!(["set_property", "speed", settings.speed]))?;
        self.command(json!([
            "set_property",
            "audio-pitch-correction",
            settings.preserve_pitch
        ]))?;
        self.command(json!(["set_property", "replaygain", settings.replay_gain]))?;
        let filters = crate::proxy::dsp_filters(settings, 1.0);
        self.command(json!([
            "set_property",
            "af",
            if filters.is_empty() {
                String::new()
            } else {
                format!("lavfi=[{filters}]")
            }
        ]))?;
        for (property, value) in &settings.mpv_properties {
            self.command(json!(["set_property", property, value]))?;
        }
        Ok(())
    }
    fn start(settings: &Settings, tx: Sender<Message>) -> Result<Self> {
        let instance = uuid::Uuid::new_v4();
        let name = format!("f{}", uuid::Uuid::new_v4().simple());
        #[cfg(windows)]
        let socket = format!(r"\\.\pipe\{name}");
        #[cfg(not(windows))]
        let socket = std::env::temp_dir()
            .join(format!("{name}.sock"))
            .to_string_lossy()
            .into_owned();
        let mut command = Command::new(&settings.mpv_path);
        command
            .args([
                "--idle=yes",
                "--no-video",
                "--no-terminal",
                "--keep-open=no",
                "--audio-display=no",
            ])
            .arg(format!("--input-ipc-server={socket}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command.args(&settings.mpv_extra_args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .context("Cannot launch MPV. Check the binary path in Settings.")?;
        let started = Instant::now();
        let stream = loop {
            #[cfg(windows)]
            let connection = Stream::connect(name.as_str().to_ns_name::<GenericNamespaced>()?);
            #[cfg(not(windows))]
            let connection = Stream::connect(socket.as_str().to_fs_name::<GenericFilePath>()?);
            if let Ok(stream) = connection {
                break stream;
            }
            if child.try_wait()?.is_some() {
                bail!("MPV exited before opening its IPC connection");
            }
            if started.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                bail!("MPV IPC connection timed out");
            }
            thread::sleep(Duration::from_millis(20));
        };
        let reader = stream.try_clone()?;
        thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line)
                    && tx.send(Message::Mpv(instance, value)).is_err()
                {
                    break;
                }
            }
        });
        let mut engine = Self {
            child,
            stream,
            #[cfg(not(windows))]
            socket,
            next_key: String::new(),
            advanced: false,
            instance,
            request_id: 0,
            pending: Default::default(),
            generation: 0,
            loaded_url: String::new(),
            arguments: settings.mpv_extra_args.clone(),
            properties: Default::default(),
            needs_restart: false,
        };
        engine.command(json!(["observe_property", 1, "time-pos"]))?;
        engine.command(json!(["observe_property", 2, "pause"]))?;
        engine.command(json!(["observe_property", 3, "metadata"]))?;
        Ok(engine)
    }

    fn command(&mut self, command: Value) -> Result<u64> {
        self.request_id += 1;
        let operation = match command[0].as_str().unwrap_or_default() {
            "set_property" | "get_property" | "observe_property" => format!(
                "{} {}",
                command[0].as_str().unwrap(),
                command[1].as_str().unwrap_or_default()
            ),
            other => other.to_owned(),
        };
        self.pending.insert(
            self.request_id,
            MpvReply::Command {
                generation: self.generation,
                operation,
            },
        );
        serde_json::to_writer(
            &mut self.stream,
            &json!({"command":command,"request_id":self.request_id}),
        )?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()?;
        Ok(self.request_id)
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self.command(json!(["quit"]));
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(not(windows))]
        let _ = std::fs::remove_file(&self.socket);
    }
}

struct Native {
    _stream: OutputStream,
    sink: Sink,
    transition: Option<Arc<crate::transition::Handle>>,
    advanced: bool,
    device: String,
    source_id: u64,
}

impl Native {
    fn new(device_name: &str) -> Result<Self> {
        use rodio::cpal::traits::HostTrait;
        let stream = if device_name.is_empty() {
            OutputStreamBuilder::open_default_stream()
                .context("Cannot open the default audio output device")?
        } else {
            let device = rodio::cpal::default_host()
                .output_devices()?
                .find(|device| device.name().ok().as_deref() == Some(device_name))
                .context("The selected audio device is unavailable")?;
            OutputStreamBuilder::from_device(device)?.open_stream()?
        };
        let sink = Sink::connect_new(stream.mixer());
        Ok(Self {
            _stream: stream,
            sink,
            transition: None,
            advanced: false,
            device: device_name.into(),
            source_id: 0,
        })
    }
}

struct Cast {
    controller: crate::dlna::Controller,
    subscription: Option<crate::dlna::events::Subscription>,
    device: crate::dlna::Device,
    uri: String,
    original_url: String,
    item: Box<crate::model::Item>,
    speed: f64,
    loading: bool,
    next_key: String,
    next: Option<(String, Box<crate::model::Item>, String)>,
    queued_uri: String,
    advanced: bool,
    position: f64,
    duration: f64,
    loaded_at: Instant,
    volume: u8,
    volume_at: Instant,
    pending_volume: Option<u8>,
    volume_polls: usize,
}

impl Cast {
    fn notification(&self, delta: crate::dlna::events::Delta) -> crate::dlna::Status {
        crate::dlna::Status {
            position: delta.position.unwrap_or(self.position / self.speed),
            duration: delta.duration.unwrap_or(self.duration / self.speed),
            uri: delta.uri.unwrap_or_else(|| self.uri.clone()),
            next_uri: delta.next_uri.unwrap_or_else(|| self.queued_uri.clone()),
            transport: delta.transport.unwrap_or_default(),
            volume: delta.volume,
        }
    }
    fn load(
        &mut self,
        url: String,
        item: &crate::model::Item,
        position: f64,
        paused: bool,
    ) -> Result<()> {
        if !(self.advanced && self.uri == url) {
            let mut metadata = item.clone();
            metadata.duration /= self.speed;
            self.controller.load(&self.device, &url, &metadata, false)?;
            self.controller.play(&self.device)?;
            if position > 0.0 {
                self.controller.seek(&self.device, position / self.speed)?;
            }
            if paused {
                self.controller.pause(&self.device)?;
            }
        }
        self.advanced = false;
        self.loading = false;
        self.uri = url;
        self.queued_uri.clear();
        self.loaded_at = Instant::now();
        self.position = position;
        self.duration = item.duration;
        Ok(())
    }
}
fn needs_proxy(settings: &Settings, gain: f32) -> bool {
    (settings.speed - 1.0).abs() > 0.001 || !crate::proxy::dsp_filters(settings, gain).is_empty()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FadeEnd {
    Playing,
    Paused,
    Stopped,
}
struct StatusFade {
    started: Instant,
    from: f32,
    to: f32,
    end: FadeEnd,
    generation: u64,
}
impl StatusFade {
    fn new(from: f32, end: FadeEnd, generation: u64, now: Instant) -> Self {
        Self {
            started: now,
            from,
            to: if end == FadeEnd::Playing { 1.0 } else { 0.0 },
            end,
            generation,
        }
    }
    fn sample(&self, now: Instant) -> (f32, bool) {
        let progress =
            (now.saturating_duration_since(self.started).as_secs_f32() / 0.3).clamp(0.0, 1.0);
        (
            self.from + (self.to - self.from) * progress,
            progress >= 1.0,
        )
    }
}
fn local_volume(
    native: &Option<Native>,
    mpv: &mut Option<Mpv>,
    settings: &Settings,
    gain: f32,
) -> Result<()> {
    let volume = if settings.muted {
        0.0
    } else {
        settings.volume * gain
    };
    if let Some(native) = native {
        native.sink.set_volume(volume / 100.0);
    }
    if let Some(mpv) = mpv {
        mpv.command(json!(["set_property", "volume", volume]))?;
    }
    Ok(())
}
fn local_status(native: &Option<Native>, mpv: &mut Option<Mpv>, end: FadeEnd) -> Result<()> {
    if let Some(native) = native {
        match end {
            FadeEnd::Playing => native.sink.play(),
            FadeEnd::Paused => native.sink.pause(),
            FadeEnd::Stopped => native.sink.clear(),
        }
    }
    if let Some(mpv) = mpv {
        mpv.command(if end == FadeEnd::Stopped {
            json!(["stop"])
        } else {
            json!(["set_property", "pause", end == FadeEnd::Paused])
        })?;
    }
    Ok(())
}

fn run(
    rx: Receiver<Message>,
    tx: Sender<Message>,
    events: Sender<Event>,
    generation: Arc<AtomicU64>,
    capture: crate::visualizer::Capture,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = events.send(Event::Error(0, error.to_string()));
            return;
        }
    };
    let mut pending: Option<tokio::task::JoinHandle<()>> = None;
    let mut paused = false;
    let mut mpv: Option<Mpv> = None;
    let mut native: Option<Native> = None;
    let mut active = 0;
    let mut settings = Settings::default();
    let dsp = Arc::new(std::sync::RwLock::new(crate::dsp::Config::default()));
    let tempo = Arc::new(std::sync::RwLock::new(crate::tempo::Options::from(
        &settings,
    )));
    let mut current_duration = 0.0;
    let mut current_media: Option<(String, Box<crate::model::Item>)> = None;
    let mut stream_offset = 0.0;
    let mut load_revision = 0;
    let mut radio = false;
    let mut resume = 0.0;
    let mut playing = false;
    let mut tick = Instant::now();
    let mut cast: Option<Cast> = None;
    let mut jukebox: Option<crate::jukebox::Engine> = None;
    let mut jukebox_poll = Instant::now();
    let mut cast_poll = Instant::now();
    let mut cast_polling = false;
    let mut current_key = String::new();
    let mut current_gain = 1.0;
    let mut next_key = String::new();
    let mut next_revision = 0;
    let mut pending_next: Option<tokio::task::JoinHandle<()>> = None;
    let mut prepared_next: Option<crate::transition::Track> = None;
    let mut proxy: Option<crate::proxy::Proxy> = None;
    let mut status_fade: Option<StatusFade> = None;
    let mut status_gain = 1.0;
    loop {
        let message = match rx.recv_timeout(Duration::from_millis(if status_fade.is_some() {
            10
        } else {
            100
        })) {
            Ok(message) => Some(message),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(_) => break,
        };
        // Merge partial notifications when consumed, after any earlier event has updated state.
        let message = match message {
            Some(Message::CastNotification(id, delta)) => cast
                .as_ref()
                .map(|cast| Message::CastStatus(id, Ok(cast.notification(delta)), true)),
            other => other,
        };
        let result: Result<()> = (|| {
            match message {
                Some(Message::Shutdown) => {
                    if let Some(cast) = &cast {
                        let _ = cast.controller.stop(&cast.device);
                    }
                    return Err(anyhow::anyhow!("shutdown"));
                }
                Some(Message::Control(id, Control::ResetMpv(new_settings))) => {
                    if generation.load(Ordering::SeqCst) != id {
                        return Ok(());
                    }
                    // Complete any pending local stop fade before replacing the engine.
                    local_status(&native, &mut mpv, FadeEnd::Stopped)?;
                    native = None;
                    mpv = None;
                    current_media = None;
                    status_fade = None;
                    playing = false;
                    active = id;
                    settings = *new_settings;
                    let mut engine = Mpv::start(&settings, tx.clone())?;
                    engine.generation = id;
                    engine.configure(&settings)?;
                    mpv = Some(engine);
                    let _ = events.send(Event::Stopped(active));
                }
                Some(Message::Control(id, Control::Jukebox(api))) => {
                    // Output selections precede a new Load in the same command queue.
                    if let Some(task) = pending.take() {
                        task.abort();
                    }
                    if let Some(task) = pending_next.take() {
                        task.abort();
                    }
                    load_revision += 1;
                    next_revision += 1;
                    prepared_next = None;
                    current_media = None;
                    next_key.clear();
                    local_status(&native, &mut mpv, FadeEnd::Stopped)?;
                    native = None;
                    mpv = None;
                    status_fade = None;
                    if let Some(cast) = cast.take() {
                        cast.controller.stop(&cast.device)?;
                    }
                    if let Some(engine) = jukebox.take() {
                        engine.api.jukebox(crate::jukebox::Command::Stop)?;
                    }
                    active = id;
                    playing = false;
                    if let Some(api) = api {
                        log::debug!("Connecting to server Jukebox");
                        let engine = crate::jukebox::Engine::new(*api)?;
                        playing = engine.state.playing;
                        paused = !playing;
                        let _ = events.send(Event::JukeboxSynced(id, engine.state.clone()));
                        jukebox = Some(engine);
                        jukebox_poll = Instant::now();
                    }
                }
                Some(Message::Control(id, control))
                    if jukebox.is_some() && !matches!(control, Control::Cast(_)) =>
                {
                    if generation.load(Ordering::SeqCst) != id {
                        return Ok(());
                    }
                    let engine = jukebox.as_mut().unwrap();
                    match control {
                        Control::Load {
                            settings: new_settings,
                            position,
                            item,
                            key,
                            ..
                        } => {
                            active = id;
                            let _ = events.send(Event::Loading(id));
                            settings = *new_settings;
                            engine.load(&item, &key, position, &settings)?;
                            playing = true;
                            paused = false;
                            let _ = events.send(Event::Started(id));
                            let _ = events.send(Event::Position(id, engine.state.position));
                        }
                        Control::NextUri { item, key, .. } => engine.next(&item, key)?,
                        Control::Pause(pause) => {
                            engine.pause(pause)?;
                            paused = pause;
                            playing = !pause;
                            let _ = events.send(Event::Paused(active, pause));
                        }
                        Control::Seek(position) => {
                            engine.seek(position)?;
                            let _ = events.send(Event::Position(active, engine.state.position));
                        }
                        Control::Configure {
                            settings: new_settings,
                            ..
                        } => {
                            settings = *new_settings;
                            engine.volume(&settings)?;
                        }
                        Control::Volume(volume) => {
                            settings.volume = volume.clamp(0., 100.);
                            settings.muted = false;
                            engine.volume(&settings)?;
                        }
                        Control::Stop => {
                            active = id;
                            playing = false;
                            engine.stop()?;
                            let _ = events.send(Event::Stopped(id));
                        }
                        Control::Speed(_) => {}
                        _ => unreachable!(),
                    }
                }
                Some(Message::Control(
                    id,
                    Control::Load {
                        url,
                        settings: new_settings,
                        position,
                        gain,
                        item,
                        key,
                    },
                )) => {
                    if generation.load(Ordering::SeqCst) != id {
                        return Ok(());
                    }
                    active = id;
                    if new_settings.jukebox {
                        bail!("Jukebox is disconnected. Reconnect in Audio outputs.");
                    }
                    load_revision += 1;
                    current_media = Some((url.clone(), item.clone()));
                    stream_offset = 0.;
                    status_fade = None;
                    status_gain = 1.0;
                    if let Some(proxy) = &proxy {
                        proxy.clear_live();
                    }
                    current_key.clone_from(&key);
                    current_gain = gain;
                    current_duration = item.duration;
                    radio = item.kind == crate::model::ItemKind::Radio;
                    if let Ok(mut tempo) = tempo.write() {
                        *tempo = crate::tempo::Options::from(new_settings.as_ref());
                    }
                    paused = false;
                    if let Some(task) = pending.take() {
                        task.abort();
                    }
                    playing = false;
                    resume = position.max(0.0);
                    let _ = events.send(Event::Loading(id));
                    if let Some(native) = &mut native
                        && resume == 0.0
                        && native.advanced
                        && native
                            .transition
                            .as_ref()
                            .is_some_and(|source| source.key() == key)
                        && native.device == new_settings.audio_device
                        && new_settings.mpv_path.is_empty()
                    {
                        native.advanced = false;
                        settings = *new_settings;
                        if let Some(source) = &native.transition {
                            source.configure((&settings).into(), gain);
                        }
                        playing = true;
                        next_key.clear();
                        let _ = events.send(Event::Started(id));
                        return Ok(());
                    }
                    if let Some(mpv) = &mut mpv
                        && resume == 0.0
                        && mpv.advanced
                        && mpv.next_key == key
                        && settings.mpv_path == new_settings.mpv_path
                        && !mpv.needs_restart
                        && mpv.arguments == new_settings.mpv_extra_args
                    {
                        mpv.generation = id;
                        settings = *new_settings;
                        mpv.advanced = false;
                        mpv.next_key.clear();
                        mpv.configure(&settings)?;
                        playing = true;
                        next_key.clear();
                        resume = 0.0;
                        let _ = events.send(Event::Started(id));
                        return Ok(());
                    }
                    if let Some(task) = pending_next.take() {
                        task.abort();
                    }
                    next_revision += 1;
                    next_key.clear();
                    prepared_next = None;
                    if let Some(native) = &mut native {
                        native.sink.clear();
                        native.transition = None;
                        native.advanced = false;
                    }
                    if let Some(mpv) = &mut mpv {
                        mpv.command(json!(["stop"]))?;
                    }
                    let restart = settings.mpv_path != new_settings.mpv_path;
                    settings = *new_settings;
                    // Only an explicit status change fades; automatic gapless transitions stay continuous.
                    if settings.fade_on_status && cast.is_none() {
                        status_gain = 0.0;
                    }
                    if let Ok(mut dsp) = dsp.write() {
                        *dsp = crate::dsp::Config::from_settings(&settings);
                    }
                    let url = if cast.is_none() && !radio {
                        if let Some(url) = transcode_at(&url, resume) {
                            stream_offset = resume;
                            resume = 0.;
                            url
                        } else {
                            url
                        }
                    } else {
                        url
                    };
                    if let Some(cast) = &mut cast {
                        cast.original_url = url.clone();
                        cast.item = item.clone();
                        cast.speed = settings.speed as f64;
                        cast.next = None;
                        mpv = None;
                        native = None;
                        cast.controller
                            .volume(&cast.device, settings.volume as u8)?;
                        cast.controller.mute(&cast.device, settings.muted)?;
                        cast.volume = settings.volume as u8;
                        cast.volume_at = Instant::now();
                        if cast.advanced && cast.next_key == key {
                            cast.load(cast.uri.clone(), &item, resume, paused)?;
                            playing = true;
                            let _ = events.send(Event::Started(id));
                        } else if needs_proxy(&settings, gain) {
                            cast.advanced = false;
                            cast.loading = true;
                            if !cast.uri.is_empty() {
                                cast.controller.stop(&cast.device)?;
                            }
                            if proxy.is_none() {
                                proxy = Some(crate::proxy::Proxy::new(&runtime)?);
                            }
                            if radio {
                                let url = proxy.as_ref().unwrap().insert_live(
                                    url,
                                    settings.clone(),
                                    gain,
                                )?;
                                let url = cast.controller.lan_url(&cast.device, &url)?;
                                cast.loading = false;
                                cast.load(url, &item, 0.0, paused)?;
                                playing = true;
                                let _ = events.send(Event::Started(id));
                                return Ok(());
                            }
                            let proxy_settings = settings.clone();
                            let tx = tx.clone();
                            pending = Some(runtime.spawn(async move {
                                let result =
                                    crate::proxy::prepare(&url, &proxy_settings, gain).await;
                                let _ = tx.send(Message::PreparedCast(id, None, result, item, key));
                            }));
                        } else {
                            let url = cast.controller.lan_url(&cast.device, &url)?;
                            cast.load(url, &item, resume, paused)?;
                            playing = true;
                            let _ = events.send(Event::Started(id));
                        }
                    } else if !settings.mpv_path.is_empty() {
                        native = None;
                        if restart
                            || mpv.as_ref().is_some_and(|mpv| {
                                mpv.needs_restart || mpv.arguments != settings.mpv_extra_args
                            })
                        {
                            mpv = None;
                        }
                        if mpv.is_none() {
                            mpv = Some(Mpv::start(&settings, tx.clone())?);
                        }
                        let mpv = mpv.as_mut().unwrap();
                        mpv.generation = id;
                        mpv.loaded_url.clone_from(&url);
                        mpv.configure(&settings)?;
                        mpv.command(json!([
                            "set_property",
                            "volume",
                            if settings.muted {
                                0.0
                            } else {
                                settings.volume * status_gain
                            }
                        ]))?;
                        mpv.command(json!(["loadfile", url, "replace"]))?;
                        mpv.command(json!(["set_property", "pause", false]))?;
                    } else {
                        mpv = None;
                        if native
                            .as_ref()
                            .is_some_and(|native| native.device != settings.audio_device)
                        {
                            native = None;
                        }
                        let tx = tx.clone();
                        let generation = generation.clone();
                        let revision = load_revision;
                        let ignore_ssl = settings.ignore_ssl;
                        pending = Some(runtime.spawn(async move {
                            let result = async {
                                let decoder = prepare_native(
                                    &url,
                                    radio.then(|| (id, tx.clone())),
                                    ignore_ssl,
                                )
                                .await?;
                                if generation.load(Ordering::SeqCst) != id {
                                    bail!("cancelled");
                                }
                                Ok(decoder)
                            }
                            .await;
                            let _ = tx.send(Message::Prepared(id, revision, result));
                        }));
                    }
                }
                Some(Message::Prepared(id, revision, decoder))
                    if id == active
                        && revision == load_revision
                        && generation.load(Ordering::SeqCst) == id =>
                {
                    let decoder = decoder?;
                    if native.is_none() {
                        native = Some(Native::new(&settings.audio_device)?);
                    }
                    let native = native.as_mut().unwrap();
                    let sink = &native.sink;
                    sink.clear();
                    sink.set_volume(if settings.muted {
                        0.0
                    } else {
                        settings.volume * status_gain / 100.0
                    });
                    sink.set_speed(1.0);
                    let track = crate::transition::Track::new(
                        crate::dsp::Equalized::new(decoder, dsp.clone()),
                        current_key.clone(),
                        current_gain,
                    )
                    .with_duration((current_duration - stream_offset).max(0.));
                    let audio_tx = tx.clone();
                    let (source, handle) =
                        crate::transition::Continuous::new(track, (&settings).into(), move |key| {
                            let _ = audio_tx.send(Message::NativeAdvanced(id, key.into()));
                        });
                    native.transition = Some(handle);
                    if let Some(track) = prepared_next.take()
                        && let Some(source) = &native.transition
                    {
                        source.prepare(Some(track));
                    }
                    native.advanced = false;
                    native.source_id = id;
                    capture.clear();
                    sink.append(crate::visualizer::Tap::new(
                        crate::tempo::Tempo::new(source, tempo.clone()),
                        capture.clone(),
                    ));
                    if resume > 0.0 {
                        sink.try_seek(Duration::from_secs_f64(resume))
                            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    }
                    if paused {
                        sink.pause();
                    } else {
                        sink.play();
                    }
                    playing = true;
                    if settings.fade_on_status && !paused {
                        status_fade = Some(StatusFade::new(
                            status_gain,
                            FadeEnd::Playing,
                            active,
                            Instant::now(),
                        ));
                    }
                    let _ = events.send(Event::Started(id));
                    let _ = events.send(Event::Paused(id, paused));
                }
                Some(Message::Control(id, control)) => {
                    if !matches!(control, Control::Cast(_))
                        && generation.load(Ordering::SeqCst) != id
                    {
                        return Ok(());
                    }
                    let was_playing = playing;
                    if matches!(control, Control::Stop) {
                        if let Some(task) = pending.take() {
                            task.abort();
                        }
                        active = id;
                        playing = false;
                        if let Some(task) = pending_next.take() {
                            task.abort();
                        }
                        next_revision += 1;
                        next_key.clear();
                    }
                    match control {
                        Control::Configure {
                            settings: new_settings,
                            gain,
                        } => {
                            // Engine selection takes effect on the next track.
                            let path = current_media.as_ref().map(|_| settings.mpv_path.clone());
                            let reload_cast = cast.is_some()
                                && (settings.speed != new_settings.speed
                                    || settings.preserve_pitch != new_settings.preserve_pitch
                                    || crate::dsp::Config::from_settings(&settings)
                                        != crate::dsp::Config::from_settings(&new_settings)
                                    || current_gain != gain);
                            settings = *new_settings;
                            if let Some(path) = path {
                                settings.mpv_path = path;
                            }
                            current_gain = gain;
                            if let Ok(mut tempo) = tempo.write() {
                                *tempo = crate::tempo::Options::from(&settings);
                            }
                            if let Ok(mut dsp) = dsp.write() {
                                *dsp = crate::dsp::Config::from_settings(&settings);
                            }
                            if let Some(cast) = &cast {
                                cast.controller.mute(&cast.device, settings.muted)?;
                                cast.controller
                                    .volume(&cast.device, settings.volume as u8)?;
                                if reload_cast && !cast.original_url.is_empty() {
                                    let _ = tx.send(Message::Control(
                                        active,
                                        Control::Load {
                                            url: cast.original_url.clone(),
                                            settings: Box::new(settings.clone()),
                                            position: cast.position,
                                            gain,
                                            item: cast.item.clone(),
                                            key: current_key.clone(),
                                        },
                                    ));
                                    if paused {
                                        let _ =
                                            tx.send(Message::Control(active, Control::Pause(true)));
                                    }
                                }
                            }
                            if let Some(mpv) = &mut mpv {
                                mpv.generation = id;
                                mpv.configure(&settings)?;
                            }
                            if let Some(native) = &native
                                && let Some(source) = &native.transition
                            {
                                source.configure((&settings).into(), gain);
                            }
                            if !settings.fade_on_status
                                && let Some(fade) = status_fade.take()
                            {
                                status_gain = fade.to;
                                local_status(&native, &mut mpv, fade.end)?;
                                if fade.end == FadeEnd::Stopped {
                                    let _ = events.send(Event::Stopped(active));
                                }
                            }
                            local_volume(&native, &mut mpv, &settings, status_gain)?;
                        }
                        Control::Cast(device) => {
                            if let Some(engine) = jukebox.take() {
                                engine.api.jukebox(crate::jukebox::Command::Stop)?;
                            }
                            status_fade = None;
                            status_gain = 1.0;
                            if let Some(proxy) = &proxy {
                                proxy.clear_live();
                            }
                            if let Some(previous) = cast.take() {
                                let _ = previous.controller.stop(&previous.device);
                            }
                            if let Some(task) = pending.take() {
                                task.abort();
                            }
                            mpv = None;
                            native = None;
                            if let Some(task) = pending_next.take() {
                                task.abort();
                            }
                            next_revision += 1;
                            next_key.clear();
                            playing = false;
                            active = id;
                            if let Some(device) = device {
                                let subscription =
                                    match crate::dlna::events::Subscription::start(&device) {
                                        Ok(subscription) => subscription,
                                        Err(error) => {
                                            log::debug!(
                                                "Speaker events unavailable, using polling: {error}"
                                            );
                                            None
                                        }
                                    };
                                cast = Some(Cast {
                                    controller: crate::dlna::Controller::new()?,
                                    subscription,
                                    device: *device,
                                    uri: String::new(),
                                    original_url: String::new(),
                                    item: Box::default(),
                                    speed: 1.0,
                                    loading: false,
                                    next_key: String::new(),
                                    next: None,
                                    queued_uri: String::new(),
                                    advanced: false,
                                    position: 0.0,
                                    duration: 0.0,
                                    loaded_at: Instant::now(),
                                    volume: settings.volume as u8,
                                    volume_at: Instant::now(),
                                    pending_volume: None,
                                    volume_polls: 0,
                                });
                            }
                        }
                        Control::AdoptCast {
                            status,
                            item,
                            url,
                            key,
                        } => {
                            if let Some(cast) = &mut cast {
                                cast.uri = status.uri.clone();
                                cast.original_url = url;
                                cast.queued_uri = status.next_uri.clone();
                                cast.position = status.position;
                                cast.duration = status.duration;
                                current_duration = status.duration;
                                current_gain = crate::dsp::replay_gain(&item, &settings);
                                current_media = Some((cast.original_url.clone(), item.clone()));
                                radio = item.kind == crate::model::ItemKind::Radio;
                                cast.item = item;
                                cast.loading = false;
                                cast.speed = 1.;
                                cast.loaded_at = Instant::now();
                                if let Some(volume) = status.volume {
                                    cast.volume = volume;
                                    settings.volume = volume as f32;
                                }
                                current_key = key;
                                playing = true;
                                paused = status.transport == "PAUSED_PLAYBACK";
                                let _ = events.send(Event::Started(active));
                                let _ = events.send(Event::Paused(active, paused));
                                let _ = events.send(Event::Position(active, status.position));
                            }
                        }
                        Control::DisconnectCast { passive, position } => {
                            let last_position = cast.take().map(|cast| {
                                let position = cast
                                    .controller
                                    .position(&cast.device)
                                    .map(|status| status.position * cast.speed)
                                    .unwrap_or(cast.position);
                                if !passive && let Err(error) = cast.controller.stop(&cast.device) {
                                    log::warn!("Speaker disconnect stop failed: {error}");
                                }
                                position
                            });
                            if let Some(proxy) = &proxy {
                                if passive {
                                    proxy.preserve_live();
                                } else {
                                    proxy.clear_live();
                                }
                            }
                            if let Some(task) = pending.take() {
                                task.abort();
                            }
                            if let Some(task) = pending_next.take() {
                                task.abort();
                            }
                            next_revision += 1;
                            next_key.clear();
                            active = id;
                            playing = false;
                            paused = true;
                            prepared_next = None;
                            current_media = None;
                            current_key.clear();
                            let _ = position.send(last_position);
                        }
                        Control::NextUri {
                            url,
                            item,
                            key,
                            gain,
                        } => {
                            if let Some(cast) = &mut cast {
                                if key == next_key {
                                    return Ok(());
                                }
                                next_key = key.clone();
                                next_revision += 1;
                                if let Some(task) = pending_next.take() {
                                    task.abort();
                                }
                                if !url.is_empty() && needs_proxy(&settings, gain) {
                                    if proxy.is_none() {
                                        proxy = Some(crate::proxy::Proxy::new(&runtime)?);
                                    }
                                    let tx = tx.clone();
                                    let proxy_settings = settings.clone();
                                    let revision = next_revision;
                                    pending_next = Some(runtime.spawn(async move {
                                        let result =
                                            crate::proxy::prepare(&url, &proxy_settings, gain)
                                                .await;
                                        let _ = tx.send(Message::PreparedCast(
                                            id,
                                            Some(revision),
                                            result,
                                            item,
                                            key,
                                        ));
                                    }));
                                } else {
                                    cast.next = Some((
                                        if url.is_empty() {
                                            url
                                        } else {
                                            cast.controller.lan_url(&cast.device, &url)?
                                        },
                                        item,
                                        key,
                                    ));
                                }
                            } else if let Some(mpv) = &mut mpv {
                                if key != next_key && !mpv.advanced {
                                    next_key = key.clone();
                                    mpv.command(json!(["playlist-clear"]))?;
                                    mpv.next_key.clear();
                                    if !url.is_empty()
                                        && settings.transition != crate::transition::Mode::Normal
                                    {
                                        mpv.command(json!(["loadfile", url, "append"]))?;
                                        mpv.next_key = key;
                                    }
                                }
                            } else if settings.mpv_path.is_empty() && key != next_key {
                                next_key = key.clone();
                                prepared_next = None;
                                next_revision += 1;
                                let revision = next_revision;
                                if let Some(task) = pending_next.take() {
                                    task.abort();
                                }
                                if let Some(source) = native
                                    .as_ref()
                                    .and_then(|native| native.transition.as_ref())
                                {
                                    source.prepare(None);
                                }
                                if !url.is_empty() {
                                    let tx = tx.clone();
                                    let dsp = dsp.clone();
                                    let ignore_ssl = settings.ignore_ssl;
                                    pending_next = Some(runtime.spawn(async move {
                                        let result = prepare_native(&url, None, ignore_ssl)
                                            .await
                                            .map(|decoder| {
                                                crate::transition::Track::new(
                                                    crate::dsp::Equalized::new(decoder, dsp),
                                                    key,
                                                    gain,
                                                )
                                                .with_duration(item.duration)
                                            });
                                        let _ =
                                            tx.send(Message::PreparedNext(id, revision, result));
                                    }));
                                }
                            }
                        }
                        Control::Pause(new_paused) => {
                            paused = new_paused;
                            if let Some(cast) = &cast
                                && !cast.loading
                            {
                                if paused {
                                    cast.controller.pause(&cast.device)?;
                                } else {
                                    cast.controller.play(&cast.device)?;
                                }
                            }
                            let end = if paused {
                                FadeEnd::Paused
                            } else {
                                FadeEnd::Playing
                            };
                            if settings.fade_on_status && cast.is_none() && playing {
                                if let Some(fade) = &status_fade {
                                    status_gain = fade.sample(Instant::now()).0;
                                }
                                status_fade =
                                    Some(StatusFade::new(status_gain, end, active, Instant::now()));
                                if !paused {
                                    local_status(&native, &mut mpv, FadeEnd::Playing)?;
                                }
                            } else {
                                status_fade = None;
                                status_gain = if paused && settings.fade_on_status {
                                    0.0
                                } else {
                                    1.0
                                };
                                local_volume(&native, &mut mpv, &settings, status_gain)?;
                                local_status(&native, &mut mpv, end)?;
                            }
                            let _ = events.send(Event::Paused(active, paused));
                        }
                        Control::Seek(position) => {
                            if radio || !position.is_finite() || position < 0.0 {
                                return Ok(());
                            }
                            if cast.is_none()
                                && let Some((url, item)) = &current_media
                                && transcode_at(url, position).is_some()
                            {
                                let _ = tx.send(Message::Control(
                                    active,
                                    Control::Load {
                                        url: url.clone(),
                                        settings: Box::new(settings.clone()),
                                        position,
                                        gain: current_gain,
                                        item: item.clone(),
                                        key: current_key.clone(),
                                    },
                                ));
                                if paused {
                                    let _ = tx.send(Message::Control(active, Control::Pause(true)));
                                }
                                return Ok(());
                            }
                            if let Some(cast) = &mut cast {
                                cast.controller.seek(&cast.device, position / cast.speed)?;
                                cast.position = position;
                                let _ = events.send(Event::Position(active, position));
                            }
                            if position.is_finite() && position >= 0.0 {
                                if let Some(mpv) = &mut mpv {
                                    mpv.command(json!(["seek", position, "absolute+exact"]))?;
                                }
                                if let Some(native) = &native {
                                    native
                                        .sink
                                        .try_seek(Duration::from_secs_f64(position))
                                        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                                }
                            }
                        }
                        Control::Volume(volume) => {
                            settings.volume = volume.clamp(0.0, 100.0);
                            settings.muted = false;
                            if let Some(cast) = &mut cast {
                                cast.controller.mute(&cast.device, false)?;
                                cast.controller
                                    .volume(&cast.device, settings.volume as u8)?;
                                cast.volume = settings.volume as u8;
                                cast.volume_at = Instant::now();
                            }
                            local_volume(&native, &mut mpv, &settings, status_gain)?;
                        }
                        Control::Speed(speed) => {
                            settings.speed = speed.clamp(0.25, 4.0);
                            if let Some(cast) = &cast
                                && !cast.original_url.is_empty()
                            {
                                let _ = tx.send(Message::Control(
                                    active,
                                    Control::Load {
                                        url: cast.original_url.clone(),
                                        settings: Box::new(settings.clone()),
                                        position: cast.position,
                                        gain: current_gain,
                                        item: cast.item.clone(),
                                        key: current_key.clone(),
                                    },
                                ));
                                if paused {
                                    let _ = tx.send(Message::Control(active, Control::Pause(true)));
                                }
                            }
                            if let Some(mpv) = &mut mpv {
                                mpv.command(json!(["set_property", "speed", settings.speed]))?;
                            }
                            if let Ok(mut tempo) = tempo.write() {
                                *tempo = crate::tempo::Options::from(&settings);
                            }
                        }
                        Control::Stop => {
                            if let Some(proxy) = &proxy {
                                proxy.clear_live();
                            }
                            if let Some(cast) = &mut cast {
                                cast.original_url.clear();
                                cast.next = None;
                                cast.loading = false;
                                cast.queued_uri.clear();
                                cast.controller.stop(&cast.device)?;
                            }
                            if settings.fade_on_status
                                && cast.is_none()
                                && !paused
                                && was_playing
                                && (native.is_some() || mpv.is_some())
                            {
                                if let Some(fade) = &status_fade {
                                    status_gain = fade.sample(Instant::now()).0;
                                }
                                status_fade = Some(StatusFade::new(
                                    status_gain,
                                    FadeEnd::Stopped,
                                    active,
                                    Instant::now(),
                                ));
                            } else {
                                status_fade = None;
                                local_status(&native, &mut mpv, FadeEnd::Stopped)?;
                                let _ = events.send(Event::Stopped(active));
                            }
                        }
                        Control::Load { .. } | Control::Jukebox(_) | Control::ResetMpv(_) => {
                            unreachable!()
                        }
                    }
                }
                Some(Message::PreparedNext(id, revision, result))
                    if id == active
                        && revision == next_revision
                        && generation.load(Ordering::SeqCst) == id =>
                {
                    match result {
                        Ok(track) => {
                            if let Some(source) = native
                                .as_ref()
                                .and_then(|native| native.transition.as_ref())
                            {
                                source.prepare(Some(track));
                            } else {
                                prepared_next = Some(track);
                            }
                        }
                        Err(error) => log::debug!("Next track preparation failed: {error}"),
                    }
                }
                Some(Message::PreparedCast(id, revision, result, mut item, key))
                    if id == active
                        && generation.load(Ordering::SeqCst) == id
                        && revision.is_none_or(|revision| revision == next_revision) =>
                {
                    let file = match result {
                        Ok(file) => file,
                        Err(error) if revision.is_some() => {
                            log::debug!("Next speaker transcode failed: {error}");
                            return Ok(());
                        }
                        Err(error) => return Err(error),
                    };
                    if let (Some(cast), Some(proxy)) = (&mut cast, &proxy) {
                        let url = cast.controller.lan_url(&cast.device, &proxy.insert(file))?;
                        item.container = "mp3".into();
                        if revision.is_some() {
                            item.duration /= cast.speed;
                            cast.next = Some((url, item, key));
                        } else {
                            cast.load(url, &item, resume, paused)?;
                            playing = true;
                            let _ = events.send(Event::Started(id));
                            let _ = events.send(Event::Paused(id, paused));
                        }
                    }
                }
                Some(Message::NativeAdvanced(source_id, key)) => {
                    if generation.load(Ordering::SeqCst) == active
                        && let Some(native) = &mut native
                        && native.source_id == source_id
                        && native
                            .transition
                            .as_ref()
                            .is_some_and(|source| source.key() == key)
                    {
                        native.advanced = true;
                        playing = false;
                        let _ = events.send(Event::Ended(active));
                    }
                }
                Some(Message::CastStatus(id, result, notification)) => {
                    if !notification {
                        cast_polling = false;
                    }
                    if id != active || generation.load(Ordering::SeqCst) != id {
                        return Ok(());
                    }
                    if let Some(cast) = &mut cast {
                        let mut status = result?;
                        status.position *= cast.speed;
                        status.duration *= cast.speed;
                        if !cast.queued_uri.is_empty()
                            && status.uri == cast.queued_uri
                            && (status.uri != cast.uri
                                || (cast.duration > 0.0
                                    && cast.position >= cast.duration - 3.0
                                    && status.position + 3.0 < cast.position))
                        {
                            cast.uri = status.uri.clone();
                            cast.advanced = true;
                            cast.queued_uri.clear();
                            playing = false;
                            let _ = events.send(Event::Ended(active));
                        } else if status.transport == "STOPPED"
                            && playing
                            && !paused
                            && cast.loaded_at.elapsed() > Duration::from_secs(2)
                            && cast.duration > 0.0
                            && cast.position >= cast.duration - 2.0
                        {
                            playing = false;
                            let _ = events.send(Event::Ended(active));
                        } else if !cast.advanced
                            && (status.uri.is_empty() || status.uri == cast.uri)
                        {
                            cast.position = status.position;
                            let _ = events.send(Event::Position(active, status.position));
                            if matches!(status.transport.as_str(), "PLAYING" | "PAUSED_PLAYBACK") {
                                paused = status.transport == "PAUSED_PLAYBACK";
                                let _ = events.send(Event::Paused(active, paused));
                            }
                        }
                        if let Some(volume) = status.volume.filter(|volume| *volume != cast.volume)
                        {
                            if cast.pending_volume == Some(volume) {
                                cast.volume_polls += 1;
                            } else {
                                cast.pending_volume = Some(volume);
                                cast.volume_polls = 1;
                            }
                            if (notification && cast.volume_at.elapsed() > Duration::from_secs(1))
                                || (cast.volume_at.elapsed() > Duration::from_secs(5)
                                    && cast.volume_polls >= if volume == 0 { 10 } else { 3 })
                            {
                                cast.volume = volume;
                                settings.volume = volume as f32;
                                let _ = events.send(Event::DeviceVolume(active, volume));
                            }
                        } else {
                            cast.pending_volume = None;
                            cast.volume_polls = 0;
                        }
                    }
                }
                Some(Message::RadioMetadata(id, value))
                    if id == active && radio && generation.load(Ordering::SeqCst) == id =>
                {
                    let _ = events.send(Event::RadioMetadata(id, value));
                }
                Some(Message::Mpv(instance, value))
                    if mpv.as_ref().is_some_and(|mpv| mpv.instance == instance) =>
                {
                    let mpv = mpv.as_mut().unwrap();
                    if let Some(request_id) = value["request_id"].as_u64()
                        && let Some(reply) = mpv.pending.remove(&request_id)
                    {
                        match reply {
                            MpvReply::Command {
                                generation: id,
                                operation,
                            } if id == active
                                && value["error"]
                                    .as_str()
                                    .is_some_and(|error| error != "success") =>
                            {
                                bail!(
                                    "MPV {operation} failed: {}",
                                    value["error"].as_str().unwrap()
                                );
                            }
                            MpvReply::Loaded {
                                generation: id,
                                url,
                            } if id == active
                                && generation.load(Ordering::SeqCst) == id
                                && value["data"].as_str() == Some(&url) =>
                            {
                                let position = resume + stream_offset;
                                if resume > 0.0 {
                                    mpv.command(json!(["seek", resume, "absolute+exact"]))?;
                                    resume = 0.0;
                                }
                                playing = true;
                                if settings.fade_on_status && !paused {
                                    status_fade = Some(StatusFade::new(
                                        status_gain,
                                        FadeEnd::Playing,
                                        active,
                                        Instant::now(),
                                    ));
                                }
                                let _ = events.send(Event::Started(id));
                                // A paused MPV may have reported time-pos before the load reply.
                                let _ = events.send(Event::Position(id, position));
                            }
                            _ => {}
                        }
                    }
                    if generation.load(Ordering::SeqCst) != active {
                        return Ok(());
                    }
                    match value["event"].as_str().unwrap_or_default() {
                        "file-loaded" => {
                            let request = mpv.command(json!(["get_property", "path"]))?;
                            mpv.pending.insert(
                                request,
                                MpvReply::Loaded {
                                    generation: active,
                                    url: mpv.loaded_url.clone(),
                                },
                            );
                        }
                        "property-change" if value["name"] == "time-pos" => {
                            if playing && let Some(position) = value["data"].as_f64() {
                                let _ =
                                    events.send(Event::Position(active, position + stream_offset));
                            }
                        }
                        "property-change" if value["name"] == "pause" => {
                            if let Some(observed) = value["data"].as_bool() {
                                let _ = events.send(Event::Paused(
                                    active,
                                    if status_fade.is_some() {
                                        paused
                                    } else {
                                        observed
                                    },
                                ));
                            }
                        }
                        "property-change" if value["name"] == "metadata" && radio => {
                            let _ = events.send(Event::RadioMetadata(
                                active,
                                RadioMetadata::from_value(&value["data"]),
                            ));
                        }
                        "end-file" if value["reason"] == "eof" && playing => {
                            playing = false;
                            mpv.advanced = !mpv.next_key.is_empty();
                            let _ = events.send(Event::Ended(active));
                        }
                        "end-file" if value["reason"] == "error" => {
                            bail!("MPV could not play this stream");
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            if let Some(fade) = &status_fade {
                if generation.load(Ordering::SeqCst) != fade.generation {
                    status_fade = None;
                } else {
                    let (gain, done) = fade.sample(Instant::now());
                    status_gain = gain;
                    local_volume(&native, &mut mpv, &settings, gain)?;
                    if done {
                        let fade = status_fade.take().unwrap();
                        local_status(&native, &mut mpv, fade.end)?;
                        if fade.end == FadeEnd::Stopped {
                            let _ = events.send(Event::Stopped(active));
                        }
                    }
                }
            }
            if let Some(engine) = &mut jukebox
                && generation.load(Ordering::SeqCst) == active
                && jukebox_poll.elapsed() >= Duration::from_secs(1)
            {
                jukebox_poll = Instant::now();
                let volume = engine.state.gain;
                let was_playing = engine.state.playing;
                if engine.poll()? {
                    playing = false;
                    let _ = events.send(Event::Ended(active));
                } else {
                    playing = engine.state.playing;
                    if was_playing != playing {
                        let _ = events.send(Event::Paused(active, !playing));
                    }
                    let _ = events.send(Event::Position(active, engine.state.position));
                }
                if (volume - engine.state.gain).abs() > 0.001 {
                    let _ = events.send(Event::DeviceVolume(
                        active,
                        (engine.state.gain * 100.).round() as u8,
                    ));
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            if error.to_string() == "shutdown" {
                break;
            }
            playing = false;
            status_fade = None;
            log::warn!("Playback failed: {error}");
            let _ = events.send(Event::Error(active, error.to_string()));
        }
        if let Some(cast) = &mut cast {
            if playing
                && cast.loaded_at.elapsed() > Duration::from_secs(1)
                && let Some((url, item, key)) = cast.next.take()
                && url != cast.queued_uri
            {
                let result = if url.is_empty() {
                    cast.controller.clear_next(&cast.device)
                } else {
                    cast.controller.load(&cast.device, &url, &item, true)
                };
                match result {
                    Ok(()) => {
                        cast.queued_uri = url;
                        cast.next_key = key;
                    }
                    Err(error) => log::debug!("Speaker next URI unavailable: {error}"),
                }
            }
            if playing && !cast_polling && cast_poll.elapsed() >= Duration::from_secs(1) {
                cast_poll = Instant::now();
                cast_polling = true;
                let controller = cast.controller.clone();
                let device = cast.device.clone();
                let tx = tx.clone();
                let id = active;
                runtime.spawn_blocking(move || {
                    let _ = tx.send(Message::CastStatus(id, controller.status(&device), false));
                });
            }
            if let Some(subscription) = &cast.subscription {
                for mut delta in subscription.events.try_iter() {
                    if let Some(groups) = delta.groups.take() {
                        let _ = events.send(Event::SpeakerTopology(active, groups));
                    }
                    let _ = tx.send(Message::CastNotification(active, delta));
                }
            }
        }
        if tick.elapsed() >= Duration::from_millis(250) {
            tick = Instant::now();
            if let Some(native) = &native {
                if playing && native.sink.empty() {
                    playing = false;
                    let _ = events.send(Event::Ended(active));
                } else if playing {
                    let _ = events.send(Event::Position(
                        active,
                        stream_offset
                            + native.transition.as_ref().map_or_else(
                                || native.sink.get_pos().as_secs_f64(),
                                |source| source.position(),
                            ),
                    ));
                }
            }
            if let Some(engine) = &mut mpv
                && engine.child.try_wait().ok().flatten().is_some()
            {
                mpv = None;
                playing = false;
                let _ = events.send(Event::Error(active, "MPV exited unexpectedly".into()));
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rodio::Source;
    use std::io::Cursor;
    #[cfg(unix)]
    #[test]
    fn mpv_device_discovery_uses_ipc_ids_deduplicates_and_reports_errors_without_playback() {
        use std::os::unix::fs::PermissionsExt;
        let dir =
            std::env::temp_dir().join(format!("feishin-mpv-devices-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let binary = dir.join("mpv-fixture");
        let log = dir.join("commands.jsonl");
        std::fs::write(
            &binary,
            include_str!("../../../scripts/native-tests/mpv-device-fixture.py"),
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut settings = Settings {
            mpv_path: binary.to_string_lossy().into(),
            mpv_extra_args: vec![format!("--log-file={}", log.display())],
            ..Default::default()
        };
        assert_eq!(
            mpv_audio_devices(&settings).unwrap(),
            [
                ("auto".into(), "Automatic".into()),
                ("coreaudio/42".into(), "Studio DAC".into()),
                ("null".into(), "null".into()),
            ]
        );
        settings.mpv_extra_args.push("--fail".into());
        assert!(
            mpv_audio_devices(&settings)
                .unwrap_err()
                .to_string()
                .contains("property unavailable")
        );
        let commands = std::fs::read_to_string(&log).unwrap();
        assert!(!commands.contains("loadfile") && !commands.contains("set_property"));
        assert_eq!(
            commands
                .lines()
                .filter(|line| line.contains("audio-device-list"))
                .count(),
            2
        );
        settings.mpv_extra_args.pop();
        let player = Player::new();
        for expected in 1..=2 {
            let id = player.send(Control::ResetMpv(Box::new(settings.clone())));
            wait_for(
                &player,
                |event| matches!(event, Event::Stopped(generation) if *generation == id),
            );
            // IPC writes are asynchronous. Wait until this instance consumes its
            // configuration before replacing it, without relying on scheduler speed.
            let deadline = Instant::now() + Duration::from_secs(4);
            loop {
                let configured = std::fs::read_to_string(&log)
                    .unwrap()
                    .lines()
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .filter(|command| {
                        command["command"][0] == "set_property"
                            && command["command"][1] == "audio-device"
                    })
                    .count();
                if configured == expected {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "MPV did not consume its configuration"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
        let commands = std::fs::read_to_string(&log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            commands
                .iter()
                .filter(|command| command["command"] == json!(["observe_property", 1, "time-pos"]))
                .count(),
            4
        );
        assert_eq!(
            commands
                .iter()
                .filter(|command| command["command"][0] == "set_property"
                    && command["command"][1] == "audio-device")
                .count(),
            2
        );
        assert!(
            !commands
                .iter()
                .any(|command| command["command"][0] == "loadfile")
        );
        drop(player);
        settings.mpv_path = dir.join("missing-binary").to_string_lossy().into();
        assert!(
            mpv_audio_devices(&settings)
                .unwrap_err()
                .to_string()
                .contains("Cannot launch MPV")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn speaker_notifications_advance_without_waiting_for_stale_polling() {
        use axum::{
            Router,
            body::Bytes,
            extract::State,
            http::{HeaderMap, Method},
            routing::any,
        };
        use std::sync::Mutex;
        #[derive(Default)]
        struct Fixture {
            callback: String,
            next: String,
            loads: usize,
            unsubscribed: usize,
        }
        async fn speaker(
            State(state): State<Arc<Mutex<Fixture>>>,
            method: Method,
            headers: HeaderMap,
            body: Bytes,
        ) -> (HeaderMap, String) {
            let mut state = state.lock().unwrap();
            let mut response = HeaderMap::new();
            if method.as_str() == "SUBSCRIBE" {
                if let Some(callback) = headers.get("CALLBACK") {
                    state.callback = callback.to_str().unwrap().trim_matches(['<', '>']).into();
                }
                response.insert("SID", "uuid:playback".parse().unwrap());
                response.insert("TIMEOUT", "Second-1800".parse().unwrap());
                return (response, String::new());
            }
            if method.as_str() == "UNSUBSCRIBE" {
                state.unsubscribed += 1;
                return (response, String::new());
            }
            let action = headers
                .get("SOAPACTION")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .trim_matches('"')
                .rsplit('#')
                .next()
                .unwrap_or_default();
            let fields = match action {
                "SetAVTransportURI" => { state.loads += 1; String::new() },
                "SetNextAVTransportURI" => {
                    let doc = roxmltree::Document::parse(std::str::from_utf8(&body).unwrap()).unwrap();
                    state.next = doc.descendants().find(|node| node.tag_name().name() == "NextURI").and_then(|node| node.text()).unwrap().into();
                    String::new()
                },
                "GetPositionInfo" => "<TrackURI>http://media/first</TrackURI><RelTime>0:00:00</RelTime><TrackDuration>0:00:30</TrackDuration>".into(),
                "GetTransportInfo" => "<CurrentTransportState>PLAYING</CurrentTransportState>".into(),
                "GetVolume" => "<CurrentVolume>50</CurrentVolume>".into(),
                _ => String::new(),
            };
            (
                response,
                format!(
                    "<s:Envelope xmlns:s='http://schemas.xmlsoap.org/soap/envelope/'><s:Body><Response>{fields}</Response></s:Body></s:Envelope>"
                ),
            )
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(Fixture::default()));
        let server_state = state.clone();
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let worker = thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let app = Router::new()
                        .route("/control", any(speaker))
                        .route("/events", any(speaker))
                        .with_state(server_state);
                    axum::serve(tokio::net::TcpListener::from_std(listener).unwrap(), app)
                        .with_graceful_shutdown(async {
                            let _ = stopped.await;
                        })
                        .await
                        .unwrap();
                });
        });
        let device = crate::dlna::parse_device("<root><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType><friendlyName>Fixture</friendlyName><UDN>uuid:test</UDN><serviceList><service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/control</controlURL><eventSubURL>/events</eventSubURL></service><service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/control</controlURL></service></serviceList></device></root>", &format!("{base}/description.xml")).unwrap();
        let player = Player::new();
        player.send(Control::Cast(Some(Box::new(device))));
        let item = crate::model::Item {
            duration: 30.0,
            ..Default::default()
        };
        let first = player.send(Control::Load {
            url: "http://media/first".into(),
            settings: Box::default(),
            position: 0.0,
            gain: 1.0,
            item: Box::new(item.clone()),
            key: "first".into(),
        });
        wait_for(
            &player,
            |event| matches!(event, Event::Started(id) if *id == first),
        );
        player.send(Control::NextUri {
            url: "http://media/second".into(),
            item: Box::new(item.clone()),
            key: "next".into(),
            gain: 1.0,
        });
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            let ready = {
                let state = state.lock().unwrap();
                !state.callback.is_empty() && state.next == "http://media/second"
            };
            if ready {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Next URI or event subscription was not established"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let callback = state.lock().unwrap().callback.clone();
        let body = format!(
            "<e:propertyset xmlns:e='urn:schemas-upnp-org:event-1-0'><e:property><LastChange>{}</LastChange></e:property></e:propertyset>",
            crate::dlna::escape(
                "<Event><InstanceID val='0'><AVTransportURI val='http://media/second'/><TransportState val='PLAYING'/></InstanceID></Event>"
            )
        );
        let reply = reqwest::blocking::Client::new()
            .request(Method::from_bytes(b"NOTIFY").unwrap(), callback)
            .header("NT", "upnp:event")
            .header("NTS", "upnp:propchange")
            .header("SID", "uuid:playback")
            .header("SEQ", "0")
            .body(body)
            .send()
            .unwrap();
        assert!(reply.status().is_success());
        wait_for(
            &player,
            |event| matches!(event, Event::Ended(id) if *id == first),
        );
        let second = player.send(Control::Load {
            url: "http://media/second".into(),
            settings: Box::default(),
            position: 0.0,
            gain: 1.0,
            item: Box::new(item),
            key: "next".into(),
        });
        wait_for(
            &player,
            |event| matches!(event, Event::Started(id) if *id == second),
        );
        assert_eq!(
            state.lock().unwrap().loads,
            1,
            "The event-driven transition must reuse the queued track"
        );
        drop(player);
        let deadline = Instant::now() + Duration::from_secs(4);
        while state.lock().unwrap().unsubscribed == 0 {
            assert!(
                Instant::now() < deadline,
                "Playback did not unsubscribe at shutdown"
            );
            thread::sleep(Duration::from_millis(10));
        }
        stop.send(()).unwrap();
        worker.join().unwrap();
    }
    #[track_caller]
    fn wait_for(player: &Player, expected: impl Fn(&Event) -> bool) -> Event {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut recent = std::collections::VecDeque::new();
        loop {
            let event = match player
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(event) => event,
                Err(error) => panic!("Playback wait failed: {error}; recent events: {recent:?}"),
            };
            if recent.len() == 8 {
                recent.pop_front();
            }
            recent.push_back(format!("{event:?}"));
            if expected(&event) {
                return event;
            }
            if let Event::Error(_, error) = &event {
                panic!("{error}");
            }
        }
    }
    fn audio_server() -> (
        String,
        Arc<std::sync::atomic::AtomicBool>,
        thread::JoinHandle<usize>,
    ) {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/stream", listener.local_addr().unwrap());
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_stop = stop.clone();
        let worker = thread::spawn(move || {
            let mut requests = 0;
            while !worker_stop.load(Ordering::SeqCst) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                let mut bytes = [0; 4096];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = socket.read(&mut bytes).unwrap();
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&bytes[..count]);
                }
                if request.is_empty() {
                    continue;
                }
                requests += 1;
                let wav = wav();
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",wav.len()).unwrap();
                let _ = socket.write_all(&wav);
            }
            requests
        });
        (url, stop, worker)
    }
    #[test]
    fn status_fades_have_exact_endpoints_and_reverse_without_a_volume_jump() {
        let now = Instant::now();
        let fade = StatusFade::new(1.0, FadeEnd::Paused, 7, now);
        assert_eq!(fade.sample(now), (1.0, false));
        let halfway = now + Duration::from_millis(150);
        assert!((fade.sample(halfway).0 - 0.5).abs() < 0.001);
        let resumed = StatusFade::new(fade.sample(halfway).0, FadeEnd::Playing, 7, halfway);
        assert_eq!(resumed.sample(halfway).0, fade.sample(halfway).0);
        assert_eq!(
            resumed.sample(halfway + Duration::from_millis(300)),
            (1.0, true)
        );
        assert_eq!(fade.sample(now + Duration::from_secs(1)), (0.0, true));
        let stop = StatusFade::new(0.4, FadeEnd::Stopped, 8, now);
        assert_eq!(stop.generation, 8);
        assert_eq!(stop.sample(now + Duration::from_millis(300)), (0.0, true));
    }
    #[test]
    fn real_mpv_status_fade_resume_cancels_pause_and_new_load_cancels_stop() {
        let binary = std::env::var("FEISHIN_TEST_MPV").unwrap_or_else(|_| "mpv".into());
        if Command::new(&binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            assert!(std::env::var("FEISHIN_TEST_MPV").is_err());
            return;
        }
        let (url, shutdown, worker) = audio_server();
        let player = Player::new();
        let settings = Settings {
            mpv_path: binary,
            mpv_extra_args: vec!["--no-config".into(), "--ao=null".into()],
            ..Default::default()
        };
        let load = || Control::Load {
            url: url.clone(),
            settings: Box::new(settings.clone()),
            position: 0.0,
            gain: 1.0,
            item: Box::new(crate::model::Item {
                duration: 1.0,
                ..Default::default()
            }),
            key: uuid::Uuid::new_v4().to_string(),
        };
        let first = player.send(load());
        wait_for(
            &player,
            |event| matches!(event,Event::Started(id) if *id==first),
        );
        player.send(Control::Pause(true));
        wait_for(
            &player,
            |event| matches!(event,Event::Paused(id,true) if *id==first),
        );
        thread::sleep(Duration::from_millis(60));
        player.send(Control::Pause(false));
        wait_for(
            &player,
            |event| matches!(event,Event::Paused(id,false) if *id==first),
        );
        let resumed = Instant::now();
        while resumed.elapsed() < Duration::from_millis(350) {
            if let Ok(event) = player.events.recv_timeout(Duration::from_millis(20)) {
                assert!(
                    !matches!(event, Event::Paused(id, true) if id == first),
                    "An old pause fade interrupted resume"
                );
                assert!(!matches!(event, Event::Error(_, _)), "{event:?}");
            }
        }
        let stop_time = Instant::now();
        let stopped = player.send(Control::Stop);
        wait_for(
            &player,
            |event| matches!(event,Event::Stopped(id) if *id==stopped),
        );
        assert!(stop_time.elapsed() >= Duration::from_millis(250));
        let second = player.send(load());
        wait_for(
            &player,
            |event| matches!(event,Event::Started(id) if *id==second),
        );
        let cancelled_stop = player.send(Control::Stop);
        thread::sleep(Duration::from_millis(60));
        let third = player.send(load());
        wait_for(
            &player,
            |event| matches!(event,Event::Started(id) if *id==third),
        );
        let deadline = Instant::now() + Duration::from_millis(450);
        while Instant::now() < deadline {
            if let Ok(event) = player.events.recv_timeout(Duration::from_millis(20)) {
                assert!(
                    !matches!(event, Event::Stopped(id) if id == cancelled_stop),
                    "A stale stop fade ended the new track"
                );
                assert!(!matches!(event, Event::Error(_, _)), "{event:?}");
            }
        }
        drop(player);
        shutdown.store(true, Ordering::SeqCst);
        assert_eq!(worker.join().unwrap(), 3);
    }

    #[test]
    fn jellyfin_transcode_resume_and_paused_seek_restart_chunked_streams() {
        use std::io::Read;
        let binary = std::env::var("FEISHIN_TEST_MPV").unwrap_or_else(|_| "mpv".into());
        if Command::new(&binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            assert!(
                std::env::var("FEISHIN_TEST_MPV").is_err(),
                "Configured test MPV is missing"
            );
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let original = format!(
            "http://{}/music/Audio/song/stream.wav?apiKey=private-token&Static=false&AudioCodec=wav&PlaySessionId=original",
            listener.local_addr().unwrap()
        );
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shutdown = stop.clone();
        let server = thread::spawn(move || {
            let mut starts = vec![];
            while !shutdown.load(Ordering::SeqCst) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = vec![];
                let mut chunk = [0; 4096];
                while !request.windows(4).any(|part| part == b"\r\n\r\n") {
                    let count = socket.read(&mut chunk).unwrap();
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&chunk[..count]);
                }
                if request.is_empty() {
                    continue;
                }
                let request = String::from_utf8(request).unwrap();
                let path = request.split_whitespace().nth(1).unwrap();
                let url = url::Url::parse(&format!("http://fixture{path}")).unwrap();
                let params = url
                    .query_pairs()
                    .collect::<std::collections::HashMap<_, _>>();
                assert_eq!(params["apiKey"], "private-token");
                let ticks = params["StartTimeTicks"].parse::<u64>().unwrap();
                let start = ticks as f64 / 10_000_000.;
                starts.push((ticks, params["PlaySessionId"].to_string()));
                let mut wav = wav();
                let bytes = ((1. - start).max(0.) * 16000.) as usize / 2 * 2;
                wav.truncate(44 + bytes);
                wav[4..8].copy_from_slice(&(36 + bytes as u32).to_le_bytes());
                wav[40..44].copy_from_slice(&(bytes as u32).to_le_bytes());
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").unwrap();
                for bytes in wav.chunks(512) {
                    if write!(socket, "{:x}\r\n", bytes.len())
                        .and_then(|_| socket.write_all(bytes))
                        .and_then(|_| socket.write_all(b"\r\n"))
                        .is_err()
                    {
                        break;
                    }
                }
                let _ = socket.write_all(b"0\r\n\r\n");
            }
            starts
        });
        let player = Player::new();
        let observed_paused = std::cell::Cell::new(false);
        let wait = |expected: &dyn Fn(&Event) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut recent = std::collections::VecDeque::new();
            loop {
                let event = player
                    .events
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .unwrap_or_else(|error| {
                        panic!("Playback wait failed: {error}; recent events: {recent:?}")
                    });
                if recent.len() == 8 {
                    recent.pop_front();
                }
                recent.push_back(format!("{event:?}"));
                if let Event::Paused(_, value) = &event {
                    observed_paused.set(*value);
                }
                if expected(&event) {
                    break;
                }
                assert!(!matches!(event, Event::Error(_, _)), "{event:?}");
            }
        };
        let settings = Settings {
            mpv_path: binary,
            mpv_extra_args: vec!["--no-config".into(), "--ao=null".into()],
            fade_on_status: false,
            ..Default::default()
        };
        let id = player.send(Control::Load {
            url: original.clone(),
            settings: Box::new(settings),
            position: 0.2,
            gain: 1.,
            item: Box::new(crate::model::Item {
                duration: 1.,
                ..Default::default()
            }),
            key: "entry".into(),
        });
        player.send(Control::Pause(true));
        wait(&|event| matches!(event,Event::Started(generation) if *generation==id));
        wait(
            &|event| matches!(event,Event::Position(generation,position) if *generation==id && (0.19..0.4).contains(position)),
        );
        player.send(Control::Seek(0.6));
        wait(&|event| matches!(event,Event::Started(generation) if *generation==id));
        assert!(observed_paused.get(), "A paused seek resumed playback");
        wait(
            &|event| matches!(event,Event::Position(generation,position) if *generation==id && (0.59..0.8).contains(position)),
        );
        let deadline = Instant::now() + Duration::from_millis(150);
        while Instant::now() < deadline {
            if let Ok(event) = player.events.recv_timeout(Duration::from_millis(20)) {
                assert!(
                    !matches!(event, Event::Ended(_)),
                    "A paused seek resumed playback"
                );
                assert!(!matches!(event, Event::Error(_, _)), "{event:?}");
            }
        }
        player.send(Control::Pause(false));
        wait(&|event| matches!(event,Event::Ended(generation) if *generation==id));
        drop(player);
        stop.store(true, Ordering::SeqCst);
        let starts = server.join().unwrap();
        assert_eq!(
            starts.iter().map(|(ticks, _)| *ticks).collect::<Vec<_>>(),
            [2_000_000, 6_000_000]
        );
        assert_ne!(starts[0].1, starts[1].1);
        assert!(starts.iter().all(|(_, session)| session != "original"));
        assert!(transcode_at("https://example.com/radio", 1.).is_none());
        assert!(transcode_at(&original, f64::NAN).is_none());
        assert!(transcode_at(&original, -1.).is_none());
        let replacement = transcode_at(&(original + "&startTimeTicks=10"), 12.25).unwrap();
        assert_eq!(
            url::Url::parse(&replacement)
                .unwrap()
                .query_pairs()
                .filter(|(key, _)| key.eq_ignore_ascii_case("startTimeTicks"))
                .collect::<Vec<_>>(),
            [("StartTimeTicks".into(), "122500000".into())]
        );
    }

    #[test]
    fn real_mpv_ipc_gapless_duplicate_tracks_pause_seek_volume_and_stop() {
        let binary = std::env::var("FEISHIN_TEST_MPV").unwrap_or_else(|_| "mpv".into());
        if Command::new(&binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            assert!(
                std::env::var("FEISHIN_TEST_MPV").is_err(),
                "Configured test MPV is missing"
            );
            eprintln!("MPV integration check skipped: optional binary is unavailable");
            return;
        }
        let (url, stop, worker) = audio_server();
        let player = Player::new();
        let settings = Settings {
            mpv_path: binary,
            mpv_extra_args: vec!["--no-config".into(), "--ao=null".into()],
            ..Default::default()
        };
        let item = crate::model::Item {
            name: "Fixture".into(),
            duration: 1.0,
            ..Default::default()
        };
        let first = player.send(Control::Load {
            url: url.clone(),
            settings: Box::new(settings.clone()),
            position: 0.0,
            gain: 1.0,
            item: Box::new(item.clone()),
            key: "first".into(),
        });
        wait_for(
            &player,
            |event| matches!(event,Event::Started(id) if *id==first),
        );
        player.send(Control::Pause(true));
        wait_for(
            &player,
            |event| matches!(event,Event::Paused(id,true) if *id==first),
        );
        player.send(Control::Seek(0.4));
        wait_for(
            &player,
            |event| matches!(event,Event::Position(id,position) if *id==first && *position>=0.35),
        );
        player.send(Control::Volume(0.0));
        player.send(Control::NextUri {
            url: url.clone(),
            item: Box::new(item.clone()),
            key: "duplicate".into(),
            gain: 1.0,
        });
        player.send(Control::Pause(false));
        wait_for(
            &player,
            |event| matches!(event,Event::Ended(id) if *id==first),
        );
        let second = player.send(Control::Load {
            url,
            settings: Box::new(settings),
            position: 0.0,
            gain: 1.0,
            item: Box::new(item),
            key: "duplicate".into(),
        });
        wait_for(
            &player,
            |event| matches!(event,Event::Started(id) if *id==second),
        );
        wait_for(
            &player,
            |event| matches!(event,Event::Ended(id) if *id==second),
        );
        let stopped = player.send(Control::Stop);
        wait_for(
            &player,
            |event| matches!(event,Event::Stopped(id) if *id==stopped),
        );
        player.send(Control::Configure {
            settings: Box::new(Settings {
                mpv_properties: std::collections::BTreeMap::from([(
                    "feishin-invalid-property".into(),
                    true.into(),
                )]),
                ..Default::default()
            }),
            gain: 1.0,
        });
        wait_for(
            &player,
            |event| matches!(event, Event::Error(id, error) if *id == stopped && error.contains("feishin-invalid-property")),
        );
        drop(player);
        stop.store(true, Ordering::SeqCst);
        assert_eq!(
            worker.join().unwrap(),
            2,
            "Gapless duplicate transition must reuse the queued stream"
        );
    }
    #[test]
    fn speaker_ffmpeg_proxy_loads_an_opaque_url_and_scales_seek_time() {
        let binary = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        if Command::new(&binary)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            return;
        }
        let (url, stop, worker) = audio_server();
        let (device, speaker) = crate::dlna::tests::fixture(vec![crate::dlna::tests::ok(); 7]);
        let player = Player::new();
        player.send(Control::Cast(Some(Box::new(device))));
        let id = player.send(Control::Load {
            url,
            settings: Box::new(Settings {
                ffmpeg_path: binary,
                speed: 2.0,
                ..Default::default()
            }),
            position: 12.0,
            gain: 1.0,
            item: Box::new(crate::model::Item {
                duration: 60.0,
                ..Default::default()
            }),
            key: "proxy".into(),
        });
        wait_for(
            &player,
            |event| matches!(event,Event::Started(current) if *current==id),
        );
        let stopped = player.send(Control::Stop);
        wait_for(
            &player,
            |event| matches!(event,Event::Stopped(current) if *current==stopped),
        );
        drop(player);
        let requests = speaker.join().unwrap();
        assert!(requests[2].contains("/audio/"));
        assert!(!requests[2].contains("/stream"));
        assert!(requests[2].contains("duration=&quot;0:00:30&quot;"));
        assert!(requests[4].contains("<Target>0:00:06</Target>"));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(worker.join().unwrap(), 1);
    }
    #[test]
    fn cast_selection_load_pause_stop_and_shutdown_use_the_network_backend() {
        let (device, worker) = crate::dlna::tests::fixture(vec![crate::dlna::tests::ok(); 7]);
        let player = Player::new();
        player.send(Control::Cast(Some(Box::new(device))));
        let generation = player.send(Control::Load {
            url: "http://host/song".into(),
            settings: Box::default(),
            position: 0.0,
            gain: 1.0,
            item: Box::new(crate::model::Item {
                name: "Test".into(),
                ..Default::default()
            }),
            key: "test".into(),
        });
        loop {
            match player.events.recv_timeout(Duration::from_secs(5)).unwrap() {
                Event::Started(id) => {
                    assert_eq!(id, generation);
                    break;
                }
                Event::Error(_, error) => panic!("{error}"),
                _ => {}
            }
        }
        player.send(Control::Pause(true));
        loop {
            if let Event::Paused(id, true) =
                player.events.recv_timeout(Duration::from_secs(5)).unwrap()
            {
                assert_eq!(id, generation);
                break;
            }
        }
        let stopped = player.send(Control::Stop);
        loop {
            if let Event::Stopped(id) = player.events.recv_timeout(Duration::from_secs(5)).unwrap()
            {
                assert_eq!(id, stopped);
                break;
            }
        }
        drop(player);
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("#SetVolume"));
        assert!(requests[2].contains("#SetAVTransportURI"));
        assert!(requests[3].contains("#Play"));
        assert!(requests[4].contains("#Pause"));
        assert!(requests[5].contains("#Stop"));
        assert!(requests[6].contains("#Stop"));
    }
    pub(crate) fn wav() -> Vec<u8> {
        let samples = vec![0u8; 16000];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(&samples);
        wav
    }
    #[test]
    fn decode_pcm_without_an_audio_device() {
        let decoder = Decoder::try_from(Cursor::new(wav())).unwrap();
        assert_eq!(decoder.sample_rate(), 8000);
        assert_eq!(decoder.channels(), 1);
        assert_eq!(decoder.count(), 8000);
    }
    #[test]
    fn waveform_stream_decodes_without_opening_an_audio_device() {
        let (url, stop, worker) = audio_server();
        let data = waveform(
            &url,
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            false,
        )
        .unwrap();
        assert_eq!(data.duration, 1.);
        assert!(data.peaks.iter().all(|peak| *peak == 0.));
        stop.store(true, Ordering::SeqCst);
        assert_eq!(worker.join().unwrap(), 1);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn icecast_metadata_is_removed_from_audio_and_keeps_station_identity_separate() {
        use std::{io::Read, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/radio", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .to_lowercase()
                    .contains("icy-metadata: 1")
            );
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nicy-metaint: 1024\r\nConnection: close\r\n\r\n").unwrap();
            for (index, chunk) in wav().chunks(1024).enumerate() {
                socket.write_all(chunk).unwrap();
                if chunk.len() == 1024 {
                    if index == 0 {
                        let metadata = b"StreamTitle='Artist - Song';";
                        socket.write_all(&[2]).unwrap();
                        socket.write_all(metadata).unwrap();
                        socket.write_all(&vec![0; 32 - metadata.len()]).unwrap();
                    } else {
                        socket.write_all(&[0]).unwrap();
                    }
                }
            }
        });
        let (tx, rx) = mpsc::channel();
        let decoder = prepare_native(&url, Some((12, tx)), false).await.unwrap();
        let count = tokio::task::spawn_blocking(move || decoder.count())
            .await
            .unwrap();
        assert_eq!(count, 8000);
        let Message::RadioMetadata(id, metadata) = rx.recv_timeout(Duration::from_secs(3)).unwrap()
        else {
            panic!("Missing metadata");
        };
        assert_eq!(id, 12);
        assert_eq!(
            metadata,
            RadioMetadata {
                artist: "Artist".into(),
                title: "Song".into()
            }
        );
        worker.join().unwrap();
        assert_eq!(
            RadioMetadata::from_value(
                &json!({"ARTIST":"Solo", "TITLE":"Track", "StreamTitle":"Other - Track"})
            ),
            RadioMetadata {
                artist: "Solo".into(),
                title: "Track".into()
            }
        );
        assert_eq!(
            RadioMetadata::from_value(&json!({"title":"Track", "stream-title":"Band - Track"}))
                .artist,
            "Band"
        );
        assert_eq!(
            RadioMetadata::from_value(&json!({"icy-title":"Announcement"})).title,
            "Announcement"
        );
        assert_eq!(
            RadioMetadata::from_value(&json!(null)),
            RadioMetadata::default()
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn streamed_http_audio_decodes_and_seeks_without_an_audio_device() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/stream", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let count = socket.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..count])
                    .unwrap()
                    .starts_with("GET /stream ")
            );
            let wav = wav();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",wav.len()).unwrap();
            for chunk in wav.chunks(512) {
                socket.write_all(chunk).unwrap();
            }
        });
        let mut decoder = prepare_native(&url, None, false).await.unwrap();
        tokio::task::spawn_blocking(move || {
            assert_eq!(decoder.sample_rate(), 8000);
            assert_eq!(decoder.by_ref().take(100).count(), 100);
            decoder.try_seek(Duration::from_millis(750)).unwrap();
            let remaining = decoder.count();
            assert!(
                (1900..=2100).contains(&remaining),
                "{remaining} samples after seeking to 750 ms"
            );
        })
        .await
        .unwrap();
        server.join().unwrap();
    }
    #[test]
    fn speaker_adoption_and_passive_disconnect_preserve_remote_transport() {
        for passive in [false, true] {
            for paused in [false, true] {
                let mut replies = vec!["<Position><RelTime>0:01:23</RelTime></Position>".into()];
                if !passive {
                    replies.push("<Stopped/>".into());
                }
                let (device, worker) = crate::dlna::tests::fixture(replies);
                let player = Player::new();
                let generation = player.send(Control::Cast(Some(Box::new(device))));
                player.send(Control::AdoptCast {
                    status: Box::new(crate::dlna::Status {
                        position: 23.,
                        duration: 100.,
                        uri: "http://fixture/current".into(),
                        transport: if paused { "PAUSED_PLAYBACK" } else { "PLAYING" }.into(),
                        volume: Some(37),
                        ..Default::default()
                    }),
                    item: Box::default(),
                    url: "http://fixture/original".into(),
                    key: "song".into(),
                });
                let deadline = Instant::now() + Duration::from_secs(3);
                loop {
                    let event = player
                        .events
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .unwrap();
                    if let Event::Paused(id, actual) = event {
                        assert_eq!(id, generation);
                        assert_eq!(actual, paused);
                        break;
                    }
                }
                let (position, result) = mpsc::channel();
                player.send(Control::DisconnectCast { passive, position });
                assert_eq!(
                    result.recv_timeout(Duration::from_secs(3)).unwrap(),
                    Some(83.)
                );
                let requests = worker.join().unwrap();
                assert_eq!(requests.len(), if passive { 1 } else { 2 });
                assert!(requests[0].contains("#GetPositionInfo"));
                if !passive {
                    assert!(requests[1].contains("#Stop"));
                }
                assert!(
                    requests
                        .iter()
                        .all(|request| !request.contains("#SetAVTransportURI")
                            && !request.contains("#SetVolume")
                            && !request.contains("#Play\""))
                );
            }
        }
    }
}
