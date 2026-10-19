//! Disposable loopback HTTPS tests. The embedded certificate/key are public test fixtures.
use crate::{
    api::Api,
    model::{Server, ServerType},
    playback,
    storage::Settings,
};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

struct Https {
    child: Child,
    dir: PathBuf,
    url: String,
}
impl Drop for Https {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
        std::fs::remove_dir_all(&self.dir).ok();
    }
}
impl Https {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("feishin-https-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(
            dir.join("cert.pem"),
            include_bytes!("../assets/native-test-cert.pem"),
        )
        .unwrap();
        std::fs::write(
            dir.join("key.pem"),
            include_bytes!("../assets/native-test-key.pem"),
        )
        .unwrap();
        std::fs::write(dir.join("audio.wav"), playback::tests::wav()).unwrap();
        let script = r#"
import http.server, json, pathlib, ssl, sys
root = pathlib.Path(sys.argv[1])
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        audio = self.path.startswith('/audio.wav') or self.path.startswith('/rest/stream.view')
        data = (root / 'audio.wav').read_bytes() if audio else json.dumps({'subsonic-response': {'status': 'ok', 'version': '1.16.1'}}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'audio/wav' if audio else 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        try: self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError): pass
    def log_message(self, *args): pass
server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(root / 'cert.pem', root / 'key.pem')
server.socket = context.wrap_socket(server.socket, server_side=True)
print(server.server_address[1], flush=True)
server.serve_forever()
"#;
        let spawn = |binary: &str| {
            Command::new(binary)
                .args(["-u", "-c", script])
                .arg(&dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
        };
        let child = spawn("python3")
            .or_else(|_| spawn("python"))
            .expect("Python is required for the local HTTPS fixture");
        let mut fixture = Self {
            child,
            dir,
            url: String::new(),
        };
        let mut port = String::new();
        BufReader::new(fixture.child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        let port: u16 = port.trim().parse().expect("HTTPS fixture did not start");
        fixture.url = format!("https://127.0.0.1:{port}");
        fixture
    }
}

#[test]
fn self_signed_https_is_rejected_by_default_and_opt_in_applies_to_api_audio_and_speaker_proxy() {
    let fixture = Https::new();
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "TLS fixture".into(),
        url: fixture.url.clone(),
        username: "fixture".into(),
        kind: ServerType::Subsonic,
        legacy_auth: false,
        options: Default::default(),
    };
    let strict = Api::new(server.clone(), Default::default()).unwrap();
    assert!(strict.subsonic("ping", &[]).is_err());
    assert!(!strict.is_signed_out());
    let mut opted_in = server.clone();
    opted_in.options.ignore_ssl = true;
    let opted_in = Api::new(opted_in, Default::default()).unwrap();
    assert_eq!(opted_in.subsonic("ping", &[]).unwrap()["status"], "ok");
    let reverted = opted_in.reconfigure(server).unwrap();
    assert!(reverted.subsonic("ping", &[]).is_err());
    assert_eq!(opted_in.subsonic("ping", &[]).unwrap()["status"], "ok");
    assert!(strict.subsonic("ping", &[]).is_err());

    let url = format!("{}/audio.wav", fixture.url);
    let cancel = || Arc::new(AtomicBool::new(false));
    assert!(playback::waveform(&url, cancel(), false).is_err());
    let waveform = playback::waveform(&url, cancel(), true).unwrap();
    assert_eq!(waveform.duration, 1.);
    assert!(!waveform.peaks.is_empty());
    assert!(playback::waveform(&url, cancel(), false).is_err());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let binary = std::env::var("FEISHIN_TEST_FFMPEG").unwrap_or_else(|_| "ffmpeg".into());
    if Command::new(&binary)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        let mut settings = Settings {
            ffmpeg_path: binary,
            ..Default::default()
        };
        assert!(
            runtime
                .block_on(crate::proxy::prepare(&url, &settings, 1.))
                .is_err()
        );
        settings.ignore_ssl = true;
        let audio = runtime
            .block_on(crate::proxy::prepare(&url, &settings, 1.))
            .unwrap();
        assert!(audio.length > 0);
        settings.ignore_ssl = false;
        assert!(
            runtime
                .block_on(crate::proxy::prepare(&url, &settings, 1.))
                .is_err()
        );
    } else {
        assert!(
            std::env::var("FEISHIN_TEST_FFMPEG").is_err(),
            "Configured FFmpeg is missing"
        );
    }
}

#[test]
fn mpv_https_uses_the_opt_in_certificate_policy_without_an_audio_device() {
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
            "Configured MPV is missing"
        );
        return;
    }
    let fixture = Https::new();
    let player = playback::Player::new();
    for ignore_ssl in [false, true, false] {
        let id = player.send(playback::Control::Load {
            url: format!("{}/audio.wav", fixture.url),
            settings: Box::new(Settings {
                ignore_ssl,
                mpv_path: binary.clone(),
                mpv_extra_args: vec!["--ao=null".into(), "--vo=null".into()],
                fade_on_status: false,
                ..Default::default()
            }),
            position: 0.,
            gain: 1.,
            item: Box::new(crate::model::Item {
                duration: 1.,
                ..Default::default()
            }),
            key: format!("tls-{ignore_ssl}"),
        });
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let event = player
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("MPV TLS result was not reported");
            match event {
                playback::Event::Started(generation) if generation == id => {
                    assert!(ignore_ssl, "MPV accepted an untrusted certificate");
                    break;
                }
                playback::Event::Error(generation, _) if generation == id => {
                    assert!(
                        !ignore_ssl,
                        "MPV rejected the enabled certificate exception"
                    );
                    break;
                }
                _ => {}
            }
        }
        player.send(playback::Control::Stop);
    }
}
