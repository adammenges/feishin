//! Seekable speaker transcoding. Public URLs contain a random capability, never
//! the music server URL or credentials. Files disappear with their owner.
use crate::storage::Settings;
use anyhow::{Context, Result, ensure};
use axum::{
    Router,
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub struct AudioFile {
    path: PathBuf,
    pub length: u64,
}
impl Drop for AudioFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}
#[derive(Clone)]
enum AudioResource {
    File(Arc<AudioFile>),
    Live(Arc<LiveAudio>),
}
struct LiveAudio {
    url: String,
    settings: Settings,
    gain: f32,
    cancel: tokio::sync::watch::Sender<bool>,
    connections: Arc<tokio::sync::Semaphore>,
    detached: AtomicBool,
}
impl AudioResource {
    fn cancel(&self) {
        if let Self::Live(source) = self {
            source.cancel.send_replace(true);
        }
    }
}
type Files = Arc<Mutex<VecDeque<(String, AudioResource)>>>;
pub struct Proxy {
    files: Files,
    server: tokio::task::JoinHandle<()>,
    port: u16,
}
impl Proxy {
    pub fn new(runtime: &tokio::runtime::Runtime) -> Result<Self> {
        let listener = std::net::TcpListener::bind(("0.0.0.0", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let files = Arc::new(Mutex::new(VecDeque::new()));
        let shared = files.clone();
        let server = runtime.spawn(async move {
            let result = async {
                let listener = tokio::net::TcpListener::from_std(listener)?;
                axum::serve(
                    listener,
                    Router::new()
                        .route("/audio/{token}", get(audio))
                        .with_state(shared),
                )
                .await
            }
            .await;
            if let Err(error) = result {
                log::warn!("Speaker audio proxy failed: {error}");
            }
        });
        Ok(Self {
            files,
            server,
            port,
        })
    }
    pub fn insert(&self, file: AudioFile) -> String {
        self.insert_resource(AudioResource::File(Arc::new(file)))
    }
    pub fn insert_live(&self, url: String, settings: Settings, gain: f32) -> Result<String> {
        let source = url::Url::parse(&url)?;
        ensure!(
            matches!(source.scheme(), "http" | "https"),
            "Unsupported speaker audio source"
        );
        let (cancel, _) = tokio::sync::watch::channel(false);
        Ok(
            self.insert_resource(AudioResource::Live(Arc::new(LiveAudio {
                url,
                settings,
                gain,
                cancel,
                connections: Arc::new(tokio::sync::Semaphore::new(2)),
                detached: AtomicBool::new(false),
            }))),
        )
    }
    fn insert_resource(&self, resource: AudioResource) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        if let Ok(mut files) = self.files.lock() {
            files.push_back((token.clone(), resource));
            while files.len() > 16 {
                if let Some((_, resource)) = files.pop_front() {
                    resource.cancel();
                }
            }
        }
        format!("http://127.0.0.1:{}/audio/{token}", self.port)
    }
    pub fn clear_live(&self) {
        if let Ok(mut files) = self.files.lock() {
            files.retain(|(_, resource)| {
                if matches!(resource, AudioResource::Live(source) if !source.detached.load(Ordering::Relaxed)) {
                    resource.cancel();
                    false
                } else {
                    true
                }
            });
        }
    }
    pub fn preserve_live(&self) {
        if let Ok(files) = self.files.lock() {
            for (_, resource) in files.iter() {
                if let AudioResource::Live(source) = resource {
                    source.detached.store(true, Ordering::Relaxed);
                }
            }
        }
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        if let Ok(files) = self.files.lock() {
            for (_, resource) in files.iter() {
                resource.cancel();
            }
        }
        self.server.abort();
    }
}

pub fn filters(settings: &Settings, gain: f32) -> String {
    let speed = settings.speed.clamp(0.25, 4.0);
    let mut filters = Vec::new();
    if (speed - 1.0).abs() > 0.001 {
        if settings.preserve_pitch {
            let mut remaining = speed;
            while remaining > 2.0 {
                filters.push("atempo=2".into());
                remaining /= 2.0;
            }
            while remaining < 0.5 {
                filters.push("atempo=0.5".into());
                remaining /= 0.5;
            }
            filters.push(format!("atempo={remaining:.6}"));
        } else {
            filters.push(format!(
                "aresample=48000,asetrate={},aresample=48000",
                (48000.0 * speed).round() as u32
            ));
        }
    }
    let dsp = dsp_filters(settings, gain);
    if !dsp.is_empty() {
        filters.push(dsp);
    }
    if filters.is_empty() {
        "anull".into()
    } else {
        filters.join(",")
    }
}
pub fn dsp_filters(settings: &Settings, gain: f32) -> String {
    let mut filters = Vec::new();
    if settings.equalizer_enabled && settings.equalizer_preamp.abs() > 0.001 {
        filters.push(format!(
            "volume={}dB",
            settings.equalizer_preamp.clamp(-12.0, 12.0)
        ));
    }
    if settings.equalizer_enabled {
        for ((frequency, width), db) in crate::dsp::FREQUENCIES
            .into_iter()
            .zip(crate::dsp::WIDTHS)
            .zip(settings.equalizer)
        {
            if db.abs() > 0.001 {
                filters.push(format!(
                    "equalizer=f={frequency}:width_type=o:w={width}:g={}",
                    db.clamp(-24.0, 24.0)
                ));
            }
        }
    }
    if gain.is_finite() && (gain - 1.0).abs() > 0.001 {
        filters.push(format!("volume={:.6}", gain.clamp(0.0, 32.0)));
    }
    let compressor = settings.compressor;
    if compressor.enabled {
        filters.push(format!(
            "acompressor=threshold={:.6}:ratio={}:attack={}:release={}:makeup={:.6}:knee={}",
            10.0f32.powf(compressor.threshold.clamp(-60.0, 0.0) / 20.0),
            compressor.ratio.clamp(1.0, 20.0),
            compressor.attack.clamp(0.01, 2000.0),
            compressor.release.clamp(1.0, 9000.0),
            10.0f64.powf(f64::from(compressor.makeup.clamp(0.0, 30.0)) / 20.0),
            // The original editor permits 10, but FFmpeg rejects values above 8.
            compressor.knee.clamp(1.0, 8.0)
        ));
    }
    filters.join(",")
}
pub async fn prepare(url: &str, settings: &Settings, gain: f32) -> Result<AudioFile> {
    let source = url::Url::parse(url)?;
    ensure!(
        matches!(source.scheme(), "http" | "https"),
        "Unsupported speaker audio source"
    );
    let dir = std::env::temp_dir().join(format!("feishin-speaker-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut file = AudioFile {
        path: dir.join("audio.mp3"),
        length: 0,
    };
    let mut command = transcode_command(url, settings, gain);
    command.arg(&file.path).stdout(Stdio::null());
    let mut child = command
        .spawn()
        .context("Cannot launch FFmpeg. Set its binary path in Settings.")?;
    let status = tokio::time::timeout(Duration::from_secs(180), child.wait())
        .await
        .context("Speaker transcode timed out")??;
    ensure!(
        status.success(),
        "FFmpeg could not transcode the audio stream"
    );
    file.length = std::fs::metadata(&file.path)?.len();
    ensure!(file.length > 0, "FFmpeg produced an empty audio file");
    Ok(file)
}
fn transcode_command(url: &str, settings: &Settings, gain: f32) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(&settings.ffmpeg_path);
    if url.starts_with("https://") {
        command.args(["-tls_verify", if settings.ignore_ssl { "0" } else { "1" }]);
    }
    command
        .args([
            "-nostdin",
            "-loglevel",
            "error",
            "-rw_timeout",
            "15000000",
            "-i",
            url,
            "-vn",
            "-af",
            &filters(settings, gain),
            "-codec:a",
            "libmp3lame",
            "-b:a",
            "192k",
            "-f",
            "mp3",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        command.creation_flags(0x08000000);
    }
    command
}

fn range(header: Option<&str>, length: u64) -> Result<(u64, u64, bool)> {
    ensure!(length > 0, "Empty file");
    let Some(header) = header else {
        return Ok((0, length - 1, false));
    };
    let (start, end) = header
        .strip_prefix("bytes=")
        .context("Invalid range unit")?
        .split_once('-')
        .context("Invalid byte range")?;
    let (start, end) = if start.is_empty() {
        let count: u64 = end.parse()?;
        ensure!(count > 0, "Invalid suffix range");
        (length.saturating_sub(count), length - 1)
    } else {
        (
            start.parse()?,
            if end.is_empty() {
                length - 1
            } else {
                end.parse::<u64>()?.min(length - 1)
            },
        )
    };
    ensure!(start < length && start <= end, "Unsatisfiable byte range");
    Ok((start, end, true))
}
async fn audio(
    Path(token): Path<String>,
    State(files): State<Files>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let file = files.lock().ok().and_then(|files| {
        files
            .iter()
            .find(|(key, _)| key == &token)
            .map(|(_, file)| file.clone())
    });
    let Some(resource) = file else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let file = match resource {
        AudioResource::File(file) => file,
        AudioResource::Live(source) => return live_audio(source, method, headers).await,
    };
    let (start, end, partial) = match range(
        headers.get("range").and_then(|range| range.to_str().ok()),
        file.length,
    ) {
        Ok(range) => range,
        Err(_) => {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [("Content-Range", format!("bytes */{}", file.length))],
            )
                .into_response();
        }
    };
    let length = end - start + 1;
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let Ok(mut input) = tokio::fs::File::open(&file.path).await else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if input.seek(std::io::SeekFrom::Start(start)).await.is_err() {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        let stream = futures_util::stream::unfold(
            (input, length, file.clone()),
            |(mut input, remaining, owner)| async move {
                if remaining == 0 {
                    return None;
                }
                let mut bytes = vec![0; remaining.min(32768) as usize];
                match input.read(&mut bytes).await {
                    Ok(0) => None,
                    Ok(count) => {
                        bytes.truncate(count);
                        Some((
                            Ok::<_, std::io::Error>(bytes),
                            (input, remaining - count as u64, owner),
                        ))
                    }
                    Err(error) => Some((Err(error), (input, 0, owner))),
                }
            },
        );
        Body::from_stream(stream)
    };
    let mut response = Response::builder()
        .status(if partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header("Content-Type", "audio/mpeg")
        .header("Content-Length", length)
        .header("Accept-Ranges", "bytes")
        .header("Cache-Control", "private, no-store");
    if partial {
        response = response.header(
            "Content-Range",
            format!("bytes {start}-{end}/{}", file.length),
        );
    }
    response.body(body).unwrap()
}

async fn live_audio(source: Arc<LiveAudio>, method: Method, headers: HeaderMap) -> Response {
    if headers.contains_key("range") {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    }
    let response = Response::builder()
        .header("Content-Type", "audio/mpeg")
        .header("Accept-Ranges", "none")
        .header("Cache-Control", "private, no-store")
        .header("transferMode.dlna.org", "Streaming");
    if method == Method::HEAD {
        return response
            .body(Body::from_stream(futures_util::stream::empty::<
                Result<Vec<u8>, std::io::Error>,
            >()))
            .unwrap();
    }
    let Ok(permit) = source.connections.clone().try_acquire_owned() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let cancel = source.cancel.subscribe();
    if *cancel.borrow() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let child = transcode_command(&source.url, &source.settings, source.gain)
        .args(["-flush_packets", "1", "pipe:1"])
        .stdout(Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        log::warn!("Could not start the live speaker transcoder");
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let stdout = child.stdout.take().unwrap();
    let stream = futures_util::stream::unfold(
        (stdout, child, permit, source, cancel),
        |(mut stdout, mut child, permit, source, mut cancel)| async move {
            let mut bytes = vec![0; 16384];
            let result = tokio::select! {
                _ = cancel.changed() => return None,
                result = stdout.read(&mut bytes) => result,
            };
            match result {
                Ok(0) => {
                    if let Ok(status) = child.wait().await
                        && !status.success()
                    {
                        log::warn!("Live speaker transcoding ended with an error");
                    }
                    None
                }
                Ok(count) => {
                    bytes.truncate(count);
                    Some((
                        Ok::<_, std::io::Error>(bytes),
                        (stdout, child, permit, source, cancel),
                    ))
                }
                Err(_) => None,
            }
        },
    );
    response.body(Body::from_stream(stream)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn passive_speaker_streams_survive_local_changes_and_stop_when_proxy_exits() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let proxy = Proxy::new(&runtime).unwrap();
        let url = proxy
            .insert_live("http://127.0.0.1:1/radio".into(), Settings::default(), 1.)
            .unwrap();
        let source = match &proxy.files.lock().unwrap()[0].1 {
            AudioResource::Live(source) => source.clone(),
            _ => unreachable!(),
        };
        proxy.preserve_live();
        let other = proxy
            .insert_live("http://127.0.0.1:1/other".into(), Settings::default(), 1.)
            .unwrap();
        proxy.clear_live();
        let client = reqwest::blocking::Client::new();
        assert_eq!(client.head(&url).send().unwrap().status(), 200);
        assert_eq!(client.head(other).send().unwrap().status(), 404);
        assert!(!*source.cancel.borrow());
        drop(proxy);
        assert!(*source.cancel.borrow());
    }
    #[test]
    fn live_radio_proxy_streams_without_exposing_credentials_and_revokes_active_streams() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let binary = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        if std::process::Command::new(&binary)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            assert!(
                std::env::var("FEISHIN_TEST_FFMPEG").is_err(),
                "Configured test FFmpeg is missing"
            );
            eprintln!("Live FFmpeg integration check skipped: optional binary is unavailable");
            return;
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let source_url = format!(
            "http://{}/radio?token=private",
            listener.local_addr().unwrap()
        );
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                socket.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let mut data = crate::playback::tests::wav();
            let samples = data[44..].repeat(120);
            data[4..8].copy_from_slice(&(36 + samples.len() as u32).to_le_bytes());
            data[40..44].copy_from_slice(&(samples.len() as u32).to_le_bytes());
            data.truncate(44);
            data.extend(samples);
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", data.len()).unwrap();
            let mut written = 0;
            for chunk in data.chunks(4096) {
                if socket.write_all(chunk).is_err() {
                    break;
                }
                written += chunk.len();
                thread::sleep(Duration::from_millis(2));
            }
            (String::from_utf8(request).unwrap(), written, data.len())
        });
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let proxy = Proxy::new(&runtime).unwrap();
        let url = proxy
            .insert_live(
                source_url,
                Settings {
                    ffmpeg_path: binary,
                    speed: 0.5,
                    equalizer_enabled: true,
                    ..Default::default()
                },
                1.0,
            )
            .unwrap();
        assert!(!url.contains("private"));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        let head = client.head(&url).send().unwrap();
        assert_eq!(head.status(), 200);
        assert_eq!(head.headers()["accept-ranges"], "none");
        assert!(!head.headers().contains_key("content-length"));
        assert_eq!(
            client
                .get(&url)
                .header("Range", "bytes=0-")
                .send()
                .unwrap()
                .status(),
            416
        );
        let mut response = client.get(&url).send().unwrap();
        assert_eq!(response.status(), 200);
        assert!(!response.headers().contains_key("content-length"));
        let mut first = [0; 10];
        response.read_exact(&mut first).unwrap();
        assert_eq!(&first[..3], b"ID3");
        proxy.clear_live();
        let mut remainder = Vec::new();
        response.read_to_end(&mut remainder).unwrap();
        assert_eq!(client.get(&url).send().unwrap().status(), 404);
        let (request, written, length) = worker.join().unwrap();
        assert!(request.contains("token=private"));
        assert!(
            written < length,
            "Revoking the proxy must close the upstream radio stream"
        );
        assert!(remainder.len() < 1_000_000);
    }
    #[test]
    fn ffmpeg_transcodes_http_audio_at_requested_speed_and_keeps_it_seekable() {
        use rodio::Source;
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let binary = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
        if std::process::Command::new(&binary)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_err()
        {
            assert!(
                std::env::var("FEISHIN_TEST_FFMPEG").is_err(),
                "Configured test FFmpeg is missing"
            );
            eprintln!("FFmpeg integration check skipped: optional binary is unavailable");
            return;
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/audio", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let count = socket.read(&mut bytes).unwrap();
            assert!(String::from_utf8_lossy(&bytes[..count]).starts_with("GET /audio "));
            let wav = crate::playback::tests::wav();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",wav.len()).unwrap();
            socket.write_all(&wav).unwrap();
        });
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut settings = Settings {
            ffmpeg_path: binary,
            speed: 2.0,
            equalizer_enabled: true,
            equalizer_preamp: -3.0,
            compressor: crate::dsp::Compressor {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        settings.equalizer[5] = 6.0;
        settings.compressor.makeup = 30.;
        settings.compressor.knee = 10.;
        let file = runtime.block_on(prepare(&url, &settings, 1.0)).unwrap();
        worker.join().unwrap();
        let mut decoder =
            rodio::Decoder::try_from(std::fs::File::open(&file.path).unwrap()).unwrap();
        assert!(decoder.total_duration().unwrap().as_secs_f64() < 0.8);
        decoder.try_seek(Duration::from_millis(250)).unwrap();
        assert!(decoder.count() > 0);
        let path = file.path.clone();
        drop(file);
        assert!(!path.exists());
    }
    #[test]
    fn range_service_supports_seek_head_and_opaque_capabilities() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let proxy = Proxy::new(&runtime).unwrap();
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("fixture.mp3");
        std::fs::write(&path, b"0123456789").unwrap();
        let url = proxy.insert(AudioFile {
            path: path.clone(),
            length: 10,
        });
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let response = client
            .get(&url)
            .header("Range", "bytes=3-6")
            .send()
            .unwrap();
        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()["Content-Range"], "bytes 3-6/10");
        assert_eq!(response.text().unwrap(), "3456");
        let response = client.head(&url).send().unwrap();
        assert_eq!(response.headers()["Content-Length"], "10");
        assert!(response.bytes().unwrap().is_empty());
        assert_eq!(
            client
                .get(&url)
                .header("Range", "bytes=-2")
                .send()
                .unwrap()
                .text()
                .unwrap(),
            "89"
        );
        assert_eq!(
            client
                .get(&url)
                .header("Range", "bytes=20-")
                .send()
                .unwrap()
                .status(),
            416
        );
        assert_eq!(
            client
                .get(format!("http://127.0.0.1:{}/audio/invalid", proxy.port))
                .send()
                .unwrap()
                .status(),
            404
        );
        drop(proxy);
        drop(runtime);
        assert!(!path.exists());
        let mut settings = Settings {
            speed: 0.25,
            ..Default::default()
        };
        assert_eq!(filters(&settings, 1.0), "atempo=0.5,atempo=0.500000");
        settings.speed = 4.0;
        assert_eq!(filters(&settings, 1.0), "atempo=2,atempo=2.000000");
        settings.preserve_pitch = false;
        assert!(filters(&settings, 0.5).contains("asetrate=192000"));
    }
}
