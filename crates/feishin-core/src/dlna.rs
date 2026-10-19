use crate::model::Item;
use anyhow::{Context, Result, bail, ensure};
use reqwest::blocking::Client;
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    net::{Ipv4Addr, UdpSocket},
    time::{Duration, Instant},
};
use url::Url;

const AVT: &str = "urn:schemas-upnp-org:service:AVTransport:";
const RC: &str = "urn:schemas-upnp-org:service:RenderingControl:";
const DP: &str = "urn:schemas-upnp-org:service:DeviceProperties:1";
const TOPOLOGY: &str = "urn:schemas-upnp-org:service:ZoneGroupTopology:1";
pub mod events;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub location: String,
    pub control_url: String,
    pub rendering_url: String,
    pub transport_service: String,
    pub rendering_service: String,
    pub sonos: bool,
    #[serde(default)]
    pub transport_event_url: String,
    #[serde(default)]
    pub rendering_event_url: String,
    #[serde(default)]
    pub topology_event_url: String,
}
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub position: f64,
    pub duration: f64,
    pub uri: String,
    pub next_uri: String,
    pub transport: String,
    pub volume: Option<u8>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub id: String,
    pub coordinator: String,
    pub stereo_pair: bool,
    pub members: Vec<Member>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub location: String,
    pub satellite: bool,
}
#[derive(Clone, Copy)]
pub enum Service {
    Transport,
    Rendering,
    Properties,
    Topology,
}
#[derive(Clone, Debug, Default)]
pub struct Properties {
    pub bass: Option<i32>,
    pub treble: Option<i32>,
    pub loudness: Option<bool>,
    pub crossfade: Option<bool>,
    pub led: Option<bool>,
    pub touch: Option<bool>,
}
#[derive(Clone, Copy)]
pub enum Property {
    Bass(i32),
    Treble(i32),
    Loudness(bool),
    Crossfade(bool),
    Led(bool),
    Touch(bool),
}

fn xml(input: &str) -> Result<Document<'_>> {
    Document::parse_with_options(
        input,
        roxmltree::ParsingOptions {
            nodes_limit: 50_000,
            ..Default::default()
        },
    )
    .context("Invalid speaker XML")
}
fn field<'a, 'i>(node: Node<'a, 'i>, key: &str) -> Option<&'a str> {
    node.descendants()
        .find(|node| node.is_element() && node.tag_name().name() == key)
        .and_then(|node| node.text())
}
fn child<'a, 'i>(node: Node<'a, 'i>, key: &str) -> Option<&'a str> {
    node.children()
        .find(|node| node.is_element() && node.tag_name().name() == key)
        .and_then(|node| node.text())
}
pub fn escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn address(input: &str) -> Result<Url> {
    let url = Url::parse(input)?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none(),
        "Invalid speaker URL"
    );
    Ok(url)
}
fn response_text(response: reqwest::blocking::Response) -> Result<String> {
    let status = response.status();
    let mut bytes = vec![];
    response.take(4_000_001).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 4_000_000, "Speaker response is too large");
    let text = String::from_utf8(bytes)?;
    if !status.is_success() {
        let code = xml(&text)
            .ok()
            .and_then(|doc| field(doc.root(), "errorCode").map(str::to_owned));
        bail!(
            "Speaker command failed (HTTP {}, UPnP {})",
            status.as_u16(),
            code.unwrap_or_else(|| "unknown".into())
        );
    }
    Ok(text)
}

pub fn parse_device(input: &str, location: &str) -> Result<Device> {
    let doc = xml(input)?;
    let root = doc.root();
    let location_url = address(location)?;
    let base = field(root, "URLBase")
        .filter(|url| !url.trim().is_empty())
        .map(address)
        .transpose()?
        .unwrap_or_else(|| location_url.clone());
    ensure!(
        base.host_str() == location_url.host_str(),
        "Speaker description changed hosts"
    );
    let renderer = root
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "device"
                && child(*node, "deviceType").is_some_and(|kind| kind.contains(":MediaRenderer:"))
        })
        .context("Device is not a media renderer")?;
    let room = child(renderer, "roomName").or_else(|| field(root, "roomName"));
    let model = child(renderer, "modelName");
    let friendly = child(renderer, "friendlyName");
    let name = match (room, model, friendly) {
        (Some(room), Some(model), _) => format!("{room} ({model})"),
        (Some(room), _, _) => room.into(),
        (_, _, Some(name)) if !name.starts_with("RINCON_") => name.into(),
        (_, Some(model), _) => model.into(),
        _ => "DLNA speaker".into(),
    };
    let service = |prefix: &str| -> Result<(String, String, String)> {
        let node = renderer
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "service"
                    && child(*node, "serviceType").is_some_and(|kind| kind.starts_with(prefix))
            })
            .context("Speaker service is missing")?;
        let url =
            base.join(child(node, "controlURL").context("Speaker control URL is missing")?)?;
        ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str() == location_url.host_str()
                && url.username().is_empty()
                && url.password().is_none(),
            "Invalid speaker control URL"
        );
        let event = child(node, "eventSubURL")
            .filter(|value| !value.is_empty())
            .map(|value| base.join(value))
            .transpose()?;
        ensure!(
            event
                .as_ref()
                .is_none_or(|url| matches!(url.scheme(), "http" | "https")
                    && url.host_str() == location_url.host_str()
                    && url.username().is_empty()
                    && url.password().is_none()),
            "Invalid speaker event URL"
        );
        Ok((
            url.to_string(),
            child(node, "serviceType").unwrap().into(),
            event.map(|url| url.to_string()).unwrap_or_default(),
        ))
    };
    let (control_url, transport_service, transport_event_url) = service(AVT)?;
    let (rendering_url, rendering_service, rendering_event_url) =
        service(RC).unwrap_or_else(|_| (control_url.clone(), format!("{RC}1"), String::new()));
    let topology_event_url = service("urn:schemas-upnp-org:service:ZoneGroupTopology:")
        .map(|(_, _, event)| event)
        .unwrap_or_default();
    let id = child(renderer, "UDN").unwrap_or(location).to_owned();
    let sonos = id.contains("RINCON_")
        || field(root, "manufacturer").is_some_and(|name| name.to_lowercase().contains("sonos"));
    Ok(Device {
        id,
        name,
        location: location.into(),
        control_url,
        rendering_url,
        transport_service,
        rendering_service,
        sonos,
        transport_event_url,
        rendering_event_url,
        topology_event_url,
    })
}
fn ssdp_location(input: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(input).ok()?;
    if !text.starts_with("HTTP/1.1 200") && !text.starts_with("NOTIFY ") {
        return None;
    }
    text.lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("location")
                .then(|| value.trim().to_owned())
        })
        .filter(|url| address(url).is_ok())
}
pub fn discover(timeout: Duration) -> Result<Vec<Device>> {
    let mut addresses = if_addrs::get_if_addrs()?
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .filter_map(|interface| match interface.ip() {
            std::net::IpAddr::V4(ip) => Some(ip),
            _ => None,
        })
        .collect::<Vec<_>>();
    addresses.sort();
    addresses.dedup();
    if addresses.is_empty() {
        addresses.push(Ipv4Addr::UNSPECIFIED);
    }
    let mut sockets = vec![];
    for ip in addresses {
        match UdpSocket::bind((ip, 0)) {
            Ok(socket) => {
                socket.set_nonblocking(true)?;
                socket.set_multicast_ttl_v4(4)?;
                if !ip.is_unspecified() {
                    socket2::SockRef::from(&socket).set_multicast_if_v4(&ip)?;
                }
                sockets.push(socket);
            }
            Err(error) => log::debug!("Speaker discovery interface unavailable: {error}"),
        }
    }
    ensure!(
        !sockets.is_empty(),
        "Could not open a speaker discovery socket"
    );
    let message=b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 3\r\nST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n";
    let start = Instant::now();
    let deadline = timeout.clamp(Duration::from_secs(1), Duration::from_secs(10));
    let mut retries = 0;
    let mut locations = HashSet::new();
    let mut bytes = [0; 16384];
    while start.elapsed() < deadline {
        if retries < 3 && start.elapsed() >= Duration::from_millis([0, 500, 1500][retries]) {
            for socket in &sockets {
                let _ = socket.send_to(message, (Ipv4Addr::new(239, 255, 255, 250), 1900));
            }
            retries += 1;
        }
        for socket in &sockets {
            loop {
                match socket.recv_from(&mut bytes) {
                    Ok((count, _)) => {
                        if locations.len() < 64
                            && let Some(location) = ssdp_location(&bytes[..count])
                        {
                            locations.insert(location);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => {
                        log::debug!("Speaker discovery receive failed: {error}");
                        break;
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let client = Controller::new()?;
    let locations = locations.into_iter().collect::<Vec<_>>();
    let mut devices = HashMap::new();
    for batch in locations.chunks(8) {
        let results = std::thread::scope(|scope| {
            batch
                .iter()
                .map(|location| {
                    let client = &client;
                    scope.spawn(move || client.describe(location))
                })
                .collect::<Vec<_>>()
                .into_iter()
                .filter_map(|worker| worker.join().ok())
                .collect::<Vec<_>>()
        });
        for result in results {
            match result {
                Ok(device) => {
                    devices.insert(device.id.clone(), device);
                }
                Err(error) => log::debug!("Speaker description unavailable: {error}"),
            }
        }
    }
    let mut devices = devices.into_values().collect::<Vec<_>>();
    devices.sort_by_key(|device| device.name.to_lowercase());
    Ok(devices)
}

#[derive(Clone)]
pub struct Controller {
    client: Client,
}
impl Controller {
    pub fn lan_url(&self, device: &Device, input: &str) -> Result<String> {
        let mut url = address(input)?;
        if matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
        ) {
            let target = address(&device.control_url)?
                .socket_addrs(|| None)?
                .into_iter()
                .next()
                .context("Speaker address is unavailable")?;
            let socket = UdpSocket::bind(if target.is_ipv4() {
                "0.0.0.0:0"
            } else {
                "[::]:0"
            })?;
            socket.connect(target)?;
            url.set_host(Some(&socket.local_addr()?.ip().to_string()))?;
        }
        Ok(url.to_string())
    }
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(15))
                .redirect(reqwest::redirect::Policy::none())
                .build()?,
        })
    }
    pub fn describe(&self, location: &str) -> Result<Device> {
        let text = response_text(
            self.client
                .get(address(location)?)
                .timeout(Duration::from_secs(4))
                .send()
                .map_err(|e| e.without_url())?,
        )?;
        parse_device(&text, location)
    }
    fn request(
        &self,
        device: &Device,
        service: Service,
        action: &str,
        params: &[(&str, String)],
        poll: bool,
    ) -> Result<String> {
        ensure!(
            action.chars().all(|c| c.is_ascii_alphanumeric()),
            "Invalid speaker action"
        );
        let (url, urn) = match service {
            Service::Transport => (device.control_url.clone(), device.transport_service.clone()),
            Service::Rendering => (
                device.rendering_url.clone(),
                device.rendering_service.clone(),
            ),
            Service::Properties => (self.sonos_url(device, "DeviceProperties")?, DP.into()),
            Service::Topology => (
                self.sonos_url(device, "ZoneGroupTopology")?,
                TOPOLOGY.into(),
            ),
        };
        let mut body = String::new();
        if matches!(service, Service::Transport | Service::Rendering) {
            body.push_str("<InstanceID>0</InstanceID>");
        }
        for (key, value) in params {
            ensure!(
                key.chars().all(|c| c.is_ascii_alphanumeric()),
                "Invalid speaker parameter"
            );
            body.push_str(&format!("<{key}>{}</{key}>", escape(value)));
        }
        let payload = format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{}\">{body}</u:{action}></s:Body></s:Envelope>",
            escape(&urn)
        );
        let response = self
            .client
            .post(address(&url)?)
            .timeout(Duration::from_secs(if poll { 4 } else { 15 }))
            .header("Content-Type", "text/xml; charset=\"utf-8\"")
            .header("SOAPAction", format!("\"{urn}#{action}\""))
            .body(payload)
            .send()
            .map_err(|e| e.without_url())?;
        let text = response_text(response)?;
        let doc = xml(&text)?;
        ensure!(
            !doc.descendants()
                .any(|node| node.is_element() && node.tag_name().name() == "Fault"),
            "Speaker rejected {action}"
        );
        Ok(text)
    }
    fn sonos_url(&self, device: &Device, service: &str) -> Result<String> {
        let mut url = address(&device.location)?;
        url.set_path(&format!("/{service}/Control"));
        url.set_query(None);
        Ok(url.to_string())
    }
    pub fn play(&self, device: &Device) -> Result<()> {
        self.request(
            device,
            Service::Transport,
            "Play",
            &[("Speed", "1".into())],
            false,
        )?;
        Ok(())
    }
    pub fn pause(&self, device: &Device) -> Result<()> {
        self.request(device, Service::Transport, "Pause", &[], false)?;
        Ok(())
    }
    pub fn stop(&self, device: &Device) -> Result<()> {
        self.request(device, Service::Transport, "Stop", &[], false)?;
        Ok(())
    }
    pub fn seek(&self, device: &Device, position: f64) -> Result<()> {
        ensure!(
            position.is_finite() && position >= 0.0,
            "Invalid speaker seek position"
        );
        self.request(
            device,
            Service::Transport,
            "Seek",
            &[("Unit", "REL_TIME".into()), ("Target", duration(position))],
            false,
        )?;
        Ok(())
    }
    pub fn get_volume(&self, device: &Device) -> Result<u8> {
        let response = self.request(
            device,
            Service::Rendering,
            "GetVolume",
            &[("Channel", "Master".into())],
            true,
        )?;
        let doc = xml(&response)?;
        let volume: u8 = field(doc.root(), "CurrentVolume")
            .context("Speaker returned no volume")?
            .parse()
            .context("Speaker returned an invalid volume")?;
        ensure!(volume <= 100, "Speaker volume must be between 0 and 100");
        Ok(volume)
    }
    pub fn volume(&self, device: &Device, volume: u8) -> Result<()> {
        self.request(
            device,
            Service::Rendering,
            "SetVolume",
            &[
                ("Channel", "Master".into()),
                ("DesiredVolume", volume.min(100).to_string()),
            ],
            false,
        )?;
        Ok(())
    }
    pub fn mute(&self, device: &Device, mute: bool) -> Result<()> {
        self.request(
            device,
            Service::Rendering,
            "SetMute",
            &[
                ("Channel", "Master".into()),
                ("DesiredMute", u8::from(mute).to_string()),
            ],
            false,
        )?;
        Ok(())
    }
    pub fn load(&self, device: &Device, url: &str, item: &Item, next: bool) -> Result<()> {
        address(url)?;
        let (action, uri, metadata) = if next {
            ("SetNextAVTransportURI", "NextURI", "NextURIMetaData")
        } else {
            ("SetAVTransportURI", "CurrentURI", "CurrentURIMetaData")
        };
        self.request(
            device,
            Service::Transport,
            action,
            &[(uri, url.into()), (metadata, didl(item, url))],
            false,
        )?;
        Ok(())
    }
    pub fn clear_next(&self, device: &Device) -> Result<()> {
        self.request(
            device,
            Service::Transport,
            "SetNextAVTransportURI",
            &[
                ("NextURI", String::new()),
                ("NextURIMetaData", String::new()),
            ],
            false,
        )?;
        Ok(())
    }
    pub fn position(&self, device: &Device) -> Result<Status> {
        let text = self.request(device, Service::Transport, "GetPositionInfo", &[], true)?;
        let doc = xml(&text)?;
        Ok(Status {
            position: parse_time(field(doc.root(), "RelTime").unwrap_or_default()),
            duration: parse_time(field(doc.root(), "TrackDuration").unwrap_or_default()),
            uri: field(doc.root(), "TrackURI").unwrap_or_default().into(),
            ..Default::default()
        })
    }
    pub fn status(&self, device: &Device) -> Result<Status> {
        let mut status = self.position(device)?;
        if let Ok(text) = self.request(device, Service::Transport, "GetTransportInfo", &[], true) {
            let doc = xml(&text)?;
            status.transport = field(doc.root(), "CurrentTransportState")
                .unwrap_or_default()
                .into();
        }
        status.volume = self.get_volume(device).ok();
        Ok(status)
    }
    pub fn connection_status(&self, device: &Device) -> Result<Status> {
        let mut status = self.status(device).or_else(|error| {
            self.get_volume(device)
                .map(|volume| Status {
                    volume: Some(volume),
                    ..Default::default()
                })
                .map_err(|_| error)
        })?;
        if let Ok(text) = self.request(device, Service::Transport, "GetMediaInfo", &[], true)
            && let Ok(doc) = xml(&text)
        {
            status.next_uri = field(doc.root(), "NextURI").unwrap_or_default().into();
        }
        Ok(status)
    }
    pub fn groups(&self, device: &Device) -> Result<Vec<Group>> {
        let response = self.request(device, Service::Topology, "GetZoneGroupState", &[], true)?;
        let doc = xml(&response)?;
        parse_groups(
            field(doc.root(), "ZoneGroupState").context("Speaker returned no group topology")?,
        )
    }
    pub fn join(&self, member: &Device, coordinator: &Device) -> Result<()> {
        ensure!(
            member.sonos && coordinator.sonos && member.id != coordinator.id,
            "Choose two different Sonos speakers"
        );
        self.request(
            member,
            Service::Transport,
            "SetAVTransportURI",
            &[
                (
                    "CurrentURI",
                    format!("x-rincon:{}", coordinator.id.trim_start_matches("uuid:")),
                ),
                ("CurrentURIMetaData", String::new()),
            ],
            false,
        )?;
        Ok(())
    }
    pub fn leave(&self, member: &Device) -> Result<()> {
        self.request(
            member,
            Service::Transport,
            "BecomeCoordinatorOfStandaloneGroup",
            &[],
            false,
        )?;
        Ok(())
    }
    pub fn properties(&self, device: &Device) -> Properties {
        let get = |service, action, key| {
            self.request(device, service, action, &[], true)
                .ok()
                .and_then(|text| {
                    xml(&text)
                        .ok()
                        .and_then(|doc| field(doc.root(), key).map(str::to_owned))
                })
        };
        let get_channel = |action, key| {
            self.request(
                device,
                Service::Rendering,
                action,
                &[("Channel", "Master".into())],
                true,
            )
            .ok()
            .and_then(|text| {
                xml(&text)
                    .ok()
                    .and_then(|doc| field(doc.root(), key).map(str::to_owned))
            })
        };
        let boolean = |value: String| matches!(value.to_lowercase().as_str(), "1" | "true" | "on");
        Properties {
            bass: get(Service::Rendering, "GetBass", "CurrentBass").and_then(|n| n.parse().ok()),
            treble: get(Service::Rendering, "GetTreble", "CurrentTreble")
                .and_then(|n| n.parse().ok()),
            loudness: get_channel("GetLoudness", "CurrentLoudness").map(boolean),
            crossfade: get(Service::Transport, "GetCrossfadeMode", "CrossfadeMode").map(boolean),
            led: get(Service::Properties, "GetLEDState", "CurrentLEDState").map(boolean),
            touch: get(
                Service::Properties,
                "GetButtonLockState",
                "CurrentButtonLockState",
            )
            .map(|value| value.eq_ignore_ascii_case("off")),
        }
    }
    pub fn set_property(&self, device: &Device, property: Property) -> Result<()> {
        let (service, action, key, value) = match property {
            Property::Bass(value) => (
                Service::Rendering,
                "SetBass",
                "DesiredBass",
                value.clamp(-10, 10).to_string(),
            ),
            Property::Treble(value) => (
                Service::Rendering,
                "SetTreble",
                "DesiredTreble",
                value.clamp(-10, 10).to_string(),
            ),
            Property::Loudness(value) => (
                Service::Rendering,
                "SetLoudness",
                "DesiredLoudness",
                u8::from(value).to_string(),
            ),
            Property::Crossfade(value) => (
                Service::Transport,
                "SetCrossfadeMode",
                "CrossfadeMode",
                u8::from(value).to_string(),
            ),
            Property::Led(value) => (
                Service::Properties,
                "SetLEDState",
                "DesiredLEDState",
                if value { "On" } else { "Off" }.into(),
            ),
            Property::Touch(value) => (
                Service::Properties,
                "SetButtonLockState",
                "DesiredButtonLockState",
                if value { "Off" } else { "On" }.into(),
            ),
        };
        let mut params = vec![(key, value)];
        if matches!(property, Property::Loudness(_)) {
            params.push(("Channel", "Master".into()));
        }
        self.request(device, service, action, &params, false)?;
        Ok(())
    }
}
fn duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0) as u64;
    format!(
        "{}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
fn parse_time(input: &str) -> f64 {
    let parts = input
        .split(':')
        .map(|part| part.parse::<f64>())
        .collect::<std::result::Result<Vec<_>, _>>();
    match parts {
        Ok(parts) if parts.len() == 3 && parts.iter().all(|n| n.is_finite() && *n >= 0.0) => {
            parts[0] * 3600.0 + parts[1] * 60.0 + parts[2]
        }
        _ => 0.0,
    }
}
fn didl(item: &Item, url: &str) -> String {
    let mime = match item.container.as_str() {
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "aac" | "m4a" => "audio/mp4",
        _ => "audio/mpeg",
    };
    format!(
        "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\"><item id=\"1\" parentID=\"0\" restricted=\"1\"><dc:title>{}</dc:title><dc:creator>{}</dc:creator><upnp:artist>{}</upnp:artist><upnp:album>{}</upnp:album><upnp:class>object.item.audioItem.musicTrack</upnp:class><res protocolInfo=\"http-get:*:{mime}:*\" duration=\"{}\">{}</res></item></DIDL-Lite>",
        escape(&item.name),
        escape(&item.artist),
        escape(&item.artist),
        escape(&item.album),
        duration(item.duration),
        escape(url)
    )
}
fn same_uri(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    match (Url::parse(a), Url::parse(b)) {
        (Ok(a), Ok(b)) => {
            let id = |url: &Url| {
                ["id", "itemId", "Id"].into_iter().find_map(|key| {
                    url.query_pairs()
                        .find(|(name, _)| name == key)
                        .map(|(_, value)| value.into_owned())
                        .filter(|id| !id.is_empty())
                })
            };
            match (id(&a), id(&b)) {
                (Some(a), Some(b)) => a == b,
                _ => a.path() == b.path(),
            }
        }
        _ => a.split('?').next() == b.split('?').next(),
    }
}

pub fn matching_queue_entry(urls: &[Option<String>], current: &str, next: &str) -> Option<usize> {
    urls.iter().enumerate().find_map(|(index, url)| {
        (url.as_ref().is_some_and(|url| same_uri(url, current))
            && (next.is_empty()
                || urls
                    .get(index + 1)
                    .and_then(Option::as_ref)
                    .is_some_and(|url| same_uri(url, next))))
        .then_some(index)
    })
}

pub fn parse_groups(input: &str) -> Result<Vec<Group>> {
    let doc = xml(input)?;
    Ok(doc
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "ZoneGroup")
        .map(|group| Group {
            id: group.attribute("ID").unwrap_or_default().into(),
            coordinator: group.attribute("Coordinator").unwrap_or_default().into(),
            stereo_pair: group
                .descendants()
                .any(|node| node.attribute("ChannelMapSet").is_some())
                && !group
                    .descendants()
                    .any(|node| node.attribute("HTSatChanMapSet").is_some()),
            members: group
                .descendants()
                .filter(|node| {
                    node.is_element()
                        && matches!(node.tag_name().name(), "ZoneGroupMember" | "Satellite")
                })
                .map(|member| Member {
                    id: member.attribute("UUID").unwrap_or_default().into(),
                    name: member.attribute("ZoneName").unwrap_or_default().into(),
                    location: member.attribute("Location").unwrap_or_default().into(),
                    satellite: member.tag_name().name() == "Satellite",
                })
                .collect(),
        })
        .collect())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture(
        responses: Vec<String>,
    ) -> (Device, std::thread::JoinHandle<Vec<String>>) {
        use std::io::Write;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let device = Device {
            id: "uuid:RINCON_Test".into(),
            name: "Fixture".into(),
            location: format!("{base}/description.xml"),
            control_url: format!("{base}/transport"),
            rendering_url: format!("{base}/rendering"),
            transport_service: format!("{AVT}1"),
            rendering_service: format!("{RC}1"),
            sonos: true,
            transport_event_url: String::new(),
            rendering_event_url: String::new(),
            topology_event_url: String::new(),
        };
        let worker = std::thread::spawn(move || {
            let mut requests = vec![];
            for response in responses {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                Instant::now() < deadline,
                                "Expected SOAP request was not sent"
                            );
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buffer = [0; 4096];
                loop {
                    let count = socket.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let length = String::from_utf8_lossy(&bytes[..end])
                            .lines()
                            .find_map(|line| {
                                line.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .and_then(|value| value.trim().parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8(bytes).unwrap());
                write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
            }
            requests
        });
        (device, worker)
    }
    pub(crate) fn ok() -> String {
        "<s:Envelope xmlns:s='http://schemas.xmlsoap.org/soap/envelope/'><s:Body><Response/></s:Body></s:Envelope>".into()
    }
    #[test]
    fn soap_transport_volume_next_uri_groups_and_status_round_trip() {
        let mut responses = vec![ok(); 10];
        responses.extend(["<Response><RelTime>0:00:12.5</RelTime><TrackDuration>0:01:30</TrackDuration><TrackURI>http://host/song?a=1&amp;b=2</TrackURI></Response>".into(),"<Response><CurrentTransportState>PLAYING</CurrentTransportState></Response>".into(),"<Response><CurrentVolume>32</CurrentVolume></Response>".into()]);
        let (device, worker) = fixture(responses);
        let controller = Controller::new().unwrap();
        let item = Item {
            name: "Test & <song>".into(),
            duration: 90.0,
            ..Default::default()
        };
        controller
            .load(&device, "http://host/song?a=1&b=2", &item, false)
            .unwrap();
        controller.play(&device).unwrap();
        controller.pause(&device).unwrap();
        controller.seek(&device, 12.5).unwrap();
        controller.volume(&device, 45).unwrap();
        controller.mute(&device, true).unwrap();
        controller
            .load(&device, "http://host/next", &item, true)
            .unwrap();
        controller.clear_next(&device).unwrap();
        controller
            .join(
                &Device {
                    id: "uuid:RINCON_Member".into(),
                    ..device.clone()
                },
                &device,
            )
            .unwrap();
        controller.leave(&device).unwrap();
        let status = controller.status(&device).unwrap();
        assert_eq!(status.position, 12.5);
        assert_eq!(status.duration, 90.0);
        assert_eq!(status.uri, "http://host/song?a=1&b=2");
        assert_eq!(status.volume, Some(32));
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("#SetAVTransportURI\""));
        let body = requests[0].split_once("\r\n\r\n").unwrap().1;
        let doc = xml(body).unwrap();
        assert_eq!(
            field(doc.root(), "CurrentURI"),
            Some("http://host/song?a=1&b=2")
        );
        let metadata = xml(field(doc.root(), "CurrentURIMetaData").unwrap()).unwrap();
        assert_eq!(field(metadata.root(), "title"), Some("Test & <song>"));
        assert!(requests[3].contains("<Target>0:00:12</Target>"));
        assert!(requests[4].starts_with("POST /rendering "));
        assert!(requests[8].contains("x-rincon:RINCON_Test"));
        assert!(requests[9].contains("#BecomeCoordinatorOfStandaloneGroup"));
    }
    #[test]
    fn independent_member_volume_reads_writes_and_rejects_invalid_reports() {
        let (device, worker) = fixture(vec![
            "<CurrentVolume>32</CurrentVolume>".into(),
            "<SetVolumeResponse/>".into(),
            "<CurrentVolume>77</CurrentVolume>".into(),
            "<CurrentVolume>201</CurrentVolume>".into(),
        ]);
        let controller = Controller::new().unwrap();
        assert_eq!(controller.get_volume(&device).unwrap(), 32);
        controller.volume(&device, 77).unwrap();
        assert_eq!(controller.get_volume(&device).unwrap(), 77);
        assert!(controller.get_volume(&device).is_err());
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("#GetVolume"));
        assert!(requests[1].contains("<DesiredVolume>77</DesiredVolume>"));
        assert!(
            requests
                .iter()
                .all(|request| request.starts_with("POST /rendering ")
                    && request.contains("<Channel>Master</Channel>"))
        );
    }
    #[test]
    fn discovery_descriptions_topology_and_metadata_are_namespace_safe() {
        let input = r#"<root xmlns="urn:schemas-upnp-org:device-1-0"><URLBase>http://127.0.0.1:1400/</URLBase><device><deviceType>urn:schemas-upnp-org:device:ZonePlayer:1</deviceType><roomName>Kitchen &amp; Dining</roomName><manufacturer>Sonos</manufacturer><deviceList><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType><UDN>uuid:RINCON_1</UDN><modelName>One</modelName><serviceList><service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>/MediaRenderer/AVTransport/Control</controlURL></service><service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType><controlURL>/MediaRenderer/RenderingControl/Control</controlURL></service></serviceList></device></deviceList></device></root>"#;
        let device = parse_device(input, "http://127.0.0.1:1400/xml/device.xml").unwrap();
        assert_eq!(device.name, "Kitchen & Dining (One)");
        assert!(device.sonos);
        assert_eq!(
            device.control_url,
            "http://127.0.0.1:1400/MediaRenderer/AVTransport/Control"
        );
        assert!(
            parse_device(
                &input.replace("http://127.0.0.1:1400/", "http://other-host/"),
                &device.location
            )
            .is_err()
        );
        assert!(xml("<!DOCTYPE a [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><a>&x;</a>").is_err());
        assert_eq!(
            ssdp_location(b"HTTP/1.1 200 OK\r\nlocation: http://127.0.0.1/device.xml\r\n\r\n")
                .unwrap(),
            "http://127.0.0.1/device.xml"
        );
        assert!(ssdp_location(b"HTTP/1.1 200 OK\r\nLOCATION: file:///etc/passwd\r\n").is_none());
        let groups=parse_groups("<ZoneGroups><ZoneGroup Coordinator='A' ID='g'><ZoneGroupMember UUID='A' ZoneName='Living &amp; Dining' Location='http://host/xml'><Satellite UUID='B' ZoneName='Right'/></ZoneGroupMember></ZoneGroup></ZoneGroups>").unwrap();
        assert_eq!(groups[0].members.len(), 2);
        assert!(groups[0].members[1].satellite);
        assert!(!groups[0].stereo_pair);
        let stereo = "<ZoneGroup Coordinator='A'><ZoneGroupMember UUID='A' ChannelMapSet='A:LF,LF;B:RF,RF'/><ZoneGroupMember UUID='B'/></ZoneGroup>";
        assert!(parse_groups(stereo).unwrap()[0].stereo_pair);
        assert!(
            !parse_groups(&stereo.replace("ChannelMapSet=", "HTSatChanMapSet=")).unwrap()[0]
                .stereo_pair
        );

        let item = Item {
            name: "Rock < & Roll".into(),
            artist: "O'Brien".into(),
            duration: 72.0,
            ..Default::default()
        };
        let metadata = didl(&item, "http://host/audio?id=1&t=secret");
        let doc = xml(&metadata).unwrap();
        assert_eq!(field(doc.root(), "title"), Some("Rock < & Roll"));
        assert_eq!(
            field(doc.root(), "res"),
            Some("http://host/audio?id=1&t=secret")
        );
        assert_eq!(parse_time("1:02:03.5"), 3723.5);
    }
    #[test]
    fn speaker_connection_reads_current_next_and_volume_without_playback_commands() {
        let (device, worker) = fixture(vec![
            "<Position><RelTime>0:00:23</RelTime><TrackDuration>0:02:00</TrackDuration><TrackURI>http://fixture/current</TrackURI></Position>".into(),
            "<Transport><CurrentTransportState>PAUSED_PLAYBACK</CurrentTransportState></Transport>".into(),
            "<Volume><CurrentVolume>37</CurrentVolume></Volume>".into(),
            "<Media><NextURI>http://fixture/next</NextURI></Media>".into(),
        ]);
        let status = Controller::new()
            .unwrap()
            .connection_status(&device)
            .unwrap();
        assert_eq!(status.uri, "http://fixture/current");
        assert_eq!(status.next_uri, "http://fixture/next");
        assert_eq!(status.position, 23.);
        assert_eq!(status.duration, 120.);
        assert_eq!(status.transport, "PAUSED_PLAYBACK");
        assert_eq!(status.volume, Some(37));
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 4);
        assert!(requests.iter().all(|request| request.contains("#Get")));
    }
}
