use feishin_core::dlna::Device;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

pub(crate) struct Speaker {
    pub address: String,
    pub calls: Arc<Mutex<Vec<Value>>>,
    pub hold: Arc<AtomicBool>,
    pub fail: Arc<AtomicBool>,
    pub properties: Arc<Mutex<feishin_core::dlna::Properties>>,
    pub snapshot: Arc<Mutex<feishin_core::dlna::Status>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Speaker {
    pub fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let hold = Arc::new(AtomicBool::new(false));
        let fail = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::new(Mutex::new(feishin_core::dlna::Status::default()));
        let state = snapshot.clone();
        let properties = Arc::new(Mutex::new(feishin_core::dlna::Properties {
            bass: Some(0),
            treble: Some(0),
            loudness: Some(true),
            crossfade: Some(false),
            led: Some(true),
            touch: Some(true),
        }));
        let property_state = properties.clone();
        let (record, gate, failure, shutdown) =
            (calls.clone(), hold.clone(), fail.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            while !shutdown.load(Ordering::SeqCst) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut bytes = [0; 8192];
                    let count = stream.read(&mut bytes).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&bytes[..count]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                        let length: usize = headers
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                let text = String::from_utf8(request).unwrap();
                let id = text
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .trim_start_matches('/');
                let body = text.split_once("\r\n\r\n").unwrap().1;
                let value = |name: &str| {
                    body.split_once(&format!("<{name}>"))
                        .and_then(|(_, value)| value.split_once('<').map(|(value, _)| value))
                };
                let action = body
                    .split("<u:")
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap();
                let mut snapshot = state.lock().unwrap();
                let time = |value: f64| {
                    format!(
                        "{}:{:02}:{:06.3}",
                        value as u64 / 3600,
                        value as u64 / 60 % 60,
                        value % 60.
                    )
                };
                let mut properties = property_state.lock().unwrap();
                let response = match action {
                    "GetBass" => format!(
                        "<CurrentBass>{}</CurrentBass>",
                        properties.bass.unwrap_or(0)
                    ),
                    "GetTreble" => format!(
                        "<CurrentTreble>{}</CurrentTreble>",
                        properties.treble.unwrap_or(0)
                    ),
                    "GetLoudness" => format!(
                        "<CurrentLoudness>{}</CurrentLoudness>",
                        u8::from(properties.loudness.unwrap_or(false))
                    ),
                    "GetCrossfadeMode" => format!(
                        "<CrossfadeMode>{}</CrossfadeMode>",
                        u8::from(properties.crossfade.unwrap_or(false))
                    ),
                    "GetLEDState" => format!(
                        "<CurrentLEDState>{}</CurrentLEDState>",
                        if properties.led.unwrap_or(false) {
                            "On"
                        } else {
                            "Off"
                        }
                    ),
                    "GetButtonLockState" => format!(
                        "<CurrentButtonLockState>{}</CurrentButtonLockState>",
                        if properties.touch.unwrap_or(false) {
                            "Off"
                        } else {
                            "On"
                        }
                    ),
                    "GetPositionInfo" => format!(
                        "<Position><RelTime>{}</RelTime><TrackDuration>{}</TrackDuration><TrackURI>{}</TrackURI></Position>",
                        time(snapshot.position),
                        time(snapshot.duration),
                        feishin_core::dlna::escape(&snapshot.uri)
                    ),
                    "GetTransportInfo" => format!(
                        "<Transport><CurrentTransportState>{}</CurrentTransportState></Transport>",
                        snapshot.transport
                    ),
                    "GetVolume" => format!(
                        "<Volume><CurrentVolume>{}</CurrentVolume></Volume>",
                        snapshot.volume.unwrap_or(50)
                    ),
                    "GetMediaInfo" => format!(
                        "<Media><NextURI>{}</NextURI></Media>",
                        feishin_core::dlna::escape(&snapshot.next_uri)
                    ),
                    _ => "<ok/>".into(),
                };
                let call = if let Some(bass) = value("DesiredBass") {
                    properties.bass = Some(bass.parse().unwrap());
                    json!(["bass", id, properties.bass])
                } else if let Some(treble) = value("DesiredTreble") {
                    properties.treble = Some(treble.parse().unwrap());
                    json!(["treble", id, properties.treble])
                } else if let Some(enabled) = value("DesiredLoudness") {
                    properties.loudness = Some(enabled == "1");
                    json!(["loudness", id, properties.loudness])
                } else if let Some(enabled) = value("CrossfadeMode") {
                    properties.crossfade = Some(enabled == "1");
                    json!(["crossfade", id, properties.crossfade])
                } else if let Some(enabled) = value("DesiredLEDState") {
                    properties.led = Some(enabled == "On");
                    json!(["led", id, properties.led])
                } else if let Some(locked) = value("DesiredButtonLockState") {
                    properties.touch = Some(locked == "Off");
                    json!(["touch", id, properties.touch])
                } else if let Some(volume) = value("DesiredVolume") {
                    let volume = volume.parse::<u8>().unwrap();
                    snapshot.volume = Some(volume);
                    json!(["volume", id, volume])
                } else if let Some(muted) = value("DesiredMute") {
                    json!(["mute", id, muted == "1"])
                } else {
                    json!([action, id])
                };
                drop(snapshot);
                drop(properties);
                record.lock().unwrap().push(call);
                while gate.load(Ordering::SeqCst) && !shutdown.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                let status = if failure.load(Ordering::SeqCst) {
                    "500 Error"
                } else {
                    "200 OK"
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response.len(),
                    response
                );
            }
        });
        Self {
            address,
            snapshot,
            properties,
            calls,
            hold,
            fail,
            stop,
            worker: Some(worker),
        }
    }
    pub fn device(&self, id: &str) -> Device {
        Device {
            id: format!("uuid:{id}"),
            name: format!("Speaker {id}"),
            location: format!("{}/{id}", self.address),
            control_url: format!("{}/{id}", self.address),
            rendering_url: format!("{}/{id}", self.address),
            transport_service: "urn:schemas-upnp-org:service:AVTransport:1".into(),
            rendering_service: "urn:schemas-upnp-org:service:RenderingControl:1".into(),
            sonos: true,
            transport_event_url: String::new(),
            rendering_event_url: String::new(),
            topology_event_url: String::new(),
        }
    }
}
impl Drop for Speaker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.hold.store(false, Ordering::SeqCst);
        self.worker.take().unwrap().join().unwrap();
    }
}
