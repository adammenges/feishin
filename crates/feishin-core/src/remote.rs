use crate::{
    api::Api,
    model::{Item, ItemKind},
    queue::Repeat,
    storage::State as AppState,
};
use anyhow::{Result, ensure};
use axum::{
    Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, OnceLock, RwLock, mpsc},
    thread,
    time::Duration,
};
use tokio::sync::{Semaphore, broadcast, oneshot};

#[derive(Clone)]
struct Artwork {
    api: Api,
    item: Item,
    path: PathBuf,
}

#[derive(Clone)]
pub struct ArtworkHandle {
    entries: Arc<RwLock<HashMap<String, Artwork>>>,
    capability: String,
    permits: Arc<Semaphore>,
}
impl ArtworkHandle {
    pub fn item_json(&self, item: &Item, api: &Api, data_dir: &std::path::Path) -> Value {
        let mut value = item_json(item);
        if (item.cover_id.is_empty() && item.kind != ItemKind::Radio)
            || item.server_id != api.server.id
            || api.is_signed_out()
        {
            return value;
        }
        let key = crate::api::artwork_key(item);
        if let Ok(mut entries) = self.entries.write() {
            entries.entry(key.clone()).or_insert_with(|| Artwork {
                api: api.clone(),
                item: item.clone(),
                path: data_dir.join("covers").join(format!("{key}.img")),
            });
            value["imageUrl"] = format!("/artwork/{}/{key}", self.capability).into();
        }
        value
    }
}

pub struct Request {
    pub message: Value,
    pub reply: oneshot::Sender<Vec<Value>>,
}

#[derive(Clone)]
struct RemoteState {
    tx: mpsc::Sender<Request>,
    snapshot: Arc<RwLock<Vec<Value>>>,
    broadcast: broadcast::Sender<Vec<Value>>,
    credential: Arc<RwLock<Option<String>>>,
    stop: broadcast::Sender<u16>,
    artwork: ArtworkHandle,
}

pub struct Remote {
    state: RemoteState,
    shutdown: Option<oneshot::Sender<()>>,
    pub requests: mpsc::Receiver<Request>,
    pub port: u16,
}

fn message(event: &str, data: Value) -> Value {
    json!({"event":event,"data":data})
}

pub fn item_json(item: &Item) -> Value {
    json!({"id":item.id,"_serverId":item.server_id,"name":item.name,"artistName":item.artist,"albumArtistName":if item.album_artist.is_empty(){&item.artist}else{&item.album_artist},"album":item.album,"duration":item.duration,"imageUrl":null,"releaseYear":item.year,"userFavorite":item.favorite,"userRating":item.rating,"homepageUrl":item.homepage})
}

fn accent_colors(settings: &crate::storage::Settings) -> Value {
    static PALETTES: OnceLock<[crate::appearance::Palette; 2]> = OnceLock::new();
    let palettes = PALETTES.get_or_init(|| {
        ["defaultDark", "defaultLight"].map(|id| {
            crate::appearance::resolve(id, std::path::Path::new("")).expect("Bundled remote theme")
        })
    });
    let [dark, light] = palettes.each_ref().map(|palette| {
        crate::mantine::remote_primary(palette, settings).unwrap_or_else(|_| {
            crate::mantine::remote_primary(palette, &Default::default())
                .expect("Bundled remote accent")
        })
    });
    json!({"dark":dark,"light":light})
}

pub fn reorder_queue(queue: &mut crate::queue::Queue, value: &Value) -> Result<()> {
    let identity = |field: &str| {
        value[field]
            .as_str()
            .and_then(|id| uuid::Uuid::parse_str(id).ok())
            .ok_or_else(|| anyhow::anyhow!("Invalid queue identity"))
    };
    let key = identity("uniqueId")?;
    let target = identity("targetUniqueId")?;
    ensure!(
        queue.order.contains(&key) && matches!(value["edge"].as_str(), Some("top" | "bottom")),
        "Invalid queue entry or edge"
    );
    ensure!(
        queue.order.contains(&target),
        "Target queue entry no longer exists"
    );
    if key != target {
        queue.move_at(&[key], Some(target), value["edge"] == "bottom");
    }
    Ok(())
}

pub fn snapshot(state: &AppState, playing: bool) -> Vec<Value> {
    let song = state.queue.current().map(|entry| {
        let mut item = item_json(&entry.item);
        item["_uniqueId"] = entry.key.to_string().into();
        item
    });
    let status = if playing { "playing" } else { "paused" };
    let repeat = match state.queue.repeat {
        Repeat::Off => "none",
        Repeat::All => "all",
        Repeat::One => "one",
    };
    let queue = state
        .queue
        .ordered()
        .iter()
        .map(|entry| {
            let mut item = item_json(&entry.item);
            item["uniqueId"] = entry.key.to_string().into();
            item
        })
        .collect::<Vec<_>>();
    vec![
        message(
            "state",
            json!({"song":song,"status":status,"position":state.queue.position,"volume":state.settings.volume,"repeat":repeat,"shuffle":state.queue.shuffle}),
        ),
        message(
            "queue-state",
            json!({"currentUniqueId":state.queue.current,"items":queue}),
        ),
        message(
            "confirm-queue-changes-setting",
            json!(state.settings.confirm_queue_changes),
        ),
        message(
            "radio-status",
            if state
                .queue
                .current()
                .is_some_and(|entry| entry.item.kind == crate::model::ItemKind::Radio)
            {
                json!({"isActive":true,"stationName":state.queue.current().unwrap().item.name,"imageUrl":null})
            } else {
                json!({"isActive":false})
            },
        ),
        message("accent-color", accent_colors(&state.settings)),
    ]
}

impl Remote {
    pub fn validate_credentials(username: &str, password: &str) -> Result<()> {
        ensure!(
            username.len() <= 128
                && !username.contains(':')
                && !username.chars().any(char::is_control)
                && password.len() <= 4096
                && !password.contains('\0'),
            "Invalid remote credentials"
        );
        Ok(())
    }

    pub fn update_credentials(&self, username: &str, password: &str) -> Result<()> {
        Self::validate_credentials(username, password)?;
        let next = credential(username, password);
        let mut current = self
            .state
            .credential
            .write()
            .map_err(|_| anyhow::anyhow!("Remote credentials are unavailable"))?;
        if *current != next {
            *current = next;
            let _ = self.state.stop.send(4002);
            log::info!("Remote credentials changed; existing clients disconnected");
        }
        Ok(())
    }

    pub fn forget_server(&self, id: uuid::Uuid) {
        if let Ok(mut entries) = self.state.artwork.entries.write() {
            entries.retain(|_, entry| entry.api.server.id != id);
        }
    }
    pub fn start(port: u16, username: &str, password: &str) -> Result<Self> {
        Self::validate_credentials(username, password)?;
        let listener = std::net::TcpListener::bind(("0.0.0.0", port))?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;
        let (tx, requests) = mpsc::channel();
        let (broadcast, _) = broadcast::channel(32);
        let (stop, _) = broadcast::channel(1);
        let (shutdown, shutdown_rx) = oneshot::channel();
        let state = RemoteState {
            tx,
            snapshot: Arc::new(RwLock::new(vec![])),
            broadcast,
            stop,
            credential: Arc::new(RwLock::new(credential(username, password))),
            artwork: ArtworkHandle {
                entries: Arc::new(RwLock::new(HashMap::new())),
                capability: uuid::Uuid::new_v4().to_string(),
                permits: Arc::new(Semaphore::new(4)),
            },
        };
        let server_state = state.clone();
        thread::spawn(move || {
            let result = (|| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()?;
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener)?;
                    let app = Router::new()
                        .route("/", get(index))
                        .route("/ws", get(upgrade))
                        .route("/remote.js", get(script))
                        .route("/worker.js", get(worker))
                        .route("/manifest.json", get(manifest))
                        .route("/icon-256.png", get(icon256))
                        .route("/icon-512.png", get(icon512))
                        .route("/artwork/{capability}/{key}", get(artwork))
                        .with_state(server_state);
                    axum::serve(listener, app)
                        .with_graceful_shutdown(async move {
                            let _ = shutdown_rx.await;
                        })
                        .await
                })?;
                Ok::<_, anyhow::Error>(())
            })();
            if let Err(error) = result {
                log::warn!("Remote server failed: {error}");
            }
        });
        log::info!("Remote server listening on port {port}");
        Ok(Self {
            state,
            shutdown: Some(shutdown),
            requests,
            port,
        })
    }

    pub fn publish(&self, state: &AppState, playing: bool) {
        self.publish_snapshot(snapshot(state, playing));
    }

    pub fn artwork(&self) -> ArtworkHandle {
        self.state.artwork.clone()
    }

    pub fn publish_with_artwork(
        &self,
        state: &AppState,
        playing: bool,
        sessions: &HashMap<uuid::Uuid, Api>,
        data_dir: &std::path::Path,
        radio: Option<(&crate::queue::Entry, &crate::playback::RadioMetadata, f64)>,
    ) {
        let mut values = snapshot(state, playing);
        let decorate = |item: &Item| {
            sessions.get(&item.server_id).map_or_else(
                || item_json(item),
                |api| self.state.artwork.item_json(item, api, data_dir),
            )
        };
        if let Some(entry) = radio
            .map(|(entry, _, _)| entry)
            .or_else(|| state.queue.current())
        {
            let mut item = decorate(&entry.item);
            item["_uniqueId"] = entry.key.to_string().into();
            values[0]["data"]["song"] = item;
            if entry.item.kind == crate::model::ItemKind::Radio {
                values[3]["data"]["imageUrl"] = values[0]["data"]["song"]["imageUrl"].clone();
                values[3]["data"]["isActive"] = true.into();
                values[3]["data"]["stationName"] = entry.item.name.clone().into();
                if let Some((_, metadata, position)) = radio {
                    values[0]["data"]["position"] = position.into();
                    if !metadata.title.is_empty() {
                        values[0]["data"]["song"]["name"] = metadata.title.clone().into();
                    }
                    if !metadata.artist.is_empty() {
                        values[0]["data"]["song"]["artistName"] = metadata.artist.clone().into();
                    }
                    values[3]["data"]["title"] = metadata.title.clone().into();
                    values[3]["data"]["artist"] = metadata.artist.clone().into();
                }
            }
        }
        values[1]["data"]["items"] = state
            .queue
            .ordered()
            .into_iter()
            .map(|entry| {
                let mut item = decorate(&entry.item);
                item["uniqueId"] = entry.key.to_string().into();
                item
            })
            .collect::<Vec<_>>()
            .into();
        self.publish_snapshot(values);
    }

    fn publish_snapshot(&self, snapshot: Vec<Value>) {
        if let Ok(mut previous) = self.state.snapshot.write()
            && *previous != snapshot
        {
            *previous = snapshot.clone();
            let _ = self.state.broadcast.send(snapshot);
        }
    }
}

async fn artwork(
    Path((capability, key)): Path<(String, String)>,
    State(state): State<RemoteState>,
) -> axum::response::Response {
    if capability != state.artwork.capability {
        return StatusCode::NOT_FOUND.into_response();
    }
    let entry = state
        .artwork
        .entries
        .read()
        .ok()
        .and_then(|entries| entries.get(&key).cloned());
    let Some(entry) = entry else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(_permit) = state.artwork.permits.acquire().await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let result = tokio::task::spawn_blocking(move || {
        use std::io::Read;
        ensure!(!entry.api.is_signed_out(), "Server was signed out");
        let bytes = if entry.path.exists() {
            let mut bytes = Vec::new();
            std::fs::File::open(&entry.path)?
                .take(20_000_001)
                .read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 20_000_000, "Cover image is too large");
            bytes
        } else {
            let bytes = entry.api.cover_bytes(&entry.item)?;
            crate::storage::atomic_write(&entry.path, &bytes)?;
            bytes
        };
        ensure!(!entry.api.is_signed_out(), "Server was signed out");
        Ok::<_, anyhow::Error>(bytes)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => {
            let content_type = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
                "image/png"
            } else if bytes.starts_with(b"\xff\xd8\xff") {
                "image/jpeg"
            } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                "image/gif"
            } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
                "image/webp"
            } else {
                "application/octet-stream"
            };
            (
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, "private, max-age=3600"),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
                    (
                        header::CONTENT_SECURITY_POLICY,
                        "default-src 'none'; sandbox",
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        let _ = self.state.stop.send(4000);
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn index(
    ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
    State(state): State<RemoteState>,
) -> axum::response::Response {
    match ws {
        Ok(ws) => upgrade(ws, State(state)).await.into_response(),
        Err(_) => Html(include_str!("remote.html")).into_response(),
    }
}
async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("remote.js"),
    )
}
async fn worker() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("remote-worker.js"),
    )
}
async fn manifest() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/manifest+json"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        include_str!("remote-manifest.json"),
    )
}
async fn icon256() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../../assets/icons/256x256.png").as_slice(),
    )
}
async fn icon512() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "image/png")],
        include_bytes!("../../../assets/icons/512x512.png").as_slice(),
    )
}
async fn upgrade(ws: WebSocketUpgrade, State(state): State<RemoteState>) -> impl IntoResponse {
    ws.max_message_size(65536)
        .on_upgrade(move |socket| connection(socket, state))
}

async fn send(socket: &mut WebSocket, values: Vec<Value>) -> bool {
    for value in values {
        if socket
            .send(Message::Text(value.to_string().into()))
            .await
            .is_err()
        {
            return false;
        }
    }
    true
}

fn credential(username: &str, password: &str) -> Option<String> {
    (!username.is_empty() || !password.is_empty()).then(|| {
        format!(
            "Basic {}",
            STANDARD.encode(format!("{username}:{password}"))
        )
    })
}

async fn close(socket: &mut WebSocket, code: u16) {
    let _ = socket
        .send(Message::Close(Some(axum::extract::ws::CloseFrame {
            code,
            reason: match code {
                4002 => "Credentials changed",
                4004 => "Authentication failed",
                _ => "Server stopped",
            }
            .into(),
        })))
        .await;
}

async fn connection(mut socket: WebSocket, state: RemoteState) {
    let mut stop = state.stop.subscribe();
    let mut updates = state.broadcast.subscribe();
    let unprotected = state.credential.read().is_ok_and(|value| value.is_none());
    let authenticated = if unprotected {
        true
    } else {
        let first = tokio::select! {
            code=stop.recv()=>{close(&mut socket, code.unwrap_or(4000)).await; return;},
            first=tokio::time::timeout(Duration::from_secs(10), socket.recv())=>first,
        };
        match first {
            Ok(Some(Ok(Message::Text(text)))) => serde_json::from_str::<Value>(&text)
                .ok()
                .is_some_and(|value| {
                    value["event"] == "authenticate"
                        && state.credential.read().is_ok_and(|credential| {
                            credential
                                .as_deref()
                                .is_none_or(|expected| value["header"].as_str() == Some(expected))
                        })
                }),
            _ => false,
        }
    };
    if !authenticated {
        close(&mut socket, 4004).await;
        return;
    }
    let snapshot = state.snapshot.read().map(|s| s.clone()).unwrap_or_default();
    if !send(&mut socket, snapshot).await {
        return;
    }
    loop {
        tokio::select! {
            code=stop.recv()=>{close(&mut socket, code.unwrap_or(4000)).await; break;},
            update=updates.recv()=>match update{
                Ok(values)=>if !send(&mut socket,values).await{break;},
                Err(broadcast::error::RecvError::Lagged(_))=>{let values=state.snapshot.read().map(|s|s.clone()).unwrap_or_default();if !send(&mut socket,values).await{break;}},
                Err(_)=>break,
            },
            incoming=socket.recv()=>match incoming{
                Some(Ok(Message::Text(text)))=>{
                    let Ok(value)=serde_json::from_str::<Value>(&text)else{continue;};
                    if value["event"] == "authenticate" { continue; }
                    let(request_tx,request_rx)=oneshot::channel();
                    if state.tx.send(Request{message:value,reply:request_tx}).is_err(){break;}
                    let reply=tokio::select!{
                        code=stop.recv()=>{close(&mut socket, code.unwrap_or(4000)).await; break;},
                        reply=tokio::time::timeout(Duration::from_secs(30),request_rx)=>reply,
                    };
                    match reply{
                        Ok(Ok(values))=>if !send(&mut socket,values).await{break;},
                        _=>{if !send(&mut socket,vec![message("error",json!("Operation timed out"))]).await{break;}},
                    }
                },
                Some(Ok(Message::Ping(payload)))=>{if socket.send(Message::Pong(payload)).await.is_err(){break;}},
                Some(Ok(Message::Close(_)))|None|Some(Err(_))=>break,
                _=>{},
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn remote_credentials_match_original_and_change_without_rebinding() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};
        let reference: Value =
            serde_json::from_str(include_str!("../assets/remote-settings-reference.json")).unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            for case in reference["authorization"].as_array().unwrap() {
                let remote = Remote::start(0, case["username"].as_str().unwrap(), case["password"].as_str().unwrap()).unwrap();
                remote.publish(&AppState::default(), false);
                let (mut client, _) = connect_async(format!("ws://127.0.0.1:{}/ws", remote.port)).await.unwrap();
                if let Some(header) = case["header"].as_str() {
                    client.send(ClientMessage::Text(json!({"event":"authenticate","header":header}).to_string().into())).await.unwrap();
                }
                if case["header"].is_null() && case["accepted"] == false {
                    assert!(tokio::time::timeout(Duration::from_millis(30), client.next()).await.is_err());
                } else {
                    let message = client.next().await.unwrap().unwrap();
                    if case["accepted"] == true {
                        assert_eq!(serde_json::from_str::<Value>(&message.into_text().unwrap()).unwrap()["event"], "state", "{case}");
                    } else {
                        let ClientMessage::Close(Some(frame)) = message else { panic!("Private state sent for {case}"); };
                        assert_eq!(u16::from(frame.code), 4004);
                    }
                }
            }
            let remote = Remote::start(0, "phone", "old").unwrap();
            let port = remote.port;
            let address = format!("ws://127.0.0.1:{port}/ws");
            remote.publish(&AppState::default(), false);
            let mut clients = vec![];
            for _ in 0..2 {
                let (mut client, _) = connect_async(&address).await.unwrap();
                client.send(ClientMessage::Text(json!({"event":"authenticate","header":credential("phone","old")}).to_string().into())).await.unwrap();
                for _ in 0..5 { client.next().await.unwrap().unwrap(); }
                clients.push(client);
            }
            let (mut waiting, _) = connect_async(&address).await.unwrap();
            remote.update_credentials("phone", "old").unwrap();
            assert!(tokio::time::timeout(Duration::from_millis(30), clients[0].next()).await.is_err());
            clients[1].send(ClientMessage::Text(json!({"event":"pause","requestId":"pending"}).to_string().into())).await.unwrap();
            let pending = loop {
                if let Ok(request) = remote.requests.try_recv() { break request; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            remote.update_credentials("other", "new").unwrap();
            let expected = reference["changes"].as_array().unwrap().iter()
                .find(|case| case["action"] == "username").unwrap()["closed"][0].as_u64().unwrap() as u16;
            for client in clients.iter_mut().chain(std::iter::once(&mut waiting)) {
                let ClientMessage::Close(Some(frame)) = client.next().await.unwrap().unwrap() else { panic!("Changed credentials did not disconnect a client"); };
                assert_eq!(u16::from(frame.code), expected);
            }
            assert!(pending.reply.is_closed());
            assert_eq!(remote.port, port);
            for (username, password, accepted) in [("phone", "old", false), ("other", "new", true)] {
                let (mut client, _) = connect_async(&address).await.unwrap();
                client.send(ClientMessage::Text(json!({"event":"authenticate","header":credential(username,password)}).to_string().into())).await.unwrap();
                let message = client.next().await.unwrap().unwrap();
                assert_eq!(message.is_text(), accepted);
            }
            remote.update_credentials("", "").unwrap();
            let (mut public, _) = connect_async(&address).await.unwrap();
            for _ in 0..5 { assert!(public.next().await.unwrap().unwrap().is_text()); }
            remote.update_credentials("phone", "restored").unwrap();
            let ClientMessage::Close(Some(frame)) = public.next().await.unwrap().unwrap() else { panic!("Unprotected connection survived enabling authentication"); };
            assert_eq!(u16::from(frame.code), expected);
            assert_eq!(remote.port, port);
        }).await.expect("Remote credential regression timed out");
    }

    #[tokio::test]
    async fn remote_installable_shell_routes_return_correct_types_and_icons() {
        let remote = Remote::start(0, "phone", "secret").unwrap();
        let client = reqwest::Client::new();
        for (path, content_type) in [
            ("/", "text/html"),
            ("/worker.js", "text/javascript"),
            ("/manifest.json", "application/manifest+json"),
            ("/icon-256.png", "image/png"),
            ("/icon-512.png", "image/png"),
        ] {
            let response = client
                .get(format!("http://127.0.0.1:{}{path}", remote.port))
                .send()
                .await
                .unwrap();
            assert!(response.status().is_success());
            assert!(
                response.headers()[header::CONTENT_TYPE]
                    .to_str()
                    .unwrap()
                    .starts_with(content_type)
            );
            let bytes = response.bytes().await.unwrap();
            if content_type == "image/png" {
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            }
            if path == "/manifest.json" {
                let value: Value = serde_json::from_slice(&bytes).unwrap();
                assert_eq!(value["display"], "standalone");
                assert_eq!(value["icons"][0]["sizes"], "256x256");
            }
        }
    }
    #[test]
    fn remote_artwork_uses_revocable_urls_and_cached_images_without_server_credentials() {
        use crate::model::{Server, ServerType, Session};
        let dir = std::env::temp_dir().join(format!("feishin-remote-art-{}", uuid::Uuid::new_v4()));
        let remote = Remote::start(0, "phone", "secret").unwrap();
        let api = Api::new(
            Server {
                id: uuid::Uuid::new_v4(),
                name: "Music".into(),
                username: "server-user".into(),
                url: "https://music.example.com".into(),
                kind: ServerType::Jellyfin,
                legacy_auth: false,
                options: Default::default(),
            },
            Session {
                token: "server-secret".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let item = Item {
            id: "song".into(),
            cover_id: "cover".into(),
            server_id: api.server.id,
            ..Default::default()
        };
        let handle = remote.artwork();
        let decorated = handle.item_json(&item, &api, &dir);
        let path = handle
            .entries
            .read()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .path
            .clone();
        let bytes = b"\x89PNG\r\n\x1a\nfixture";
        crate::storage::atomic_write(&path, bytes).unwrap();
        let mut state = AppState::default();
        state
            .queue
            .add(vec![item.clone(), item], crate::queue::Enqueue::Now);
        remote.publish_with_artwork(
            &state,
            true,
            &HashMap::from([(api.server.id, api)]),
            &dir,
            None,
        );
        let values = remote.state.snapshot.read().unwrap().clone();
        assert_eq!(values[0]["data"]["song"]["imageUrl"], decorated["imageUrl"]);
        assert_ne!(
            values[1]["data"]["items"][0]["uniqueId"],
            values[1]["data"]["items"][1]["uniqueId"]
        );
        let station = crate::queue::Entry {
            key: uuid::Uuid::new_v4(),
            item: Item {
                id: "station".into(),
                name: "Station".into(),
                kind: ItemKind::Radio,
                ..Default::default()
            },
        };
        let metadata = crate::playback::RadioMetadata {
            title: "Live track".into(),
            artist: "Live artist".into(),
        };
        remote.publish_with_artwork(
            &state,
            true,
            &HashMap::new(),
            &dir,
            Some((&station, &metadata, 12.0)),
        );
        let radio = remote.state.snapshot.read().unwrap().clone();
        assert_eq!(radio[0]["data"]["song"]["id"], "station");
        assert_eq!(radio[0]["data"]["song"]["name"], "Live track");
        assert_eq!(radio[0]["data"]["position"], 12.0);
        assert_eq!(radio[3]["data"]["isActive"], true);
        assert_eq!(radio[3]["data"]["stationName"], "Station");
        assert_eq!(radio[1]["data"]["items"][0]["id"], "song");
        assert_eq!(
            radio[1]["data"]["items"][1]["uniqueId"],
            values[1]["data"]["items"][1]["uniqueId"]
        );
        let encoded = values.iter().map(Value::to_string).collect::<String>();
        assert!(!encoded.contains("server-secret") && !encoded.contains("music.example.com"));
        let client = reqwest::blocking::Client::new();
        let address = format!("http://127.0.0.1:{}", remote.port);
        let response = client
            .get(format!(
                "{address}{}",
                decorated["imageUrl"].as_str().unwrap()
            ))
            .send()
            .unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["content-type"], "image/png");
        assert_eq!(response.bytes().unwrap().as_ref(), bytes);
        let wrong = decorated["imageUrl"]
            .as_str()
            .unwrap()
            .replace(&handle.capability, "wrong");
        assert_eq!(
            client
                .get(format!("{address}{wrong}"))
                .send()
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .get(format!("{address}/artwork/{}/missing", handle.capability))
                .send()
                .unwrap()
                .status(),
            404
        );
        drop(remote);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn remote_reordering_uses_target_edges_in_both_directions_without_losing_duplicates() {
        for count in 2..=8 {
            let mut original = crate::queue::Queue::default();
            original.add(
                (0..count)
                    .map(|index| Item {
                        id: "duplicate".into(),
                        name: format!("Entry {index}"),
                        ..Default::default()
                    })
                    .collect(),
                crate::queue::Enqueue::Now,
            );
            original.position = 42.;
            for shuffle in [false, true] {
                for from in 0..count {
                    for to in 0..count {
                        for edge in ["top", "bottom"] {
                            let mut queue = original.clone();
                            queue.shuffle = shuffle;
                            let key = queue.order[from];
                            let target = queue.order[to];
                            let mut expected = queue.order.clone();
                            if key != target {
                                expected.remove(from);
                                let target = expected.iter().position(|id| *id == target).unwrap();
                                expected.insert(target + usize::from(edge == "bottom"), key);
                            }
                            reorder_queue(
                                &mut queue,
                                &json!({"uniqueId":key,"targetUniqueId":target,"edge":edge}),
                            )
                            .unwrap();
                            assert_eq!(queue.order, expected, "{count}: {from} to {to} {edge}");
                            assert_eq!(queue.position, 42.);
                            assert_eq!(queue.current, original.current);
                            assert_eq!(queue.entries.len(), count);
                            assert_eq!(
                                queue
                                    .ordered()
                                    .iter()
                                    .map(|entry| entry.key)
                                    .collect::<Vec<_>>(),
                                expected
                            );
                        }
                    }
                }
            }
            for value in [
                json!({"uniqueId":"invalid","targetUniqueId":original.order[0],"edge":"top"}),
                json!({"uniqueId":original.order[0],"targetUniqueId":uuid::Uuid::new_v4(),"edge":"top"}),
                json!({"uniqueId":uuid::Uuid::new_v4(),"targetUniqueId":original.order[0],"edge":"bottom"}),
                json!({"uniqueId":original.order[0],"targetUniqueId":original.order[1],"edge":"invalid"}),
            ] {
                let mut queue = original.clone();
                assert!(reorder_queue(&mut queue, &value).is_err());
                assert_eq!(queue.order, original.order);
                assert_eq!(queue.current, original.current);
                assert_eq!(queue.position, original.position);
            }
        }
    }
    #[test]
    fn remote_accents_match_original_generated_colors_and_theme_shades() {
        let reference: Vec<(String, [String; 10])> =
            serde_json::from_str(include_str!("../assets/accent-reference.json")).unwrap();
        let scale = |color: &str| &reference.iter().find(|(name, _)| name == color).unwrap().1;
        let mut state = AppState::default();
        for (color, colors) in &reference {
            for use_theme_accent_color in [false, true] {
                for use_theme_primary_shade in [false, true] {
                    for primary_shade in (0..=10).chain([u8::MAX]) {
                        state.settings.accent = color.clone();
                        state.settings.use_theme_accent_color = use_theme_accent_color;
                        state.settings.use_theme_primary_shade = use_theme_primary_shade;
                        state.settings.primary_shade = primary_shade;
                        let data = snapshot(&state, false);
                        assert_eq!(data[4]["event"], "accent-color");
                        for (mode, builtin, theme_shade) in [
                            ("dark", "rgb(53, 116, 252)", 6),
                            ("light", "rgb(0, 122, 255)", 4),
                        ] {
                            let index = if use_theme_primary_shade {
                                theme_shade
                            } else {
                                usize::from(primary_shade.min(9))
                            };
                            let expected = if use_theme_accent_color {
                                &scale(builtin)[index]
                            } else {
                                &colors[index]
                            };
                            assert_eq!(
                                data[4]["data"][mode], *expected,
                                "{color}: {mode}, theme accent {use_theme_accent_color}, theme shade {use_theme_primary_shade}, shade {primary_shade}"
                            );
                        }
                    }
                }
            }
        }
        state.settings.accent = "invalid-private-value".into();
        state.settings.use_theme_accent_color = false;
        let data = snapshot(&state, true);
        assert_eq!(data[4]["data"], json!({"dark":"#268bd2","light":"#57a6df"}));
        assert!(
            !serde_json::to_string(&data)
                .unwrap()
                .contains("invalid-private-value")
        );
    }
    #[test]
    fn remote_snapshot_contains_no_credentials_and_keeps_duplicate_identity() {
        let mut state = AppState::default();
        state.queue.add(
            vec![
                Item {
                    id: "a".into(),
                    name: "Title".into(),
                    ..Item::default()
                },
                Item {
                    id: "a".into(),
                    ..Item::default()
                },
            ],
            crate::queue::Enqueue::Now,
        );
        let data = snapshot(&state, true);
        let queue = &data[1]["data"]["items"];
        assert_ne!(queue[0]["uniqueId"], queue[1]["uniqueId"]);
        let encoded = serde_json::to_string(&data).unwrap();
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("password"));
        assert_eq!(data[0]["data"]["status"], "playing");
    }
    #[test]
    fn rejects_invalid_remote_credentials() {
        assert!(Remote::start(0, "invalid:name", "secret").is_err());
        assert!(Remote::start(0, "invalid\nname", "secret").is_err());
        assert!(Remote::start(0, "phone", "invalid\0password").is_err());
        assert!(Remote::start(0, "user:bad", "password").is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn websocket_authentication_replies_updates_and_shutdown() {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};
        tokio::time::timeout(Duration::from_secs(5), async {
            let remote = Remote::start(0, "phone", "secret:with:colons").unwrap();
            let address = format!("ws://127.0.0.1:{}", remote.port);
            let mut state = AppState::default();
            remote.publish(&state, false);
            let (mut unauthorized, _) = connect_async(&address).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(50), unauthorized.next())
                    .await
                    .is_err()
            );
            unauthorized
                .send(ClientMessage::Text(
                    json!({"event":"authenticate","header":"Basic wrong"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            let ClientMessage::Close(Some(close)) = unauthorized.next().await.unwrap().unwrap()
            else {
                panic!("Unauthenticated client received application data")
            };
            assert_eq!(u16::from(close.code), 4004);
            let (mut client, _) = connect_async(format!("{address}/ws")).await.unwrap();
            client
                .send(ClientMessage::Text(
                    json!({"event":"authenticate","header":credential("phone", "secret:with:colons")})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            for event in [
                "state",
                "queue-state",
                "confirm-queue-changes-setting",
                "radio-status",
                "accent-color",
            ] {
                let response = client.next().await.unwrap().unwrap().into_text().unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&response).unwrap()["event"],
                    event
                );
            }
            client
                .send(ClientMessage::Text(
                    json!({"event":"pause","requestId":"operation-1"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            let request = loop {
                if let Ok(request) = remote.requests.try_recv() {
                    break request;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            };
            assert_eq!(request.message["requestId"], "operation-1");
            request
                .reply
                .send(vec![message(
                    "operation-ack",
                    json!({"requestId":"operation-1","error":"Test error"}),
                )])
                .unwrap();
            let response = client.next().await.unwrap().unwrap().into_text().unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&response).unwrap()["data"]["error"],
                "Test error"
            );
            state.settings.volume = 27.0;
            remote.publish(&state, true);
            let response = client.next().await.unwrap().unwrap().into_text().unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(&response).unwrap()["data"]["volume"],
                27.0
            );
            for _ in 0..4 {
                client.next().await.unwrap().unwrap();
            }
            // Shutdown must also cancel clients waiting for authentication or an app reply.
            let (_waiting_auth, _) = connect_async(&address).await.unwrap();
            client
                .send(ClientMessage::Text(
                    json!({"event":"play","requestId":"pending"})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(30)).await;
            let port = remote.port;
            drop(remote);
            let _ = client.next().await;
            loop {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Remote server did not complete authentication, operation, or shutdown");
    }
}
