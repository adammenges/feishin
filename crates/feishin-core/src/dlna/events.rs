//! UPnP GENA event subscriptions. Position polling remains a fallback for incomplete devices.
use super::{Device, Group, address, field, parse_groups, xml};
use anyhow::{Context as _, Result, ensure};
use axum::{
    Router,
    body::Bytes,
    extract::{ConnectInfo, DefaultBodyLimit, Path, State},
    http::{HeaderMap, Method, StatusCode},
    routing::any,
};
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr, TcpListener, UdpSocket},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

#[derive(Clone, Debug, Default)]
pub struct Delta {
    pub transport: Option<String>,
    pub uri: Option<String>,
    pub next_uri: Option<String>,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub volume: Option<u8>,
    pub groups: Option<Vec<Group>>,
}

pub fn parse(input: &str) -> Result<Delta> {
    let doc = xml(input)?;
    let mut delta = if let Some(change) =
        field(doc.root(), "LastChange").filter(|text| !text.trim().is_empty())
    {
        parse_changes(change)?
    } else {
        parse_changes(input)?
    };
    if let Some(groups) = field(doc.root(), "ZoneGroupState").filter(|text| !text.trim().is_empty())
    {
        delta.groups = Some(parse_groups(groups)?);
    }
    Ok(delta)
}
fn parse_changes(input: &str) -> Result<Delta> {
    let doc = xml(input)?;
    let root = doc
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "InstanceID"
                && node.attribute("val") == Some("0")
        })
        .unwrap_or(doc.root());
    let mut delta = Delta::default();
    for node in root.descendants().filter(|node| node.is_element()) {
        if node.ancestors().any(|parent| {
            parent.is_element()
                && parent.tag_name().name() == "InstanceID"
                && parent.attribute("val") != Some("0")
        }) {
            continue;
        }
        let Some(value) = node.attribute("val").or_else(|| node.text()) else {
            continue;
        };
        match node.tag_name().name() {
            "TransportState" | "CurrentTransportState"
                if matches!(
                    value,
                    "PLAYING"
                        | "PAUSED_PLAYBACK"
                        | "STOPPED"
                        | "TRANSITIONING"
                        | "NO_MEDIA_PRESENT"
                ) =>
            {
                delta.transport = Some(value.into())
            }
            "AVTransportURI" | "CurrentTrackURI" | "TrackURI" => delta.uri = Some(value.into()),
            "NextAVTransportURI" => delta.next_uri = Some(value.into()),
            "RelativeTimePosition" | "RelTime" => delta.position = clock(value),
            "CurrentTrackDuration" | "TrackDuration" => delta.duration = clock(value),
            "Volume" | "CurrentVolume"
                if node
                    .attribute("channel")
                    .is_none_or(|channel| channel == "Master") =>
            {
                delta.volume = value.parse::<u8>().ok().filter(|n| *n <= 100)
            }
            _ => {}
        }
    }
    Ok(delta)
}
fn clock(value: &str) -> Option<f64> {
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<u32>().ok()?;
    let minutes = parts.next()?.parse::<u32>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    (parts.next().is_none() && minutes < 60 && seconds.is_finite() && (0. ..60.).contains(&seconds))
        .then_some(hours as f64 * 3600. + minutes as f64 * 60. + seconds)
}

struct Endpoint {
    url: String,
    sid: Option<String>,
    sequence: Option<u32>,
    renew_at: Instant,
}
struct Shared {
    token: String,
    peers: HashSet<IpAddr>,
    endpoints: Mutex<Vec<Endpoint>>,
    events: SyncSender<Delta>,
}

pub struct Subscription {
    pub events: Receiver<Delta>,
    stop: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
}
impl Subscription {
    pub fn start(device: &Device) -> Result<Option<Self>> {
        let urls: Vec<_> = [
            &device.transport_event_url,
            &device.rendering_event_url,
            &device.topology_event_url,
        ]
        .into_iter()
        .filter(|url| !url.is_empty())
        .cloned()
        .collect();
        if urls.is_empty() {
            return Ok(None);
        }
        let target = address(&device.control_url)?.socket_addrs(|| None)?;
        let remote = *target.first().context("Speaker address is unavailable")?;
        let route = UdpSocket::bind(if remote.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })?;
        route.connect(remote)?;
        let listener = TcpListener::bind(SocketAddr::new(route.local_addr()?.ip(), 0))?;
        listener.set_nonblocking(true)?;
        let callback = listener.local_addr()?;
        for url in &urls {
            ensure!(
                address(url)?.host_str() == address(&device.control_url)?.host_str(),
                "Speaker event URL changed hosts"
            );
        }
        let (sender, events) = sync_channel(32);
        let shared = Arc::new(Shared {
            token: uuid::Uuid::new_v4().to_string(),
            peers: target.iter().map(|address| address.ip()).collect(),
            endpoints: Mutex::new(
                urls.into_iter()
                    .map(|url| Endpoint {
                        url,
                        sid: None,
                        sequence: None,
                        renew_at: Instant::now(),
                    })
                    .collect(),
            ),
            events: sender,
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (stop, stopped) = oneshot::channel();
        let worker = std::thread::Builder::new()
            .name("feishin-speaker-events".into())
            .spawn(move || {
                runtime.block_on(async move {
                    if let Err(error) = run(listener, callback, shared, stopped).await {
                        log::debug!("Speaker events stopped: {error}");
                    }
                });
            })?;
        Ok(Some(Self {
            events,
            stop: Some(stop),
            worker: Some(worker),
        }))
    }
    /// Explicitly wait for cleanup in protocol tests. Dropping does not stall playback.
    pub fn close(mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Speaker event worker panicked"))?;
        }
        Ok(())
    }
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

async fn notify(
    State(shared): State<Arc<Shared>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Path((token, index)): Path<(String, usize)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    if method.as_str() != "NOTIFY" {
        return StatusCode::METHOD_NOT_ALLOWED;
    }
    if token != shared.token || !shared.peers.contains(&peer.ip()) {
        return StatusCode::FORBIDDEN;
    }
    let header = |name| headers.get(name).and_then(|value| value.to_str().ok());
    if header("nt").is_none_or(|value| !value.eq_ignore_ascii_case("upnp:event"))
        || header("nts").is_none_or(|value| !value.eq_ignore_ascii_case("upnp:propchange"))
    {
        return StatusCode::PRECONDITION_FAILED;
    }
    let Some(sid) = header("sid") else {
        return StatusCode::PRECONDITION_FAILED;
    };
    let Some(sequence) = header("seq").and_then(|value| value.trim().parse::<u32>().ok()) else {
        return StatusCode::PRECONDITION_FAILED;
    };
    let Ok(text) = std::str::from_utf8(&body) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(delta) = parse(text) else {
        return StatusCode::BAD_REQUEST;
    };
    let Ok(mut endpoints) = shared.endpoints.lock() else {
        return StatusCode::INTERNAL_SERVER_ERROR;
    };
    let Some(endpoint) = endpoints.get_mut(index) else {
        return StatusCode::PRECONDITION_FAILED;
    };
    if endpoint.sid.as_deref() != Some(sid) {
        return StatusCode::PRECONDITION_FAILED;
    }
    if endpoint.sequence.is_some_and(|old| !newer(old, sequence)) {
        return StatusCode::OK;
    }
    endpoint.sequence = Some(sequence);
    let _ = shared.events.try_send(delta);
    StatusCode::OK
}
fn newer(old: u32, new: u32) -> bool {
    let delta = new.wrapping_sub(old);
    new != 0 && delta > 0 && delta < (1 << 31)
}

async fn run(
    listener: TcpListener,
    callback: SocketAddr,
    shared: Arc<Shared>,
    mut stop: oneshot::Receiver<()>,
) -> Result<()> {
    let router = Router::new()
        .route("/notify/{token}/{index}", any(notify))
        .layer(DefaultBodyLimit::max(1_000_000))
        .with_state(shared.clone());
    let server = tokio::spawn(async move {
        axum::serve(
            tokio::net::TcpListener::from_std(listener)?,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    loop {
        let due: Vec<_> = shared
            .endpoints
            .lock()
            .unwrap()
            .iter_mut()
            .enumerate()
            .filter_map(|(index, endpoint)| {
                if endpoint.renew_at > Instant::now() {
                    return None;
                }
                endpoint.renew_at = Instant::now() + Duration::from_secs(10);
                Some((index, endpoint.url.clone(), endpoint.sid.clone()))
            })
            .collect();
        for (index, url, sid) in due {
            let callback_url = format!("http://{callback}/notify/{}/{index}", shared.token);
            let subscription = subscribe(&client, &url, sid.as_deref(), &callback_url);
            let result = tokio::select! {
                _=&mut stop => { server.abort(); unsubscribe(&client,&shared).await; return Ok(()); }
                result=subscription => result,
            };
            let mut endpoints = shared.endpoints.lock().unwrap();
            let endpoint = &mut endpoints[index];
            match result {
                Ok((sid, timeout)) => {
                    if endpoint.sid.as_ref() != Some(&sid) {
                        endpoint.sequence = None;
                    }
                    endpoint.sid = Some(sid);
                    endpoint.renew_at = Instant::now() + timeout;
                }
                Err(error) => {
                    log::debug!("Speaker subscription unavailable: {error}");
                    endpoint.sid = None;
                    endpoint.sequence = None;
                    if sid.is_some() {
                        endpoint.renew_at = Instant::now() + Duration::from_secs(1);
                    }
                }
            }
        }
        tokio::select! { _=&mut stop=>break, _=tokio::time::sleep(Duration::from_millis(100))=>{} }
    }
    server.abort();
    unsubscribe(&client, &shared).await;
    Ok(())
}

async fn subscribe(
    client: &reqwest::Client,
    url: &str,
    sid: Option<&str>,
    callback: &str,
) -> Result<(String, Duration)> {
    let mut request = client
        .request(Method::from_bytes(b"SUBSCRIBE")?, address(url)?)
        .header("TIMEOUT", "Second-1800");
    if let Some(sid) = sid {
        request = request.header("SID", sid);
    } else {
        request = request
            .header("CALLBACK", format!("<{callback}>"))
            .header("NT", "upnp:event");
    }
    let response = request.send().await?.error_for_status()?;
    let new_sid = response
        .headers()
        .get("sid")
        .and_then(|value| value.to_str().ok())
        .or(sid)
        .context("Speaker returned no subscription SID")?
        .to_owned();
    ensure!(
        new_sid.starts_with("uuid:") && new_sid.len() <= 512,
        "Invalid subscription SID"
    );
    let seconds = response
        .headers()
        .get("timeout")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Second-"))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(1800)
        .clamp(1, 86400);
    Ok((new_sid, Duration::from_secs_f64(seconds as f64 * 0.8)))
}
async fn unsubscribe(client: &reqwest::Client, shared: &Shared) {
    let endpoints: Vec<_> = shared
        .endpoints
        .lock()
        .unwrap()
        .iter()
        .filter_map(|endpoint| endpoint.sid.clone().map(|sid| (endpoint.url.clone(), sid)))
        .collect();
    for (url, sid) in endpoints {
        if let Err(error) = client
            .request(Method::from_bytes(b"UNSUBSCRIBE").unwrap(), url)
            .header("SID", sid)
            .send()
            .await
        {
            log::debug!("Speaker unsubscribe failed: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_xml_preserves_namespace_instances_channels_and_serial_wrap() {
        let input =
            "<e:propertyset xmlns:e='urn:schemas-upnp-org:event-1-0'><e:property><LastChange>"
                .to_owned()
                + &super::super::escape(
                    "<Event><InstanceID val='0'><TransportState val='PLAYING'/><AVTransportURI val='http://host/next?a=1&amp;b=2'/><RelativeTimePosition val='0:00:12.5'/><CurrentTrackDuration val='0:01:30'/><Volume channel='Master' val='42'/><Volume channel='LF' val='99'/></InstanceID><InstanceID val='1'><TransportState val='STOPPED'/><Volume channel='Master' val='12'/></InstanceID></Event>",
                )
                + "</LastChange></e:property></e:propertyset>";
        let delta = parse(&input).unwrap();
        assert_eq!(delta.transport.as_deref(), Some("PLAYING"));
        assert_eq!(delta.uri.as_deref(), Some("http://host/next?a=1&b=2"));
        assert_eq!(delta.position, Some(12.5));
        assert_eq!(delta.duration, Some(90.));
        assert_eq!(delta.volume, Some(42));
        let delta=parse("<Event><Volume val='201'/><RelTime val='0:00:NaN'/><CurrentTransportState>INVALID</CurrentTransportState></Event>").unwrap();
        assert!(delta.volume.is_none() && delta.position.is_none() && delta.transport.is_none());
        assert!(parse("<!DOCTYPE x [<!ENTITY x 'bad'>]><x>&x;</x>").is_err());
        assert!(newer(0, 1) && newer(u32::MAX, 1));
        assert!(!newer(1, 1) && !newer(2, 1) && !newer(u32::MAX, 0) && !newer(1, u32::MAX));
    }

    #[derive(Default)]
    struct Fixture {
        callback: Option<String>,
        sid: String,
        initial: usize,
        renewed: usize,
        unsubscribed: usize,
        expire_once: bool,
    }
    async fn fixture(
        State(state): State<Arc<Mutex<Fixture>>>,
        method: Method,
        headers: HeaderMap,
    ) -> (StatusCode, HeaderMap) {
        let mut state = state.lock().unwrap();
        let mut output = HeaderMap::new();
        let get = |name| headers.get(name).and_then(|value| value.to_str().ok());
        match method.as_str() {
            "SUBSCRIBE" if get("callback").is_some() => {
                assert_eq!(get("nt"), Some("upnp:event"));
                assert_eq!(get("timeout"), Some("Second-1800"));
                assert!(get("sid").is_none());
                state.initial += 1;
                state.callback = Some(
                    get("callback")
                        .unwrap()
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .to_owned(),
                );
                state.sid = format!("uuid:fixture-{}", state.initial);
            }
            "SUBSCRIBE" => {
                assert_eq!(get("sid"), Some(state.sid.as_str()));
                assert!(get("nt").is_none() && get("callback").is_none());
                state.renewed += 1;
                if state.expire_once {
                    state.expire_once = false;
                    return (StatusCode::PRECONDITION_FAILED, output);
                }
            }
            "UNSUBSCRIBE" => {
                assert_eq!(get("sid"), Some(state.sid.as_str()));
                state.unsubscribed += 1;
            }
            _ => return (StatusCode::METHOD_NOT_ALLOWED, output),
        }
        output.insert("sid", state.sid.parse().unwrap());
        output.insert("timeout", "Second-1".parse().unwrap());
        (StatusCode::OK, output)
    }
    fn until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(4);
        while !condition() {
            assert!(
                Instant::now() < deadline,
                "Timed out waiting for subscription fixture"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn real_gena_subscribe_notify_renew_resubscribe_and_shutdown() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fixture_state = Arc::new(Mutex::new(Fixture {
            expire_once: true,
            ..Default::default()
        }));
        let server_state = fixture_state.clone();
        let (finish, finished) = oneshot::channel();
        let server = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let app = Router::new()
                        .route("/events", any(fixture))
                        .with_state(server_state);
                    axum::serve(tokio::net::TcpListener::from_std(listener).unwrap(), app)
                        .with_graceful_shutdown(async {
                            let _ = finished.await;
                        })
                        .await
                        .unwrap();
                });
        });
        let description = String::from(
            "<root><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType><friendlyName>Fixture</friendlyName><UDN>uuid:test</UDN><serviceList><service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/control</controlURL><eventSubURL>/events</eventSubURL></service></serviceList></device></root>",
        );
        let device =
            super::super::parse_device(&description, &format!("{base}/description.xml")).unwrap();
        assert_eq!(device.transport_event_url, format!("{base}/events"));
        assert!(
            super::super::parse_device(
                &description.replace("/events", "http://other/events"),
                &format!("{base}/description.xml")
            )
            .is_err()
        );
        let subscription = Subscription::start(&device).unwrap().unwrap();
        until(|| fixture_state.lock().unwrap().initial == 1);
        let (callback, sid) = {
            let state = fixture_state.lock().unwrap();
            (state.callback.clone().unwrap(), state.sid.clone())
        };
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let body = "<e:propertyset xmlns:e='urn:schemas-upnp-org:event-1-0'><e:property><CurrentVolume>42</CurrentVolume></e:property></e:propertyset>";
        let send = |url: &str, sid: &str, sequence: u32, body: &str| {
            client
                .request(Method::from_bytes(b"NOTIFY").unwrap(), url)
                .header("NT", "upnp:event")
                .header("NTS", "upnp:propchange")
                .header("SID", sid)
                .header("SEQ", sequence)
                .body(body.to_owned())
                .send()
                .unwrap()
                .status()
        };
        // A device may send the first notification as its SUBSCRIBE response is being processed.
        until(|| send(&callback, &sid, 0, body) == StatusCode::OK);
        assert_eq!(
            subscription
                .events
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .volume,
            Some(42)
        );
        assert_eq!(send(&callback, &sid, 0, body), StatusCode::OK);
        assert!(subscription.events.try_recv().is_err());
        assert_eq!(
            send(&callback, "uuid:other", 1, body),
            StatusCode::PRECONDITION_FAILED
        );
        assert_eq!(
            send(
                &callback.replace("/notify/", "/notify/wrong-"),
                &sid,
                1,
                body
            ),
            StatusCode::FORBIDDEN
        );
        assert_eq!(send(&callback, &sid, 1, "bad xml"), StatusCode::BAD_REQUEST);
        assert_eq!(send(&callback, &sid, 1, body), StatusCode::OK);
        subscription
            .events
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        assert_eq!(send(&callback, &sid, 0, body), StatusCode::OK);
        assert!(subscription.events.try_recv().is_err());
        assert_eq!(
            send(&callback, &sid, 2, &"x".repeat(1_000_001)),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        until(|| fixture_state.lock().unwrap().initial >= 2);
        let sid = fixture_state.lock().unwrap().sid.clone();
        until(|| send(&callback, &sid, 0, body) == StatusCode::OK);
        assert_eq!(
            subscription
                .events
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .volume,
            Some(42)
        );
        assert!(fixture_state.lock().unwrap().renewed >= 1);
        subscription.close().unwrap();
        assert_eq!(fixture_state.lock().unwrap().unsubscribed, 1);
        assert!(client.get(&callback).send().is_err());
        finish.send(()).unwrap();
        server.join().unwrap();
    }
}
