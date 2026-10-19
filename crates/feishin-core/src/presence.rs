use crate::presence_ipc::Client as DiscordIpcClient;
use crate::{model::Item, storage::Settings};
use discord_rich_presence::DiscordIpc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub display_type: String,
    pub link_type: String,
    pub show_as_listening: bool,
    pub show_paused: bool,
    pub show_server_image: bool,
    pub show_state_icon: bool,
    // None restores credentials written by native versions before this preference.
    pub last_fm_key_enabled: Option<bool>,
    #[serde(skip_serializing)]
    pub last_fm_api_key: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            display_type: "feishin".into(),
            link_type: "none".into(),
            show_as_listening: false,
            show_paused: true,
            show_server_image: false,
            show_state_icon: true,
            last_fm_key_enabled: None,
            last_fm_api_key: String::new(),
        }
    }
}
impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(self.display_type.as_str(), "feishin" | "artist" | "song")
                && matches!(
                    self.link_type.as_str(),
                    "none" | "last_fm" | "musicbrainz" | "musicbrainz_last_fm"
                ),
            "Invalid Discord display or link type"
        );
        anyhow::ensure!(
            self.last_fm_api_key.len() <= 1024,
            "Last.fm API key is too long"
        );
        Ok(())
    }
}

pub fn image_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && value.len() <= 2048)
        .then(|| url.into())
}

pub fn activity(
    item: Option<&Item>,
    playing: bool,
    position: f64,
    private: bool,
    now: u64,
    config: &Config,
    cover: Option<&str>,
) -> Value {
    let Some(item) = item.filter(|_| !private && (playing || config.show_paused)) else {
        return Value::Null;
    };
    let limit = |value: &str, fallback: &str| {
        let mut value = if value.is_empty() { fallback } else { value }
            .chars()
            .take(128)
            .collect::<String>();
        while value.chars().count() < 2 {
            value.push(' ');
        }
        value
    };
    let display = match config.display_type.as_str() {
        "artist" => 1,
        "song" => 2,
        _ => 0,
    };
    let mut activity = json!({"type":if config.show_as_listening {2} else {0},"status_display_type":display,"details":limit(&item.name, "Idle"),"state":limit(&item.artist, "Unknown artist"),"assets":{"large_image":cover.and_then(image_url).unwrap_or_else(|| "icon".into()),"large_text":limit(&item.album, "Unknown album")},"instance":false});
    if !playing || config.show_state_icon {
        activity["assets"]["small_image"] = if playing { "playing" } else { "paused" }.into();
        activity["assets"]["small_text"] = if playing { "Playing" } else { "Paused" }.into();
    }
    if item.kind == crate::model::ItemKind::Radio {
        return activity;
    }
    if playing && item.duration > 0.0 && position.is_finite() && position >= 0.0 {
        let start = now.saturating_sub(position as u64);
        activity["timestamps"] =
            json!({"start":start,"end":start.saturating_add(item.duration as u64)});
    }
    let mut details = None;
    let mut album = None;
    if matches!(config.link_type.as_str(), "last_fm" | "musicbrainz_last_fm")
        && !item.primary_artist().is_empty()
    {
        let encode = |value: &str| {
            url::form_urlencoded::byte_serialize(value.as_bytes())
                .collect::<String>()
                .replace('+', "%20")
        };
        let artist = item.primary_album_artist();
        let base = format!(
            "https://www.last.fm/music/{}/{}",
            encode(artist),
            encode(if item.album.is_empty() {
                "_"
            } else {
                &item.album
            })
        );
        activity["state_url"] = format!(
            "https://www.last.fm/music/{}",
            encode(item.primary_artist())
        )
        .into();
        details = Some(format!("{base}/{}", encode(&item.name)));
        album = Some(base);
    }
    if matches!(
        config.link_type.as_str(),
        "musicbrainz" | "musicbrainz_last_fm"
    ) {
        for (key, path, is_album) in [
            ("mbzRecordingId", "recording", false),
            ("mbzTrackId", "track", false),
            ("mbzAlbumId", "release", true),
        ] {
            if let Some(id) = item
                .column_data
                .get(key)
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
            {
                let url = Some(format!("https://musicbrainz.org/{path}/{id}"));
                if is_album {
                    album = url;
                } else {
                    details = url;
                }
            }
        }
    }
    if let Some(url) = details.filter(|url| url.len() <= 256) {
        activity["details_url"] = url.into();
    }
    if let Some(url) = album.filter(|url| url.len() <= 256) {
        activity["assets"]["large_url"] = url.into();
    }
    if activity["state_url"]
        .as_str()
        .is_some_and(|url| url.len() > 256)
    {
        activity.as_object_mut().unwrap().remove("state_url");
    }
    activity
}

pub fn lastfm_image(key: &str, artist: &str, album: &str) -> anyhow::Result<Option<String>> {
    lastfm_image_at("https://ws.audioscrobbler.com/2.0/", key, artist, album)
}
fn lastfm_image_at(
    endpoint: &str,
    key: &str,
    artist: &str,
    album: &str,
) -> anyhow::Result<Option<String>> {
    if key.is_empty() || artist.is_empty() || album.is_empty() {
        return Ok(None);
    }
    use std::io::Read;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = client
        .get(endpoint)
        .query(&[
            ("method", "album.getinfo"),
            ("api_key", key),
            ("artist", artist),
            ("album", album),
            ("format", "json"),
        ])
        .send()
        .map_err(|e| e.without_url())?
        .error_for_status()
        .map_err(|e| e.without_url())?;
    let mut bytes = vec![];
    response.take(1_000_001).read_to_end(&mut bytes)?;
    anyhow::ensure!(bytes.len() <= 1_000_000, "Last.fm reply is too large");
    let value: Value = serde_json::from_slice(&bytes)?;
    Ok(value["album"]["image"][3]["#text"]
        .as_str()
        .and_then(image_url))
}

pub struct Presence {
    snapshot: Arc<Mutex<(String, Value)>>,
    wake: mpsc::SyncSender<()>,
}
impl Default for Presence {
    fn default() -> Self {
        Self::new()
    }
}
impl Presence {
    pub fn new() -> Self {
        let snapshot = Arc::new(Mutex::new((String::new(), Value::Null)));
        let worker_snapshot = snapshot.clone();
        let (wake, requests) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let mut client: Option<DiscordIpcClient> = None;
            let mut previous = Value::Null;
            while let Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) =
                requests.recv_timeout(Duration::from_secs(30))
            {
                let (id, value) = worker_snapshot
                    .lock()
                    .map(|snapshot| snapshot.clone())
                    .unwrap_or_default();
                if id.is_empty() || value.is_null() {
                    if let Some(mut client) = client.take() {
                        let _ = client.clear_activity();
                        let _ = client.close();
                    }
                    previous = Value::Null;
                    continue;
                }
                if client.as_ref().is_some_and(|client| client.client_id != id)
                    && let Some(mut client) = client.take()
                {
                    let _ = client.clear_activity();
                    let _ = client.close();
                }
                if client.is_none() {
                    let mut connection = DiscordIpcClient::new(&id);
                    if let Err(error) = connection.connect() {
                        log::debug!("Discord IPC unavailable: {error}");
                        continue;
                    }
                    client = Some(connection);
                    previous = Value::Null;
                }
                if value != previous {
                    let nonce = uuid::Uuid::new_v4().to_string();
                    let payload = json!({"cmd":"SET_ACTIVITY","args":{"pid":std::process::id(),"activity":value},"nonce":nonce});
                    let connection = client.as_mut().unwrap();
                    let result = connection
                        .send(payload, 1)
                        .and_then(|_| connection.reply(&nonce));
                    match result {
                        Ok(reply) if reply["evt"] != "ERROR" => previous = value,
                        _ => {
                            log::debug!("Discord rejected or disconnected the presence update");
                            if let Some(mut connection) = client.take() {
                                let _ = connection.clear_activity();
                                let _ = connection.close();
                            }
                        }
                    }
                }
            }
            if let Some(mut client) = client {
                let _ = client.clear_activity();
                let _ = client.close();
            }
        });
        Self { snapshot, wake }
    }
    pub fn publish(&self, item: Option<&Item>, playing: bool, position: f64, settings: &Settings) {
        self.publish_with_cover(item, playing, position, settings, None);
    }
    pub fn publish_with_cover(
        &self,
        item: Option<&Item>,
        playing: bool,
        position: f64,
        settings: &Settings,
        cover: Option<&str>,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let value = activity(
            item,
            playing,
            position,
            settings.private_mode,
            now,
            &settings.discord_config,
            cover,
        );
        if let Ok(mut snapshot) = self.snapshot.lock() {
            *snapshot = (
                if settings.discord_enabled {
                    settings.discord_client_id.clone()
                } else {
                    String::new()
                },
                value,
            );
        }
        let _ = self.wake.try_send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_discord_options_preserve_links_paused_icons_and_radio_payloads() {
        let item = Item {
            name: "曲".into(),
            artist: "A & B / +".into(),
            album_artist: "Album artist".into(),
            album: "Album".into(),
            duration: 200.,
            column_data: std::collections::BTreeMap::from([
                (
                    "mbzRecordingId".into(),
                    "c907087d-7e28-4b5b-920f-e12ffb3f0c65".into(),
                ),
                (
                    "mbzTrackId".into(),
                    "9bbb5031-0bfe-4c68-a7cc-8d0931e7f930".into(),
                ),
                (
                    "mbzAlbumId".into(),
                    "82661d7d-a97f-431b-91df-d295f92b1d91".into(),
                ),
            ]),
            ..Default::default()
        };
        let mut config = Config {
            show_as_listening: true,
            display_type: "song".into(),
            link_type: "musicbrainz_last_fm".into(),
            ..Default::default()
        };
        let playing = activity(
            Some(&item),
            true,
            30.,
            false,
            1000,
            &config,
            Some("https://images.example/art.jpg"),
        );
        assert_eq!(playing["type"], 2);
        assert_eq!(playing["status_display_type"], 2);
        assert_eq!(playing["details"], "曲 ");
        assert_eq!(
            playing["details_url"],
            "https://musicbrainz.org/track/9bbb5031-0bfe-4c68-a7cc-8d0931e7f930"
        );
        assert_eq!(
            playing["assets"]["large_url"],
            "https://musicbrainz.org/release/82661d7d-a97f-431b-91df-d295f92b1d91"
        );
        assert_eq!(
            playing["state_url"],
            "https://www.last.fm/music/A%20%26%20B%20%2F%20%2B"
        );
        assert_eq!(
            playing["assets"]["large_image"],
            "https://images.example/art.jpg"
        );
        assert_eq!(playing["assets"]["small_image"], "playing");
        config.show_state_icon = false;
        config.show_as_listening = false;
        config.display_type = "artist".into();
        let playing = activity(
            Some(&item),
            true,
            0.,
            false,
            1000,
            &config,
            Some("file:///private/music.jpg"),
        );
        assert_eq!(playing["type"], 0);
        assert_eq!(playing["status_display_type"], 1);
        assert!(playing["assets"]["small_image"].is_null());
        assert_eq!(playing["assets"]["large_image"], "icon");
        let paused = activity(Some(&item), false, 0., false, 1000, &config, None);
        assert_eq!(paused["assets"]["small_image"], "paused");
        assert!(paused["timestamps"].is_null());
        config.show_paused = false;
        assert!(activity(Some(&item), false, 0., false, 1000, &config, None).is_null());
        let radio = Item {
            kind: crate::model::ItemKind::Radio,
            ..item
        };
        let value = activity(Some(&radio), true, 0., false, 1000, &config, None);
        assert!(value["timestamps"].is_null() && value["details_url"].is_null());
        assert!(activity(Some(&radio), true, 0., true, 1000, &config, None).is_null());
        assert!(image_url("https://user:password@example.com/art").is_none());
        config.last_fm_api_key = "fixture-private-api-key".into();
        assert!(
            !serde_json::to_string(&config)
                .unwrap()
                .contains("fixture-private-api-key")
        );
    }

    #[test]
    fn lastfm_cover_lookup_encodes_fields_bounds_replies_and_rejects_private_urls() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        for (body, expected) in [
            (
                json!({"album":{"image":[{},{},{},{"#text":"https://images.example/cover.jpg"}]}})
                    .to_string(),
                Some("https://images.example/cover.jpg"),
            ),
            (
                json!({"album":{"image":[{},{},{},{"#text":"file:///secret"}]}}).to_string(),
                None,
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            let worker = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = socket.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let target = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = url::Url::parse(&format!("http://localhost{target}")).unwrap();
                let query = url
                    .query_pairs()
                    .collect::<std::collections::HashMap<_, _>>();
                assert_eq!(query["api_key"], "fixture-key");
                assert_eq!(query["artist"], "A & B / +");
                assert_eq!(query["album"], "曲");
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            });
            assert_eq!(
                lastfm_image_at(&endpoint, "fixture-key", "A & B / +", "曲")
                    .unwrap()
                    .as_deref(),
                expected
            );
            worker.join().unwrap();
        }
        assert!(
            lastfm_image_at("http://127.0.0.1:1", "", "artist", "album")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn presence_timestamps_follow_seek_and_private_or_paused_playback_clears_activity() {
        let item = Item {
            name: "Song".into(),
            artist: "Artist".into(),
            duration: 200.0,
            stream_url: "http://private/stream?token=secret".into(),
            ..Default::default()
        };
        let config = Config {
            show_paused: false,
            ..Default::default()
        };
        let value = activity(Some(&item), true, 30.0, false, 1000, &config, None);
        assert_eq!(value["timestamps"]["start"], 970);
        assert_eq!(value["timestamps"]["end"], 1170);
        assert!(!value.to_string().contains("secret"));
        assert!(activity(Some(&item), true, 0.0, true, 1000, &config, None).is_null());
        assert!(activity(Some(&item), false, 0.0, false, 1000, &config, None).is_null());
    }
}
