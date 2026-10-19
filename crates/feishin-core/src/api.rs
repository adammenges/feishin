use crate::{
    lyrics::{self, Line},
    model::*,
};
use anyhow::{Context, Result, bail, ensure};
use rand::{Rng, distributions::Alphanumeric};
use reqwest::{Method, blocking::Client};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
use url::Url;

type TagCatalog = std::sync::Arc<crate::tags::Fields>;
type CachedTagCatalog = Option<(std::time::Instant, u64, TagCatalog)>;

#[derive(Clone)]
pub struct Api {
    pub server: Server,
    base_url: String,
    session: std::sync::Arc<std::sync::RwLock<Session>>,
    refresh: std::sync::Arc<std::sync::Mutex<()>>,
    persist_session: std::sync::Arc<std::sync::atomic::AtomicBool>,
    save_password: std::sync::Arc<std::sync::atomic::AtomicBool>,
    client: Client,
    features: std::sync::Arc<std::sync::RwLock<ServerFeatures>>,
    cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    revoked: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) playlist_writes: crate::playlist_sync::WriteLocks,
    tag_catalog: std::sync::Arc<std::sync::Mutex<CachedTagCatalog>>,
    tag_catalog_revision: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

// The authorization secret stays in memory and must never enter logs or settings.
pub struct QuickConnect {
    pub code: String,
    secret: String,
}

#[derive(Clone, Debug, Default)]
pub struct ServerFeatures {
    pub jukebox: bool,
    pub version: String,
    pub index_queue: bool,
    pub native_queue: bool,
    pub form_post: bool,
    pub playback_report: bool,
    pub artist_roles: bool,
    pub artists_search: bool,
    pub track_rating_filter: bool,
    pub top_songs_by_id: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanStatus {
    pub scanning: bool,
    pub count: u64,
    pub folder_count: u64,
    pub last_scan: Option<String>,
}

fn s(value: &Value, key: &str) -> String {
    match &value[key] {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn n(value: &Value, key: &str) -> f64 {
    value[key].as_f64().unwrap_or(0.0)
}
fn array(value: &Value) -> Vec<Value> {
    value.as_array().cloned().unwrap_or_default()
}
fn p(key: &str, value: impl ToString) -> (String, String) {
    (key.into(), value.to_string())
}

pub fn cover_hash(value: &str) -> md5::Digest {
    md5::compute(value)
}

pub fn artwork_key(item: &Item) -> String {
    let identity = if item.kind == ItemKind::Radio {
        format!("radio-{}", item.id)
    } else {
        item.cover_id.clone()
    };
    let identity = if item.cover_revision.is_empty() {
        identity
    } else {
        format!("{identity}:{}", item.cover_revision)
    };
    format!("{}-{:x}", item.server_id, cover_hash(&identity))
}

pub fn is_network_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause.downcast_ref::<reqwest::Error>().is_some_and(|error| {
            error.is_connect()
                || error.is_timeout()
                || error.is_body()
                || (error.is_request() && !error.is_redirect())
        }) || cause.downcast_ref::<std::io::Error>().is_some_and(|error| {
            matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::UnexpectedEof
            )
        })
    })
}

pub fn normalize_url(input: &str) -> Result<String> {
    let url =
        Url::parse(input.trim()).context("Enter a complete http:// or https:// server URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
        "Server URL must use HTTP or HTTPS"
    );
    ensure!(
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "Server URL cannot contain credentials, a query, or a fragment"
    );
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

impl Api {
    pub fn new(mut server: Server, session: Session) -> Result<Self> {
        server.url = normalize_url(&server.url)?;
        if !server.options.remote_url.is_empty() {
            server.options.remote_url = normalize_url(&server.options.remote_url)?;
        }
        ensure!(
            !server.options.prefer_remote_url || !server.options.remote_url.is_empty(),
            "Enter a remote server URL"
        );
        ensure!(
            server.options.music_folder_ids.len() <= 100
                && server
                    .options
                    .music_folder_ids
                    .iter()
                    .all(|id| !id.is_empty()
                        && id.len() <= 256
                        && !id.contains(['\0', '/', '?', '#'])),
            "Invalid library selection"
        );
        let base_url = if server.options.prefer_remote_url {
            server.options.remote_url.clone()
        } else {
            server.url.clone()
        };
        let origin = Url::parse(&base_url)?.origin();
        let client = Client::builder()
            .danger_accept_invalid_certs(server.options.ignore_ssl)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Feishin/1.17.0 (Rust GPUI)")
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 10 {
                    attempt.error("Too many redirects")
                } else if attempt.url().origin() == origin {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()?;
        let save_password = server.kind != ServerType::Navidrome || server.options.save_password;
        Ok(Self {
            server,
            base_url,
            session: std::sync::Arc::new(std::sync::RwLock::new(session)),
            refresh: Default::default(),
            persist_session: Default::default(),
            save_password: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(save_password)),
            client,
            features: Default::default(),
            cancellation: None,
            revoked: Default::default(),
            playlist_writes: Default::default(),
            tag_catalog: Default::default(),
            tag_catalog_revision: Default::default(),
        })
    }

    pub fn with_cancellation(
        mut self,
        cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        self.cancellation = Some(cancelled);
        self
    }
    pub fn without_cancellation(mut self) -> Self {
        self.cancellation = None;
        self
    }
    fn check_cancellation(&self) -> Result<()> {
        ensure!(!self.is_signed_out(), "Sign in to this server again");
        ensure!(
            !self
                .cancellation
                .as_ref()
                .is_some_and(|flag| flag.load(std::sync::atomic::Ordering::Relaxed)),
            "Library fetch cancelled"
        );
        Ok(())
    }
    pub fn reconfigure(&self, server: Server) -> Result<Self> {
        ensure!(
            server.id == self.server.id
                && server.kind == self.server.kind
                && server.username == self.server.username,
            "Sign in to change the server account"
        );
        let password_choice_changed =
            self.server.options.save_password != server.options.save_password;
        let mut api = Self::new(server, self.session())?;
        api.session = self.session.clone();
        api.refresh = self.refresh.clone();
        api.persist_session = self.persist_session.clone();
        api.save_password = self.save_password.clone();
        api.features = self.features.clone();
        api.revoked = self.revoked.clone();
        api.playlist_writes = self.playlist_writes.clone();
        let _guard = api
            .refresh
            .lock()
            .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
        api.save_password.store(
            api.server.kind != ServerType::Navidrome || api.server.options.save_password,
            std::sync::atomic::Ordering::SeqCst,
        );
        if password_choice_changed
            && !api.is_signed_out()
            && api
                .persist_session
                .load(std::sync::atomic::Ordering::SeqCst)
        {
            // A failed removal must stay visible and be retryable. Old request
            // clones already share the new policy, so they cannot restore it.
            crate::storage::store_session(&api.server, &api.stored_session())?;
        } else {
            api.persist_credentials();
        }
        drop(_guard);
        Ok(api)
    }

    fn url(&self, path: &str, params: &[(String, String)]) -> Result<Url> {
        ensure!(
            !path.contains(['?', '#', '\\', '%'])
                && !path.split('/').any(|part| matches!(part, "." | "..")),
            "Invalid server resource path"
        );
        let mut url = Url::parse(&format!(
            "{}/{}",
            self.base_url,
            path.trim_start_matches('/')
        ))?;
        if !params.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(params.iter().map(|(k, v)| (k, v)));
        }
        Ok(url)
    }

    fn jf_header(&self, token: &str) -> String {
        let mut header = format!(
            "MediaBrowser Client=\"Feishin\", Device=\"GPUI\", DeviceId=\"{}\", Version=\"1.17.0\"",
            self.server.id
        );
        if !token.is_empty() {
            header.push_str(&format!(", Token=\"{token}\""));
        }
        header
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        params: &[(String, String)],
        body: Option<Value>,
    ) -> Result<(Value, Option<usize>)> {
        self.request_with_retry(method, path, params, body, true)
    }
    fn request_with_retry(
        &self,
        method: Method,
        path: &str,
        params: &[(String, String)],
        body: Option<Value>,
        retry: bool,
    ) -> Result<(Value, Option<usize>)> {
        self.check_cancellation()?;
        let used_session = self.session();
        let used_token = used_session.token;
        let used_user = if used_session.user_id.is_empty() {
            &self.server.username
        } else {
            &used_session.user_id
        };
        let user_path = format!("Users/{used_user}");
        let form = method == Method::POST && path.starts_with("rest/") && body.is_none();
        let mut request = self.client.request(
            method.clone(),
            self.url(path, if form { &[] } else { params })?,
        );
        if form {
            request = request.form(params);
        }
        match self.server.kind {
            ServerType::Jellyfin => {
                request = request.header(
                    "Authorization",
                    self.jf_header(if path == "Users/AuthenticateByName" {
                        ""
                    } else {
                        &used_token
                    }),
                );
            }
            ServerType::Navidrome if path.starts_with("api/") => {
                request = request.header("x-nd-authorization", format!("Bearer {used_token}"));
            }
            _ => {}
        }
        if let Some(body) = &body {
            request = request.json(&body);
        }
        let response = request.send().map_err(|e| e.without_url())?;
        self.check_cancellation()?;
        let status = response.status();
        if (status.as_u16() == 401
            || (status.as_u16() == 403
                && self.server.kind == ServerType::Jellyfin
                && path == user_path))
            && retry
            && self.can_renew_session(path)
        {
            self.renew_session(&used_token)?;
            // Password sign-in can return a new identity after an account was recreated.
            let path = if self.server.kind == ServerType::Jellyfin
                && (path == user_path || path.starts_with(&format!("{user_path}/")))
            {
                format!(
                    "Users/{}{}",
                    self.session().user_id,
                    &path[user_path.len()..]
                )
            } else {
                path.to_owned()
            };
            return self.request_with_retry(method, &path, params, body, false);
        }
        if status.as_u16() == 401
            && self.server.kind == ServerType::Jellyfin
            && (path.starts_with("QuickConnect/") || path == "Users/AuthenticateWithQuickConnect")
            && used_token.is_empty()
        {
            if path == "QuickConnect/Initiate" {
                bail!("Quick Connect is not active on this server");
            }
            bail!("Quick Connect request expired or was disabled");
        }
        self.check_authorization(status.as_u16())?;
        ensure!(
            status.is_success(),
            "Server request failed (HTTP {})",
            status.as_u16()
        );
        if self.server.kind == ServerType::Navidrome && path.starts_with("api/") {
            self.rotate_session(response.headers(), &used_token)?;
        }
        let total = response
            .headers()
            .get("x-total-count")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        use std::io::Read;
        let mut bytes = Vec::new();
        response.take(20_000_001).read_to_end(&mut bytes)?;
        self.check_cancellation()?;
        ensure!(bytes.len() <= 20_000_000, "Server response is too large");
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).context("Server returned an invalid JSON response")?
        };
        Ok((body, total))
    }

    fn check_authorization(&self, status: u16) -> Result<()> {
        if status == 401 {
            let session = self.session();
            if !session.token.is_empty()
                || !session.subsonic_token.is_empty()
                || session.legacy_password.is_some()
            {
                self.sign_out();
            }
            bail!("Authentication expired. Sign in again.");
        }
        if status == 403 {
            bail!("Access denied by the server.");
        }
        Ok(())
    }

    fn can_renew_session(&self, path: &str) -> bool {
        match self.server.kind {
            ServerType::Navidrome => path.starts_with("api/"),
            ServerType::Jellyfin => {
                let session = self.session();
                path != "Users/AuthenticateByName"
                    && !session.token.is_empty()
                    && session.refresh_password.is_some()
            }
            ServerType::Subsonic => false,
        }
    }
    fn renew_session(&self, used_token: &str) -> Result<()> {
        let _guard = self
            .refresh
            .lock()
            .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
        self.check_cancellation()?;
        let session = self.session();
        if session.token == used_token {
            let Some(password) = session.refresh_password.as_deref() else {
                self.sign_out();
                bail!("Authentication expired. Sign in again to renew this saved session.");
            };
            let renewed = if self.server.kind == ServerType::Jellyfin {
                self.jellyfin_login(password)?
            } else {
                self.navidrome_login(password)?
            };
            self.check_cancellation()?;
            *self.session.write().unwrap() = renewed;
            self.persist_credentials();
        }
        Ok(())
    }
    fn rotate_session(&self, headers: &reqwest::header::HeaderMap, used_token: &str) -> Result<()> {
        if let Some(token) = headers
            .get("x-nd-authorization")
            .and_then(|value| value.to_str().ok())
            .filter(|value| !value.is_empty() && value.len() <= 16_384)
        {
            let token = token.strip_prefix("Bearer ").unwrap_or(token);
            let _guard = self
                .refresh
                .lock()
                .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
            let changed = {
                let mut session = self.session.write().unwrap();
                if session.token == used_token && session.token != token {
                    session.token = token.into();
                    true
                } else {
                    false
                }
            };
            if changed {
                self.persist_credentials();
            }
        }
        Ok(())
    }

    fn ss_params(&self) -> Vec<(String, String)> {
        let mut params = vec![
            p("u", &self.server.username),
            p("v", "1.16.1"),
            p("c", "Feishin"),
            p("f", "json"),
        ];
        let session = self.session();
        if let Some(password) = &session.legacy_password {
            params.push(p("p", password));
        } else {
            params.extend([p("s", &session.salt), p("t", &session.subsonic_token)]);
        }
        params
    }

    pub fn subsonic(&self, endpoint: &str, params: &[(String, String)]) -> Result<Value> {
        let mut query = self.ss_params();
        query.extend_from_slice(params);
        let (value, _) = self.request(
            if self.features().form_post {
                Method::POST
            } else {
                Method::GET
            },
            &format!("rest/{endpoint}.view"),
            &query,
            None,
        )?;
        let value = value
            .get("subsonic-response")
            .context("Missing Subsonic response envelope")?;
        let error_code = value["error"]["code"].as_f64().or_else(|| {
            value["error"]["code"]
                .as_str()
                .and_then(|code| code.trim().parse::<f64>().ok())
        });
        if value["status"] != "ok" && error_code == Some(40.0) {
            let session = self.session();
            if !session.token.is_empty()
                || !session.subsonic_token.is_empty()
                || session.legacy_password.is_some()
            {
                self.sign_out();
            }
        }
        // Navidrome uses error 0 when optional external metadata is unlinked.
        // Mutations must still reject a failed response carrying that generic code.
        ensure!(
            value["status"] == "ok"
                || (value["error"]["code"].as_u64() == Some(0)
                    && matches!(
                        endpoint,
                        "getArtistInfo"
                            | "getArtistInfo2"
                            | "getSimilarSongs"
                            | "getSimilarSongs2"
                            | "getTopSongs"
                    )),
            "Subsonic error {}: {}",
            s(&value["error"], "code"),
            s(&value["error"], "message")
        );
        Ok(value.clone())
    }

    pub fn features(&self) -> ServerFeatures {
        self.features
            .read()
            .map(|features| features.clone())
            .unwrap_or_default()
    }
    pub fn validate_session(&self) -> Result<()> {
        let session = self.session();
        let is_admin = if self.server.kind == ServerType::Jellyfin {
            let id = if session.user_id.is_empty() {
                &self.server.username
            } else {
                &session.user_id
            };
            ensure!(!id.is_empty(), "No user ID or username available");
            ensure!(!id.contains(['/', '?', '#']), "Invalid user ID");
            let (user, _) = self.request(Method::GET, &format!("Users/{id}"), &[], None)?;
            ensure!(
                user["Id"].as_str().is_some_and(|id| !id.is_empty()),
                "Failed to get user info"
            );
            user["Policy"]["IsAdministrator"].as_bool().unwrap_or(false)
        } else {
            ensure!(
                !self.server.username.is_empty(),
                "No user ID or username available"
            );
            let value = self.subsonic("getUser", &[p("username", &self.server.username)])?;
            ensure!(
                value["user"]["username"]
                    .as_str()
                    .is_some_and(|id| !id.is_empty()),
                "Failed to get user info"
            );
            value["user"]["adminRole"].as_bool().unwrap_or(false)
        };
        // Update only permissions: requests may have rotated the shared token meanwhile.
        let _guard = self
            .refresh
            .lock()
            .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
        self.check_cancellation()?;
        self.session
            .write()
            .map_err(|_| anyhow::anyhow!("Session update lock failed"))?
            .is_admin = is_admin;
        self.persist_credentials();
        Ok(())
    }

    pub fn discover_features(&self) {
        if self.server.kind == ServerType::Jellyfin {
            return;
        }
        let mut features = ServerFeatures {
            jukebox: self.server.kind == ServerType::Navidrome,
            ..Default::default()
        };
        match self.subsonic("ping", &[]) {
            Ok(value) => {
                features.version = s(&value, "serverVersion");
                let version = features
                    .version
                    .split('-')
                    .next()
                    .unwrap_or_default()
                    .split('.')
                    .take(3)
                    .map(|number| number.parse::<u32>().unwrap_or(0))
                    .collect::<Vec<_>>();
                features.native_queue = self.server.kind == ServerType::Navidrome
                    && version.len() == 3
                    && version.as_slice() >= [0, 57, 0].as_slice();
                features.artist_roles = self.server.kind == ServerType::Navidrome
                    && version.len() == 3
                    && version.as_slice() >= [0, 55, 0].as_slice();
                features.track_rating_filter = self.server.kind == ServerType::Navidrome
                    && version.len() == 3
                    && version.as_slice() >= [0, 60, 4].as_slice();
                features.artists_search = self.server.kind == ServerType::Navidrome
                    && version.len() == 3
                    && version.as_slice() >= [0, 56, 0].as_slice();
            }
            Err(error) => log::debug!("Server version unavailable: {error}"),
        }
        match self.subsonic("getOpenSubsonicExtensions", &[]) {
            Ok(value) => {
                for extension in array(&value["openSubsonicExtensions"]) {
                    if !array(&extension["versions"])
                        .iter()
                        .any(|version| version.as_u64() == Some(1))
                    {
                        continue;
                    }
                    match extension["name"].as_str().unwrap_or_default() {
                        "indexBasedQueue" => features.index_queue = true,
                        "formPost" => features.form_post = true,
                        "playbackReport" => features.playback_report = true,
                        "topSongsByArtistId" => features.top_songs_by_id = true,
                        _ => {}
                    }
                }
            }
            Err(error) => log::debug!("OpenSubsonic extensions unavailable: {error}"),
        }
        if self.server.kind == ServerType::Subsonic {
            features.jukebox = self
                .jukebox(crate::jukebox::Command::Status)
                .is_ok_and(|state| state.is_some());
        }
        if let Ok(mut stored) = self.features.write() {
            *stored = features;
        }
    }

    pub fn jukebox(
        &self,
        command: crate::jukebox::Command<'_>,
    ) -> Result<Option<crate::jukebox::Snapshot>> {
        ensure!(
            self.server.kind != ServerType::Jellyfin,
            "Jukebox requires Navidrome or Subsonic"
        );
        let query = command.query()?;
        let value = self.subsonic("jukeboxControl", &query)?;
        crate::jukebox::Snapshot::parse(self, &value)
    }

    pub fn authenticate(server: Server, password: &str) -> Result<(Self, Session)> {
        ensure!(!server.username.trim().is_empty(), "Enter a username");
        let api = Self::new(server, Session::default())?;
        let mut session = Session::default();
        match api.server.kind {
            ServerType::Navidrome => session = api.navidrome_login(password)?,
            ServerType::Jellyfin => session = api.jellyfin_login(password)?,
            ServerType::Subsonic => {
                if api.server.legacy_auth {
                    session.legacy_password = Some(password.into());
                } else {
                    session.salt = rand::thread_rng()
                        .sample_iter(Alphanumeric)
                        .take(16)
                        .map(char::from)
                        .collect();
                    session.subsonic_token =
                        format!("{:x}", md5::compute(format!("{password}{}", session.salt)));
                }
                *api.session.write().unwrap() = session.clone();
                let v = api.subsonic("getUser", &[p("username", &api.server.username)])?;
                session.user_id = api.server.username.clone();
                session.is_admin = v["user"]["adminRole"].as_bool().unwrap_or(false);
            }
        }
        *api.session.write().unwrap() = session.clone();
        Ok((api, session))
    }

    fn jellyfin_login(&self, password: &str) -> Result<Session> {
        let (value, _) = self.request_with_retry(
            Method::POST,
            "Users/AuthenticateByName",
            &[],
            Some(json!({"Username": self.server.username, "Pw": password})),
            false,
        )?;
        let mut session = Self::jellyfin_session(&value)?;
        session.refresh_password = Some(password.into());
        Ok(session)
    }
    fn jellyfin_session(value: &Value) -> Result<Session> {
        let session = Session {
            token: value["AccessToken"].as_str().unwrap_or_default().into(),
            user_id: value["User"]["Id"].as_str().unwrap_or_default().into(),
            is_admin: value["User"]["Policy"]["IsAdministrator"]
                .as_bool()
                .unwrap_or(false),
            ..Session::default()
        };
        ensure!(
            !session.token.is_empty() && !session.user_id.is_empty(),
            "Jellyfin returned no authentication token"
        );
        Ok(session)
    }

    pub fn initiate_quick_connect(&self) -> Result<QuickConnect> {
        ensure!(
            self.server.kind == ServerType::Jellyfin,
            "Quick Connect requires Jellyfin"
        );
        let (value, _) = self.request(Method::POST, "QuickConnect/Initiate", &[], None)?;
        let code = value["Code"].as_str().unwrap_or_default().to_owned();
        let secret = value["Secret"].as_str().unwrap_or_default().to_owned();
        ensure!(
            !code.is_empty() && code.len() <= 128 && !secret.is_empty() && secret.len() <= 16_384,
            "Quick Connect is not active on this server"
        );
        Ok(QuickConnect { code, secret })
    }

    pub fn quick_connect_authenticated(&self, request: &QuickConnect) -> Result<bool> {
        ensure!(
            self.server.kind == ServerType::Jellyfin,
            "Quick Connect requires Jellyfin"
        );
        let (value, _) = self.request(
            Method::GET,
            "QuickConnect/Connect",
            &[p("secret", &request.secret)],
            None,
        )?;
        value["Authenticated"]
            .as_bool()
            .context("Quick Connect request expired")
    }

    pub fn authenticate_quick_connect(mut self, request: &QuickConnect) -> Result<Self> {
        ensure!(
            self.server.kind == ServerType::Jellyfin,
            "Quick Connect requires Jellyfin"
        );
        let (value, _) = self.request(
            Method::POST,
            "Users/AuthenticateWithQuickConnect",
            &[],
            Some(json!({"Secret": request.secret})),
        )?;
        let session = Self::jellyfin_session(&value)?;
        self.server.username = value["User"]["Name"].as_str().unwrap_or_default().into();
        ensure!(
            !self.server.username.trim().is_empty(),
            "Jellyfin returned no username"
        );
        *self.session.write().unwrap() = session;
        Ok(self)
    }
    fn navidrome_login(&self, password: &str) -> Result<Session> {
        let (v, _) = self.request(
            Method::POST,
            "auth/login",
            &[],
            Some(json!({"username": self.server.username, "password": password})),
        )?;
        let session = Session {
            token: s(&v, "token"),
            user_id: s(&v, "id"),
            salt: s(&v, "subsonicSalt"),
            subsonic_token: s(&v, "subsonicToken"),
            is_admin: v["isAdmin"].as_bool().unwrap_or(false),
            refresh_password: Some(password.into()),
            ..Session::default()
        };
        ensure!(
            !session.token.is_empty() && !session.subsonic_token.is_empty(),
            "Navidrome returned no authentication token"
        );
        Ok(session)
    }
    pub fn session(&self) -> Session {
        if self.is_signed_out() {
            return Session::default();
        }
        self.session
            .read()
            .map(|session| session.clone())
            .unwrap_or_default()
    }
    pub fn is_signed_out(&self) -> bool {
        self.revoked.load(std::sync::atomic::Ordering::SeqCst)
    }
    pub fn sign_out(&self) {
        self.revoked
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.persist_session
            .store(false, std::sync::atomic::Ordering::SeqCst);
        *self
            .session
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Session::default();
    }
    pub fn forget_saved_session(&self) -> Result<()> {
        // The background cleanup waits for an existing renewal or keychain write.
        let _guard = self
            .refresh
            .lock()
            .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
        crate::storage::delete_session(&self.server)
    }
    pub fn remember_session(&self) -> Result<()> {
        let _guard = self
            .refresh
            .lock()
            .map_err(|_| anyhow::anyhow!("Session renewal lock failed"))?;
        self.check_cancellation()?;
        crate::storage::store_session(&self.server, &self.stored_session())?;
        self.enable_session_persistence();
        Ok(())
    }
    pub fn enable_session_persistence(&self) {
        self.persist_session
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
    fn stored_session(&self) -> Session {
        let mut session = self.session();
        if !self.save_password.load(std::sync::atomic::Ordering::SeqCst) {
            session.refresh_password = None;
        }
        session
    }
    fn persist_credentials(&self) {
        if !self.is_signed_out()
            && self
                .persist_session
                .load(std::sync::atomic::Ordering::SeqCst)
            && crate::storage::store_session(&self.server, &self.stored_session()).is_err()
        {
            log::warn!("Could not update renewed server credentials in the OS password store");
        }
    }

    pub fn normalize(&self, v: &Value, kind: ItemKind) -> Item {
        let mut item = Item {
            server_id: self.server.id,
            kind,
            ..Item::default()
        };
        if self.server.kind == ServerType::Jellyfin {
            item.id = s(v, "Id");
            item.name = s(v, "Name");
            item.artist = array(&v["Artists"])
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            if item.artist.is_empty() {
                item.artist = s(v, "AlbumArtist");
            }
            item.album = s(v, "Album");
            item.album_id = s(v, "AlbumId");
            item.artist_id = s(&v["ArtistItems"][0], "Id");
            item.album_artist = s(v, "AlbumArtist");
            item.album_artist_id = s(&v["AlbumArtists"][0], "Id");
            item.musicbrainz_id = s(
                &v["ProviderIds"],
                if kind == ItemKind::Album {
                    "MusicBrainzAlbum"
                } else {
                    "MusicBrainzArtist"
                },
            );
            item.musicbrainz_release_group_id = s(&v["ProviderIds"], "MusicBrainzReleaseGroup");
            item.duration = n(v, "RunTimeTicks") / 10_000_000.0;
            item.resumable = s(v, "Type") == "AudioBook";
            if item.resumable {
                let position = n(&v["UserData"], "PlaybackPositionTicks") / 10_000_000.0;
                if position.is_finite()
                    && position > 0.0
                    && (item.duration <= 0.0 || position < item.duration)
                {
                    item.resume_position = position;
                }
            }
            item.track = n(v, "IndexNumber") as u32;
            item.disc = n(v, "ParentIndexNumber") as u32;
            item.year = n(v, "ProductionYear") as u32;
            item.favorite = v["UserData"]["IsFavorite"].as_bool().unwrap_or(false);
            item.rating = n(&v["UserData"], "Rating") as u8;
            item.play_count = n(&v["UserData"], "PlayCount") as u32;
            item.genre = array(&v["Genres"])
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            item.cover_id = if !v["ImageTags"]["Primary"].is_null() {
                item.id.clone()
            } else {
                item.album_id.clone()
            };
            item.blur_hash = v["ImageTags"]["Primary"]
                .as_str()
                .and_then(|tag| v["ImageBlurHashes"]["Primary"][tag].as_str())
                .unwrap_or_default()
                .into();
            item.comment = s(v, "Overview");
            item.path = s(v, "Path");
            item.container = s(v, "Container");
            if item.container.is_empty() {
                item.container = s(&v["MediaSources"][0], "Container");
            }
            item.playlist_entry_id = s(v, "PlaylistItemId");
            item.public = v["IsPublic"].as_bool().unwrap_or(false);
            if v["IsFolder"] == true && kind == ItemKind::Song {
                item.kind = ItemKind::Folder;
            }
            if item.kind == ItemKind::Song && item.album_id.is_empty() && !item.id.is_empty() {
                item.album_id = format!("dummy/{}", item.id);
            }
        } else {
            item.id = s(v, "id");
            item.name = s(v, "title");
            if item.name.is_empty() {
                item.name = s(v, "name");
            }
            if item.name.is_empty() {
                item.name = s(v, "value");
            }
            if kind == ItemKind::Genre && item.id.is_empty() {
                item.id = item.name.clone();
            }
            item.artist = s(v, "artist");
            if item.artist.is_empty() {
                item.artist = s(v, "albumArtist");
            }
            item.album = s(v, "album");
            item.path = s(v, "path");
            item.container = s(v, "suffix");
            let gain = |native: &str, subsonic: &str| {
                v[native]
                    .as_f64()
                    .or_else(|| v["replayGain"][subsonic].as_f64())
                    .map(|v| v as f32)
            };
            item.gain_track = gain("rgTrackGain", "trackGain");
            item.gain_album = gain("rgAlbumGain", "albumGain");
            item.peak_track = gain("rgTrackPeak", "trackPeak");
            item.peak_album = gain("rgAlbumPeak", "albumPeak");
            if v["isDir"] == true {
                item.kind = ItemKind::Folder;
            }
            item.album_id = s(v, "albumId");
            item.artist_id = s(v, "artistId");
            item.album_artist = s(v, "albumArtist");
            item.album_artist_id = s(v, "albumArtistId");
            item.roles = v["roles"]
                .as_object()
                .map(|roles| roles.keys().cloned().collect())
                .unwrap_or_default();
            item.duration = n(v, "duration");
            item.track = n(v, "track") as u32;
            if item.track == 0 {
                item.track = n(v, "trackNumber") as u32;
            }
            item.disc = n(v, "discNumber") as u32;
            item.year = n(v, "year") as u32;
            item.favorite = v["starred"]
                .as_bool()
                .unwrap_or_else(|| v["starred"].as_str().is_some_and(|s| !s.is_empty()));
            item.rating = n(v, "userRating").max(n(v, "rating")) as u8;
            item.play_count = n(v, "playCount") as u32;
            item.cover_id = s(v, "coverArt");
            if self.server.kind == ServerType::Navidrome {
                item.thumb_hash = s(v, "thumbHash");
                item.dominant_color = s(v, "dominantColor");
            }
            if item.cover_id.is_empty() && self.server.kind == ServerType::Navidrome {
                item.cover_id = match kind {
                    ItemKind::Artist | ItemKind::AlbumArtist => format!("ar-{}", item.id),
                    ItemKind::Playlist => format!("pl-{}", item.id),
                    _ => item.id.clone(),
                };
            }
            if v["uploadedImage"]
                .as_str()
                .is_some_and(|image| !image.is_empty())
            {
                item.cover_revision = s(v, "updatedAt");
            }
            item.genre = s(v, "genre");
            item.comment = s(v, "comment");
            item.stream_url = s(v, "streamUrl");
            item.musicbrainz_id = s(
                v,
                if kind == ItemKind::Album {
                    "mbzAlbumId"
                } else {
                    "mbzArtistId"
                },
            );
            item.musicbrainz_release_group_id = s(v, "mbzReleaseGroupId");
            item.homepage = s(v, "homePageUrl");
            if item.homepage.is_empty() {
                item.homepage = s(v, "homepageUrl");
            }
            item.public = v["public"].as_bool().unwrap_or(false);
            item.owner_id = s(v, "ownerId");
            item.owner_name = s(v, "ownerName");
            if self.server.kind == ServerType::Subsonic {
                item.owner_id = s(v, "owner");
                item.owner_name.clone_from(&item.owner_id);
            }
            item.sync = v["sync"].as_bool();
            if kind == ItemKind::Playlist && self.server.kind == ServerType::Navidrome {
                item.cover_revision = s(v, "updatedAt");
            }
            item.rules = if v["rules"].is_null() {
                None
            } else if let Some(raw) = v["rules"].as_str() {
                serde_json::from_str(raw).ok()
            } else {
                Some(v["rules"].clone())
            };
            if v["isDir"] == true {
                item.kind = ItemKind::Folder;
            }
        }
        if self.server.kind == ServerType::Jellyfin {
            item.album_artist_ids = array(&v["AlbumArtists"])
                .iter()
                .map(|value| s(value, "Id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.album_artist_names = array(&v["AlbumArtists"])
                .iter()
                .map(|value| s(value, "Name"))
                .filter(|name| !name.is_empty())
                .collect();
        } else {
            let artist_values = if self.server.kind == ServerType::Navidrome {
                &v["participants"]["albumartist"]
            } else if kind == ItemKind::Album {
                &v["artists"]
            } else {
                &v["albumArtists"]
            };
            item.album_artist_ids = array(artist_values)
                .iter()
                .map(|value| s(value, "id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.album_artist_names = array(artist_values)
                .iter()
                .map(|value| s(value, "name"))
                .filter(|name| !name.is_empty())
                .collect();
            if item.album_artist_ids.is_empty() && !item.album_artist_id.is_empty() {
                item.album_artist_ids.push(item.album_artist_id.clone());
            }
            if item.album_artist_ids.is_empty()
                && self.server.kind == ServerType::Subsonic
                && !item.artist_id.is_empty()
            {
                item.album_artist_ids.push(item.artist_id.clone());
            }
            item.release_types = array(&v["releaseTypes"])
                .iter()
                .chain(array(&v["tags"]["releasetype"]).iter())
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
            let primary = s(v, "mbzAlbumType");
            if !primary.is_empty() {
                item.release_types.push(primary);
            }
            item.is_compilation = v["compilation"].as_bool().unwrap_or(false);
        }
        if self.server.kind == ServerType::Jellyfin {
            item.artist_ids = array(&v["ArtistItems"])
                .iter()
                .map(|value| s(value, "Id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.artist_names = array(&v["Artists"])
                .iter()
                .filter_map(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect();
            if item.artist_names.is_empty() {
                item.artist_names = array(&v["ArtistItems"])
                    .iter()
                    .map(|value| s(value, "Name"))
                    .filter(|name| !name.is_empty())
                    .collect();
            }
            item.genre_ids = array(&v["GenreItems"])
                .iter()
                .map(|value| s(value, "Id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.genre_names = array(&v["GenreItems"])
                .iter()
                .map(|value| s(value, "Name"))
                .filter(|name| !name.is_empty())
                .collect();
            if item.genre_names.is_empty() {
                item.genre_names = array(&v["Genres"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect();
            }
        } else {
            let artists = if self.server.kind == ServerType::Navidrome {
                &v["participants"]["artist"]
            } else {
                &v["artists"]
            };
            item.artist_ids = array(artists)
                .iter()
                .map(|value| s(value, "id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.artist_names = array(artists)
                .iter()
                .map(|value| s(value, "name"))
                .filter(|name| !name.is_empty())
                .collect();
            item.genre_ids = array(&v["genres"])
                .iter()
                .map(|value| s(value, "id"))
                .filter(|id| !id.is_empty())
                .collect();
            item.genre_names = array(&v["genres"])
                .iter()
                .map(|value| s(value, "name"))
                .filter(|name| !name.is_empty())
                .collect();
            if item.genre_names.is_empty() && !item.genre.is_empty() {
                item.genre_names.push(item.genre.clone());
            }
            if self.server.kind == ServerType::Subsonic && item.genre_ids.is_empty() {
                item.genre_ids.clone_from(&item.genre_names);
            }
        }
        let (artists, album_artists) = crate::artists::normalize(v, self.server.kind, kind);
        item.artist_links = Some(artists);
        item.album_artist_links = Some(album_artists);
        item.participants = crate::artists::participants(v, self.server.kind, kind);
        item.genre_links = Some(crate::genres::normalize(v, self.server.kind));
        let audio_stream = v["MediaStreams"]
            .as_array()
            .or_else(|| v["MediaSources"][0]["MediaStreams"].as_array())
            .and_then(|streams| {
                streams
                    .iter()
                    .find(|stream| stream["Type"].as_str() == Some("Audio"))
                    .or_else(|| {
                        streams
                            .iter()
                            .find(|stream| stream["SampleRate"].is_number())
                    })
            })
            .unwrap_or(&Value::Null);
        let fields = if self.server.kind == ServerType::Jellyfin {
            vec![
                ("sortName", &v["SortName"]),
                ("mbzTrackId", &v["ProviderIds"]["MusicBrainzTrack"]),
                ("mbzAlbumId", &v["ProviderIds"]["MusicBrainzAlbum"]),
                ("communityRating", &v["CommunityRating"]),
                ("criticRating", &v["CriticRating"]),
                ("createdAt", &v["DateCreated"]),
                ("date", &v["PremiereDate"]),
                ("releaseDate", &v["PremiereDate"]),
                ("albumCount", &v["AlbumCount"]),
                (
                    "songCount",
                    if matches!(kind, ItemKind::Artist | ItemKind::AlbumArtist) {
                        &v["SongCount"]
                    } else {
                        &v["ChildCount"]
                    },
                ),
                ("lastPlayedAt", &v["UserData"]["LastPlayedDate"]),
                ("bitRate", &v["MediaSources"][0]["Bitrate"]),
                ("size", &v["Size"]),
                ("channels", &audio_stream["Channels"]),
                ("sampleRate", &audio_stream["SampleRate"]),
                ("bitDepth", &audio_stream["BitDepth"]),
            ]
        } else {
            vec![
                ("sortName", &v["sortName"]),
                ("mbzTrackId", &v["mbzReleaseTrackId"]),
                (
                    "mbzRecordingId",
                    if self.server.kind == ServerType::Navidrome {
                        &v["mbzRecordingID"]
                    } else {
                        &v["musicBrainzId"]
                    },
                ),
                (
                    "mbzAlbumId",
                    if self.server.kind == ServerType::Navidrome {
                        &v["mbzAlbumId"]
                    } else {
                        &Value::Null
                    },
                ),
                ("updatedAt", &v["updatedAt"]),
                ("starredAt", &v["starredAt"]),
                ("explicitStatus", &v["explicitStatus"]),
                ("createdAt", &v["createdAt"]),
                ("date", &v["date"]),
                ("releaseDate", &v["releaseDate"]),
                ("releaseYear", &v["releaseYear"]),
                ("albumCount", &v["albumCount"]),
                ("songCount", &v["songCount"]),
                ("lastPlayedAt", &v["playedAt"]),
                ("bitRate", &v["bitRate"]),
                ("size", &v["size"]),
                ("channels", &v["channels"]),
                ("sampleRate", &v["samplingRate"]),
                ("bitDepth", &v["bitDepth"]),
                ("bpm", &v["bpm"]),
            ]
        };
        for (key, value) in fields {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(n) => n.to_string(),
                Value::Array(items) => items
                    .iter()
                    .filter_map(|item| item.as_str().or_else(|| item["name"].as_str()))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            if !text.is_empty() {
                item.column_data.insert(key.into(), text);
            }
        }
        if self.server.kind == ServerType::Jellyfin
            && let Some(rate) = item
                .column_data
                .get("bitRate")
                .and_then(|rate| rate.parse::<f64>().ok())
        {
            item.column_data
                .insert("bitRate".into(), (rate / 1000.).to_string());
        }
        if kind == ItemKind::Song {
            let source = &v["MediaSources"][0];
            let audio = source["MediaStreams"]
                .as_array()
                .and_then(|streams| streams.iter().find(|stream| stream["Type"] == "Audio"))
                .unwrap_or(&Value::Null);
            let jf = self.server.kind == ServerType::Jellyfin;
            let nd = self.server.kind == ServerType::Navidrome;
            for (key, value) in [
                (
                    "createdAt",
                    if jf {
                        &v["DateCreated"]
                    } else if nd {
                        &v["createdAt"]
                    } else {
                        &v["created"]
                    },
                ),
                (
                    "sortName",
                    if jf {
                        if v["SortName"].as_str().is_some_and(|name| !name.is_empty()) {
                            &v["SortName"]
                        } else {
                            &v["Name"]
                        }
                    } else if nd {
                        &v["orderTitle"]
                    } else {
                        &v["title"]
                    },
                ),
                (
                    "bitDepth",
                    if jf {
                        &audio["BitDepth"]
                    } else {
                        &v["bitDepth"]
                    },
                ),
                (
                    "sampleRate",
                    if jf {
                        &audio["SampleRate"]
                    } else if nd {
                        &v["sampleRate"]
                    } else {
                        &v["samplingRate"]
                    },
                ),
                (
                    "channels",
                    if jf {
                        &audio["Channels"]
                    } else if nd {
                        &v["channels"]
                    } else {
                        &v["channelCount"]
                    },
                ),
                ("bpm", if jf { &Value::Null } else { &v["bpm"] }),
                ("size", if jf { &source["Size"] } else { &v["size"] }),
            ] {
                item.column_data.remove(key);
                let nonzero = matches!(key, "bitDepth" | "sampleRate" | "channels" | "bpm");
                match value {
                    Value::String(value) if !value.is_empty() => {
                        item.column_data.insert(key.into(), value.clone());
                    }
                    Value::Number(value) if !nonzero || value.as_f64() != Some(0.) => {
                        item.column_data.insert(key.into(), value.to_string());
                    }
                    _ => {}
                }
            }
            if !nd {
                let rate = if jf {
                    (n(audio, "BitRate") / 1000.).trunc()
                } else {
                    n(v, "bitRate")
                };
                item.column_data.insert("bitRate".into(), rate.to_string());
                item.disc = item.disc.max(1);
            }
            if jf && v["MediaSources"].as_array().is_none_or(Vec::is_empty) {
                item.column_data.insert("size".into(), "0".into());
            }
            item.column_data.insert(
                "discNumber".into(),
                if nd && v["discNumber"].is_null() {
                    String::new()
                } else {
                    item.disc.to_string()
                },
            );
        }
        crate::album_metadata::normalize_details(&mut item, v, self.server.kind);
        crate::information::normalize(&mut item, v, self.server.kind);
        if let Some(composers) = item.participants.get("composer") {
            item.column_data.insert(
                "composer".into(),
                composers
                    .iter()
                    .map(|(_, name)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if self.server.kind == ServerType::Navidrome
            && matches!(kind, ItemKind::Artist | ItemKind::AlbumArtist)
        {
            item.comment = s(v, "biography");
            if v["stats"].is_object() {
                for key in ["albumCount", "songCount"] {
                    let count =
                        n(&v["stats"]["albumartist"], key).max(n(&v["stats"]["artist"], key));
                    item.column_data.insert(key.into(), count.to_string());
                }
            }
        }
        if matches!(kind, ItemKind::Artist | ItemKind::AlbumArtist) {
            match self.server.kind {
                ServerType::Jellyfin if v["RunTimeTicks"].is_number() => {
                    item.column_data
                        .insert("duration".into(), item.duration.to_string());
                }
                ServerType::Subsonic => {
                    item.column_data
                        .entry("albumCount".into())
                        .or_insert_with(|| "0".into());
                    item.column_data.remove("songCount");
                }
                _ => (),
            }
        }
        item
    }

    fn page(
        &self,
        values: Vec<Value>,
        kind: ItemKind,
        total: Option<usize>,
        query: &ListQuery,
    ) -> Page {
        let has_more = total.map_or(values.len() >= query.limit, |n| {
            query.offset + values.len() < n
        });
        Page {
            items: values.iter().map(|v| self.normalize(v, kind)).collect(),
            total,
            has_more,
        }
    }

    pub fn list(&self, query: &ListQuery) -> Result<Page> {
        ensure!(
            query.limit > 0 && query.limit <= 500,
            "Page size must be between 1 and 500"
        );
        let mut scoped = query.clone();
        if matches!(
            query.kind,
            ItemKind::Song
                | ItemKind::Album
                | ItemKind::Artist
                | ItemKind::AlbumArtist
                | ItemKind::Genre
        ) && scoped.folder_id.is_none()
            && self.server.kind != ServerType::Navidrome
        {
            scoped.folder_id = self.server.options.music_folder_ids.first().cloned();
        }
        scoped.filters.validate()?;
        if !scoped.filters.library_ids.is_empty() {
            scoped.folder_id = scoped.filters.library_ids.first().cloned();
        }
        let query = &scoped;
        if query.favorites && query.filters.favorite == Some(false) {
            return Ok(Page {
                items: vec![],
                total: Some(0),
                has_more: false,
            });
        }
        if self.server.kind == ServerType::Subsonic
            && matches!(query.kind, ItemKind::Album | ItemKind::Song)
            && !query.filters.genre_ids.is_empty()
        {
            ensure!(
                query.filters.custom.is_empty(),
                "Custom server tag filters are unavailable on Subsonic"
            );
            // Classic album lists can omit genres. Use the server's genre
            // endpoints before applying the remaining client-side predicates.
            let mut base = query.clone();
            base.filters = Default::default();
            base.favorites = false;
            base.artist_id = None;
            base.search.clear();
            base.sort = Sort::Name;
            base.descending = false;
            let mut items = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for (index, genre) in query.filters.genre_ids.iter().enumerate() {
                base.genre = Some(genre.clone());
                let mut matching = self.all_items(base.clone())?;
                if index == 0 || !query.filters.genre_all {
                    matching.retain(|item| seen.insert(item.id.clone()));
                    items.extend(matching);
                } else {
                    let ids = matching
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<std::collections::HashSet<_>>();
                    items.retain(|item| ids.contains(item.id.as_str()));
                }
            }
            if let Some(genre) = &query.genre
                && !(query.filters.genre_ids.len() == 1 && query.filters.genre_ids[0] == *genre)
            {
                base.genre = Some(genre.clone());
                let parent = self.all_items(base)?;
                let ids = parent
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<std::collections::HashSet<_>>();
                items.retain(|item| ids.contains(item.id.as_str()));
            }
            if let Some(artist) = &query.artist_id {
                items.retain(|item| item.artist_id == *artist || item.artist_ids.contains(artist));
            }
            crate::lists::sort_items(&mut items, query.sort, query.descending);
            let mut filters = query.filters.clone();
            filters.genre_ids.clear();
            filters.genre_all = false;
            if query.favorites {
                filters.favorite = Some(true);
            }
            return Ok(filters.page(items, &query.search, query.offset, query.limit));
        }
        let local_rating = self.server.kind == ServerType::Navidrome
            && query.kind == ItemKind::Song
            && query.filters.has_rating.is_some()
            && !self.features().track_rating_filter;
        let local_selections = self.server.kind == ServerType::Navidrome
            && !self.features().version.is_empty()
            && !self.features().artist_roles
            && (query.filters.artist_ids.len() > 1 || query.filters.genre_ids.len() > 1);
        if local_rating
            || local_selections
            || (query.filters.artist_all && !query.filters.artist_ids.is_empty())
            || (query.filters.album_artist_all && !query.filters.album_artist_ids.is_empty())
            || (query.filters.genre_all && !query.filters.genre_ids.is_empty())
            || !query.filters.album_artist_ids.is_empty()
            || (self.server.kind == ServerType::Subsonic && !query.filters.is_empty())
            || (self.server.kind == ServerType::Jellyfin
                && (query.filters.min_year.is_some() || query.filters.max_year.is_some())
                && query
                    .filters
                    .max_year
                    .unwrap_or(9999)
                    .saturating_sub(query.filters.min_year.unwrap_or(0))
                    > 200)
            || (self.server.kind == ServerType::Navidrome
                && (query.filters.min_year.is_some() || query.filters.max_year.is_some())
                && query
                    .filters
                    .min_year
                    .zip(query.filters.max_year)
                    .is_none_or(|(min, max)| min != max))
        {
            let mut base = query.clone();
            if self.server.kind == ServerType::Subsonic {
                ensure!(
                    base.filters.custom.is_empty(),
                    "Custom server tag filters are unavailable on Subsonic"
                );
                base.filters = Default::default();
            } else {
                if local_selections {
                    base.filters.artist_ids.clear();
                    base.filters.genre_ids.clear();
                }
                if local_rating {
                    base.filters.has_rating = None;
                }
                base.filters.artist_all = false;
                base.filters.album_artist_all = false;
                base.filters.genre_all = false;
                base.filters.album_artist_ids.clear();
                base.filters.min_year = None;
                base.filters.max_year = None;
            }
            let items = self.all_items(base)?;
            return Ok(query.filters.page(items, "", query.offset, query.limit));
        }
        if query
            .sort
            .native_field(self.server.kind, query.kind)
            .is_none()
            || (self.server.kind == ServerType::Subsonic
                && query.descending
                && !matches!(query.sort, Sort::Year | Sort::Random))
        {
            let mut base = query.clone();
            base.sort = Sort::Name;
            base.descending = false;
            let mut items = self.all_items(base)?;
            crate::lists::sort_items(&mut items, query.sort, query.descending);
            return Ok(crate::model::filtered_page(
                items,
                "",
                query.offset,
                query.limit,
            ));
        }
        if self.server.kind == ServerType::Jellyfin {
            return self.jf_list(query);
        }
        if query.kind == ItemKind::Radio {
            if self.server.kind == ServerType::Navidrome {
                let (values, total) = self.request(
                    Method::GET,
                    "api/radio",
                    &[
                        p("_start", query.offset),
                        p("_end", query.offset + query.limit),
                        p("_sort", "name"),
                        p("_order", if query.descending { "DESC" } else { "ASC" }),
                        p("q", &query.search),
                    ],
                    None,
                )?;
                return Ok(self.page(array(&values), ItemKind::Radio, total, query));
            }
            let v = self.subsonic("getInternetRadioStations", &[])?;
            let values = array(&v["internetRadioStations"]["internetRadioStation"])
                .into_iter()
                .filter(|value| {
                    query.search.is_empty()
                        || s(value, "name")
                            .to_lowercase()
                            .contains(&query.search.to_lowercase())
                })
                .collect::<Vec<_>>();
            let total = values.len();
            return Ok(self.page(
                values
                    .into_iter()
                    .skip(query.offset)
                    .take(query.limit)
                    .collect(),
                ItemKind::Radio,
                Some(total),
                query,
            ));
        }
        if query.kind == ItemKind::Folder {
            let v = self.subsonic("getMusicFolders", &[])?;
            let values = array(&v["musicFolders"]["musicFolder"])
                .into_iter()
                .filter(|value| {
                    query.search.is_empty()
                        || s(value, "name")
                            .to_lowercase()
                            .contains(&query.search.to_lowercase())
                })
                .collect::<Vec<_>>();
            let total = values.len();
            let mut page = self.page(
                values
                    .into_iter()
                    .skip(query.offset)
                    .take(query.limit)
                    .collect(),
                ItemKind::Folder,
                Some(total),
                query,
            );
            for folder in &mut page.items {
                folder.folder_root = true;
            }
            return Ok(page);
        }
        if self.server.kind == ServerType::Navidrome {
            if query.kind == ItemKind::Genre && self.features().artist_roles {
                let mut params = vec![
                    p("tag_name", "genre"),
                    p("tag_value", &query.search),
                    p("_start", query.offset),
                    p("_end", query.offset + query.limit),
                    p(
                        "_sort",
                        if query.sort == Sort::Name {
                            "tagValue"
                        } else {
                            query
                                .sort
                                .native_field(ServerType::Navidrome, ItemKind::Genre)
                                .unwrap_or("tagValue")
                        },
                    ),
                    p("_order", if query.descending { "DESC" } else { "ASC" }),
                ];
                let libraries = if !query.filters.library_ids.is_empty() {
                    &query.filters.library_ids
                } else {
                    &self.server.options.music_folder_ids
                };
                params.extend(libraries.iter().map(|id| p("library_id", id)));
                let (values, total) = self.request(Method::GET, "api/tag", &params, None)?;
                let values = array(if values.is_array() {
                    &values
                } else {
                    &values["data"]
                })
                .into_iter()
                .map(|mut value| {
                    value["name"] = value["tagValue"].clone();
                    value
                })
                .collect();
                return Ok(self.page(values, ItemKind::Genre, total, query));
            }
            let resource = match query.kind {
                ItemKind::Album => "album",
                ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                ItemKind::Playlist => "playlist",
                ItemKind::Genre => "genre",
                _ => "song",
            };
            let sort = query
                .sort
                .native_field(self.server.kind, query.kind)
                .unwrap_or("name");
            let mut params = vec![
                p("_start", query.offset),
                p("_end", query.offset + query.limit),
                p("_sort", sort),
                p("_order", if query.descending { "DESC" } else { "ASC" }),
            ];
            if !query.search.is_empty() {
                params.push(p(
                    if query.kind == ItemKind::Song {
                        "title"
                    } else {
                        "name"
                    },
                    &query.search,
                ));
            }
            if query.favorites || query.filters.favorite.is_some() {
                params.push(p(
                    "starred",
                    query.favorites || query.filters.favorite.unwrap_or(false),
                ));
            }
            for (key, value) in [
                ("has_rating", query.filters.has_rating),
                ("compilation", query.filters.compilation),
                ("recently_played", query.filters.recently_played),
            ] {
                if let Some(value) = value {
                    params.push(p(key, value));
                }
            }
            if let Some(year) = query.filters.min_year {
                params.push(p("year", year));
            }
            params.extend(query.filters.genre_ids.iter().map(|id| p("genre_id", id)));
            let artist_key = if query.kind == ItemKind::Song {
                if self.features().artists_search {
                    "artists_id"
                } else {
                    "album_artist_id"
                }
            } else {
                "artist_id"
            };
            params.extend(query.filters.artist_ids.iter().map(|id| p(artist_key, id)));
            params.extend(query.filters.custom_parameters(false));

            if !query.filters.library_ids.is_empty() {
                params.extend(
                    query
                        .filters
                        .library_ids
                        .iter()
                        .map(|id| p("library_id", id)),
                );
            } else if let Some(id) = &query.folder_id {
                params.push(p("library_id", id));
            } else if matches!(
                query.kind,
                ItemKind::Song
                    | ItemKind::Album
                    | ItemKind::Artist
                    | ItemKind::AlbumArtist
                    | ItemKind::Genre
            ) {
                params.extend(
                    self.server
                        .options
                        .music_folder_ids
                        .iter()
                        .map(|id| p("library_id", id)),
                );
            }
            if let Some(genre) = &query.genre {
                params.push(p("genre_id", genre));
            }
            if let Some(artist) = &query.artist_id {
                params.push(p("artist_id", artist));
            }
            if self.features().artist_roles {
                if query.kind == ItemKind::AlbumArtist {
                    params.push(p("role", "albumartist"));
                } else if let Some(role) = &query.role {
                    params.push(p("role", role));
                }
            }
            let (v, total) =
                self.request(Method::GET, &format!("api/{resource}"), &params, None)?;
            return Ok(self.page(array(&v), query.kind, total, query));
        }
        self.ss_list(query)
    }

    fn ss_list(&self, query: &ListQuery) -> Result<Page> {
        let mut params = Vec::new();
        if let Some(id) = &query.folder_id {
            params.push(p("musicFolderId", id));
        }
        if let Some(artist) = &query.artist_id {
            let item = Item {
                id: artist.clone(),
                server_id: self.server.id,
                kind: ItemKind::Artist,
                ..Default::default()
            };
            if query.kind == ItemKind::Album {
                return self.detail(&item, query.offset, query.limit);
            }
            let mut songs = self.all_tracks(&item)?;
            if query.sort == Sort::Random {
                use rand::seq::SliceRandom;
                songs.shuffle(&mut rand::thread_rng());
            }
            return Ok(crate::model::filtered_page(
                songs,
                &query.search,
                query.offset,
                query.limit,
            ));
        }
        if let Some(genre) = &query.genre
            && query.kind == ItemKind::Song
        {
            params.extend([
                p("genre", genre),
                p("count", query.limit),
                p("offset", query.offset),
            ]);
            let value = self.subsonic("getSongsByGenre", &params)?;
            return Ok(self.page(
                array(&value["songsByGenre"]["song"]),
                ItemKind::Song,
                None,
                query,
            ));
        }
        if query.favorites {
            let v = self.subsonic("getStarred2", &params)?;
            let key = match query.kind {
                ItemKind::Album => "album",
                ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                _ => "song",
            };
            let mut items = array(&v["starred2"][key])
                .iter()
                .map(|value| self.normalize(value, query.kind))
                .collect::<Vec<_>>();
            crate::lists::sort_items(&mut items, query.sort, query.descending);
            return Ok(crate::model::filtered_page(
                items,
                &query.search,
                query.offset,
                query.limit,
            ));
        }
        let values = match query.kind {
            ItemKind::Album if query.search.is_empty() => {
                let sort = if query.genre.is_some() {
                    "byGenre"
                } else {
                    query
                        .sort
                        .native_field(ServerType::Subsonic, query.kind)
                        .unwrap_or("alphabeticalByName")
                };
                if let Some(genre) = &query.genre {
                    params.push(p("genre", genre));
                }
                params.extend([
                    p("type", sort),
                    p("size", query.limit),
                    p("offset", query.offset),
                ]);
                if query.sort == Sort::Year {
                    params.extend([
                        p("fromYear", if query.descending { 9999 } else { 0 }),
                        p("toYear", if query.descending { 0 } else { 9999 }),
                    ]);
                }
                array(&self.subsonic("getAlbumList2", &params)?["albumList2"]["album"])
            }
            ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                if !query.search.is_empty() || query.kind == ItemKind::Song =>
            {
                let resource = match query.kind {
                    ItemKind::Album => "album",
                    ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                    _ => "song",
                };
                params.push(p("query", &query.search));
                for key in ["song", "album", "artist"] {
                    params.push(p(
                        &format!("{key}Count"),
                        if key == resource { query.limit } else { 0 },
                    ));
                    params.push(p(&format!("{key}Offset"), query.offset));
                }
                array(&self.subsonic("search3", &params)?["searchResult3"][resource])
            }
            ItemKind::Artist | ItemKind::AlbumArtist => {
                let v = self.subsonic("getArtists", &params)?;
                let values: Vec<_> = array(&v["artists"]["index"])
                    .iter()
                    .flat_map(|index| array(&index["artist"]))
                    .collect();
                let mut items = values
                    .iter()
                    .map(|value| self.normalize(value, query.kind))
                    .collect::<Vec<_>>();
                crate::lists::sort_items(&mut items, query.sort, query.descending);
                return Ok(crate::model::filtered_page(
                    items,
                    &query.search,
                    query.offset,
                    query.limit,
                ));
            }
            ItemKind::Playlist | ItemKind::Genre => {
                let (endpoint, root, key) = if query.kind == ItemKind::Playlist {
                    ("getPlaylists", "playlists", "playlist")
                } else {
                    ("getGenres", "genres", "genre")
                };
                let values = array(&self.subsonic(endpoint, &params)?[root][key]);
                let mut items = values
                    .iter()
                    .map(|value| self.normalize(value, query.kind))
                    .collect::<Vec<_>>();
                crate::lists::sort_items(&mut items, query.sort, query.descending);
                return Ok(crate::model::filtered_page(
                    items,
                    &query.search,
                    query.offset,
                    query.limit,
                ));
            }
            _ => Vec::new(),
        };
        Ok(self.page(values, query.kind, None, query))
    }

    fn jf_song_types(&self) -> &'static str {
        if self.server.options.enable_audiobooks {
            "Audio,AudioBook"
        } else {
            "Audio"
        }
    }
    fn jf_list(&self, query: &ListQuery) -> Result<Page> {
        let types = match query.kind {
            ItemKind::Song => self.jf_song_types(),
            ItemKind::Album => "MusicAlbum",
            ItemKind::Artist | ItemKind::AlbumArtist => "MusicArtist",
            ItemKind::Playlist => "Playlist",
            ItemKind::Folder => "Folder",
            ItemKind::Genre => "MusicGenre",
            ItemKind::Radio => return Ok(Page::default()),
        };
        let sort = query
            .sort
            .native_field(ServerType::Jellyfin, query.kind)
            .unwrap_or("SortName");
        let mut params = vec![
            p("UserId", &self.session().user_id),
            p("Recursive", true),
            p("IncludeItemTypes", types),
            p("StartIndex", query.offset),
            p("Limit", query.limit),
            p(
                "Fields",
                "Genres,DateCreated,Tags,Overview,MediaSources,MediaStreams,GenreItems,ArtistItems,ParentId,SortName,Path,ProviderIds",
            ),
            p("SortBy", sort),
            p(
                "SortOrder",
                if query.descending {
                    "Descending"
                } else {
                    "Ascending"
                },
            ),
        ];
        // These endpoints enumerate names attached to library items. Filtering
        // their source items to MusicArtist/MusicGenre produces an empty list.
        if matches!(
            query.kind,
            ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Genre
        ) {
            params.retain(|(key, _)| key != "IncludeItemTypes");
        }
        if !query.search.is_empty() {
            params.push(p("SearchTerm", &query.search));
        }
        if query.favorites || query.filters.favorite.is_some() {
            params.push(p(
                "IsFavorite",
                query.favorites || query.filters.favorite.unwrap_or(false),
            ));
        }
        if !query.filters.genre_ids.is_empty() {
            params.push(p("GenreIds", query.filters.genre_ids.join(",")));
        }
        if !query.filters.artist_ids.is_empty() {
            let key = if query.kind == ItemKind::Album {
                match query.filters.compilation {
                    Some(true) => "ContributingArtistIds",
                    Some(false) => "AlbumArtistIds",
                    None => "ArtistIds",
                }
            } else {
                "ArtistIds"
            };
            params.push(p(key, query.filters.artist_ids.join(",")));
        }
        if query.filters.min_year.is_some() || query.filters.max_year.is_some() {
            let years = (query.filters.min_year.unwrap_or(0)
                ..=query.filters.max_year.unwrap_or(9999))
                .map(|year| year.to_string())
                .collect::<Vec<_>>()
                .join(",");
            params.push(p("Years", years));
        }
        params.extend(query.filters.custom_parameters(true));

        if let Some(id) = &query.folder_id {
            params.push(p("ParentId", id));
        }
        if let Some(genre) = &query.genre {
            params.push(p("Genres", genre));
        }
        if let Some(artist) = &query.artist_id {
            params.push(p("ArtistIds", artist));
        }
        let path = match query.kind {
            ItemKind::Genre => "MusicGenres".into(),
            ItemKind::Artist => "Artists".into(),
            ItemKind::AlbumArtist => "Artists/AlbumArtists".into(),
            ItemKind::Folder => format!("Users/{}/Views", self.session().user_id),
            _ => format!("Users/{}/Items", self.session().user_id),
        };
        if query.kind == ItemKind::Folder {
            params
                .retain(|(key, _)| !matches!(key.as_str(), "StartIndex" | "Limit" | "SearchTerm"));
        }
        let (v, _) = self.request(Method::GET, &path, &params, None)?;
        let mut items = array(&v["Items"]);
        if query.kind == ItemKind::Folder {
            items.retain(|item| {
                item["CollectionType"].is_null()
                    || item["CollectionType"] == "music"
                    || (self.server.options.enable_audiobooks && item["CollectionType"] == "books")
            });
            items.retain(|item| {
                s(item, "Name")
                    .to_lowercase()
                    .contains(&query.search.to_lowercase())
            });
            let total = items.len();
            return Ok(self.page(
                items
                    .into_iter()
                    .skip(query.offset)
                    .take(query.limit)
                    .collect(),
                query.kind,
                Some(total),
                query,
            ));
        }
        Ok(self.page(
            items,
            query.kind,
            v["TotalRecordCount"].as_u64().map(|n| n as usize),
            query,
        ))
    }

    pub fn search(&self, query: &ListQuery, secondary_search: &str) -> Result<Page> {
        ensure!(
            (1..=500).contains(&query.limit),
            "Page size must be between 1 and 500"
        );
        if secondary_search.trim().is_empty() {
            return self.list(query);
        }
        let items = self.all_items(query.clone())?;
        Ok(crate::model::filtered_page(
            items,
            secondary_search,
            query.offset,
            query.limit,
        ))
    }

    pub fn item(&self, id: &str, kind: ItemKind) -> Result<Item> {
        ensure!(
            !id.is_empty() && !id.contains(['/', '?', '#']) && id != "..",
            "Invalid item ID"
        );
        let v = match self.server.kind {
            ServerType::Jellyfin => {
                self.request(
                    Method::GET,
                    &format!("Users/{}/Items/{id}", self.session().user_id),
                    &[],
                    None,
                )?
                .0
            }
            ServerType::Navidrome => {
                let path = match kind {
                    ItemKind::Song => "song",
                    ItemKind::Album => "album",
                    ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                    ItemKind::Playlist => "playlist",
                    _ => bail!("Unsupported item type"),
                };
                self.request(Method::GET, &format!("api/{path}/{id}"), &[], None)?
                    .0
            }
            ServerType::Subsonic => {
                let (endpoint, key) = match kind {
                    ItemKind::Song => ("getSong", "song"),
                    ItemKind::Album => ("getAlbum", "album"),
                    ItemKind::Artist | ItemKind::AlbumArtist => ("getArtist", "artist"),
                    ItemKind::Playlist => ("getPlaylist", "playlist"),
                    _ => bail!("Unsupported item type"),
                };
                self.subsonic(endpoint, &[p("id", id)])?[key].clone()
            }
        };
        let item = self.normalize(&v, kind);
        ensure!(!item.id.is_empty(), "Server returned no item");
        Ok(item)
    }

    pub fn detail(&self, item: &Item, offset: usize, limit: usize) -> Result<Page> {
        ensure!(
            limit > 0 && limit <= 500,
            "Page size must be between 1 and 500"
        );
        ensure!(
            item.server_id == self.server.id,
            "Item belongs to a different server"
        );
        if self.server.kind == ServerType::Jellyfin
            && item.kind == ItemKind::Album
            && let Some(song_id) = item.id.strip_prefix("dummy/")
        {
            let song = self.item(song_id, ItemKind::Song)?;
            return Ok(crate::model::filtered_page(vec![song], "", offset, limit));
        }
        if self.server.kind == ServerType::Jellyfin && item.kind == ItemKind::Folder {
            let items = self.jf_folder_items(item)?;
            let total = items.len();
            return Ok(Page {
                items: items.into_iter().skip(offset).take(limit).collect(),
                total: Some(total),
                has_more: offset.saturating_add(limit) < total,
            });
        }
        let kind = if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
            ItemKind::Album
        } else {
            ItemKind::Song
        };
        let query = ListQuery {
            kind,
            offset,
            limit,
            ..ListQuery::default()
        };
        if self.server.kind == ServerType::Jellyfin {
            let mut params = vec![
                p("UserId", &self.session().user_id),
                p("StartIndex", offset),
                p("Limit", limit),
                p(
                    "Fields",
                    "Genres,DateCreated,Tags,Overview,MediaSources,MediaStreams,GenreItems,ArtistItems,ParentId,SortName,Path,ProviderIds",
                ),
                p("Recursive", item.kind != ItemKind::Folder),
            ];
            let path = if item.kind == ItemKind::Playlist {
                format!("Playlists/{}/Items", item.id)
            } else {
                match item.kind {
                    ItemKind::Artist | ItemKind::AlbumArtist => {
                        params.extend([
                            p(
                                if item.kind == ItemKind::AlbumArtist {
                                    "AlbumArtistIds"
                                } else {
                                    "ArtistIds"
                                },
                                &item.id,
                            ),
                            p("IncludeItemTypes", "MusicAlbum"),
                            p("SortBy", "ProductionYear,SortName"),
                        ]);
                    }
                    ItemKind::Genre => {
                        params.extend([
                            p("GenreIds", &item.id),
                            p("IncludeItemTypes", self.jf_song_types()),
                        ]);
                    }
                    _ => {
                        params.push(p("ParentId", &item.id));
                        if item.kind != ItemKind::Folder {
                            params.push(p(
                                "IncludeItemTypes",
                                format!("{},Folder", self.jf_song_types()),
                            ));
                        }
                        params.push(p("SortBy", "ParentIndexNumber,IndexNumber,SortName"));
                    }
                }
                format!("Users/{}/Items", self.session().user_id)
            };
            let (v, _) = self.request(Method::GET, &path, &params, None)?;
            let mut page = self.page(
                array(&v["Items"]),
                kind,
                v["TotalRecordCount"].as_u64().map(|n| n as usize),
                &query,
            );
            if item.kind == ItemKind::Playlist {
                for (index, song) in page.items.iter_mut().enumerate() {
                    song.playlist_position = Some(offset + index);
                }
            }
            return Ok(page);
        }
        if self.server.kind == ServerType::Navidrome && item.kind != ItemKind::Folder {
            let (path, mut params) = match item.kind {
                ItemKind::Playlist => (format!("api/playlist/{}/tracks", item.id), vec![]),
                ItemKind::Artist | ItemKind::AlbumArtist => {
                    ("api/album".into(), vec![p("artist_id", &item.id)])
                }
                ItemKind::Genre => ("api/song".into(), vec![p("genre_id", &item.id)]),
                _ => ("api/song".into(), vec![p("album_id", &item.id)]),
            };
            params.extend([
                p("_start", offset),
                p("_end", offset + limit),
                p(
                    "_sort",
                    if item.kind == ItemKind::Playlist {
                        "id"
                    } else if kind == ItemKind::Album {
                        "year"
                    } else {
                        "discNumber,trackNumber"
                    },
                ),
                p("_order", "ASC"),
            ]);
            let (v, total) = self.request(Method::GET, &path, &params, None)?;
            let mut page = self.page(array(&v), kind, total, &query);
            if item.kind == ItemKind::Playlist {
                for (index, (item, v)) in page.items.iter_mut().zip(array(&v)).enumerate() {
                    item.playlist_position = Some(offset + index);
                    item.playlist_entry_id = s(&v, "id");
                    if !v["mediaFileId"].is_null() {
                        item.id = s(&v, "mediaFileId");
                    }
                }
            }
            return Ok(page);
        }
        if item.kind == ItemKind::Folder && item.folder_root {
            let response = self.subsonic("getIndexes", &[p("musicFolderId", &item.id)])?;
            let mut values = array(&response["indexes"]["index"])
                .iter()
                .flat_map(|index| array(&index["artist"]))
                .map(|mut value| {
                    value["isDir"] = true.into();
                    value
                })
                .collect::<Vec<_>>();
            values.extend(array(&response["indexes"]["child"]));
            let total = values.len();
            return Ok(self.page(
                values.into_iter().skip(offset).take(limit).collect(),
                ItemKind::Song,
                Some(total),
                &query,
            ));
        }
        let (endpoint, root, key, params) = match item.kind {
            ItemKind::Artist | ItemKind::AlbumArtist => {
                ("getArtist", "artist", "album", vec![p("id", &item.id)])
            }
            ItemKind::Playlist => ("getPlaylist", "playlist", "entry", vec![p("id", &item.id)]),
            ItemKind::Folder => (
                "getMusicDirectory",
                "directory",
                "child",
                vec![p("id", &item.id)],
            ),
            ItemKind::Genre => (
                "getSongsByGenre",
                "songsByGenre",
                "song",
                vec![
                    p("genre", &item.name),
                    p("offset", offset),
                    p("count", limit),
                ],
            ),
            _ => ("getAlbum", "album", "song", vec![p("id", &item.id)]),
        };
        let response = self.subsonic(endpoint, &params)?;
        let mut values = array(&response[root][key]);
        if item.kind == ItemKind::Album {
            for song in &mut values {
                let disc = n(song, "discNumber").max(1.);
                if let Some(title) = array(&response[root]["discTitles"])
                    .iter()
                    .find(|title| n(title, "disc") == disc)
                {
                    song["discSubtitle"] = title["title"].clone();
                }
            }
        }
        if item.kind == ItemKind::Genre {
            return Ok(self.page(values, kind, None, &query));
        }
        let total = values.len();
        let mut page = self.page(
            values.into_iter().skip(offset).take(limit).collect(),
            kind,
            Some(total),
            &query,
        );
        if item.kind == ItemKind::Playlist {
            for (index, song) in page.items.iter_mut().enumerate() {
                song.playlist_position = Some(offset + index);
            }
        }
        Ok(page)
    }

    fn jf_folder_items(&self, parent: &Item) -> Result<Vec<Item>> {
        ensure!(
            parent.server_id == self.server.id && parent.kind == ItemKind::Folder,
            "Invalid folder for this server"
        );
        let mut items = Vec::new();
        let mut previous = Vec::new();
        let mut offset = 0;
        loop {
            let (response, _) = self.request(Method::GET, &format!("Users/{}/Items", self.session().user_id), &[
                p("UserId", &self.session().user_id), p("ParentId", &parent.id),
                p("StartIndex", offset), p("Limit", 500), p("Recursive", false),
                p("SortBy", "ParentIndexNumber,IndexNumber,SortName"),
                p("Fields", "Genres,DateCreated,Tags,Overview,MediaSources,MediaStreams,GenreItems,ArtistItems,ParentId,SortName,Path,ProviderIds"),
            ], None)?;
            let values = array(&response["Items"]);
            if values.is_empty() {
                break;
            }
            let ids = values
                .iter()
                .map(|value| s(value, "Id"))
                .collect::<Vec<_>>();
            ensure!(
                ids != previous,
                "Server repeated a page instead of advancing pagination"
            );
            previous = ids;
            // Advance by the server's rows even when every audio row is unplayable.
            offset += values.len();
            let has_more = response["TotalRecordCount"]
                .as_u64()
                .map_or(values.len() >= 500, |total| offset < total as usize);
            for value in values {
                let audio = value["Type"] == "Audio"
                    || (self.server.options.enable_audiobooks && value["Type"] == "AudioBook");
                if audio && !value["MediaSources"].is_array() {
                    continue;
                }
                let kind = if audio {
                    ItemKind::Song
                } else {
                    ItemKind::Folder
                };
                let mut item = self.normalize(&value, kind);
                item.kind = kind;
                items.push(item);
            }
            if !has_more {
                break;
            }
        }
        Ok(items)
    }

    pub fn folder_children(&self, parent: Option<&Item>) -> Result<Vec<Item>> {
        if let Some(parent) = parent {
            ensure!(parent.kind == ItemKind::Folder, "Expected a folder");
            return self.all_detail_items(parent);
        }
        if self.server.kind == ServerType::Jellyfin {
            if let Some(id) = self.server.options.music_folder_ids.first() {
                let mut children = self.all_detail_items(&Item {
                    id: id.clone(),
                    server_id: self.server.id,
                    kind: ItemKind::Folder,
                    ..Default::default()
                })?;
                children.retain(|item| item.kind == ItemKind::Folder);
                return Ok(children);
            }
            return self.all_items(ListQuery {
                kind: ItemKind::Folder,
                ..Default::default()
            });
        }
        let params = self
            .server
            .options
            .music_folder_ids
            .first()
            .map(|id| vec![p("musicFolderId", id)])
            .unwrap_or_default();
        let response = self.subsonic("getIndexes", &params)?;
        Ok(array(&response["indexes"]["index"])
            .iter()
            .flat_map(|index| array(&index["artist"]))
            .map(|value| self.normalize(&value, ItemKind::Folder))
            .collect())
    }

    pub fn all_detail_items(&self, item: &Item) -> Result<Vec<Item>> {
        if self.server.kind == ServerType::Jellyfin && item.kind == ItemKind::Folder {
            return self.jf_folder_items(item);
        }
        let mut items = vec![];
        let mut previous = vec![];
        loop {
            let page = self.detail(item, items.len(), 500)?;
            let identities = page
                .items
                .iter()
                .map(|item| {
                    (
                        item.kind,
                        item.id.clone(),
                        item.playlist_entry_id.clone(),
                        if self.server.kind == ServerType::Subsonic {
                            item.playlist_position
                        } else {
                            None
                        },
                    )
                })
                .collect::<Vec<_>>();
            ensure!(
                identities.is_empty() || identities != previous,
                "Server repeated a page instead of advancing pagination"
            );
            previous = identities;
            let empty = page.items.is_empty();
            items.extend(page.items);
            if empty || !page.has_more {
                break;
            }
        }
        Ok(items)
    }

    pub fn all_tracks(&self, item: &Item) -> Result<Vec<Item>> {
        if !item.playlist_songs.is_empty() {
            ensure!(
                item.playlist_songs
                    .iter()
                    .all(|song| song.server_id == self.server.id
                        && song.kind == ItemKind::Song
                        && song.playlist_songs.is_empty()),
                "Invalid playlist album tracks"
            );
            return Ok(crate::playlist_albums::ordered_tracks(item));
        }
        self.collect_tracks(item, &mut std::collections::HashSet::new())
    }

    pub fn all_items(&self, mut query: ListQuery) -> Result<Vec<Item>> {
        query.offset = 0;
        query.limit = 500;
        let mut result = vec![];
        let mut previous = vec![];
        loop {
            let page = self.list(&query)?;
            if page.items.is_empty() {
                break;
            }
            let ids = page
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>();
            ensure!(
                ids != previous,
                "Server repeated a page instead of advancing pagination"
            );
            previous = ids;
            query.offset += page.items.len();
            result.extend(page.items);
            if !page.has_more {
                break;
            }
        }
        Ok(result)
    }

    fn collect_tracks(
        &self,
        item: &Item,
        ancestors: &mut std::collections::HashSet<String>,
    ) -> Result<Vec<Item>> {
        ensure!(
            item.server_id == self.server.id,
            "Item belongs to a different server"
        );
        if matches!(item.kind, ItemKind::Song | ItemKind::Radio) {
            return Ok(vec![item.clone()]);
        }
        let identity = format!("{:?}:{}:{}", item.kind, item.folder_root, item.id);
        ensure!(
            ancestors.len() < 100 && ancestors.insert(identity.clone()),
            "Server returned a circular folder hierarchy"
        );
        let mut songs = Vec::new();
        for child in self.all_detail_items(item)? {
            if matches!(child.kind, ItemKind::Song | ItemKind::Radio) {
                songs.push(child);
            } else if matches!(
                child.kind,
                ItemKind::Album | ItemKind::Folder | ItemKind::Artist | ItemKind::AlbumArtist
            ) {
                songs.extend(self.collect_tracks(&child, ancestors)?);
            }
        }
        ancestors.remove(&identity);
        Ok(songs)
    }

    pub fn artist_info(&self, item: &Item) -> Result<ArtistInfo> {
        ensure!(
            item.server_id == self.server.id
                && matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist),
            "Invalid artist for this server"
        );
        if self.server.kind == ServerType::Jellyfin {
            let detail = self.item(&item.id, item.kind)?;
            let (value, _) = self.request(
                Method::GET,
                &format!("Artists/{}/Similar", item.id),
                &[p("UserId", &self.session().user_id), p("Limit", 10)],
                None,
            )?;
            return Ok(ArtistInfo {
                biography: crate::lyric_sources::text_from_html(&detail.comment),
                biography_html: crate::lyric_sources::formatted_html(&detail.comment),
                homepage: String::new(),
                similar: array(&value["Items"])
                    .iter()
                    .map(|value| self.normalize(value, item.kind))
                    .filter(|artist| artist.name != "Various Artists" && artist.id != item.id)
                    .collect(),
            });
        }
        let value = self.subsonic(
            "getArtistInfo2",
            &[
                p("id", &item.id),
                p("count", 10),
                p("includeNotPresent", false),
            ],
        )?;
        let value = &value["artistInfo2"];
        let homepage = s(value, "lastFmUrl");
        let homepage = Url::parse(&homepage)
            .ok()
            .filter(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.username().is_empty()
                    && url.password().is_none()
            })
            .map(|url| url.to_string())
            .unwrap_or_default();
        Ok(ArtistInfo {
            biography: crate::lyric_sources::text_from_html(&s(value, "biography")),
            biography_html: crate::lyric_sources::formatted_html(&s(value, "biography")),
            homepage,
            similar: array(&value["similarArtist"])
                .iter()
                .map(|value| self.normalize(value, item.kind))
                .filter(|artist| !artist.id.is_empty() && artist.id != "-1" && artist.id != item.id)
                .collect(),
        })
    }
    pub fn artist_page(
        &self,
        item: &Item,
        tab: ArtistTab,
        offset: usize,
        limit: usize,
    ) -> Result<Page> {
        ensure!(
            item.server_id == self.server.id
                && matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist),
            "Invalid artist for this server"
        );
        ensure!(limit > 0 && limit <= 500, "Invalid page size");
        if tab == ArtistTab::Albums {
            return self.detail(item, offset, limit);
        }
        let mut query = ListQuery {
            kind: ItemKind::Song,
            offset,
            limit,
            ..Default::default()
        };
        if tab == ArtistTab::Similar {
            let info = self.artist_info(item)?;
            let total = info.similar.len();
            return Ok(Page {
                items: info.similar.into_iter().skip(offset).take(limit).collect(),
                total: Some(total),
                has_more: offset.saturating_add(limit) < total,
            });
        }
        if tab == ArtistTab::CommunityTop {
            ensure!(
                self.server.kind != ServerType::Jellyfin,
                "This server does not provide community top songs"
            );
            let mut params = vec![p("artist", &item.name), p("count", 500)];
            if self.features().top_songs_by_id {
                params.push(p("artistId", &item.id));
            }
            let value = self.subsonic("getTopSongs", &params)?;
            let items = array(&value["topSongs"]["song"]);
            let total = items.len();
            return Ok(self.page(
                items.into_iter().skip(offset).take(limit).collect(),
                ItemKind::Song,
                Some(total),
                &query,
            ));
        }
        if matches!(tab, ArtistTab::Favorites | ArtistTab::Rated) {
            let mut items: Vec<_> = self
                .all_artist_items(item, ArtistTab::Songs)?
                .into_iter()
                .filter(|song| {
                    if tab == ArtistTab::Favorites {
                        song.favorite
                    } else {
                        song.rating > 2
                    }
                })
                .collect();
            items.sort_by(|a, b| {
                if self.server.kind == ServerType::Jellyfin {
                    return b.play_count.cmp(&a.play_count);
                }
                b.rating
                    .cmp(&a.rating)
                    .then_with(|| b.favorite.cmp(&a.favorite))
                    .then_with(|| b.play_count.cmp(&a.play_count))
                    .then_with(|| a.album_id.cmp(&b.album_id))
                    .then_with(|| a.track.cmp(&b.track))
            });
            return Ok(crate::model::filtered_page(items, "", offset, limit));
        }
        let personal = tab == ArtistTab::PersonalTop;
        if self.server.kind == ServerType::Navidrome {
            let params = [
                p(
                    if self.features().artists_search {
                        "artists_id"
                    } else {
                        "album_artist_id"
                    },
                    &item.id,
                ),
                p("_start", offset),
                p("_end", offset + limit),
                p("_sort", if personal { "playCount" } else { "title" }),
                p("_order", if personal { "DESC" } else { "ASC" }),
            ];
            let (value, total) = self.request(Method::GET, "api/song", &params, None)?;
            return Ok(self.page(array(&value), ItemKind::Song, total, &query));
        }
        if self.server.kind == ServerType::Jellyfin {
            let params = [
                p("UserId", &self.session().user_id),
                p("ArtistIds", &item.id),
                p("Recursive", true),
                p("IncludeItemTypes", self.jf_song_types()),
                p("StartIndex", offset),
                p("Limit", limit),
                p("SortBy", if personal { "PlayCount" } else { "SortName" }),
                p(
                    "SortOrder",
                    if personal { "Descending" } else { "Ascending" },
                ),
                p(
                    "Fields",
                    "Genres,DateCreated,Tags,Overview,MediaSources,MediaStreams,GenreItems,ArtistItems,ParentId,SortName,Path,ProviderIds",
                ),
            ];
            let (value, _) = self.request(
                Method::GET,
                &format!("Users/{}/Items", self.session().user_id),
                &params,
                None,
            )?;
            return Ok(self.page(
                array(&value["Items"]),
                ItemKind::Song,
                value["TotalRecordCount"]
                    .as_u64()
                    .map(|total| total as usize),
                &query,
            ));
        }
        let mut items = self.all_tracks(item)?;
        if personal {
            items.sort_by_key(|song| std::cmp::Reverse(song.play_count));
        } else {
            items.sort_by_key(|song| song.name.to_lowercase());
        }
        let total = items.len();
        query.sort = if personal { Sort::Frequent } else { Sort::Name };
        Ok(Page {
            items: items.into_iter().skip(offset).take(limit).collect(),
            total: Some(total),
            has_more: offset.saturating_add(limit) < total,
        })
    }

    pub fn genre_query(&self, item: &Item, mut query: ListQuery) -> Result<ListQuery> {
        ensure!(
            item.kind == ItemKind::Genre
                && item.server_id == self.server.id
                && matches!(query.kind, ItemKind::Album | ItemKind::Song),
            "Invalid genre target"
        );
        // The original genre route overrides the list's selected genres while
        // retaining its search, sort and other server filters.
        query.genre = None;
        query.filters.genre_ids = vec![if self.server.kind == ServerType::Subsonic {
            item.name.clone()
        } else {
            item.id.clone()
        }];
        query.filters.genre_all = false;
        Ok(query)
    }
    pub fn all_artist_items(&self, item: &Item, tab: ArtistTab) -> Result<Vec<Item>> {
        let mut items = vec![];
        let mut previous = Vec::new();
        loop {
            let page = self.artist_page(item, tab, items.len(), 500)?;
            if page.items.is_empty() {
                break;
            }
            let ids = page
                .items
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>();
            ensure!(ids != previous, "Server repeated an artist page");
            previous = ids;
            items.extend(page.items);
            if !page.has_more {
                break;
            }
        }
        Ok(items)
    }
    pub fn random_songs(&self, count: usize, similar_to: Option<&Item>) -> Result<Vec<Item>> {
        self.random_songs_filtered(count, similar_to, None, None)
    }
    pub fn similar_songs(&self, item: &Item) -> Result<Vec<Item>> {
        self.random_songs_request(50, Some(item), None, None)
    }
    pub fn random_songs_filtered(
        &self,
        count: usize,
        similar_to: Option<&Item>,
        genre: Option<&str>,
        folder: Option<&str>,
    ) -> Result<Vec<Item>> {
        ensure!(count > 0 && count <= 500, "Invalid song count");
        let folder = folder.or_else(|| {
            self.server
                .options
                .music_folder_ids
                .first()
                .map(String::as_str)
        });
        self.random_songs_request(count, similar_to, genre, folder)
    }
    fn random_songs_request(
        &self,
        count: usize,
        similar_to: Option<&Item>,
        genre: Option<&str>,
        folder: Option<&str>,
    ) -> Result<Vec<Item>> {
        if let Some(item) = similar_to {
            ensure!(
                item.server_id == self.server.id,
                "Item belongs to a different server"
            );
        }
        if self.server.kind == ServerType::Jellyfin {
            if let Some(item) = similar_to {
                ensure!(
                    item.server_id == self.server.id,
                    "Item belongs to a different server"
                );
                let params = vec![
                    p("UserId", &self.session().user_id),
                    p("Limit", count),
                    p(
                        "Fields",
                        "Genres,DateCreated,Tags,Overview,MediaSources,MediaStreams,GenreItems,ArtistItems,ParentId,SortName,Path,ProviderIds",
                    ),
                ];
                if !self.server.options.prefer_instant_mix
                    && !matches!(
                        item.kind,
                        ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                    )
                    && let Ok((value, _)) = self.request(
                        Method::GET,
                        &format!("Items/{}/Similar", item.id),
                        &params,
                        None,
                    )
                {
                    let songs = array(&value["Items"])
                        .iter()
                        .map(|value| self.normalize(value, ItemKind::Song))
                        .filter(|song| song.id != item.id && !song.id.is_empty())
                        .collect::<Vec<_>>();
                    if !songs.is_empty() {
                        return Ok(songs);
                    }
                }
                let (value, _) = self.request(
                    Method::GET,
                    &format!("Items/{}/InstantMix", item.id),
                    &params,
                    None,
                )?;
                return Ok(array(&value["Items"])
                    .iter()
                    .map(|value| self.normalize(value, ItemKind::Song))
                    .filter(|song| song.id != item.id)
                    .collect());
            }
            return Ok(self
                .list(&ListQuery {
                    kind: ItemKind::Song,
                    limit: count,
                    sort: Sort::Random,
                    filters: crate::filters::Filters {
                        genre_ids: genre.into_iter().map(str::to_owned).collect(),
                        ..Default::default()
                    },
                    folder_id: folder.map(str::to_owned),
                    ..ListQuery::default()
                })?
                .items);
        }
        let (endpoint, key, mut params) = if let Some(item) = similar_to {
            (
                if matches!(item.kind, ItemKind::Album | ItemKind::Song) {
                    "getSimilarSongs"
                } else {
                    "getSimilarSongs2"
                },
                if matches!(item.kind, ItemKind::Album | ItemKind::Song) {
                    "similarSongs"
                } else {
                    "similarSongs2"
                },
                vec![p("id", &item.id), p("count", count)],
            )
        } else {
            ("getRandomSongs", "randomSongs", vec![p("size", count)])
        };
        if let Some(genre) = genre {
            params.push(p("genre", genre));
        }
        if let Some(folder) = folder {
            params.push(p("musicFolderId", folder));
        }
        let v = self.subsonic(endpoint, &params)?;
        Ok(array(&v[key]["song"])
            .iter()
            .map(|v| self.normalize(v, ItemKind::Song))
            .filter(|song| similar_to.is_none_or(|item| song.id != item.id))
            .collect())
    }

    pub fn random_library_items(&self, config: &crate::shuffle::Config) -> Result<Vec<Item>> {
        use crate::{auto_dj::Mode, shuffle::Played};
        config.validate()?;
        self.check_cancellation()?;
        let albums = config.kind == Mode::Albums;
        let kind = if albums {
            ItemKind::Album
        } else {
            ItemKind::Song
        };
        let folder = if config.music_folder.is_empty() {
            self.server
                .options
                .music_folder_ids
                .first()
                .map(String::as_str)
        } else {
            Some(config.music_folder.as_str())
        };
        let genre = (!config.genre.is_empty()).then_some(config.genre.as_str());
        let mut items = if self.server.kind == ServerType::Jellyfin {
            let mut query = ListQuery {
                kind,
                limit: config.limit,
                sort: Sort::Random,
                folder_id: folder.map(str::to_owned),
                ..Default::default()
            };
            query.filters.genre_ids = genre.into_iter().map(str::to_owned).collect();
            // The original Jellyfin controller sends Years only when both bounds are enabled.
            if let Some((min, max)) = config.min().zip(config.max()) {
                query.filters.min_year = Some(min);
                query.filters.max_year = Some(max);
            }
            if !albums && config.played != Played::All {
                query
                    .filters
                    .custom
                    .insert("IsPlayed".into(), json!(config.played == Played::Played));
            }
            self.jf_list(&query)?.items
        } else if albums && self.server.kind == ServerType::Navidrome {
            let mut query = ListQuery {
                kind,
                limit: config.limit,
                sort: Sort::Random,
                folder_id: folder.map(str::to_owned),
                genre: genre.map(str::to_owned),
                ..Default::default()
            };
            // The original native album endpoint uses a single year parameter.
            query.filters.min_year = config.min();
            query.filters.max_year = config.min();
            self.list(&query)?.items
        } else {
            let mut params = vec![p("size", config.limit)];
            if let Some(folder) = folder {
                params.push(p("musicFolderId", folder));
            }
            if let Some(genre) = genre {
                params.push(p("genre", genre));
            }
            if albums {
                params.push(p("offset", 0));
                params.push(p(
                    "type",
                    if config.min().is_some() {
                        "byYear"
                    } else if genre.is_some() {
                        "byGenre"
                    } else {
                        "random"
                    },
                ));
                if let Some(year) = config.min() {
                    params.push(p("fromYear", year));
                    params.push(p("toYear", chrono::Local::now().format("%Y")));
                }
                array(&self.subsonic("getAlbumList2", &params)?["albumList2"]["album"])
                    .iter()
                    .map(|item| self.normalize(item, kind))
                    .collect()
            } else {
                if let Some(year) = config.min() {
                    params.push(p("fromYear", year));
                }
                if let Some(year) = config.max() {
                    params.push(p("toYear", year));
                }
                array(&self.subsonic("getRandomSongs", &params)?["randomSongs"]["song"])
                    .iter()
                    .map(|item| self.normalize(item, kind))
                    .collect()
            }
        };
        items.truncate(config.limit);
        Ok(items)
    }

    pub fn random_library_tracks(
        &self,
        config: &crate::shuffle::Config,
        shuffle: bool,
    ) -> Result<Vec<Item>> {
        let mut tracks = Vec::new();
        for item in self.random_library_items(config)? {
            tracks.extend(self.all_tracks(&item)?);
        }
        ensure!(!tracks.is_empty(), "No playable tracks found");
        if shuffle {
            use rand::seq::SliceRandom;
            tracks.shuffle(&mut rand::thread_rng());
        }
        Ok(tracks)
    }

    pub fn stream_url(&self, item: &Item, settings: &crate::storage::Settings) -> Result<String> {
        self.check_cancellation()?;
        ensure!(
            item.server_id == self.server.id,
            "Track belongs to a different server"
        );
        if item.kind == ItemKind::Radio {
            let url = Url::parse(&item.stream_url)?;
            ensure!(
                matches!(url.scheme(), "http" | "https"),
                "Radio URL must use HTTP or HTTPS"
            );
            return Ok(url.into());
        }
        if settings.transcode {
            settings.validate_transcoding()?;
        }
        if self.server.kind == ServerType::Jellyfin {
            let format = if settings.format.is_empty() {
                "mp3".to_owned()
            } else {
                settings.format.to_ascii_lowercase()
            };
            let path = if settings.transcode {
                format!("Audio/{}/stream.{format}", item.id)
            } else {
                format!("Items/{}/Download", item.id)
            };
            let mut params = vec![
                p("apiKey", &self.session().token),
                p("UserId", &self.session().user_id),
            ];
            if settings.transcode {
                params.extend([
                    p("Static", false),
                    p("AudioCodec", &format),
                    p("PlaySessionId", uuid::Uuid::new_v4()),
                ]);
                if let Some(bitrate) = settings.bitrate {
                    params.push(p("AudioBitRate", u64::from(bitrate) * 1000));
                }
                let mut cap = settings.max_sample_rate.filter(|rate| *rate > 0);
                if matches!(
                    format.to_lowercase().as_str(),
                    "mp3" | "aac" | "ogg" | "opus" | "vorbis"
                ) {
                    cap = cap.map(|rate| rate.min(48000));
                }
                if let Some(rate) = cap
                    && item
                        .column_data
                        .get("sampleRate")
                        .and_then(|rate| rate.parse::<u32>().ok())
                        .is_some_and(|source| source > rate)
                {
                    params.push(p("AudioSampleRate", rate));
                }
            }
            return Ok(self.url(&path, &params)?.into());
        }
        let mut params = self.ss_params();
        params.push(p("id", &item.id));
        if settings.transcode {
            if !settings.format.is_empty() {
                params.push(p("format", &settings.format));
            }
            if let Some(bitrate) = settings.bitrate {
                params.push(p("maxBitRate", bitrate));
            }
        }
        Ok(self.url("rest/stream.view", &params)?.into())
    }

    pub fn presence_image(&self, item: &Item) -> Result<Option<String>> {
        ensure!(
            item.server_id == self.server.id,
            "Artwork belongs to a different server"
        );
        let mut server = self.server.clone();
        if !server.options.remote_url.is_empty() {
            server.options.prefer_remote_url = true;
        }
        let mut api = Self::new(server, self.session())?;
        api.client = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(self.server.options.ignore_ssl)
            .timeout(Duration::from_secs(3))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        if self.server.kind == ServerType::Jellyfin {
            if item.cover_id.is_empty() {
                return Ok(None);
            }
            return Ok(Some(
                api.url(
                    &format!("Items/{}/Images/Primary", item.cover_id),
                    &[p("apiKey", &self.session().token), p("MaxWidth", 400)],
                )?
                .into(),
            ));
        }
        if item.album_id.is_empty() {
            return Ok(None);
        }
        let value = api.subsonic("getAlbumInfo2", &[p("id", &item.album_id)])?;
        let info = &value["albumInfo"];
        Ok(["largeImageUrl", "mediumImageUrl", "smallImageUrl"]
            .into_iter()
            .find_map(|key| {
                info[key]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .and_then(crate::presence::image_url)
            }))
    }

    pub fn cover(&self, item: &Item, path: &Path) -> Result<()> {
        crate::storage::atomic_write(path, &self.cover_bytes(item)?)
    }

    pub fn cover_bytes(&self, item: &Item) -> Result<Vec<u8>> {
        self.cover_bytes_sized(item, 400.)
    }

    pub fn cover_sized(&self, item: &Item, path: &Path, size: f64) -> Result<()> {
        crate::storage::atomic_write(path, &self.cover_bytes_sized(item, size)?)
    }

    pub fn cover_bytes_sized(&self, item: &Item, size: f64) -> Result<Vec<u8>> {
        self.check_cancellation()?;
        ensure!(size.is_finite(), "Artwork resolution must be finite");
        ensure!(
            item.server_id == self.server.id,
            "Cover belongs to a different server"
        );
        ensure!(!item.cover_id.is_empty(), "No cover art");
        let url = if self.server.kind == ServerType::Jellyfin {
            let mut params = vec![p("apiKey", &self.session().token), p("quality", 96)];
            if size != 0. {
                params.push(p("width", size));
            }
            self.url(&format!("Items/{}/Images/Primary", item.cover_id), &params)?
        } else {
            let mut params = self.ss_params();
            params.push(p("id", &item.cover_id));
            if size != 0. {
                params.push(p("size", size));
            }
            self.url("rest/getCoverArt.view", &params)?
        };
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| e.without_url())?
            .error_for_status()
            .map_err(|e| e.without_url())?;
        use std::io::Read;
        let mut bytes = Vec::new();
        response.take(20_000_001).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 20_000_000, "Cover image is too large");
        self.check_cancellation()?;
        Ok(bytes)
    }

    pub fn favorite(&self, item: &Item, enabled: bool) -> Result<()> {
        ensure!(
            item.server_id == self.server.id,
            "Favorite belongs to a different server"
        );
        if self.server.kind == ServerType::Jellyfin {
            self.request(
                if enabled {
                    Method::POST
                } else {
                    Method::DELETE
                },
                &format!("Users/{}/FavoriteItems/{}", self.session().user_id, item.id),
                &[],
                None,
            )?;
        } else {
            let key = match item.kind {
                ItemKind::Album => "albumId",
                ItemKind::Artist | ItemKind::AlbumArtist => "artistId",
                _ => "id",
            };
            self.subsonic(if enabled { "star" } else { "unstar" }, &[p(key, &item.id)])?;
        }
        Ok(())
    }

    pub fn rate(&self, item: &Item, rating: u8) -> Result<()> {
        ensure!(
            item.server_id == self.server.id,
            "Rating belongs to a different server"
        );
        ensure!(rating <= 5, "Rating must be between 0 and 5");
        if self.server.kind == ServerType::Jellyfin {
            bail!("Jellyfin does not support five-star ratings");
        }
        self.subsonic("setRating", &[p("id", &item.id), p("rating", rating)])?;
        Ok(())
    }

    pub fn playlist_users(&self) -> Result<Vec<(String, String)>> {
        ensure!(
            self.server.kind == ServerType::Navidrome && self.session().is_admin,
            "Changing playlist ownership requires a Navidrome administrator"
        );
        let mut users = Vec::new();
        loop {
            let offset = users.len();
            ensure!(offset < 10_000, "Too many users to display");
            let (value, total) = self.request(
                Method::GET,
                "api/user",
                &[
                    p("_start", offset),
                    p("_end", offset + 500),
                    p("_sort", "userName"),
                    p("_order", "ASC"),
                ],
                None,
            )?;
            let page = value.as_array().context("Invalid user list")?;
            for user in page {
                let id = s(user, "id");
                if !id.is_empty() {
                    users.push((id, s(user, "userName")));
                }
            }
            if page.len() < 500 || total.is_some_and(|total| users.len() >= total) {
                return Ok(users);
            }
            ensure!(users.len() > offset, "User pagination did not advance");
        }
    }

    pub fn create_playlist(
        &self,
        name: &str,
        public: bool,
        rules: Option<Value>,
        songs: &[Item],
    ) -> Result<String> {
        ensure!(!name.trim().is_empty(), "Enter a playlist name");
        if rules.is_some() {
            ensure!(
                self.server.kind == ServerType::Navidrome,
                "Smart playlists require Navidrome"
            );
        }
        let id = match self.server.kind {
            ServerType::Jellyfin => s(&self.request(Method::POST, "Playlists", &[], Some(json!({"Name":name,"UserId": self.session().user_id,"MediaType":"Audio","IsPublic":public})))?.0, "Id"),
            ServerType::Navidrome => s(&self.request(Method::POST, "api/playlist", &[], Some(json!({"name":name,"comment":"","public":public,"rules":rules})))?.0, "id"),
            ServerType::Subsonic => s(&self.subsonic("createPlaylist", &[p("name", name)])?["playlist"], "id"),
        };
        ensure!(!id.is_empty(), "Server returned no playlist ID");
        if !songs.is_empty() {
            self.add_to_playlist(&id, songs)?;
        }
        Ok(id)
    }

    pub fn update_playlist(
        &self,
        playlist: &Item,
        name: &str,
        comment: &str,
        public: bool,
        rules: Option<Value>,
    ) -> Result<()> {
        ensure!(!name.trim().is_empty(), "Enter a playlist name");
        ensure!(
            playlist.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        match self.server.kind {
            ServerType::Navidrome => {
                let mut body = json!({"name":name,"comment":comment,"public":public,"rules":rules});
                if !playlist.owner_id.is_empty() {
                    body["ownerId"] = playlist.owner_id.clone().into();
                }
                if let Some(sync) = playlist.sync {
                    body["sync"] = sync.into();
                }
                self.request(
                    Method::PUT,
                    &format!("api/playlist/{}", playlist.id),
                    &[],
                    Some(body),
                )?;
            }
            ServerType::Subsonic => {
                self.subsonic(
                    "updatePlaylist",
                    &[
                        p("playlistId", &playlist.id),
                        p("name", name),
                        p("comment", comment),
                        p("public", public),
                    ],
                )?;
            }
            ServerType::Jellyfin => {
                // Updating the playlist can reload older Jellyfin's stored
                // metadata. Apply an edited description after that operation.
                self.request(
                    Method::POST,
                    &format!("Playlists/{}", playlist.id),
                    &[],
                    Some(json!({"Name":name,"IsPublic":public})),
                )?;
                let path = format!("Items/{}", playlist.id);
                let (mut existing, _) = self.request(
                    Method::GET,
                    &format!("Users/{}/Items/{}", self.session().user_id, playlist.id),
                    &[],
                    None,
                )?;
                if s(&existing, "Overview") != comment {
                    existing["Name"] = name.into();
                    existing["Overview"] = comment.into();
                    self.request(Method::POST, &path, &[], Some(existing))?;
                }
            }
        }
        Ok(())
    }

    pub fn add_to_playlist(&self, id: &str, songs: &[Item]) -> Result<()> {
        ensure!(
            songs
                .iter()
                .all(|song| song.server_id == self.server.id && song.kind == ItemKind::Song),
            "Playlist tracks must belong to the same server"
        );
        if self.server.kind == ServerType::Jellyfin {
            for chunk in songs.chunks(50) {
                self.request(
                    Method::POST,
                    &format!("Playlists/{id}/Items"),
                    &[
                        p("UserId", &self.session().user_id),
                        p(
                            "Ids",
                            chunk
                                .iter()
                                .map(|s| s.id.as_str())
                                .collect::<Vec<_>>()
                                .join(","),
                        ),
                    ],
                    None,
                )?;
            }
        } else if self.server.kind == ServerType::Navidrome {
            self.request(
                Method::POST,
                &format!("api/playlist/{id}/tracks"),
                &[],
                Some(json!({"ids": songs.iter().map(|s| &s.id).collect::<Vec<_>>()})),
            )?;
        } else {
            let mut params = vec![p("playlistId", id)];
            params.extend(songs.iter().map(|song| p("songIdToAdd", &song.id)));
            self.subsonic("updatePlaylist", &params)?;
        }
        Ok(())
    }

    pub fn remove_playlist_track(&self, playlist: &Item, song: &Item, index: usize) -> Result<()> {
        ensure!(
            playlist.server_id == self.server.id && song.server_id == self.server.id,
            "Playlist track belongs to a different server"
        );
        match self.server.kind {
            ServerType::Jellyfin => {
                ensure!(
                    !song.playlist_entry_id.is_empty(),
                    "Missing playlist entry ID"
                );
                self.request(
                    Method::DELETE,
                    &format!("Playlists/{}/Items", playlist.id),
                    &[p("EntryIds", &song.playlist_entry_id)],
                    None,
                )?;
            }
            ServerType::Navidrome => {
                ensure!(
                    !song.playlist_entry_id.is_empty(),
                    "Missing playlist entry ID"
                );
                self.request(
                    Method::DELETE,
                    &format!("api/playlist/{}/tracks", playlist.id),
                    &[p("id", &song.playlist_entry_id)],
                    None,
                )?;
            }
            ServerType::Subsonic => {
                self.subsonic(
                    "updatePlaylist",
                    &[p("playlistId", &playlist.id), p("songIndexToRemove", index)],
                )?;
            }
        }
        Ok(())
    }

    pub fn replace_playlist_tracks(&self, playlist: &Item, songs: &[Item]) -> Result<()> {
        ensure!(
            playlist.kind == ItemKind::Playlist && playlist.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        ensure!(
            songs
                .iter()
                .all(|song| song.kind == ItemKind::Song && song.server_id == self.server.id),
            "Playlist tracks must belong to the same server"
        );
        if self.server.kind == ServerType::Jellyfin {
            self.request(
                Method::POST,
                &format!("Playlists/{}", playlist.id),
                &[],
                Some(json!({"Ids":songs.iter().map(|song| &song.id).collect::<Vec<_>>()})),
            )?;
        } else {
            let mut params = vec![p("playlistId", &playlist.id)];
            params.extend(songs.iter().map(|song| p("songId", &song.id)));
            self.subsonic("createPlaylist", &params)?;
        }
        Ok(())
    }

    pub fn delete_playlist(&self, id: &str) -> Result<()> {
        match self.server.kind {
            ServerType::Jellyfin => {
                self.request(Method::DELETE, &format!("Items/{id}"), &[], None)?;
            }
            ServerType::Navidrome => {
                self.request(Method::DELETE, &format!("api/playlist/{id}"), &[], None)?;
            }
            ServerType::Subsonic => {
                self.subsonic("deletePlaylist", &[p("id", id)])?;
            }
        }
        Ok(())
    }

    pub fn edit_playlist_selection(
        &self,
        playlist: &Item,
        expected: &[Item],
        selected: &[usize],
        action: PlaylistSelectionAction,
    ) -> Result<()> {
        ensure!(
            playlist.kind == ItemKind::Playlist && playlist.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        ensure!(
            !selected.is_empty() && selected.len() <= 500,
            "Select 1 to 500 playlist tracks"
        );
        let positions = expected
            .iter()
            .enumerate()
            .map(|(index, item)| item.playlist_position.unwrap_or(index))
            .collect::<Vec<_>>();
        ensure!(
            selected.iter().all(|index| positions.contains(index)),
            "Invalid playlist selection"
        );
        let tracks = self.all_tracks(playlist)?;
        ensure!(
            expected.iter().zip(&positions).all(|(old, index)| {
                tracks.get(*index).is_some_and(|fresh| {
                    old.id == fresh.id
                        && old.server_id == fresh.server_id
                        && old.playlist_entry_id == fresh.playlist_entry_id
                })
            }),
            "Playlist changed; refresh before editing"
        );
        let selected = selected
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        // Newer Jellyfin versions identify entries by song ID, so a remove
        // deletes every duplicate and a move addresses only the first copy.
        let ambiguous_entries = self.server.kind == ServerType::Jellyfin && {
            let mut entries = std::collections::HashSet::new();
            tracks
                .iter()
                .any(|track| !entries.insert(&track.playlist_entry_id))
        };
        if ambiguous_entries && matches!(action, PlaylistSelectionAction::Remove) {
            let remaining = tracks
                .into_iter()
                .enumerate()
                .filter_map(|(index, track)| (!selected.contains(&index)).then_some(track))
                .collect::<Vec<_>>();
            return self.replace_playlist_tracks(playlist, &remaining);
        }
        match action {
            PlaylistSelectionAction::Remove => match self.server.kind {
                ServerType::Subsonic => {
                    let mut params = vec![p("playlistId", &playlist.id)];
                    params.extend(
                        selected
                            .iter()
                            .rev()
                            .map(|index| p("songIndexToRemove", index)),
                    );
                    self.subsonic("updatePlaylist", &params)?;
                }
                ServerType::Navidrome | ServerType::Jellyfin => {
                    ensure!(
                        selected
                            .iter()
                            .all(|index| !tracks[*index].playlist_entry_id.is_empty()),
                        "Missing playlist entry identity"
                    );
                    let entries = selected
                        .iter()
                        .map(|index| tracks[*index].playlist_entry_id.as_str())
                        .collect::<Vec<_>>();
                    if self.server.kind == ServerType::Navidrome {
                        self.request(
                            Method::DELETE,
                            &format!("api/playlist/{}/tracks", playlist.id),
                            &entries.iter().map(|id| p("id", id)).collect::<Vec<_>>(),
                            None,
                        )?;
                    } else {
                        for chunk in entries.chunks(50) {
                            self.request(
                                Method::DELETE,
                                &format!("Playlists/{}/Items", playlist.id),
                                &[p("EntryIds", chunk.join(","))],
                                None,
                            )?;
                        }
                    }
                }
            },
            PlaylistSelectionAction::MoveBefore(before) => {
                let moves = playlist_moves(tracks.len(), &selected, before)?;
                let mut order = (0..tracks.len()).collect::<Vec<_>>();
                if self.server.kind == ServerType::Subsonic || ambiguous_entries {
                    for (from, to) in moves {
                        let item = order.remove(from);
                        order.insert(to, item);
                    }
                    let reordered = order
                        .iter()
                        .map(|index| tracks[*index].clone())
                        .collect::<Vec<_>>();
                    self.replace_playlist_tracks(playlist, &reordered)?;
                } else {
                    for (from, to) in moves {
                        self.move_playlist_track(playlist, &tracks[order[from]], from, to)?;
                        let index = order.remove(from);
                        order.insert(to, index);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn move_playlist_track(
        &self,
        playlist: &Item,
        song: &Item,
        from: usize,
        to: usize,
    ) -> Result<()> {
        ensure!(
            playlist.server_id == self.server.id && song.server_id == self.server.id,
            "Playlist belongs to a different server"
        );
        match self.server.kind {
            ServerType::Navidrome => {
                self.request(
                    Method::PUT,
                    &format!("api/playlist/{}/tracks/{}", playlist.id, from + 1),
                    &[],
                    Some(json!({"insert_before":(to+1).to_string()})),
                )?;
            }
            ServerType::Jellyfin => {
                ensure!(
                    !song.playlist_entry_id.is_empty(),
                    "Missing playlist entry ID"
                );
                self.request(
                    Method::POST,
                    &format!(
                        "Playlists/{}/Items/{}/Move/{to}",
                        playlist.id, song.playlist_entry_id
                    ),
                    &[],
                    None,
                )?;
            }
            ServerType::Subsonic => {
                let mut tracks = self.all_tracks(playlist)?;
                ensure!(
                    from < tracks.len() && to < tracks.len() && tracks[from].id == song.id,
                    "Playlist changed; refresh before reordering"
                );
                let moved = tracks.remove(from);
                tracks.insert(to, moved);
                let mut params = vec![p("playlistId", &playlist.id)];
                params.extend(tracks.iter().map(|song| p("songId", &song.id)));
                self.subsonic("createPlaylist", &params)?;
            }
        }
        Ok(())
    }

    pub fn update_radio(
        &self,
        station: &Item,
        name: &str,
        stream_url: &str,
        homepage: &str,
    ) -> Result<()> {
        ensure!(
            station.server_id == self.server.id,
            "Radio station belongs to a different server"
        );
        ensure!(!name.trim().is_empty(), "Enter a station name");
        let url = Url::parse(stream_url)?;
        ensure!(
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some(),
            "Radio stream must use HTTP or HTTPS"
        );
        if self.server.kind == ServerType::Navidrome {
            self.request(
                Method::PUT,
                &format!("api/radio/{}", station.id),
                &[],
                Some(json!({"name":name,"streamUrl":stream_url,"homePageUrl":homepage})),
            )?;
            return Ok(());
        }
        self.subsonic(
            "updateInternetRadioStation",
            &[
                p("id", &station.id),
                p("name", name),
                p("streamUrl", stream_url),
                p("homepageUrl", homepage),
            ],
        )?;
        Ok(())
    }

    pub fn delete_radio(&self, station: &Item) -> Result<()> {
        ensure!(
            station.server_id == self.server.id,
            "Radio station belongs to a different server"
        );
        if self.server.kind == ServerType::Navidrome {
            self.request(
                Method::DELETE,
                &format!("api/radio/{}", station.id),
                &[],
                None,
            )?;
        } else {
            self.subsonic("deleteInternetRadioStation", &[p("id", &station.id)])?;
        }
        Ok(())
    }

    pub fn share(
        &self,
        items: &[Item],
        description: &str,
        downloadable: bool,
        expires: Option<u64>,
    ) -> Result<String> {
        ensure!(
            self.server.kind == ServerType::Navidrome,
            "Sharing requires Navidrome"
        );
        let first = items.first().context("Select items to share")?;
        ensure!(
            items.iter().all(|item| item.server_id == self.server.id
                && item.kind == first.kind
                && !item.id.contains(',')),
            "Shared items must have the same type and server"
        );
        let resource = match first.kind {
            ItemKind::Song => "song",
            ItemKind::Album => "album",
            ItemKind::Artist | ItemKind::AlbumArtist => "albumArtist",
            ItemKind::Playlist => "playlist",
            ItemKind::Folder => "folder",
            _ => bail!("This item cannot be shared"),
        };
        if let Some(expires) = expires {
            ensure!(
                expires == 0
                    || expires as u128
                        > std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_millis(),
                "Share expiration must be in the future"
            );
        }
        let mut body = json!({"resourceType":resource,"resourceIds":items.iter().map(|item|item.id.as_str()).collect::<Vec<_>>().join(","),"description":description,"downloadable":downloadable});
        if let Some(expires) = expires {
            body["expires"] = expires.into();
        }
        let (response, _) = self.request(Method::POST, "api/share", &[], Some(body))?;
        let id = s(&response, "id");
        ensure!(!id.is_empty(), "Server returned no share ID");
        let mut url = Url::parse(&self.base_url)?;
        url.path_segments_mut()
            .map_err(|_| anyhow::anyhow!("Invalid share URL"))?
            .push("share")
            .push(&id);
        Ok(url.to_string())
    }

    fn image_path(&self, item: &Item) -> Result<String> {
        ensure!(
            item.server_id == self.server.id,
            "Image belongs to a different server"
        );
        ensure!(
            matches!(
                item.kind,
                ItemKind::Playlist | ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Radio
            ),
            "This item does not support image editing"
        );
        match self.server.kind {
            ServerType::Jellyfin => {
                ensure!(
                    item.kind != ItemKind::Radio,
                    "Local radio artwork is stored locally"
                );
                Ok(format!("Items/{}/Images/Primary", item.id))
            }
            ServerType::Navidrome => Ok(format!(
                "api/{}/{}/image",
                match item.kind {
                    ItemKind::Playlist => "playlist",
                    ItemKind::Artist | ItemKind::AlbumArtist => "artist",
                    _ => "radio",
                },
                item.id
            )),
            _ => bail!("This server does not support image editing"),
        }
    }
    pub fn remove_image(&self, item: &Item) -> Result<()> {
        self.request(Method::DELETE, &self.image_path(item)?, &[], None)?;
        Ok(())
    }
    pub fn upload_image(&self, item: &Item, bytes: Vec<u8>) -> Result<()> {
        ensure!(
            bytes.len() <= 20_000_000,
            "Image is too large (maximum 20 MB)"
        );
        let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            "image/png"
        } else if bytes.starts_with(b"\xff\xd8\xff") {
            "image/jpeg"
        } else if bytes.starts_with(b"GIF8") {
            "image/gif"
        } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
            "image/webp"
        } else {
            bail!("Choose a PNG, JPEG, GIF, or WebP image")
        };
        self.upload_image_request(item, &bytes, mime, true)
    }
    fn upload_image_request(
        &self,
        item: &Item,
        bytes: &[u8],
        mime: &str,
        retry: bool,
    ) -> Result<()> {
        use base64::Engine;
        self.check_cancellation()?;
        let used_token = self.session().token;
        let path = self.image_path(item)?;
        let mut request = self.client.post(self.url(&path, &[])?);
        request = match self.server.kind {
            ServerType::Navidrome => request
                .header("x-nd-authorization", format!("Bearer {used_token}"))
                .multipart(
                    reqwest::blocking::multipart::Form::new().part(
                        "image",
                        reqwest::blocking::multipart::Part::bytes(bytes.to_vec())
                            .file_name("image")
                            .mime_str(mime)?,
                    ),
                ),
            ServerType::Jellyfin => request
                .header("Authorization", self.jf_header(&used_token))
                .header("Content-Type", mime)
                .body(base64::engine::general_purpose::STANDARD.encode(bytes)),
            _ => unreachable!(),
        };
        let response = request.send().map_err(|e| e.without_url())?;
        self.check_cancellation()?;
        if response.status().as_u16() == 401 && retry && self.can_renew_session(&path) {
            self.renew_session(&used_token)?;
            return self.upload_image_request(item, bytes, mime, false);
        }
        self.check_authorization(response.status().as_u16())?;
        ensure!(
            response.status().is_success(),
            "Image upload failed (HTTP {})",
            response.status().as_u16()
        );
        if self.server.kind == ServerType::Navidrome {
            self.rotate_session(response.headers(), &used_token)?;
        }
        Ok(())
    }

    pub fn filter_catalog(
        &self,
        kind: ItemKind,
    ) -> Result<std::collections::BTreeMap<String, Vec<String>>> {
        if self.server.kind == ServerType::Navidrome {
            return Ok(self
                .tag_names()?
                .into_iter()
                .map(|name| (name, vec![]))
                .collect());
        }
        if self.server.kind != ServerType::Jellyfin {
            return Ok(Default::default());
        }
        let types = if kind == ItemKind::Song {
            self.jf_song_types()
        } else {
            "MusicAlbum"
        };
        let mut params = vec![
            p("UserId", self.session().user_id),
            p("IncludeItemTypes", types),
        ];
        if let Some(folder) = self.server.options.music_folder_ids.first() {
            params.push(p("ParentId", folder));
        }
        let (filters, _) = self.request(Method::GET, "items/filters", &params, None)?;
        params.push(p("EnableTotalRecordCount", true));
        let (studios, _) = self.request(Method::GET, "studios", &params, None)?;
        let mut catalog = std::collections::BTreeMap::new();
        for (name, values) in [
            (
                "Tags",
                array(&filters["Tags"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            ),
            (
                "Studios",
                array(&studios["Items"])
                    .iter()
                    .filter_map(|item| item["Name"].as_str())
                    .map(str::to_owned)
                    .collect(),
            ),
        ] {
            let values = values
                .into_iter()
                .filter(|value| value.len() <= 4096 && !value.contains('\0'))
                .collect::<std::collections::BTreeSet<_>>();
            if !values.is_empty() {
                catalog.insert(name.into(), values.into_iter().collect());
            }
        }
        Ok(catalog)
    }

    pub fn tag_names(&self) -> Result<Vec<String>> {
        ensure!(
            self.server.kind == ServerType::Navidrome,
            "Server tag names require Navidrome"
        );
        let parameters = self
            .server
            .options
            .music_folder_ids
            .iter()
            .map(|id| p("library_id", id))
            .collect::<Vec<_>>();
        let (value, _) = self.request(Method::GET, "api/tag", &parameters, None)?;
        let names = array(if value.is_array() {
            &value
        } else {
            &value["data"]
        })
        .iter()
        .filter_map(|tag| {
            let name = tag["tagName"].as_str()?;
            (!name.is_empty()
                && name.len() <= 128
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
            .then(|| name.to_owned())
        })
        .collect::<std::collections::BTreeSet<_>>();
        Ok(names.into_iter().collect())
    }

    pub fn tag_filter_values(
        &self,
        name: &str,
        search: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Page> {
        ensure!(
            self.server.kind == ServerType::Navidrome,
            "Server tag filters require Navidrome"
        );
        ensure!(
            !name.is_empty()
                && name.len() <= 128
                && !name.contains(['\0', '\n'])
                && (1..=500).contains(&limit),
            "Invalid tag filter"
        );
        let mut params = vec![
            p("tag_name", name),
            p("tag_value", search),
            p("_start", offset),
            p("_end", offset.saturating_add(limit)),
            p("_sort", "tagValue"),
            p("_order", "ASC"),
        ];
        params.extend(
            self.server
                .options
                .music_folder_ids
                .iter()
                .map(|id| p("library_id", id)),
        );
        let (value, total) = self.request(Method::GET, "api/tag", &params, None)?;
        let items: Vec<_> = array(if value.is_array() {
            &value
        } else {
            &value["data"]
        })
        .into_iter()
        .take(limit)
        .map(|value| Item {
            id: s(&value, "id"),
            name: s(&value, "tagValue"),
            kind: ItemKind::Genre,
            server_id: self.server.id,
            ..Default::default()
        })
        .collect();
        let has_more = !items.is_empty()
            && total.map_or(items.len() == limit, |total| {
                offset.saturating_add(items.len()) < total
            });
        Ok(Page {
            items,
            total,
            has_more,
        })
    }
    pub fn clear_query_cache(&self) {
        self.tag_catalog_revision
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn tag_editor_catalog(&self) -> Result<TagCatalog> {
        let mut cached = self.tag_catalog.lock().unwrap();
        self.check_cancellation()?;
        let revision = self
            .tag_catalog_revision
            .load(std::sync::atomic::Ordering::Relaxed);
        if let Some((time, cached_revision, values)) = &*cached
            && *cached_revision == revision
            && time.elapsed() < Duration::from_secs(3600)
        {
            return Ok(values.clone());
        }
        let mut catalog = crate::tags::Fields::new();
        match self.server.kind {
            ServerType::Navidrome if self.features().artist_roles => {
                let (value, _) = self.request(Method::GET, "api/tag", &[], None)?;
                for tag in array(if value.is_array() {
                    &value
                } else {
                    &value["data"]
                }) {
                    if let (Some(name), Some(value)) =
                        (tag["tagName"].as_str(), tag["tagValue"].as_str())
                        && name != "genre"
                    {
                        catalog.entry(name.into()).or_default().push(value.into());
                    }
                }
            }
            ServerType::Jellyfin => {
                let params = vec![p("IncludeItemTypes", self.jf_song_types())];
                let mut filters_params = params.clone();
                filters_params.push(p("UserId", self.session().user_id));
                let (filters, _) =
                    self.request(Method::GET, "items/filters", &filters_params, None)?;
                let mut studios_params = params;
                studios_params.push(p("EnableTotalRecordCount", true));
                let (studios, _) = self.request(Method::GET, "studios", &studios_params, None)?;
                let tags = array(&filters["Tags"])
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if !tags.is_empty() {
                    catalog.insert("Tags".into(), tags);
                }
                let studios = array(&studios["Items"])
                    .iter()
                    .filter_map(|item| item["Name"].as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if !studios.is_empty() {
                    catalog.insert("Studios".into(), studios);
                }
            }
            _ => (),
        }
        for (name, values) in &mut catalog {
            let numeric = self.server.kind != ServerType::Jellyfin || name != "Studios";
            values.sort_by(|a, b| crate::i18n::compare_lowercase(a, b, numeric));
        }
        let catalog = std::sync::Arc::new(catalog);
        *cached = Some((std::time::Instant::now(), revision, catalog.clone()));
        Ok(catalog)
    }

    pub fn tag_editor_sources(&self) -> Result<Vec<String>> {
        let mut names = self
            .tag_editor_catalog()?
            .keys()
            .filter(|name| {
                self.server.kind != ServerType::Navidrome
                    || !["disctotal", "tracktotal"].contains(&name.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        names.sort_by(|a, b| {
            crate::i18n::compare(
                &crate::tags::source_label(a),
                &crate::tags::source_label(b),
                false,
            )
        });
        Ok(names)
    }

    pub fn tag_suggestions(&self, source: &str, search: &str) -> Result<Vec<String>> {
        if let Some(name) = source.strip_prefix("tag:") {
            let search = search.to_lowercase();
            return Ok(self
                .tag_editor_catalog()?
                .get(name)
                .into_iter()
                .flatten()
                .filter(|value| !value.is_empty() && value.to_lowercase().contains(&search))
                .take(25)
                .cloned()
                .collect());
        }
        let kind = match source {
            "serverArtists" => ItemKind::AlbumArtist,
            "serverGenres" => ItemKind::Genre,
            _ => return Ok(Vec::new()),
        };
        Ok(self
            .list(&ListQuery {
                kind,
                search: search.into(),
                limit: 25,
                ..Default::default()
            })?
            .items
            .into_iter()
            .map(|item| item.name)
            .filter(|name| !name.is_empty())
            .collect())
    }

    pub fn refresh_items(&self, ids: &[String]) -> Result<()> {
        ensure!(
            self.session().is_admin,
            "Only server administrators can refresh metadata"
        );
        ensure!(
            !ids.is_empty()
                && ids.len() <= 10000
                && ids
                    .iter()
                    .all(|id| !id.is_empty() && !id.contains(['/', '?', '#']) && id != ".."),
            "Invalid refresh selection"
        );
        if self.server.kind == ServerType::Jellyfin {
            for id in ids {
                self.request(
                    Method::POST,
                    &format!("Items/{id}/Refresh"),
                    &[p("MetadataRefreshMode", "FullRefresh")],
                    None,
                )?;
            }
        } else {
            self.subsonic("startScan", &[])?;
        }
        Ok(())
    }

    pub fn server_lyrics(&self, item: &Item) -> Result<Vec<Line>> {
        let variants = self.server_lyric_variants(item)?;
        let selected = variants
            .iter()
            .find(|variant| variant.synchronized && variant.kind == "main")
            .or_else(|| variants.first());
        Ok(selected
            .map(|variant| variant.lines.clone())
            .unwrap_or_default())
    }
    pub fn server_lyric_variants(&self, item: &Item) -> Result<Vec<lyrics::Variant>> {
        ensure!(
            item.server_id == self.server.id,
            "Lyrics belong to a different server"
        );
        if self.server.kind == ServerType::Jellyfin {
            if let Ok((v, _)) =
                self.request(Method::GET, &format!("Audio/{}/Lyrics", item.id), &[], None)
            {
                let lines: Vec<_> = array(&v["Lyrics"])
                    .iter()
                    .map(|v| Line {
                        time: v["Start"].as_f64().map(|n| n / 10_000_000.0),
                        text: s(v, "Text"),
                        words: vec![],
                        cue_lines: vec![],
                    })
                    .collect();
                if !lines.is_empty() {
                    return Ok(vec![lyrics::Variant {
                        language: "und".into(),
                        kind: "main".into(),
                        synchronized: lines.iter().any(|line| line.time.is_some()),
                        offset: None,
                        agents: vec![],
                        lines,
                    }]);
                }
            }
        } else {
            if let Ok(v) = self.subsonic("getLyricsBySongId", &[p("id", &item.id)]) {
                let variants: Vec<_> = array(&v["lyricsList"]["structuredLyrics"])
                    .iter()
                    .map(lyrics::structured_variant)
                    .filter(|variant| !variant.lines.is_empty())
                    .collect();
                if !variants.is_empty() {
                    return Ok(variants);
                }
            }

            if let Ok(v) = self.subsonic(
                "getLyrics",
                &[p("artist", item.primary_artist()), p("title", &item.name)],
            ) {
                let lines = lyrics::display_lines(&s(&v["lyrics"], "value"));
                if !lines.is_empty() {
                    return Ok(vec![lyrics::Variant {
                        language: "und".into(),
                        kind: "main".into(),
                        synchronized: lines.iter().any(|line| line.time.is_some()),
                        offset: None,
                        agents: vec![],
                        lines,
                    }]);
                }
            }
        }
        Ok(vec![])
    }

    pub fn scrobble(
        &self,
        item: &Item,
        event: &str,
        position: f64,
        speed: f32,
        queue: &[String],
    ) -> Result<()> {
        ensure!(
            item.server_id == self.server.id,
            "Playback report belongs to a different server"
        );
        if item.kind != ItemKind::Song {
            return Ok(());
        }
        if self.server.kind == ServerType::Jellyfin {
            let path = match event {
                "start" => "Sessions/Playing",
                "stop" | "submit" => "Sessions/Playing/Stopped",
                _ => "Sessions/Playing/Progress",
            };
            let mut body = json!({"ItemId":item.id,"PositionTicks":(position.max(0.0)*10_000_000.0).round() as u64});
            if matches!(event, "pause" | "unpause" | "submit") {
                body["IsPaused"] = json!(event != "unpause");
            }
            if !matches!(event, "start" | "stop" | "submit") {
                body["EventName"] = json!(event);
            }
            if !matches!(event, "pause" | "stop" | "submit" | "timeupdate") && queue.len() > 1 {
                body["NowPlayingQueue"] = json!(
                    queue
                        .iter()
                        .take(20)
                        .map(|id| json!({"Id":id}))
                        .collect::<Vec<_>>()
                );
            }
            self.request(Method::POST, path, &[], Some(body))?;
        } else if event == "start" || event == "submit" {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis();
            self.subsonic(
                "scrobble",
                &[
                    p("id", &item.id),
                    p("submission", event == "submit"),
                    p("time", time),
                ],
            )?;
        }
        if self.server.kind != ServerType::Jellyfin
            && event != "submit"
            && self.features().playback_report
        {
            let state = match event {
                "start" => "starting",
                "pause" | "stop" => {
                    if event == "pause" {
                        "paused"
                    } else {
                        "stopped"
                    }
                }
                _ => "playing",
            };
            let mut params = vec![
                p("ignoreScrobble", true),
                p("mediaId", &item.id),
                p("mediaType", "song"),
                p("playbackRate", speed),
                p("positionMs", (position.max(0.0) * 1000.0).round() as u64),
                p("state", state),
            ];
            self.subsonic("reportPlayback", &params)?;
            if event == "start" {
                params.pop();
                params.push(p("state", "playing"));
                self.subsonic("reportPlayback", &params)?;
            }
        }
        Ok(())
    }

    pub fn save_queue(&self, songs: &[Item], current: Option<usize>, position: f64) -> Result<()> {
        ensure!(
            self.server.kind != ServerType::Jellyfin,
            "Jellyfin has no saved server queue"
        );
        ensure!(
            songs
                .iter()
                .all(|song| song.server_id == self.server.id && song.kind == ItemKind::Song),
            "Saved queue tracks must belong to this server"
        );
        ensure!(
            current.is_none_or(|index| index < songs.len()),
            "Invalid current queue index"
        );
        ensure!(
            position.is_finite() && position >= 0.0,
            "Invalid queue position"
        );
        let features = self.features();
        let position = (position * 1000.0).round() as u64;
        if features.native_queue && !features.index_queue {
            let mut body = json!({"ids":songs.iter().map(|song|&song.id).collect::<Vec<_>>(),"position":position});
            if let Some(current) = current {
                body["current"] = current.into();
            }
            self.request(Method::POST, "api/queue", &[], Some(body))?;
        } else {
            let mut params = songs
                .iter()
                .map(|song| p("id", &song.id))
                .collect::<Vec<_>>();
            params.push(p("position", position));
            if let Some(index) = current {
                params.push(if features.index_queue {
                    p("currentIndex", index)
                } else {
                    p("current", &songs[index].id)
                });
            }
            self.subsonic(
                if features.index_queue {
                    "savePlayQueueByIndex"
                } else {
                    "savePlayQueue"
                },
                &params,
            )?;
        }
        Ok(())
    }
    pub fn load_queue(&self) -> Result<(Vec<Item>, Option<usize>, f64)> {
        ensure!(
            self.server.kind != ServerType::Jellyfin,
            "Jellyfin has no saved server queue"
        );
        let features = self.features();
        if features.native_queue {
            let (value, _) = self.request(Method::GET, "api/queue", &[], None)?;
            let songs = array(&value["items"])
                .iter()
                .map(|item| self.normalize(item, ItemKind::Song))
                .collect::<Vec<_>>();
            let current = value["current"]
                .as_u64()
                .map(|index| index as usize)
                .filter(|index| *index < songs.len());
            return Ok((songs, current, n(&value, "position") / 1000.0));
        }
        let value = self.subsonic(
            if features.index_queue {
                "getPlayQueueByIndex"
            } else {
                "getPlayQueue"
            },
            &[],
        )?;
        let value = &value[if features.index_queue {
            "playQueueByIndex"
        } else {
            "playQueue"
        }];
        let songs = array(&value["entry"])
            .iter()
            .map(|item| self.normalize(item, ItemKind::Song))
            .collect::<Vec<_>>();
        let current = if features.index_queue {
            value["currentIndex"].as_u64().map(|index| index as usize)
        } else {
            songs
                .iter()
                .position(|song| Some(song.id.as_str()) == value["current"].as_str())
        }
        .filter(|index| *index < songs.len());
        Ok((songs, current, n(value, "position") / 1000.0))
    }

    pub fn create_radio(&self, name: &str, stream_url: &str, homepage: &str) -> Result<()> {
        ensure!(
            self.server.kind != ServerType::Jellyfin,
            "Jellyfin radio stations are stored locally"
        );
        let stream = Url::parse(stream_url.trim()).context("Invalid radio stream URL")?;
        ensure!(
            matches!(stream.scheme(), "https" | "http") && stream.host_str().is_some(),
            "Radio stream must use HTTP or HTTPS"
        );
        ensure!(!name.trim().is_empty(), "Enter a station name");
        self.subsonic(
            "createInternetRadioStation",
            &[
                p("name", name),
                p("streamUrl", stream_url),
                p("homepageUrl", homepage),
            ],
        )?;
        Ok(())
    }

    pub fn start_scan(&self) -> Result<()> {
        if self.server.kind == ServerType::Jellyfin {
            let ids = if self.server.options.music_folder_ids.is_empty() {
                self.all_items(ListQuery {
                    kind: ItemKind::Folder,
                    ..Default::default()
                })?
                .into_iter()
                .map(|folder| folder.id)
                .collect::<Vec<_>>()
            } else {
                self.server.options.music_folder_ids.clone()
            };
            ensure!(!ids.is_empty(), "No music libraries found");
            ensure!(
                ids.iter()
                    .all(|id| !id.is_empty() && !id.contains(['/', '?', '#']) && id != ".."),
                "Invalid music library ID"
            );
            for id in ids {
                self.request(
                    Method::POST,
                    &format!("Items/{id}/Refresh"),
                    &[
                        p("ImageRefreshMode", "Default"),
                        p("MetadataRefreshMode", "Default"),
                        p("Recursive", true),
                        p("RegenerateTrickplay", false),
                        p("ReplaceAllImages", false),
                        p("ReplaceAllMetadata", false),
                    ],
                    None,
                )?;
            }
        } else {
            self.subsonic("startScan", &[])?;
        }
        Ok(())
    }

    pub fn scan_status(&self) -> Result<ScanStatus> {
        if self.server.kind == ServerType::Jellyfin {
            let (tasks, _) = self.request(Method::GET, "ScheduledTasks", &[], None)?;
            let tasks = array(&tasks);
            let task = tasks
                .iter()
                .find(|task| task["Key"] == "RefreshLibrary")
                .or_else(|| {
                    tasks
                        .iter()
                        .find(|task| task["Name"] == "Scan Media Library")
                });
            return Ok(task
                .map(|task| ScanStatus {
                    scanning: matches!(task["State"].as_str(), Some("Running" | "Cancelling")),
                    last_scan: task["LastExecutionResult"]["EndTimeUtc"]
                        .as_str()
                        .map(str::to_owned),
                    ..Default::default()
                })
                .unwrap_or_default());
        }
        let response = self.subsonic("getScanStatus", &[])?;
        let value = &response["scanStatus"];
        let last_scan = s(value, "lastScan");
        Ok(ScanStatus {
            scanning: value["scanning"].as_bool().unwrap_or(false),
            count: value["count"].as_u64().unwrap_or(0),
            folder_count: value["folderCount"].as_u64().unwrap_or(0),
            last_scan: (!last_scan.is_empty()).then_some(last_scan),
        })
    }

    pub fn download(&self, item: &Item, path: &Path) -> Result<()> {
        if !item.playlist_songs.is_empty() {
            return self.download_playlist_album(item, path);
        }
        let mut response = self.download_response(item)?;
        // Stream to a temporary file, then rename, so failed downloads cannot replace a file.
        let temp = path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            std::io::copy(&mut response, &mut file)?;
            file.sync_all()?;
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }

    fn download_playlist_album(&self, item: &Item, path: &Path) -> Result<()> {
        let songs = self.all_tracks(item)?;
        let temp = path.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        let result = (|| {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let mut archive = zip::ZipWriter::new(file);
            for (index, song) in songs.iter().enumerate() {
                let name = song
                    .name
                    .chars()
                    .map(|ch| {
                        if ch.is_control() || "/\\:*?\"<>|".contains(ch) {
                            '_'
                        } else {
                            ch
                        }
                    })
                    .take(100)
                    .collect::<String>();
                let extension = if !song.container.is_empty()
                    && song.container.len() <= 16
                    && song
                        .container
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric())
                {
                    format!(".{}", song.container)
                } else {
                    String::new()
                };
                archive.start_file(
                    format!("{:04}-{name}{extension}", index + 1),
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored)
                        .large_file(true),
                )?;
                let mut response = self.download_response(song)?;
                std::io::copy(&mut response, &mut archive)?;
            }
            archive.finish()?.sync_all()?;
            std::fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temp);
        }
        result
    }

    fn download_response(&self, item: &Item) -> Result<reqwest::blocking::Response> {
        ensure!(
            item.server_id == self.server.id,
            "Download belongs to a different server"
        );
        let url = if self.server.kind == ServerType::Jellyfin {
            self.url(
                &format!("Items/{}/Download", item.id),
                &[p("apiKey", &self.session().token)],
            )?
        } else {
            let mut params = self.ss_params();
            params.push(p("id", &item.id));
            self.url("rest/download.view", &params)?
        };
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|e| e.without_url())?
            .error_for_status()
            .map_err(|e| e.without_url())?;
        Ok(response)
    }
}

#[derive(Clone, Copy)]
pub enum PlaylistSelectionAction {
    Remove,
    MoveBefore(Option<usize>),
}

pub(crate) fn playlist_moves(
    len: usize,
    selected: &std::collections::BTreeSet<usize>,
    before: Option<usize>,
) -> Result<Vec<(usize, usize)>> {
    ensure!(
        selected.iter().all(|index| *index < len) && before.is_none_or(|index| index <= len),
        "Invalid playlist position"
    );
    let anchor = before.and_then(|before| (before..len).find(|index| !selected.contains(index)));
    let mut order = (0..len).collect::<Vec<_>>();
    let mut moves = Vec::new();
    for key in selected {
        let from = order.iter().position(|index| index == key).unwrap();
        let to = anchor.map_or(len - 1, |anchor| {
            let index = order.iter().position(|key| *key == anchor).unwrap();
            index - usize::from(from < index)
        });
        if from != to {
            let key = order.remove(from);
            order.insert(to, key);
            moves.push((from, to));
        }
    }
    Ok(moves)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn random_play_requests_match_original_controller_references() {
        let fixtures: Vec<Value> =
            serde_json::from_str(include_str!("../assets/random-play-query-reference.json"))
                .unwrap();
        assert_eq!(fixtures.len(), 66);
        for fixture in fixtures {
            let kind = serde_json::from_value(fixture["backend"].clone()).unwrap();
            let response = if kind == ServerType::Jellyfin {
                r#"{"Items":[]}"#
            } else {
                r#"{"subsonic-response":{"status":"ok","randomSongs":{"song":[]}}}"#
            };
            let (mut api, worker) = mock(kind, response);
            api.server.options.enable_audiobooks = fixture["audiobooks"].as_bool().unwrap();
            api.server.options.music_folder_ids =
                serde_json::from_value(fixture["music_folders"].clone()).unwrap();
            let config = serde_json::from_value(fixture["config"].clone()).unwrap();
            assert!(api.random_library_items(&config).unwrap().is_empty());
            let request = worker.join().unwrap();
            let url = Url::parse(&format!(
                "http://fixture{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let params = url
                .query_pairs()
                .into_owned()
                .collect::<std::collections::BTreeMap<_, _>>();
            for (key, expected) in fixture["expected"].as_object().unwrap() {
                let actual = params
                    .get(key)
                    .unwrap_or_else(|| panic!("Missing {key}: {fixture}"));
                if key == "Fields" {
                    for field in expected.as_array().unwrap() {
                        assert!(
                            actual
                                .split(',')
                                .any(|value| value == field.as_str().unwrap()),
                            "{key}={actual}: {fixture}"
                        );
                    }
                } else {
                    let value = expected
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| expected.to_string());
                    assert_eq!(*actual, value, "{key}: {fixture}");
                }
            }
            for key in [
                "Years",
                "IsPlayed",
                "fromYear",
                "toYear",
                "GenreIds",
                "ParentId",
                "genre",
                "musicFolderId",
            ] {
                assert_eq!(
                    params.contains_key(key),
                    fixture["expected"].get(key).is_some(),
                    "{key}: {fixture}"
                );
            }
        }
    }
    #[test]
    fn original_random_play_queries_preserve_counts_filters_and_duplicate_tracks() {
        use crate::{
            auto_dj::Mode,
            shuffle::{Config, Played},
        };
        for kind in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            for played in [Played::All, Played::Never, Played::Played] {
                for (enable_min_year, enable_max_year) in
                    [(false, false), (true, false), (false, true), (true, true)]
                {
                    let response = if kind == ServerType::Jellyfin {
                        r#"{"Items":[{"Id":"same","Name":"First","Type":"Audio"},{"Id":"same","Name":"Second","Type":"Audio"},{"Id":"extra","Type":"Audio"}]}"#
                    } else {
                        r#"{"subsonic-response":{"status":"ok","randomSongs":{"song":[{"id":"same","title":"First"},{"id":"same","title":"Second"},{"id":"extra"}]}}}"#
                    };
                    let (api, worker) = mock(kind, response);
                    let config = Config {
                        limit: 2,
                        genre: "Jazz & soul".into(),
                        music_folder: "library-id".into(),
                        enable_min_year,
                        enable_max_year,
                        min_year: 2000,
                        max_year: 2002,
                        played,
                        ..Default::default()
                    };
                    let tracks = api.random_library_tracks(&config, false).unwrap();
                    assert_eq!(
                        tracks
                            .iter()
                            .map(|track| (track.id.as_str(), track.name.as_str()))
                            .collect::<Vec<_>>(),
                        [("same", "First"), ("same", "Second")]
                    );
                    assert!(tracks.iter().all(|track| track.server_id == api.server.id));
                    let request = worker.join().unwrap();
                    let url = Url::parse(&format!(
                        "http://fixture{}",
                        request.split_whitespace().nth(1).unwrap()
                    ))
                    .unwrap();
                    let params = url
                        .query_pairs()
                        .into_owned()
                        .collect::<std::collections::BTreeMap<_, _>>();
                    if kind == ServerType::Jellyfin {
                        assert_eq!(url.path(), "/music/Users/uid/Items");
                        for (key, value) in [
                            ("SortBy", "Random,SortName"),
                            ("SortOrder", "Ascending"),
                            ("Limit", "2"),
                            ("GenreIds", "Jazz & soul"),
                            ("ParentId", "library-id"),
                            ("Recursive", "true"),
                        ] {
                            assert_eq!(
                                params.get(key).map(String::as_str),
                                Some(value),
                                "{params:?}"
                            );
                        }
                        assert_eq!(
                            params.get("Years").map(String::as_str),
                            (enable_min_year && enable_max_year).then_some("2000,2001,2002")
                        );
                        assert_eq!(
                            params.get("IsPlayed").map(String::as_str),
                            match played {
                                Played::All => None,
                                Played::Never => Some("false"),
                                Played::Played => Some("true"),
                            }
                        );
                    } else {
                        assert_eq!(url.path(), "/music/rest/getRandomSongs.view");
                        for (key, value) in [
                            ("size", "2"),
                            ("genre", "Jazz & soul"),
                            ("musicFolderId", "library-id"),
                        ] {
                            assert_eq!(params.get(key).map(String::as_str), Some(value));
                        }
                        assert_eq!(
                            params.get("fromYear").map(String::as_str),
                            enable_min_year.then_some("2000")
                        );
                        assert_eq!(
                            params.get("toYear").map(String::as_str),
                            enable_max_year.then_some("2002")
                        );
                        assert!(!params.contains_key("IsPlayed"));
                    }
                }
            }
        }
        for kind in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            let response = match kind {
                ServerType::Navidrome => r#"[{"id":"album","name":"Album"}]"#,
                ServerType::Subsonic => {
                    r#"{"subsonic-response":{"status":"ok","albumList2":{"album":[{"id":"album","name":"Album"}]}}}"#
                }
                ServerType::Jellyfin => {
                    r#"{"Items":[{"Id":"album","Name":"Album","Type":"MusicAlbum"}]}"#
                }
            };
            let (api, worker) = mock(kind, response);
            let config = Config {
                kind: Mode::Albums,
                limit: 500,
                enable_min_year: true,
                min_year: 1991,
                enable_max_year: true,
                max_year: 1980,
                ..Default::default()
            };
            let items = api.random_library_items(&config).unwrap();
            assert_eq!(items[0].kind, ItemKind::Album);
            let request = worker.join().unwrap();
            let url = Url::parse(&format!(
                "http://fixture{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            let params = url
                .query_pairs()
                .into_owned()
                .collect::<std::collections::BTreeMap<_, _>>();
            match kind {
                ServerType::Navidrome => {
                    assert!(url.path().ends_with("/api/album"));
                    assert_eq!(params["_end"], "500");
                    assert_eq!(params["year"], "1991");
                }
                ServerType::Subsonic => {
                    assert!(url.path().ends_with("/getAlbumList2.view"));
                    assert_eq!(params["type"], "byYear");
                    assert_eq!(params["fromYear"], "1991");
                    assert!(!params.values().any(|value| value == "1980"));
                }
                ServerType::Jellyfin => {
                    assert_eq!(params["Limit"], "500");
                    assert_eq!(params["IncludeItemTypes"], "MusicAlbum");
                    assert!(!params.contains_key("Years") && !params.contains_key("IsPlayed"));
                }
            }
        }
        let api = Api::new(server(ServerType::Subsonic), Session::default()).unwrap();
        assert!(
            api.random_library_items(&Config {
                limit: 501,
                ..Default::default()
            })
            .is_err()
        );
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        assert!(
            api.with_cancellation(cancelled)
                .random_library_items(&Config::default())
                .is_err()
        );
    }
    #[test]
    fn multiple_artist_names_preserve_primary_link_identities_for_every_backend() {
        for (kind, payload) in [
            (
                ServerType::Jellyfin,
                json!({"Id":"song","Artists":["First", "Second"],"AlbumArtist":"Album first, Album second","AlbumArtists":[{"Name":"Album first"},{"Name":"Album second"}]}),
            ),
            (
                ServerType::Navidrome,
                json!({"id":"song","artist":"First, Second","albumArtist":"Album first, Album second","participants":{"artist":[{"name":"First"},{"name":"Second"}],"albumartist":[{"name":"Album first"},{"name":"Album second"}]}}),
            ),
            (
                ServerType::Subsonic,
                json!({"id":"song","artist":"First, Second","albumArtist":"Album first, Album second","artists":[{"name":"First"},{"name":"Second"}],"albumArtists":[{"name":"Album first"},{"name":"Album second"}]}),
            ),
        ] {
            let api = Api::new(server(kind), Session::default()).unwrap();
            let item = api.normalize(&payload, ItemKind::Song);
            assert_eq!(item.primary_artist(), "First");
            assert_eq!(item.primary_album_artist(), "Album first");
            assert_eq!(item.artist, "First, Second");
            let config = crate::presence::Config {
                link_type: "last_fm".into(),
                ..Default::default()
            };
            let presence =
                crate::presence::activity(Some(&item), true, 0., false, 1, &config, None);
            assert_eq!(presence["state_url"], "https://www.last.fm/music/First");
            assert_eq!(
                presence["details_url"],
                "https://www.last.fm/music/Album%20first/_/"
            );
        }
        let legacy = Item {
            artist: "A / B".into(),
            ..Default::default()
        };
        assert_eq!(legacy.primary_album_artist(), "A / B");
    }
    #[test]
    fn cancelled_library_fetch_rejects_network_work_without_affecting_the_session() {
        let api = Api::new(
            Server {
                url: "http://127.0.0.1:1".into(),
                id: uuid::Uuid::new_v4(),
                name: "Fixture".into(),
                username: "fixture".into(),
                kind: ServerType::Navidrome,
                legacy_auth: false,
                options: Default::default(),
            },
            Session::default(),
        )
        .unwrap();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let cancelled_api = api.clone().with_cancellation(cancelled.clone());
        assert!(
            cancelled_api
                .all_items(ListQuery::default())
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        assert!(api.check_cancellation().is_ok());
        cancelled.store(false, std::sync::atomic::Ordering::Relaxed);
        assert!(cancelled_api.check_cancellation().is_ok());
    }

    #[test]
    fn library_scans_preserve_selected_music_libraries_and_report_backend_status() {
        for kind in [ServerType::Navidrome, ServerType::Subsonic] {
            let (server, worker) = protocol_server(
                kind,
                vec![
                    (json!({"subsonic-response":{"status":"ok"}}), None),
                    (
                        json!({"subsonic-response":{"status":"ok","scanStatus":{"scanning":true,"count":31,"folderCount":7,"lastScan":"2026-10-10T00:00:00Z"}}}),
                        None,
                    ),
                ],
            );
            let api = Api::new(server, Session::default()).unwrap();
            api.start_scan().unwrap();
            assert_eq!(
                api.scan_status().unwrap(),
                ScanStatus {
                    scanning: true,
                    count: 31,
                    folder_count: 7,
                    last_scan: Some("2026-10-10T00:00:00Z".into())
                }
            );
            let requests = worker.join().unwrap();
            assert!(requests[0].starts_with("GET /music/rest/startScan.view?"));
            assert!(requests[1].starts_with("GET /music/rest/getScanStatus.view?"));
        }
        let (mut server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (Value::Null, None),
                (Value::Null, None),
                (
                    json!([{"Name":"Other","State":"Running"},{"Key":"RefreshLibrary","Name":"Localized scan","State":"Cancelling","LastExecutionResult":{"EndTimeUtc":"finished"}}]),
                    None,
                ),
                (json!([{"Name":"Scan Media Library","State":"Idle"}]), None),
            ],
        );
        server.options.music_folder_ids = vec!["one".into(), "two".into()];
        let api = Api::new(server, Session::default()).unwrap();
        api.start_scan().unwrap();
        assert_eq!(
            api.scan_status().unwrap(),
            ScanStatus {
                scanning: true,
                last_scan: Some("finished".into()),
                ..Default::default()
            }
        );
        assert_eq!(api.scan_status().unwrap(), ScanStatus::default());
        let requests = worker.join().unwrap();
        for (request, id) in requests.iter().zip(["one", "two"]) {
            assert!(request.starts_with(&format!("POST /music/Items/{id}/Refresh?")));
            assert!(
                request.contains("Recursive=true")
                    && request.contains("ReplaceAllMetadata=false")
                    && request.contains("MetadataRefreshMode=Default")
            );
        }
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (
                    json!({"Items":[{"Id":"music","CollectionType":"music"},{"Id":"movies","CollectionType":"movies"},{"Id":"books","CollectionType":"books"}]}),
                    None,
                ),
                (Value::Null, None),
            ],
        );
        Api::new(server, Session::default())
            .unwrap()
            .start_scan()
            .unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("GET /music/Users//Views?"));
        assert!(requests[1].starts_with("POST /music/Items/music/Refresh?"));
    }

    #[test]
    fn jellyfin_name_lists_do_not_filter_their_source_items_to_name_types() {
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![(json!({"Items":[],"TotalRecordCount":0}), None); 4],
        );
        let api = Api::new(server, Session::default()).unwrap();
        for kind in [
            ItemKind::Artist,
            ItemKind::AlbumArtist,
            ItemKind::Genre,
            ItemKind::Song,
        ] {
            api.list(&ListQuery {
                kind,
                ..Default::default()
            })
            .unwrap();
        }
        let requests = worker.join().unwrap();
        for (request, path) in
            requests
                .iter()
                .zip(["Artists", "Artists/AlbumArtists", "MusicGenres"])
        {
            assert!(request.starts_with(&format!("GET /music/{path}?")));
            assert!(!request.contains("IncludeItemTypes="), "{request}");
        }
        assert!(requests[3].contains("IncludeItemTypes=Audio"));
    }

    #[test]
    fn custom_filter_values_page_beyond_one_hundred_and_stop_at_empty_results() {
        let values = (0..100)
            .map(|index| {
                json!({
                    "id": format!("mood-{index}"), "tagValue": format!("Mood {index}")
                })
            })
            .collect::<Vec<_>>();
        let (mut server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (json!(values), Some(101)),
                (
                    json!([{"id":"mood-100","tagValue":"Final mood"}]),
                    Some(101),
                ),
                (json!([]), Some(999)),
            ],
        );
        server.options.music_folder_ids = vec!["library-two".into()];
        let api = Api::new(server, Session::default()).unwrap();
        let first = api.tag_filter_values("mood", "", 0, 100).unwrap();
        assert_eq!(first.items.len(), 100);
        assert!(first.has_more);
        let last = api
            .tag_filter_values("mood", "Final mood", 100, 100)
            .unwrap();
        assert_eq!(last.items[0].id, "mood-100");
        assert_eq!(last.items[0].name, "Final mood");
        assert_eq!(last.total, Some(101));
        assert!(!last.has_more);
        let empty = api.tag_filter_values("mood", "", 101, 100).unwrap();
        assert!(empty.items.is_empty());
        assert!(
            !empty.has_more,
            "An incorrect total must not cause endless loading"
        );
        assert!(api.tag_filter_values("mood", "", 0, 501).is_err());
        let requests = worker.join().unwrap();
        for (request, (start, end)) in requests.iter().zip([(0, 100), (100, 200), (101, 201)]) {
            assert!(request.starts_with("GET /music/api/tag?"));
            assert!(request.contains(&format!("_start={start}")), "{request}");
            assert!(request.contains(&format!("_end={end}")), "{request}");
            assert!(request.contains("library_id=library-two"), "{request}");
        }
        assert!(requests[1].contains("tag_value=Final+mood"));
    }

    #[test]
    fn jellyfin_custom_filter_catalog_and_tag_studio_parameters() {
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (json!({"Tags":["Live","Calm","Live"]}), None),
                (
                    json!({"Items":[{"Name":"Studio one"},{"Name":"Studio two"}]}),
                    None,
                ),
                (json!({"Items":[],"TotalRecordCount":0}), None),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let catalog = api.filter_catalog(ItemKind::Song).unwrap();
        assert_eq!(catalog["Tags"], ["Calm", "Live"]);
        assert_eq!(catalog["Studios"], ["Studio one", "Studio two"]);
        api.list(&ListQuery {
            kind: ItemKind::Song,
            filters: crate::filters::Filters {
                custom: std::collections::BTreeMap::from([
                    ("Tags".into(), json!(["Live", "Calm"])),
                    ("Studios".into(), json!(["Studio one"])),
                ]),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("GET /music/items/filters?"));
        assert!(requests[0].contains("IncludeItemTypes=Audio"));
        assert!(requests[1].starts_with("GET /music/studios?"));
        assert!(requests[2].contains("Tags=Live%2CCalm"));
        assert!(requests[2].contains("Studios=Studio+one"));
    }

    #[test]
    fn metadata_sort_search_pages_tag_catalog_and_large_duplicate_playlists() {
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![(
                json!([
                    {"id":"small","title":"Track 2","size":2},
                    {"id":"large","title":"Track 10","size":100},
                    {"id":"middle","title":"Track 3","size":10}
                ]),
                Some(3),
            )],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                sort: Sort::Size,
                offset: 1,
                limit: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.items[0].id, "middle");
        assert_eq!(page.total, Some(3));
        assert!(worker.join().unwrap()[0].contains("_sort=title"));
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![(
                json!([
                    {"id":"unrated","title":"Unrated"}, {"id":"rated","title":"Rated","rating":3}
                ]),
                Some(2),
            )],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                filters: crate::filters::Filters {
                    has_rating: Some(false),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["unrated"]
        );
        assert!(!worker.join().unwrap()[0].contains("has_rating"));
        for kind in [ItemKind::Song, ItemKind::Album, ItemKind::AlbumArtist] {
            let (server, worker) = protocol_server(
                ServerType::Navidrome,
                vec![(
                    json!([
                        {"id":"first","name":"Match 1","title":"Match 1"},
                        {"id":"second","name":"Match 2","title":"Match 2"},
                        {"id":"other","name":"Unrelated","title":"Unrelated"}
                    ]),
                    Some(3),
                )],
            );
            let api = Api::new(server, Session::default()).unwrap();
            let page = api
                .search(
                    &ListQuery {
                        kind,
                        search: "needle".into(),
                        offset: 1,
                        limit: 1,
                        ..Default::default()
                    },
                    "Match",
                )
                .unwrap();
            assert_eq!(page.total, Some(2));
            assert_eq!(page.items[0].id, "second");
            assert_eq!(page.items[0].kind, kind);
            assert!(!page.has_more);
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), 1);
            let endpoint = match kind {
                ItemKind::Song => "song",
                ItemKind::Album => "album",
                _ => "artist",
            };
            assert!(
                requests[0].contains(&format!("/api/{endpoint}?")),
                "{}",
                requests[0]
            );
            assert!(requests[0].contains("needle"));
        }
        let (api, worker) = mock(ServerType::Navidrome, &json!([
            {"id":"a","tagName":"mood","tagValue":"Happy"}, {"id":"b","tagName":"genre","tagValue":"Rock"},
            {"id":"c","tagName":"mood","tagValue":"Calm"}, {"tagName":"bad&token","tagValue":"invalid"}
        ]).to_string());
        assert_eq!(api.tag_names().unwrap(), ["genre", "mood"]);
        worker.join().unwrap();
        let entries = vec![json!({"id":"same","title":"Duplicate"}); 1001];
        let response = json!({"subsonic-response":{"status":"ok","playlist":{"entry":entries}}});
        let (server, worker) = protocol_server(
            ServerType::Subsonic,
            vec![
                (response.clone(), None),
                (response.clone(), None),
                (response, None),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let playlist = Item {
            id: "playlist".into(),
            kind: ItemKind::Playlist,
            server_id: api.server.id,
            ..Default::default()
        };
        let tracks = api.all_tracks(&playlist).unwrap();
        assert_eq!(tracks.len(), 1001);
        assert_eq!(tracks[1000].playlist_position, Some(1000));
        worker.join().unwrap();
    }

    #[test]
    fn client_filter_search_and_playlist_edits_keep_absolute_positions() {
        for (kind, root, key) in [
            (ItemKind::Playlist, "playlists", "playlist"),
            (ItemKind::Genre, "genres", "genre"),
        ] {
            let (api, worker) = mock(
                ServerType::Subsonic,
                &json!({"subsonic-response":{"status":"ok", root:{key:[
                    {"id":"one","name":"Unrelated","value":"Unrelated"},
                    {"id":"two","name":"Rock first","value":"Rock first"},
                    {"id":"three","name":"Rock second","value":"Rock second"}
                ]}}})
                .to_string(),
            );
            let page = api
                .list(&ListQuery {
                    kind,
                    search: "Rock".into(),
                    offset: 1,
                    limit: 1,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(page.total, Some(2));
            assert_eq!(page.items[0].name, "Rock second");
            worker.join().unwrap();
        }
        let values = json!([
            {"id":"entry-one","mediaFileId":"same","title":"Duplicate","starred":false},
            {"id":"entry-two","mediaFileId":"other","title":"Other","starred":true},
            {"id":"entry-three","mediaFileId":"same","title":"Duplicate","starred":true}
        ]);
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (values.clone(), Some(3)),
                (values, Some(3)),
                (json!({}), None),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let playlist = Item {
            id: "playlist".into(),
            kind: ItemKind::Playlist,
            server_id: api.server.id,
            ..Default::default()
        };
        let all = api.all_tracks(&playlist).unwrap();
        assert_eq!(
            all.iter()
                .map(|item| item.playlist_position)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(2)]
        );
        let filtered = crate::filters::Filters {
            favorite: Some(true),
            ..Default::default()
        }
        .page(all, "Duplicate", 0, 100)
        .items;
        assert_eq!(filtered[0].playlist_position, Some(2));
        api.edit_playlist_selection(&playlist, &filtered, &[2], PlaylistSelectionAction::Remove)
            .unwrap();
        let requests = worker.join().unwrap();
        assert!(
            requests[2].starts_with("DELETE /music/api/playlist/playlist/tracks?id=entry-three ")
        );
    }
    #[test]
    fn genre_queries_preserve_server_filters_search_sort_and_paging() {
        for backend in [ServerType::Navidrome, ServerType::Jellyfin] {
            for kind in [ItemKind::Album, ItemKind::Song] {
                let body = if backend == ServerType::Jellyfin {
                    json!({"Items":[],"TotalRecordCount":0})
                } else {
                    json!([])
                };
                let (server, worker) = protocol_server(backend, vec![(body, Some(0)); 2]);
                let api = Api::new(server, Session::default()).unwrap();
                let genre = Item {
                    id: "rock".into(),
                    name: "Rock".into(),
                    kind: ItemKind::Genre,
                    server_id: api.server.id,
                    ..Default::default()
                };
                let query = api
                    .genre_query(
                        &genre,
                        ListQuery {
                            kind,
                            offset: 100,
                            limit: 50,
                            search: "Concert".into(),
                            sort: Sort::Name,
                            descending: true,
                            filters: crate::filters::Filters {
                                favorite: Some(false),
                                min_year: Some(2000),
                                max_year: Some(2000),
                                genre_ids: vec!["other".into()],
                                genre_all: true,
                                custom: std::collections::BTreeMap::from([(
                                    if backend == ServerType::Jellyfin {
                                        "Tags"
                                    } else {
                                        "mood"
                                    }
                                    .into(),
                                    json!(["calm"]),
                                )]),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(query.filters.genre_ids, ["rock"]);
                assert!(!query.filters.genre_all);
                api.list(&query).unwrap();
                api.all_items(query).unwrap();
                for (index, request) in worker.join().unwrap().iter().enumerate() {
                    let url = Url::parse(&format!(
                        "http://localhost{}",
                        request.split_whitespace().nth(1).unwrap()
                    ))
                    .unwrap();
                    let params = url
                        .query_pairs()
                        .collect::<std::collections::HashMap<_, _>>();
                    let expected = if backend == ServerType::Navidrome {
                        vec![
                            ("genre_id", "rock"),
                            ("mood", "calm"),
                            ("starred", "false"),
                            ("year", "2000"),
                            (
                                "_sort",
                                if kind == ItemKind::Album {
                                    "name"
                                } else {
                                    "title"
                                },
                            ),
                            ("_order", "DESC"),
                            (
                                if kind == ItemKind::Album {
                                    "name"
                                } else {
                                    "title"
                                },
                                "Concert",
                            ),
                            ("_start", if index == 0 { "100" } else { "0" }),
                        ]
                    } else {
                        vec![
                            ("GenreIds", "rock"),
                            ("Tags", "calm"),
                            ("IsFavorite", "false"),
                            ("Years", "2000"),
                            ("SearchTerm", "Concert"),
                            ("SortOrder", "Descending"),
                            ("StartIndex", if index == 0 { "100" } else { "0" }),
                        ]
                    };
                    for (key, value) in expected {
                        assert_eq!(
                            params.get(key).map(|v| v.as_ref()),
                            Some(value),
                            "{request}"
                        );
                    }
                }
                let mut foreign = genre.clone();
                foreign.server_id = uuid::Uuid::new_v4();
                assert!(api.genre_query(&foreign, ListQuery::default()).is_err());
                assert!(
                    api.genre_query(
                        &genre,
                        ListQuery {
                            kind: ItemKind::Artist,
                            ..Default::default()
                        }
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn filter_requests_preserve_false_values_multi_selections_year_ranges_and_tags() {
        let (server, worker) = protocol_server(ServerType::Navidrome, vec![(json!([]), Some(0))]);
        let api = Api::new(server, Session::default()).unwrap();
        api.features.write().unwrap().artists_search = true;
        api.features.write().unwrap().track_rating_filter = true;
        api.list(&ListQuery {
            kind: ItemKind::Song,
            filters: crate::filters::Filters {
                favorite: Some(false),
                has_rating: Some(true),
                min_year: Some(2000),
                max_year: Some(2000),
                artist_ids: vec!["first".into(), "second".into()],
                genre_ids: vec!["rock".into(), "pop".into()],
                library_ids: vec!["one".into(), "two".into()],
                custom: std::collections::BTreeMap::from([(
                    "mood".into(),
                    json!(["calm", "happy"]),
                )]),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let request = &worker.join().unwrap()[0];
        for parameter in [
            "starred=false",
            "has_rating=true",
            "year=2000",
            "artists_id=first&artists_id=second",
            "genre_id=rock&genre_id=pop",
            "library_id=one&library_id=two",
            "mood=calm&mood=happy",
        ] {
            assert!(request.contains(parameter), "{parameter}: {request}");
        }
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![(json!({"Items":[],"TotalRecordCount":0}), None)],
        );
        let api = Api::new(server, Session::default()).unwrap();
        api.list(&ListQuery {
            kind: ItemKind::Album,
            filters: crate::filters::Filters {
                favorite: Some(false),
                compilation: Some(true),
                min_year: Some(1999),
                max_year: Some(2001),
                artist_ids: vec!["first".into(), "second".into()],
                genre_ids: vec!["rock".into(), "pop".into()],
                library_ids: vec!["library".into()],
                custom: std::collections::BTreeMap::from([("HasLyrics".into(), true.into())]),
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let request = &worker.join().unwrap()[0];
        for parameter in [
            "IsFavorite=false",
            "ContributingArtistIds=first%2Csecond",
            "GenreIds=rock%2Cpop",
            "ParentId=library",
            "Years=1999%2C2000%2C2001",
            "HasLyrics=true",
        ] {
            assert!(request.contains(parameter), "{parameter}: {request}");
        }
        let response = json!({"subsonic-response":{"status":"ok","searchResult3":{"song":[{"id":"old","year":1980},{"id":"one","year":2000,"starred":"date","userRating":3},{"id":"two","year":2001,"starred":"date","userRating":3}]}}});
        let (server, worker) = protocol_server(ServerType::Subsonic, vec![(response, None)]);
        let api = Api::new(server, Session::default()).unwrap();
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                offset: 1,
                limit: 1,
                filters: crate::filters::Filters {
                    favorite: Some(true),
                    has_rating: Some(true),
                    min_year: Some(1990),
                    max_year: Some(2010),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, Some(2));
        assert_eq!(page.items[0].id, "two");
        assert!(worker.join().unwrap()[0].contains("songOffset=0"));
    }
    #[test]
    fn jellyfin_bulk_removal_uses_fifty_entry_batches() {
        let items = (0..51)
            .map(|index| json!({"Id":"same","PlaylistItemId":format!("e{index}")}))
            .collect::<Vec<_>>();
        let response = json!({"Items":items,"TotalRecordCount":51});
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (response.clone(), None),
                (response, None),
                (Value::Null, None),
                (Value::Null, None),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let playlist = Item {
            id: "list".into(),
            server_id: api.server.id,
            kind: ItemKind::Playlist,
            ..Default::default()
        };
        let expected = api.all_tracks(&playlist).unwrap();
        api.edit_playlist_selection(
            &playlist,
            &expected,
            &(0..51).collect::<Vec<_>>(),
            PlaylistSelectionAction::Remove,
        )
        .unwrap();
        let requests = worker.join().unwrap();
        let first = requests[2].lines().next().unwrap();
        let second = requests[3].lines().next().unwrap();
        assert!(first.contains("e49") && !first.contains("e50"));
        assert!(second.contains("EntryIds=e50"));
    }
    #[test]
    fn tag_catalogs_and_suggestions_match_original_backends() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/tag-suggestions-reference.json")).unwrap();
        let names = reference["ordering"][0]["input"].as_array().unwrap();
        for kind in [ServerType::Navidrome, ServerType::Jellyfin] {
            let expected = &reference[if kind == ServerType::Navidrome {
                "catalog"
            } else {
                "jellyfinCatalog"
            }];
            let replies = if kind == ServerType::Navidrome {
                vec![(reference["raw"].clone(), None)]
            } else {
                vec![
                    (json!({"Tags":names}), None),
                    (
                        json!({"Items":names.iter().map(|name|json!({"Name":name})).collect::<Vec<_>>()}),
                        None,
                    ),
                ]
            };
            let (server, worker) =
                protocol_server(kind, [replies.clone(), replies.clone(), replies].concat());
            let api = Api::new(
                server,
                Session {
                    user_id: "user".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            api.features.write().unwrap().artist_roles = true;
            let catalog = api.tag_editor_catalog().unwrap();
            let expected_fields: crate::tags::Fields = expected["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tag| {
                    (
                        tag["name"].as_str().unwrap().to_owned(),
                        tag["options"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|item| item["name"].as_str().unwrap().to_owned())
                            .collect(),
                    )
                })
                .collect();
            assert_eq!(&*catalog, &expected_fields, "{kind:?}");
            let source = if kind == ServerType::Navidrome {
                "mood"
            } else {
                "Tags"
            };
            for search in ["mood", "å", "٢"] {
                let expected = expected_fields[source]
                    .iter()
                    .filter(|value| value.to_lowercase().contains(search))
                    .take(25)
                    .cloned()
                    .collect::<Vec<_>>();
                assert_eq!(
                    api.tag_suggestions(&format!("tag:{source}"), search)
                        .unwrap(),
                    expected
                );
            }
            // Catalog reads and repeated searches share the same one-hour cache.
            let names = api.tag_editor_sources().unwrap();
            if kind == ServerType::Navidrome {
                assert_eq!(names, ["mood", "style"]);
            } else {
                assert_eq!(names, ["Studios", "Tags"]);
            }
            api.tag_catalog.lock().unwrap().as_mut().unwrap().0 -= Duration::from_secs(3601);
            assert_eq!(*api.tag_editor_catalog().unwrap(), expected_fields);
            api.clear_query_cache();
            assert_eq!(*api.tag_editor_catalog().unwrap(), expected_fields);
            let requests = worker.join().unwrap();
            assert_eq!(
                requests.len(),
                if kind == ServerType::Navidrome { 3 } else { 6 }
            );
            api.sign_out();
            assert!(api.tag_suggestions(&format!("tag:{source}"), "").is_err());
        }
    }

    #[test]
    fn tag_suggestions_are_bounded_and_metadata_refresh_matches_each_backend() {
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![(json!([{"tagName":"mood","tagValue":"Calm"}]), None)],
        );
        let api = Api::new(server, Session::default()).unwrap();
        api.features.write().unwrap().artist_roles = true;
        assert_eq!(api.tag_suggestions("tag:mood", "Ca").unwrap(), ["Calm"]);
        assert_eq!(api.tag_editor_sources().unwrap(), ["mood"]);
        assert_eq!(
            api.tag_suggestions("tag:mood", "zz").unwrap(),
            Vec::<String>::new()
        );
        let request = &worker.join().unwrap()[0];
        assert!(
            request.starts_with("GET /music/api/tag ")
                && !request.contains("tag_value=")
                && !request.contains("_end=")
        );
        for kind in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            let replies = if kind == ServerType::Jellyfin {
                vec![(Value::Null, None), (Value::Null, None)]
            } else {
                vec![(json!({"subsonic-response":{"status":"ok"}}), None)]
            };
            let (server, worker) = protocol_server(kind, replies);
            let api = Api::new(server, Session::default()).unwrap();
            assert!(api.refresh_items(&["one".into()]).is_err());
            api.session.write().unwrap().is_admin = true;
            assert!(api.refresh_items(&["../path".into()]).is_err());
            api.refresh_items(&["one".into(), "two".into()]).unwrap();
            let requests = worker.join().unwrap();
            if kind == ServerType::Jellyfin {
                assert!(
                    requests[0].starts_with(
                        "POST /music/Items/one/Refresh?MetadataRefreshMode=FullRefresh"
                    )
                );
                assert!(
                    requests[1].starts_with(
                        "POST /music/Items/two/Refresh?MetadataRefreshMode=FullRefresh"
                    )
                );
            } else {
                assert!(requests[0].starts_with("GET /music/rest/startScan.view?"));
            }
        }
    }
    #[test]
    fn server_options_choose_the_requested_url_libraries_audiobooks_and_instant_mix() {
        let (mut server, worker) =
            protocol_server(ServerType::Navidrome, vec![(json!([]), Some(0))]);
        let remote = server.url.clone();
        server.url = "http://127.0.0.1:1/local".into();
        server.options = ServerOptions {
            remote_url: remote.clone(),
            prefer_remote_url: true,
            music_folder_ids: vec!["one".into(), "two".into()],
            ..Default::default()
        };
        let api = Api::new(server.clone(), Session::default()).unwrap();
        assert_eq!(api.server.url, server.url);
        assert!(
            api.url("api/song", &[])
                .unwrap()
                .as_str()
                .starts_with(&remote)
        );
        api.list(&ListQuery {
            kind: ItemKind::Song,
            ..Default::default()
        })
        .unwrap();
        assert!(worker.join().unwrap()[0].contains("library_id=one&library_id=two"));
        let mut invalid = server.clone();
        invalid.options.remote_url = "https://user:password@example.com".into();
        assert!(Api::new(invalid, Session::default()).is_err());
        let mut invalid = server;
        invalid.options.music_folder_ids.push("../path".into());
        assert!(Api::new(invalid, Session::default()).is_err());

        let (mut server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (json!({"Items":[],"TotalRecordCount":0}), None),
                (
                    json!({"Items":[{"Id":"other","Name":"Other","Type":"AudioBook"}]}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"music","Name":"Music","CollectionType":"music"},{"Id":"books","Name":"Books","CollectionType":"books"},{"Id":"films","Name":"Films","CollectionType":"movies"}]}),
                    None,
                ),
            ],
        );
        server.options = ServerOptions {
            enable_audiobooks: true,
            prefer_instant_mix: true,
            music_folder_ids: vec!["books".into()],
            ..Default::default()
        };
        let api = Api::new(
            server,
            Session {
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        api.list(&ListQuery {
            kind: ItemKind::Song,
            ..Default::default()
        })
        .unwrap();
        let seed = Item {
            id: "seed".into(),
            server_id: api.server.id,
            ..Default::default()
        };
        assert_eq!(api.random_songs(5, Some(&seed)).unwrap()[0].id, "other");
        let folders = api
            .list(&ListQuery {
                kind: ItemKind::Folder,
                offset: 1,
                limit: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(folders.total, Some(2));
        assert_eq!(folders.items[0].id, "books");
        let requests = worker.join().unwrap();
        assert!(
            requests[0].contains("IncludeItemTypes=Audio%2CAudioBook")
                && requests[0].contains("ParentId=books")
        );
        assert!(requests[1].starts_with("GET /music/Items/seed/InstantMix"));
        assert!(!requests[2].contains("StartIndex") && !requests[2].contains("ParentId="));

        let (mut server, worker) = protocol_server(
            ServerType::Subsonic,
            vec![(
                json!({"subsonic-response":{"status":"ok","searchResult3":{"song":[]}}}),
                None,
            )],
        );
        server.options.music_folder_ids = vec!["selected".into()];
        Api::new(server, Session::default())
            .unwrap()
            .list(&ListQuery {
                kind: ItemKind::Song,
                ..Default::default()
            })
            .unwrap();
        assert!(worker.join().unwrap()[0].contains("musicFolderId=selected"));
    }

    #[test]
    fn jellyfin_ambiguous_entry_ids_replace_only_the_selected_duplicate_occurrences() {
        let response = json!({"Items":[
            {"Id":"same","PlaylistItemId":"same"},
            {"Id":"other","PlaylistItemId":"other"},
            {"Id":"same","PlaylistItemId":"same"}
        ],"TotalRecordCount":3});
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (response.clone(), None),
                (response.clone(), None),
                (Value::Null, None),
                (response.clone(), None),
                (Value::Null, None),
                (response, None),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let playlist = Item {
            id: "list".into(),
            server_id: api.server.id,
            kind: ItemKind::Playlist,
            ..Default::default()
        };
        let tracks = api.all_tracks(&playlist).unwrap();
        api.edit_playlist_selection(&playlist, &tracks, &[2], PlaylistSelectionAction::Remove)
            .unwrap();
        api.edit_playlist_selection(
            &playlist,
            &tracks,
            &[2],
            PlaylistSelectionAction::MoveBefore(Some(1)),
        )
        .unwrap();
        let mut stale = tracks;
        stale[1].id = "changed".into();
        assert!(
            api.edit_playlist_selection(&playlist, &stale, &[2], PlaylistSelectionAction::Remove)
                .is_err()
        );
        let requests = worker.join().unwrap();
        for (index, ids) in [
            (2, json!(["same", "other"])),
            (4, json!(["same", "same", "other"])),
        ] {
            assert!(requests[index].starts_with("POST /music/Playlists/list "));
            let body: Value =
                serde_json::from_str(requests[index].split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body, json!({"Ids":ids}));
        }
        assert!(!requests.iter().any(|request| request.starts_with("DELETE")));
    }

    #[test]
    fn playlist_block_moves_preserve_duplicate_identity_for_every_selection_and_anchor() {
        use std::collections::BTreeSet;
        for len in 1..=8 {
            for mask in 1..(1u32 << len) {
                let selected = (0..len)
                    .filter(|index| mask & (1 << index) != 0)
                    .collect::<BTreeSet<_>>();
                for before in 0..=len {
                    let mut expected = (0..len)
                        .filter(|index| !selected.contains(index))
                        .collect::<Vec<_>>();
                    let insert = before - selected.iter().filter(|index| **index < before).count();
                    expected.splice(insert..insert, selected.iter().copied());
                    let mut actual = (0..len).collect::<Vec<_>>();
                    for (from, to) in playlist_moves(len, &selected, Some(before)).unwrap() {
                        let item = actual.remove(from);
                        actual.insert(to, item);
                    }
                    assert_eq!(
                        actual, expected,
                        "len {len}, selected {selected:?}, before {before}"
                    );
                }
            }
        }
    }

    #[test]
    fn bulk_playlist_edits_use_entry_ids_and_reject_stale_snapshots_before_mutation() {
        for kind in ServerType::ALL {
            let response = match kind {
                ServerType::Navidrome => {
                    json!([{"id":"e0","mediaFileId":"same","title":"Duplicate"},{"id":"e1","mediaFileId":"other"},{"id":"e2","mediaFileId":"same"}])
                }
                ServerType::Jellyfin => {
                    json!({"TotalRecordCount":3,"Items":[{"Id":"same","PlaylistItemId":"e0","Name":"Duplicate"},{"Id":"other","PlaylistItemId":"e1"},{"Id":"same","PlaylistItemId":"e2"}]})
                }
                ServerType::Subsonic => {
                    json!({"subsonic-response":{"status":"ok","playlist":{"entry":[{"id":"same","title":"Duplicate"},{"id":"other"},{"id":"same"}]}}})
                }
            };
            let ok = json!({"subsonic-response":{"status":"ok"}});
            let (server, worker) = protocol_server(
                kind,
                vec![
                    (response.clone(), Some(3)),
                    (response.clone(), Some(3)),
                    (ok.clone(), None),
                    (response.clone(), Some(3)),
                    (ok, None),
                    (response, Some(3)),
                ],
            );
            let api = Api::new(server, Session::default()).unwrap();
            let playlist = Item {
                id: "list".into(),
                server_id: api.server.id,
                kind: ItemKind::Playlist,
                ..Default::default()
            };
            let expected = api.all_tracks(&playlist).unwrap();
            api.edit_playlist_selection(
                &playlist,
                &expected,
                &[0, 2],
                PlaylistSelectionAction::Remove,
            )
            .unwrap();
            api.edit_playlist_selection(
                &playlist,
                &expected,
                &[0],
                PlaylistSelectionAction::MoveBefore(None),
            )
            .unwrap();
            let mut stale = expected;
            stale[0].id = "changed".into();
            assert!(
                api.edit_playlist_selection(
                    &playlist,
                    &stale,
                    &[0],
                    PlaylistSelectionAction::Remove
                )
                .unwrap_err()
                .to_string()
                .contains("Playlist changed")
            );
            let requests = worker.join().unwrap();
            match kind {
                ServerType::Navidrome => {
                    assert!(requests[2].contains("id=e0&id=e2"));
                    assert!(requests[4].starts_with("PUT /music/api/playlist/list/tracks/1"));
                }
                ServerType::Jellyfin => {
                    assert!(requests[2].contains("EntryIds=e0%2Ce2"));
                    assert!(requests[4].starts_with("POST /music/Playlists/list/Items/e0/Move/2"));
                }
                ServerType::Subsonic => {
                    assert!(requests[2].contains("songIndexToRemove=2&songIndexToRemove=0"));
                    assert!(requests[4].contains("songId=other&songId=same&songId=same"));
                }
            }
        }
    }

    #[test]
    fn artist_metadata_counts_and_duration_preserve_backend_availability() {
        for kind in [ItemKind::Artist, ItemKind::AlbumArtist] {
            let api = Api::new(server(ServerType::Jellyfin), Session::default()).unwrap();
            let artist = api.normalize(&json!({"Id":"artist", "Name":"Artist", "AlbumCount":2, "SongCount":12, "ChildCount":99, "RunTimeTicks":610000000}), kind);
            assert_eq!(artist.column_data["albumCount"], "2");
            assert_eq!(artist.column_data["songCount"], "12");
            assert_eq!(artist.column_data["duration"], "61");
            let missing = api.normalize(&json!({"Id":"artist", "Name":"Artist"}), kind);
            assert!(
                ["albumCount", "songCount", "duration"]
                    .iter()
                    .all(|key| !missing.column_data.contains_key(*key))
            );
            let zero = api.normalize(
                &json!({"Id":"artist", "Name":"Artist", "SongCount":0, "RunTimeTicks":0}),
                kind,
            );
            assert_eq!(zero.column_data["songCount"], "0");
            assert_eq!(zero.column_data["duration"], "0");
            let api = Api::new(server(ServerType::Subsonic), Session::default()).unwrap();
            let artist = api.normalize(
                &json!({"id":"artist", "name":"Artist", "songCount":12, "duration":61}),
                kind,
            );
            assert_eq!(artist.column_data["albumCount"], "0");
            assert!(!artist.column_data.contains_key("songCount"));
            assert!(!artist.column_data.contains_key("duration"));
        }
    }

    #[test]
    fn artist_roles_biographies_discography_and_top_song_contracts() {
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (
                    json!({"subsonic-response":{"status":"ok","serverVersion":"0.57.0"}}),
                    None,
                ),
                (
                    json!({"subsonic-response":{"status":"ok","openSubsonicExtensions":[]}}),
                    None,
                ),
                (
                    json!([{"id":"artist","name":"A & B","roles":{"composer":2}}]),
                    Some(1),
                ),
                (json!([{"id":"artist","name":"A & B"}]), Some(1)),
                (
                    json!({"subsonic-response":{"status":"ok","artistInfo2":{"biography":"<p>A &amp; B</p><script>hidden</script><p>Biography</p>","lastFmUrl":"https://www.last.fm/music/A","similarArtist":[{"id":"other","name":"Other"},{"id":"-1","name":"Missing"},{"id":"artist","name":"Self"}]}}}),
                    None,
                ),
                (
                    json!([{"id":"song","title":"Played","playCount":12,"albumArtist":"Album artist","albumArtistId":"aa"}]),
                    Some(1),
                ),
                (json!([{"id":"album","name":"Album"}]), Some(1)),
                (
                    json!({"subsonic-response":{"status":"ok","topSongs":{"song":[{"id":"song","title":"Popular"}]}}}),
                    None,
                ),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        api.discover_features();
        assert!(api.features().artist_roles && api.features().artists_search);
        let detail = api.normalize(&json!({
            "id":"artist", "name":"A & B", "biography":"<p>Full biography</p>",
            "albumCount":99, "songCount":99,
            "stats":{"albumartist":{"albumCount":2,"songCount":7},"artist":{"albumCount":4,"songCount":3}}
        }), ItemKind::AlbumArtist);
        assert_eq!(detail.comment, "<p>Full biography</p>");
        assert_eq!(detail.column_data["albumCount"], "4");
        assert_eq!(detail.column_data["songCount"], "7");
        let artist = api
            .list(&ListQuery {
                kind: ItemKind::AlbumArtist,
                ..Default::default()
            })
            .unwrap()
            .items
            .remove(0);
        assert_eq!(artist.kind, ItemKind::AlbumArtist);
        assert_eq!(artist.roles, ["composer"]);
        api.list(&ListQuery {
            kind: ItemKind::Artist,
            role: Some("composer".into()),
            ..Default::default()
        })
        .unwrap();
        let info = api.artist_info(&artist).unwrap();
        assert_eq!(info.biography, "A & B\nBiography");
        assert!(info.biography_html.contains("A &amp; B"));
        assert!(!info.biography_html.contains("hidden"));
        assert_eq!(info.similar.len(), 1);
        let songs = api
            .artist_page(&artist, ArtistTab::PersonalTop, 0, 100)
            .unwrap();
        assert_eq!(songs.items[0].play_count, 12);
        assert_eq!(songs.items[0].album_artist_id, "aa");
        api.artist_page(&artist, ArtistTab::Albums, 0, 100).unwrap();
        api.artist_page(&artist, ArtistTab::CommunityTop, 0, 100)
            .unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[2].contains("role=albumartist"));
        assert!(requests[3].contains("role=composer"));
        assert!(requests[4].contains("getArtistInfo2.view?") && requests[4].contains("count=10"));
        assert!(
            requests[5].contains("artists_id=artist")
                && requests[5].contains("_sort=playCount")
                && requests[5].contains("_order=DESC")
        );
        assert!(requests[6].contains("artist_id=artist"));
        assert!(requests[7].contains("artist=A+%26+B"));
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (
                    json!({"Items":[{"Id":"artist","Name":"Artist"}],"TotalRecordCount":1}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"artist","Name":"Artist"}],"TotalRecordCount":1}),
                    None,
                ),
                (
                    json!({"Id":"artist","Name":"Artist","Overview":"<p>Biography</p>"}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"similar","Name":"Related"},{"Id":"various","Name":"Various Artists"}]}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"song","Name":"Song","UserData":{"PlayCount":5}}],"TotalRecordCount":1}),
                    None,
                ),
            ],
        );
        let api = Api::new(
            server,
            Session {
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let artist = api
            .list(&ListQuery {
                kind: ItemKind::Artist,
                ..Default::default()
            })
            .unwrap()
            .items
            .remove(0);
        api.list(&ListQuery {
            kind: ItemKind::AlbumArtist,
            ..Default::default()
        })
        .unwrap();
        let info = api.artist_info(&artist).unwrap();
        assert_eq!(info.biography, "Biography");
        assert_eq!(info.similar.len(), 1);
        assert_eq!(
            api.artist_page(&artist, ArtistTab::PersonalTop, 0, 100)
                .unwrap()
                .items[0]
                .play_count,
            5
        );
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("GET /music/Artists?"));
        assert!(requests[1].starts_with("GET /music/Artists/AlbumArtists?"));
        assert!(requests[3].starts_with("GET /music/Artists/artist/Similar?"));
        assert!(requests[3].contains("Limit=10"));
        assert!(
            requests[4].contains("ArtistIds=artist") && requests[4].contains("SortBy=PlayCount")
        );
    }
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };
    use uuid::Uuid;

    fn server(kind: ServerType) -> Server {
        Server {
            id: Uuid::new_v4(),
            name: "Test".into(),
            url: "http://localhost:4533/music/".into(),
            username: "a+b & user".into(),
            kind,
            legacy_auth: false,
            options: Default::default(),
        }
    }
    fn mock(kind: ServerType, response: &str) -> (Api, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut server = server(kind);
        server.url = format!("http://{}/music", listener.local_addr().unwrap());
        let response = response.to_owned();
        let thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0; 4096];
            loop {
                let n = stream.read(&mut buf).unwrap();
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|v| v == b"\r\n\r\n") {
                    break;
                }
            }
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Total-Count: 101\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",response.len(),response).unwrap();
            String::from_utf8(request).unwrap()
        });
        (
            Api::new(
                server,
                Session {
                    token: "secret".into(),
                    user_id: "uid".into(),
                    salt: "salt".into(),
                    subsonic_token: "hash".into(),
                    ..Session::default()
                },
            )
            .unwrap(),
            thread,
        )
    }

    #[test]
    fn discord_artwork_uses_public_album_info_or_configured_jellyfin_remote_url() {
        for kind in [ServerType::Navidrome, ServerType::Subsonic] {
            let (api, worker) = mock(
                kind,
                r#"{"subsonic-response":{"status":"ok","albumInfo":{"largeImageUrl":"","mediumImageUrl":"https://images.example/medium.jpg"}}}"#,
            );
            let item = api.normalize(&json!({"id":"song", "albumId":"album", "mbzReleaseTrackId":"track", "mbzRecordingID":"recording", "mbzAlbumId":"release", "musicBrainzId":"ss-recording"}), ItemKind::Song);
            assert_eq!(
                api.presence_image(&item).unwrap().as_deref(),
                Some("https://images.example/medium.jpg")
            );
            let request = worker.join().unwrap();
            assert!(request.starts_with("GET /music/rest/getAlbumInfo2.view?"));
            assert!(request.contains("id=album"));
            assert_eq!(
                item.column_data["mbzRecordingId"],
                if kind == ServerType::Navidrome {
                    "recording"
                } else {
                    "ss-recording"
                }
            );
        }
        let mut server = server(ServerType::Jellyfin);
        server.options.remote_url = "https://public.example/music".into();
        let api = Api::new(
            server,
            Session {
                token: "key".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let item = api.normalize(&json!({"Id":"song", "AlbumId":"album", "ImageTags":{"Primary":"tag"}, "ProviderIds":{"MusicBrainzTrack":"track", "MusicBrainzAlbum":"release"}}), ItemKind::Song);
        let image = Url::parse(&api.presence_image(&item).unwrap().unwrap()).unwrap();
        assert_eq!(image.host_str(), Some("public.example"));
        assert_eq!(image.path(), "/music/Items/song/Images/Primary");
        assert!(
            image
                .query_pairs()
                .any(|(key, value)| key == "apiKey" && value == "key")
        );
        assert_eq!(item.column_data["mbzTrackId"], "track");
        assert_eq!(item.column_data["mbzAlbumId"], "release");
        let mut wrong = item;
        wrong.server_id = Uuid::new_v4();
        assert!(api.presence_image(&wrong).is_err());
    }

    #[test]
    fn server_lyrics_keep_languages_choose_synced_main_and_preserve_jellyfin_ticks() {
        for kind in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            let value = if kind == ServerType::Jellyfin {
                json!({"Lyrics":[{"Start":15_000_000,"Text":"Timed"}]})
            } else {
                json!({"subsonic-response":{"status":"ok","lyricsList":{"structuredLyrics":[{"lang":"eng","synced":false,"line":[{"value":"Plain"}]},{"lang":"jpn","kind":"translation","synced":true,"line":[{"start":1000,"value":"Translation"}]},{"lang":"eng","kind":"main","synced":true,"offset":200,"line":[{"start":1500,"value":"Timed"}]}]}}})
            };
            let (server, worker) =
                protocol_server(kind, vec![(value.clone(), None), (value, None)]);
            let item = Item {
                id: "song-id".into(),
                server_id: server.id,
                ..Default::default()
            };
            let api = Api::new(server, Session::default()).unwrap();
            let variants = api.server_lyric_variants(&item).unwrap();
            if kind != ServerType::Jellyfin {
                assert_eq!(variants.len(), 3);
                assert_eq!(variants[2].offset, Some(0.2));
                assert_eq!(variants[1].kind, "translation");
            }
            assert_eq!(api.server_lyrics(&item).unwrap()[0].time, Some(1.5));
            assert!(
                worker.join().unwrap()[0].contains(if kind == ServerType::Jellyfin {
                    "Audio/song-id/Lyrics"
                } else {
                    "getLyricsBySongId"
                })
            );
        }
    }

    #[test]
    fn artwork_hashes_and_per_view_image_sizes_match_backends() {
        for kind in ServerType::ALL {
            let sizes = [80., 300., 0., 123.5, 2001., -1.];
            let (server, worker) = protocol_server(kind, vec![(json!({}), None); sizes.len()]);
            let api = Api::new(
                server,
                Session {
                    token: "private-token".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let item = api.normalize(&json!({"id":"song", "Id":"song", "coverArt":"cover", "ImageTags":{"Primary":"tag"}, "ImageBlurHashes":{"Primary":{"other":"wrong", "tag":"LEHV6nWB2yk8pyo0adR*.7kCMdnj"}}, "thumbHash":"native-hash", "dominantColor":"#123456"}), ItemKind::Song);
            match kind {
                ServerType::Jellyfin => {
                    assert!(item.thumb_hash.is_empty());
                    assert_eq!(item.blur_hash, "LEHV6nWB2yk8pyo0adR*.7kCMdnj");
                    assert!(item.dominant_color.is_empty());
                }
                ServerType::Navidrome => {
                    assert_eq!(item.thumb_hash, "native-hash");
                    assert_eq!(item.dominant_color, "#123456");
                    assert!(item.blur_hash.is_empty());
                }
                ServerType::Subsonic => {
                    assert!(
                        item.thumb_hash.is_empty()
                            && item.blur_hash.is_empty()
                            && item.dominant_color.is_empty()
                    );
                }
            }
            for size in sizes {
                api.cover_bytes_sized(&item, size).unwrap();
            }
            assert!(api.cover_bytes_sized(&item, f64::NAN).is_err());
            assert!(api.cover_bytes_sized(&item, f64::INFINITY).is_err());
            let requests = worker.join().unwrap();
            let parameter = if kind == ServerType::Jellyfin {
                "width"
            } else {
                "size"
            };
            for (request, size) in requests.iter().zip(sizes) {
                let target = request
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = url::Url::parse(&format!("http://fixture{target}")).unwrap();
                assert_eq!(
                    url.query_pairs()
                        .find(|(key, _)| key == parameter)
                        .map(|(_, size)| size.into_owned()),
                    (size != 0.).then(|| size.to_string())
                );
                if kind == ServerType::Jellyfin {
                    assert!(
                        url.query_pairs()
                            .any(|(key, value)| key == "quality" && value == "96")
                    );
                }
                assert!(url.path().contains(if kind == ServerType::Jellyfin {
                    "Items/song/Images/Primary"
                } else {
                    "getCoverArt.view"
                }));
            }
        }
    }

    #[test]
    fn folder_browser_roots_respect_library_selection_and_include_album_directories() {
        for kind in [ServerType::Subsonic, ServerType::Navidrome] {
            for library in [None, Some("selected")] {
                let (mut server, worker) = protocol_server(
                    kind,
                    vec![
                        (
                            json!({"subsonic-response":{"status":"ok","indexes":{"index":[{"artist":[{"id":"artist-dir","name":"Artist folder"}]}]}}}),
                            None,
                        ),
                        (
                            json!({"subsonic-response":{"status":"ok","directory":{"child":[{"id":"album-dir","title":"Album folder","isDir":true},{"id":"song","title":"Song","isDir":false}]}}}),
                            None,
                        ),
                    ],
                );
                server.options.music_folder_ids = library.into_iter().map(str::to_owned).collect();
                let api = Api::new(server, Session::default()).unwrap();
                let roots = api.folder_children(None).unwrap();
                assert_eq!(roots.len(), 1);
                assert_eq!(roots[0].id, "artist-dir");
                assert_eq!(roots[0].kind, ItemKind::Folder);
                assert!(!roots[0].folder_root);
                let children = api.folder_children(Some(&roots[0])).unwrap();
                assert_eq!(
                    children.iter().map(|item| item.kind).collect::<Vec<_>>(),
                    [ItemKind::Folder, ItemKind::Song]
                );
                let requests = worker.join().unwrap();
                assert!(requests[0].contains("getIndexes.view"));
                assert_eq!(
                    requests[0].contains("musicFolderId=selected"),
                    library.is_some()
                );
                assert!(
                    requests[1].contains("getMusicDirectory.view")
                        && requests[1].contains("id=artist-dir")
                );
            }
        }
        let (mut server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (
                    json!({"Items":[{"Id":"album-dir","Name":"Album folder","Type":"MusicAlbum","IsFolder":true},{"Id":"loose","Name":"Loose track","Type":"Audio"}],"TotalRecordCount":2}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"disc-dir","Name":"Disc folder","Type":"Folder","IsFolder":true},{"Id":"track","Name":"Track","Type":"Audio","MediaSources":[]}],"TotalRecordCount":2}),
                    None,
                ),
            ],
        );
        server.options.music_folder_ids = vec!["music".into()];
        let api = Api::new(
            server,
            Session {
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let roots = api.folder_children(None).unwrap();
        assert_eq!(
            roots
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["album-dir"]
        );
        let children = api.folder_children(Some(&roots[0])).unwrap();
        assert_eq!(
            children.iter().map(|item| item.kind).collect::<Vec<_>>(),
            [ItemKind::Folder, ItemKind::Song]
        );
        let requests = worker.join().unwrap();
        for request in &requests {
            assert!(request.contains("Recursive=false"));
            assert!(!request.contains("IncludeItemTypes="), "{request}");
        }
        assert!(requests[0].contains("ParentId=music"));
        assert!(requests[1].contains("ParentId=album-dir"));
    }

    #[test]
    fn jellyfin_folder_rows_keep_type_and_raw_page_offsets_when_audio_is_unavailable() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/folder-items-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let (mut server, worker) = protocol_server(
                ServerType::Jellyfin,
                vec![(
                    json!({"Items":case["rows"], "TotalRecordCount":case["rows"].as_array().unwrap().len()}),
                    None,
                )],
            );
            server.options.enable_audiobooks = case["audiobooks"].as_bool().unwrap();
            let root = case["root"].as_bool().unwrap();
            if root {
                server.options.music_folder_ids = vec!["music".into()];
            }
            let parent = Item {
                id: "directory".into(),
                server_id: server.id,
                kind: ItemKind::Folder,
                ..Default::default()
            };
            let api = Api::new(
                server,
                Session {
                    user_id: "user".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let items = api.folder_children((!root).then_some(&parent)).unwrap();
            for (kind, expected) in [
                (ItemKind::Folder, &case["folders"]),
                (ItemKind::Song, &case["songs"]),
            ] {
                let mut ids = items
                    .iter()
                    .filter(|item| item.kind == kind)
                    .map(|item| item.id.clone())
                    .collect::<Vec<_>>();
                ids.sort();
                assert_eq!(serde_json::to_value(ids).unwrap(), *expected, "{case}");
            }
            worker.join().unwrap();
        }
        for case in reference["filterCases"].as_array().unwrap() {
            let (mut server, worker) = protocol_server(
                ServerType::Jellyfin,
                vec![(
                    json!({"Items":case["rows"], "TotalRecordCount":case["rows"].as_array().unwrap().len()}),
                    None,
                )],
            );
            server.options.enable_audiobooks = true;
            let parent = Item {
                id: "directory".into(),
                server_id: server.id,
                kind: ItemKind::Folder,
                ..Default::default()
            };
            let api = Api::new(
                server,
                Session {
                    user_id: "user".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let items = crate::lists::folder_items(
                api.folder_children(Some(&parent)).unwrap(),
                if case["sort"] == "name" {
                    Sort::Name
                } else {
                    Sort::Duration
                },
                case["descending"].as_bool().unwrap(),
                case["query"].as_str().unwrap(),
            );
            assert_eq!(
                serde_json::to_value(items.iter().map(|item| item.id.clone()).collect::<Vec<_>>())
                    .unwrap(),
                case["ids"],
                "{case}"
            );
            worker.join().unwrap();
        }
        for audiobooks in [false, true] {
            let pages = vec![
                (
                    json!({"Items":[{"Id":"unavailable", "Type":"Audio"}, {"Id":"null", "Type":"Audio", "MediaSources":null}], "TotalRecordCount":5}),
                    None,
                ),
                (
                    json!({"Items":[{"Id":"album", "Type":"MusicAlbum"}, {"Id":"book", "Type":"AudioBook", "MediaSources":[]}, {"Id":"track", "Type":"Audio", "MediaSources":[{"Container":"flac"}]}], "TotalRecordCount":5}),
                    None,
                ),
            ];
            let (mut server, worker) =
                protocol_server(ServerType::Jellyfin, [pages.clone(), pages].concat());
            server.options.enable_audiobooks = audiobooks;
            let parent = Item {
                id: "directory".into(),
                server_id: server.id,
                kind: ItemKind::Folder,
                ..Default::default()
            };
            let api = Api::new(
                server,
                Session {
                    user_id: "user".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let children = api.folder_children(Some(&parent)).unwrap();
            assert_eq!(
                children
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["album", "book", "track"]
            );
            assert_eq!(
                children.iter().map(|item| item.kind).collect::<Vec<_>>(),
                [
                    ItemKind::Folder,
                    if audiobooks {
                        ItemKind::Song
                    } else {
                        ItemKind::Folder
                    },
                    ItemKind::Song
                ]
            );
            let page = api.detail(&parent, 1, 1).unwrap();
            assert_eq!(page.items[0].id, "book");
            assert_eq!(page.total, Some(3));
            assert!(page.has_more);
            let requests = worker.join().unwrap();
            for (request, offset) in requests.iter().zip([0, 2, 0, 2]) {
                assert!(
                    request.contains(&format!("StartIndex={offset}")),
                    "{request}"
                );
                assert!(
                    request.contains("Recursive=false") && !request.contains("IncludeItemTypes="),
                    "{request}"
                );
            }
        }
        let page = (
            json!({"Items":[{"Id":"unavailable", "Type":"Audio"}], "TotalRecordCount":2}),
            None,
        );
        let (server, worker) = protocol_server(ServerType::Jellyfin, vec![page.clone(), page]);
        let parent = Item {
            id: "directory".into(),
            server_id: server.id,
            kind: ItemKind::Folder,
            ..Default::default()
        };
        let api = Api::new(server, Session::default()).unwrap();
        assert!(
            api.folder_children(Some(&parent))
                .unwrap_err()
                .to_string()
                .contains("repeated a page")
        );
        worker.join().unwrap();
    }

    #[test]
    fn playlist_album_downloads_include_only_group_tracks_and_preserve_failed_destinations() {
        use std::io::Read;
        let dir =
            std::env::temp_dir().join(format!("feishin-album-download-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        for kind in ServerType::ALL {
            let (server, worker) = protocol_server(
                kind,
                vec![
                    (json!({"audio":"first"}), None),
                    (json!({"audio":"second"}), None),
                ],
            );
            let api = Api::new(server, Session::default()).unwrap();
            let songs = (0..2)
                .map(|index| Item {
                    id: "duplicate-song".into(),
                    name: "../same".into(),
                    album_id: "album".into(),
                    server_id: api.server.id,
                    track: 2 - index,
                    playlist_position: Some(index as usize),
                    container: "flac".into(),
                    ..Default::default()
                })
                .collect();
            let album = crate::playlist_albums::rows(songs).remove(0);
            let path = dir.join(format!("{kind:?}.zip"));
            api.download(&album, &path).unwrap();
            let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
            assert_eq!(archive.len(), 2);
            for index in 0..2 {
                let mut file = archive.by_index(index).unwrap();
                assert!(!file.name().contains('/') && !file.name().contains('\\'));
                assert!(file.name().ends_with(".._same.flac"));
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&contents).unwrap()["audio"],
                    if index == 0 { "first" } else { "second" }
                );
            }
            let requests = worker.join().unwrap();
            assert!(
                requests.iter().all(|request| request
                    .lines()
                    .next()
                    .unwrap()
                    .contains("duplicate-song"))
            );
        }
        let (server, worker) = protocol_status_server(
            ServerType::Navidrome,
            vec![(200, json!({}), None, None), (500, json!({}), None, None)],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let album = Item {
            kind: ItemKind::Album,
            server_id: api.server.id,
            playlist_songs: (0..2)
                .map(|i| Item {
                    id: format!("s{i}"),
                    server_id: api.server.id,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let path = dir.join("failed.zip");
        std::fs::write(&path, b"original").unwrap();
        assert!(api.download(&album, &path).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
        assert!(
            std::fs::read_dir(&dir)
                .unwrap()
                .flatten()
                .all(|file| file.path().extension().is_none_or(|ext| ext != "part"))
        );
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn protocol_server(
        kind: ServerType,
        responses: Vec<(Value, Option<usize>)>,
    ) -> (Server, thread::JoinHandle<Vec<String>>) {
        protocol_status_server(
            kind,
            responses
                .into_iter()
                .map(|(value, total)| (200, value, total, None))
                .collect(),
        )
    }

    #[test]
    fn jellyfin_scrobbles_match_original_submission_and_queue_reports() {
        let cases: Vec<Value> =
            serde_json::from_str(include_str!("../assets/scrobble-protocol-reference.json"))
                .unwrap();
        let (server, worker) =
            protocol_server(ServerType::Jellyfin, vec![(json!({}), None); cases.len()]);
        let api = Api::new(
            server,
            Session {
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let item = Item {
            id: "song".into(),
            server_id: api.server.id,
            kind: ItemKind::Song,
            ..Default::default()
        };
        for case in &cases {
            let queue = case["queue"]
                .as_array()
                .unwrap()
                .iter()
                .map(|entry| entry["Id"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            api.scrobble(&item, case["event"].as_str().unwrap(), 12.5, 1., &queue)
                .unwrap();
        }
        for (request, case) in worker.join().unwrap().iter().zip(cases) {
            let path = match case["method"].as_str().unwrap() {
                "scrobblePlaying" => "Sessions/Playing",
                "scrobbleStopped" => "Sessions/Playing/Stopped",
                _ => "Sessions/Playing/Progress",
            };
            assert!(request.starts_with(&format!("POST /music/{path} ")));
            let body: Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body, case["body"], "{case}");
        }
    }

    #[test]
    fn player_similar_tracks_use_original_query_count_and_ignore_library_scope() {
        for kind in ServerType::ALL {
            for fallback in [false, true] {
                if fallback && kind != ServerType::Jellyfin {
                    continue;
                }
                let songs = if kind == ServerType::Jellyfin {
                    json!({"Items":[{"Id":"seed","Name":"Seed"},{"Id":"b","Name":"B"},{"Id":"a","Name":"A"}]})
                } else {
                    json!({"subsonic-response":{"status":"ok","similarSongs":{"song":[{"id":"seed","title":"Seed"},{"id":"b","title":"B"},{"id":"a","title":"A"}]}}})
                };
                let mut responses = vec![];
                if fallback {
                    responses.push((json!({"Items":[]}), None));
                }
                responses.push((songs, None));
                let (mut server, worker) = protocol_server(kind, responses);
                server.options.music_folder_ids = vec!["unrelated-library".into()];
                let api = Api::new(
                    server,
                    Session {
                        user_id: "user".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let item = Item {
                    id: "seed".into(),
                    server_id: api.server.id,
                    ..Default::default()
                };
                assert_eq!(
                    api.similar_songs(&item)
                        .unwrap()
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<Vec<_>>(),
                    ["b", "a"]
                );
                for (index, request) in worker.join().unwrap().iter().enumerate() {
                    let target = request
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let url = Url::parse(&format!("http://localhost{target}")).unwrap();
                    assert!(
                        url.query_pairs()
                            .all(|(key, _)| !matches!(key.as_ref(), "musicFolderId" | "ParentId"))
                    );
                    assert!(url.query_pairs().any(|(key, value)| key
                        == if kind == ServerType::Jellyfin {
                            "Limit"
                        } else {
                            "count"
                        }
                        && value == "50"));
                    assert_eq!(
                        url.path(),
                        if kind == ServerType::Jellyfin {
                            if index == 0 {
                                "/music/Items/seed/Similar"
                            } else {
                                "/music/Items/seed/InstantMix"
                            }
                        } else {
                            "/music/rest/getSimilarSongs.view"
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn jellyfin_quick_connect_keeps_secrets_private_and_uses_the_authorized_account() {
        let (mut server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (
                    json!({"Code":"123456", "Secret":"private /?& secret"}),
                    None,
                ),
                (json!({"Authenticated":false}), None),
                (json!({"Authenticated":true}), None),
                (
                    json!({"AccessToken":"authorized-token", "User":{"Id":"approved-user", "Name":"Approved Account", "Policy":{"IsAdministrator":true}}}),
                    None,
                ),
            ],
        );
        server.username.clear();
        let api = Api::new(server, Default::default()).unwrap();
        let request = api.initiate_quick_connect().unwrap();
        assert_eq!(request.code, "123456");
        assert!(!api.quick_connect_authenticated(&request).unwrap());
        assert!(api.quick_connect_authenticated(&request).unwrap());
        let api = api.authenticate_quick_connect(&request).unwrap();
        assert_eq!(api.server.username, "Approved Account");
        assert_eq!(api.session().token, "authorized-token");
        assert_eq!(api.session().user_id, "approved-user");
        assert!(api.session().is_admin);
        assert!(
            !serde_json::to_string(&api.server)
                .unwrap()
                .contains("secret")
        );
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("POST /music/QuickConnect/Initiate HTTP"));
        assert!(!requests[0].contains("Token="));
        for request in &requests[1..3] {
            let target = request
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            let url = Url::parse(&format!("http://localhost{target}")).unwrap();
            assert_eq!(url.path(), "/music/QuickConnect/Connect");
            assert_eq!(
                url.query_pairs()
                    .find(|(key, _)| key == "secret")
                    .unwrap()
                    .1,
                "private /?& secret"
            );
        }
        assert!(requests[3].starts_with("POST /music/Users/AuthenticateWithQuickConnect HTTP"));
        assert_eq!(
            serde_json::from_str::<Value>(requests[3].split("\r\n\r\n").nth(1).unwrap()).unwrap(),
            json!({"Secret":"private /?& secret"})
        );
    }

    #[test]
    fn quick_connect_rejects_disabled_expired_malformed_and_cancelled_requests() {
        for payload in [
            json!({"Code":"123456"}),
            json!({"Code":123456,"Secret":"secret"}),
            json!({"Code":"123456","Secret":123456}),
        ] {
            let (fixture_server, worker) =
                protocol_server(ServerType::Jellyfin, vec![(payload, None)]);
            let error = Api::new(fixture_server, Default::default())
                .unwrap()
                .initiate_quick_connect()
                .err()
                .unwrap();
            assert!(error.to_string().contains("not active"));
            worker.join().unwrap();
        }
        let (fixture_server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (json!({"Code":"123456", "Secret":"never-log-this"}), None),
                (json!({"Authenticated":"false"}), None),
                (json!({"AccessToken":"token", "User":{"Id":"user"}}), None),
                (
                    json!({"AccessToken":"token", "User":{"Id":"user", "Name":123}}),
                    None,
                ),
                (
                    json!({"AccessToken":123, "User":{"Id":"user", "Name":"Name"}}),
                    None,
                ),
            ],
        );
        let api = Api::new(fixture_server, Default::default()).unwrap();
        let request = api.initiate_quick_connect().unwrap();
        assert!(
            api.quick_connect_authenticated(&request)
                .unwrap_err()
                .to_string()
                .contains("expired")
        );
        assert!(
            api.clone()
                .authenticate_quick_connect(&request)
                .err()
                .unwrap()
                .to_string()
                .contains("username")
        );
        assert!(
            api.clone()
                .authenticate_quick_connect(&request)
                .err()
                .unwrap()
                .to_string()
                .contains("username")
        );
        assert!(
            api.clone()
                .authenticate_quick_connect(&request)
                .err()
                .unwrap()
                .to_string()
                .contains("authentication token")
        );
        worker.join().unwrap();
        let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let api = api.with_cancellation(flag);
        assert!(api.quick_connect_authenticated(&request).is_err());
        assert!(api.authenticate_quick_connect(&request).is_err());
        assert!(
            Api::new(server(ServerType::Subsonic), Default::default())
                .unwrap()
                .initiate_quick_connect()
                .is_err()
        );
    }

    #[test]
    fn sign_out_revokes_cloned_clients_and_rejects_in_flight_responses() {
        use std::sync::mpsc;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let (received, wait_received) = mpsc::channel();
        let (release, wait_release) = mpsc::channel();
        let mut server = server(ServerType::Jellyfin);
        server.url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = vec![];
            let mut buffer = [0; 4096];
            while !bytes.windows(4).any(|b| b == b"\r\n\r\n") {
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0);
                bytes.extend_from_slice(&buffer[..count]);
            }
            received.send(()).unwrap();
            wait_release.recv_timeout(Duration::from_secs(5)).unwrap();
            let body = r#"{"Items":[{"Id":"private-song","Name":"Private"}],"TotalRecordCount":1}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8(bytes).unwrap()
        });
        let api = Api::new(
            server,
            Session {
                token: "old-token".into(),
                user_id: "old-account".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let configured = api.reconfigure(api.server.clone()).unwrap();
        let client = api.clone();
        let request = thread::spawn(move || {
            client.list(&ListQuery {
                kind: ItemKind::Song,
                ..Default::default()
            })
        });
        wait_received.recv_timeout(Duration::from_secs(5)).unwrap();
        api.sign_out();
        release.send(()).unwrap();
        assert!(
            request
                .join()
                .unwrap()
                .unwrap_err()
                .to_string()
                .contains("Sign in")
        );
        assert!(worker.join().unwrap().contains("Token=\"old-token\""));
        for client in [api, configured] {
            assert!(client.is_signed_out());
            assert!(client.session().token.is_empty());
            assert!(client.session().user_id.is_empty());
            assert!(client.list(&ListQuery::default()).is_err());
            assert!(client.remember_session().is_err());
            let item = Item {
                id: "song".into(),
                cover_id: "cover".into(),
                server_id: client.server.id,
                ..Default::default()
            };
            assert!(client.stream_url(&item, &Default::default()).is_err());
            assert!(client.cover_bytes(&item).is_err());
        }
    }
    fn protocol_status_server(
        kind: ServerType,
        responses: Vec<(u16, Value, Option<usize>, Option<&'static str>)>,
    ) -> (Server, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut server = server(kind);
        server.url = format!("http://{}/music", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let mut requests = vec![];
            for (status, response, total, token) in responses {
                let deadline = std::time::Instant::now() + Duration::from_secs(5);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(
                                std::time::Instant::now() < deadline,
                                "Expected API request was never sent"
                            );
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let mut buffer = [0; 4096];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]);
                        let length = headers
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
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let response = response.to_string();
                write!(stream, "HTTP/1.1 {status} Response\r\nContent-Type: application/json\r\n{}{}Content-Length: {}\r\nConnection: close\r\n\r\n{}", total.map(|n| format!("X-Total-Count: {n}\r\n")).unwrap_or_default(), token.map(|token| format!("x-nd-authorization: {token}\r\n")).unwrap_or_default(), response.len(), response).unwrap();
            }
            requests
        });
        (server, worker)
    }

    #[test]
    fn auto_dj_requests_and_selection_match_original_collectors() {
        use crate::auto_dj::{Config, Mode, Strategy};
        use std::collections::BTreeMap;
        let fixtures: Vec<Value> =
            serde_json::from_str(include_str!("../assets/auto-dj-reference.json")).unwrap();
        assert_eq!(fixtures.len(), 180);
        for fixture in fixtures {
            let backend: ServerType = serde_json::from_value(fixture["backend"].clone()).unwrap();
            let albums = fixture["mode"] == "albums";
            for server_scope in [false, true] {
                let mut responses = vec![];
                let mut expected = vec![];
                for request in fixture["requests"].as_array().unwrap() {
                    let items = &request["items"];
                    let kind = request["kind"].as_str().unwrap();
                    let (response, total) = if backend == ServerType::Jellyfin {
                        (
                            json!({"Items": items.as_array().unwrap().iter().map(|item| json!({"Id":item["id"], "AlbumId":item["albumId"]})).collect::<Vec<_>>(), "TotalRecordCount":items.as_array().unwrap().len()}),
                            None,
                        )
                    } else if kind == "similar" || kind == "random" {
                        (
                            json!({"subsonic-response":{"status":"ok", if kind == "similar" { "similarSongs" } else { "randomSongs" }: {"song":items}}}),
                            None,
                        )
                    } else if backend == ServerType::Subsonic {
                        (
                            json!({"subsonic-response":{"status":"ok", "albumList2":{"album":items}}}),
                            None,
                        )
                    } else {
                        (items.clone(), Some(items.as_array().unwrap().len()))
                    };
                    responses.push((response.clone(), total));
                    expected.push((request.clone(), false));
                    if backend == ServerType::Jellyfin
                        && kind == "similar"
                        && items.as_array().unwrap().is_empty()
                    {
                        responses.push((response, total));
                        expected.push((request.clone(), true));
                    }
                }
                let (mut server, worker) = protocol_server(backend, responses);
                let folder = fixture["folder"].as_str().map(str::to_owned);
                if server_scope {
                    server.options.music_folder_ids = folder.clone().into_iter().collect();
                }
                let api = Api::new(
                    server,
                    Session {
                        user_id: "user".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                api.features.write().unwrap().artists_search = server_scope;
                let current = Item {
                    id: "current".into(),
                    server_id: api.server.id,
                    genre: "Jazz, Rock".into(),
                    genre_links: Some(if fixture["genre"] == true {
                        vec![
                            ("genre-id".into(), "Jazz".into()),
                            ("second".into(), "Rock".into()),
                        ]
                    } else {
                        vec![]
                    }),
                    album_artist_links: Some(if fixture["artist"] == true {
                        vec![
                            ("album-artist".into(), "Album Artist".into()),
                            ("second-artist".into(), "Other".into()),
                        ]
                    } else {
                        vec![]
                    }),
                    artist_id: "track-artist-must-not-be-used".into(),
                    ..Default::default()
                };
                let strategy = if fixture["strategy"] == "library_random" {
                    Strategy::LibraryRandom
                } else {
                    Strategy::Similar
                };
                let config = Config {
                    mode: if albums { Mode::Albums } else { Mode::Songs },
                    song_strategy: strategy,
                    album_strategy: strategy,
                    item_count: fixture["count"].as_u64().unwrap() as usize,
                    allow_duplicates: fixture["duplicates"].as_bool().unwrap(),
                    only_similar: fixture["only"].as_bool().unwrap(),
                    music_folder: if server_scope { None } else { folder.clone() },
                    ..Default::default()
                };
                let queue = [Item {
                    id: "queued".into(),
                    album_id: "queued".into(),
                    server_id: api.server.id,
                    ..Default::default()
                }];
                let selected = api
                    .auto_dj_selection(&current, &config, &queue)
                    .unwrap_or_else(|error| panic!("{}: {error:#}", fixture["name"]));
                let mut counts = BTreeMap::<String, usize>::new();
                for item in selected {
                    *counts
                        .entry(item.id.split('-').next().unwrap().to_owned())
                        .or_default() += 1;
                }
                assert_eq!(
                    serde_json::to_value(counts).unwrap(),
                    fixture["counts"],
                    "{}",
                    fixture["name"]
                );
                let requests = worker.join().unwrap();
                assert_eq!(requests.len(), expected.len());
                for (actual, (reference, mix)) in requests.iter().zip(expected) {
                    let url = Url::parse(&format!(
                        "http://fixture{}",
                        actual.split_whitespace().nth(1).unwrap()
                    ))
                    .unwrap();
                    let params = url.query_pairs().into_owned().collect::<BTreeMap<_, _>>();
                    let kind = reference["kind"].as_str().unwrap();
                    let query = &reference["query"];
                    let similar = kind == "similar";
                    let limit = if similar {
                        &query["count"]
                    } else {
                        &query["limit"]
                    };
                    let genre = query["genre"]
                        .as_str()
                        .or_else(|| query["genreIds"][0].as_str());
                    let artist = query["artistIds"][0]
                        .as_str()
                        .or_else(|| query["albumArtistIds"][0].as_str());
                    let mut wanted = BTreeMap::<String, String>::new();
                    let path = if backend == ServerType::Jellyfin {
                        wanted.insert("Limit".into(), limit.to_string());
                        if similar {
                            format!(
                                "/music/Items/current/{}",
                                if mix { "InstantMix" } else { "Similar" }
                            )
                        } else {
                            wanted.insert("SortBy".into(), "Random,SortName".into());
                            wanted.insert("SortOrder".into(), "Ascending".into());
                            wanted.insert("StartIndex".into(), "0".into());
                            if let Some(genre) = genre {
                                wanted.insert("GenreIds".into(), genre.into());
                            }
                            if let Some(artist) = artist {
                                wanted.insert("ArtistIds".into(), artist.into());
                            }
                            if let Some(folder) = &folder {
                                wanted.insert("ParentId".into(), folder.clone());
                            }
                            "/music/Users/user/Items".into()
                        }
                    } else if similar || kind == "random" || backend == ServerType::Subsonic {
                        wanted.insert(
                            if similar { "count" } else { "size" }.into(),
                            limit.to_string(),
                        );
                        if similar {
                            wanted.insert("id".into(), "current".into());
                        }
                        if let Some(genre) = genre {
                            wanted.insert("genre".into(), genre.into());
                        }
                        if let Some(folder) = &folder {
                            wanted.insert("musicFolderId".into(), folder.clone());
                        }
                        let endpoint = if similar {
                            "getSimilarSongs"
                        } else if kind == "random" {
                            "getRandomSongs"
                        } else {
                            wanted.insert(
                                "type".into(),
                                if genre.is_some() { "byGenre" } else { "random" }.into(),
                            );
                            "getAlbumList2"
                        };
                        format!("/music/rest/{endpoint}.view")
                    } else {
                        wanted.extend([
                            ("_start".into(), "0".into()),
                            ("_end".into(), limit.to_string()),
                            ("_sort".into(), "random".into()),
                            ("_order".into(), "ASC".into()),
                        ]);
                        if let Some(genre) = genre {
                            wanted.insert("genre_id".into(), genre.into());
                        }
                        if let Some(artist) = artist {
                            wanted.insert(
                                if albums {
                                    "artist_id"
                                } else if server_scope {
                                    "artists_id"
                                } else {
                                    "album_artist_id"
                                }
                                .into(),
                                artist.into(),
                            );
                        }
                        if let Some(folder) = &folder {
                            wanted.insert("library_id".into(), folder.clone());
                        }
                        format!("/music/api/{}", if albums { "album" } else { "song" })
                    };
                    assert_eq!(url.path(), path, "{}", fixture["name"]);
                    for key in [
                        "Limit",
                        "SortBy",
                        "SortOrder",
                        "StartIndex",
                        "GenreIds",
                        "Genres",
                        "ArtistIds",
                        "ParentId",
                        "count",
                        "size",
                        "id",
                        "genre",
                        "type",
                        "musicFolderId",
                        "_start",
                        "_end",
                        "_sort",
                        "_order",
                        "genre_id",
                        "artist_id",
                        "artists_id",
                        "album_artist_id",
                        "library_id",
                    ] {
                        assert_eq!(
                            params.get(key),
                            wanted.get(key),
                            "{}: {key} in {url}",
                            fixture["name"]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn auto_dj_propagates_failed_pools_and_similarity_excludes_the_seed() {
        for backend in [ServerType::Navidrome, ServerType::Subsonic] {
            let (server, worker) = protocol_server(
                backend,
                vec![(
                    json!({"subsonic-response":{"status":"failed","error":{"code":70,"message":"Similarity unavailable"}}}),
                    None,
                )],
            );
            let api = Api::new(server, Session::default()).unwrap();
            let current = Item {
                id: "seed".into(),
                server_id: api.server.id,
                genre: "Jazz".into(),
                ..Default::default()
            };
            assert!(api.auto_dj(&current, &Default::default(), &[]).is_err());
            assert_eq!(worker.join().unwrap().len(), 1);
            let (server, worker) = protocol_server(
                backend,
                vec![(
                    json!({"subsonic-response":{"status":"ok","similarSongs":{"song":[{"id":"seed"},{"id":"new"}]}}}),
                    None,
                )],
            );
            let api = Api::new(server, Session::default()).unwrap();
            let current = Item {
                server_id: api.server.id,
                ..current
            };
            let songs = api
                .auto_dj(
                    &current,
                    &crate::auto_dj::Config {
                        only_similar: true,
                        allow_duplicates: true,
                        ..Default::default()
                    },
                    &[],
                )
                .unwrap();
            assert_eq!(
                songs
                    .iter()
                    .map(|song| song.id.as_str())
                    .collect::<Vec<_>>(),
                ["new"]
            );
            let request = worker.join().unwrap().pop().unwrap();
            assert!(request.contains("getSimilarSongs.view"));
        }
    }

    #[test]
    fn auto_dj_filters_duplicates_fills_from_genre_and_fetches_complete_albums() {
        use crate::auto_dj::{Config, Mode, Strategy};
        let ok = |songs: Value, key: &str| json!({"subsonic-response":{"status":"ok",key:{"song":songs}}});
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (
                    ok(json!([{"id":"queued"},{"id":"new"}]), "similarSongs"),
                    None,
                ),
                (
                    ok(
                        json!([{"id":"queued"},{"id":"new"},{"id":"other"}]),
                        "randomSongs",
                    ),
                    None,
                ),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let current = Item {
            id: "queued".into(),
            album_id: "existing".into(),
            genre: "Jazz".into(),
            server_id: api.server.id,
            ..Default::default()
        };
        let songs = api
            .auto_dj(
                &current,
                &Config {
                    item_count: 2,
                    ..Default::default()
                },
                std::slice::from_ref(&current),
            )
            .unwrap();
        assert_eq!(songs.len(), 2);
        assert!(songs.iter().all(|song| song.id != "queued"));
        assert_ne!(songs[0].id, songs[1].id);
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("getSimilarSongs.view") && requests[0].contains("count=2"));
        assert!(requests[1].contains("getRandomSongs") && requests[1].contains("genre=Jazz"));
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (json!([{"id":"existing"},{"id":"album-new"}]), Some(2)),
                (
                    json!([{"id":"track-one","albumId":"album-new"},{"id":"track-two","albumId":"album-new"}]),
                    Some(2),
                ),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let current = Item {
            server_id: api.server.id,
            ..current
        };
        let songs = api
            .auto_dj(
                &current,
                &Config {
                    item_count: 2,
                    mode: Mode::Albums,
                    album_strategy: Strategy::LibraryRandom,
                    ..Default::default()
                },
                std::slice::from_ref(&current),
            )
            .unwrap();
        assert_eq!(songs.len(), 2);
        assert!(songs.iter().all(|song| song.album_id == "album-new"));
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("/api/album?") && requests[0].contains("_sort=random"));
        assert!(requests[1].contains("album_id=album-new"));
    }

    #[test]
    fn playlist_folder_rename_preserves_owner_sync_rules_and_description() {
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (json!([{ "id":"u", "userName":"Owner" }]), Some(1)),
                (json!({}), None),
            ],
        );
        let api = Api::new(
            server,
            Session {
                is_admin: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            api.playlist_users().unwrap(),
            vec![("u".into(), "Owner".into())]
        );
        let rules = json!({"all":[{"contains":{"genre":"Jazz"}}],"sort":"+name","limit":10});
        let item = api.normalize(&json!({"id":"p", "name":"Jazz/Live", "ownerId":"u", "ownerName":"Owner", "sync":true, "public":true, "comment":"Preserved", "rules":rules, "updatedAt":"revision"}), ItemKind::Playlist);
        assert_eq!(item.cover_revision, "revision");
        let name = crate::playlist_folders::move_playlist(&item.name, "Archive", "/");
        api.update_playlist(&item, &name, &item.comment, item.public, item.rules.clone())
            .unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("GET /music/api/user?"));
        let body: Value =
            serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(
            body,
            json!({"name":"Archive/Live", "ownerId":"u", "sync":true, "public":true, "comment":"Preserved", "rules":rules})
        );
    }

    #[test]
    fn image_upload_renews_multipart_credentials_and_radio_caches_are_independent() {
        let (server, worker) = protocol_status_server(
            ServerType::Navidrome,
            vec![
                (401, json!({}), None, None),
                (
                    200,
                    json!({"token":"renewed-image", "id":"user", "subsonicSalt":"salt", "subsonicToken":"ss"}),
                    None,
                    None,
                ),
                (200, json!({}), None, Some("rotated-image")),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "expired-image".into(),
                refresh_password: Some("image-password".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let station = Item {
            id: "station1".into(),
            kind: ItemKind::Radio,
            server_id: api.server.id,
            ..Default::default()
        };
        api.upload_image(&station, b"GIF89a-test-data".to_vec())
            .unwrap();
        assert_eq!(api.session().token, "rotated-image");
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 3);
        for index in [0, 2] {
            assert!(requests[index].starts_with("POST /music/api/radio/station1/image"));
            assert!(requests[index].contains("GIF89a-test-data"));
            assert!(requests[index].contains("name=\"image\""));
        }
        assert!(requests[2].contains("Bearer renewed-image"));
        let other = Item {
            id: "station2".into(),
            ..station.clone()
        };
        assert_ne!(artwork_key(&station), artwork_key(&other));
        let revision = Item {
            cover_revision: "2026-10-10".into(),
            ..station.clone()
        };
        assert_ne!(artwork_key(&station), artwork_key(&revision));
        let normalized = api.normalize(&json!({"id":"station1","name":"Radio","uploadedImage":"custom","updatedAt":"2026-10-10"}), ItemKind::Radio);
        assert_eq!(normalized.cover_revision, "2026-10-10");
    }

    #[test]
    fn transcoding_optional_limits_and_formats_match_original_controllers() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/transcoding-reference.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let jellyfin = case["backend"] == "jf";
            let api = Api::new(
                Server {
                    id: Uuid::new_v4(),
                    name: "Fixture".into(),
                    url: "http://localhost:1/music".into(),
                    username: "test".into(),
                    kind: if jellyfin {
                        ServerType::Jellyfin
                    } else {
                        ServerType::Subsonic
                    },
                    legacy_auth: false,
                    options: Default::default(),
                },
                Session::default(),
            )
            .unwrap();
            let item = Item {
                id: "song".into(),
                server_id: api.server.id,
                container: "source".into(),
                column_data: [("sampleRate".into(), "192000".into())].into(),
                ..Default::default()
            };
            let settings = crate::storage::Settings {
                transcode: true,
                format: case["format"].as_str().unwrap().into(),
                bitrate: serde_json::from_value(case["bitrate"].clone()).unwrap(),
                max_sample_rate: serde_json::from_value(case["rate"].clone()).unwrap(),
                ..Default::default()
            };
            let url = Url::parse(&api.stream_url(&item, &settings).unwrap()).unwrap();
            assert_eq!(url.path(), case["path"].as_str().unwrap());
            let keys = if jellyfin {
                &["audiocodec", "audiobitrate", "audiosamplerate", "static"][..]
            } else {
                &["format", "maxbitrate"][..]
            };
            let params = url
                .query_pairs()
                .map(|(key, value)| (key.to_ascii_lowercase(), value.into_owned()))
                .filter(|(key, _)| keys.contains(&key.as_str()))
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(
                serde_json::to_value(params).unwrap(),
                case["params"],
                "{case}"
            );
        }
    }

    #[test]
    fn jellyfin_transcoding_caps_lossy_rates_and_keeps_lossless_source_rates() {
        let api = Api::new(
            Server {
                id: Uuid::new_v4(),
                name: "Fixture".into(),
                url: "http://localhost:1/music".into(),
                username: "test".into(),
                kind: ServerType::Jellyfin,
                legacy_auth: false,
                options: Default::default(),
            },
            Session::default(),
        )
        .unwrap();
        let mut item = Item {
            server_id: api.server.id,
            id: "song".into(),
            column_data: std::collections::BTreeMap::from([("sampleRate".into(), "192000".into())]),
            ..Default::default()
        };
        for (format, cap, expected) in [
            ("mp3", 96000, Some("48000")),
            ("opus", 192000, Some("48000")),
            ("aac", 44100, Some("44100")),
            ("flac", 96000, Some("96000")),
            ("wav", 192000, None),
            ("flac", 0, None),
        ] {
            let settings = crate::storage::Settings {
                transcode: true,
                max_sample_rate: Some(cap),
                format: format.into(),
                ..Default::default()
            };
            let url = Url::parse(&api.stream_url(&item, &settings).unwrap()).unwrap();
            let params = url
                .query_pairs()
                .collect::<std::collections::HashMap<_, _>>();
            assert_eq!(
                params.get("AudioSampleRate").map(|rate| rate.as_ref()),
                expected,
                "{format}:{cap}"
            );
            assert_eq!(params["Static"], "false");
        }
        item.column_data.insert("sampleRate".into(), "44100".into());
        let settings = crate::storage::Settings {
            transcode: true,
            max_sample_rate: Some(96000),
            ..Default::default()
        };
        assert!(
            !api.stream_url(&item, &settings)
                .unwrap()
                .contains("AudioSampleRate")
        );
        let url = api
            .stream_url(
                &item,
                &crate::storage::Settings {
                    transcode: false,
                    ..settings
                },
            )
            .unwrap();
        assert!(url.contains("Items/song/Download") && !url.contains("AudioSampleRate"));
    }

    #[test]
    fn restored_session_checks_refresh_permissions_and_classify_failures_without_losing_credentials()
     {
        for kind in ServerType::ALL {
            let responses = [true, false].into_iter().map(|admin| {
                let body = if kind == ServerType::Jellyfin {
                    json!({"Id":"account","Name":"listener","Policy":{"IsAdministrator":admin}})
                } else {
                    json!({"subsonic-response":{"status":"ok","user":{"username":"listener","adminRole":admin}}})
                };
                (200, body, None, None)
            }).collect();
            let (server, worker) = protocol_status_server(kind, responses);
            let api = Api::new(
                server,
                Session {
                    token: "private-token".into(),
                    user_id: "account".into(),
                    subsonic_token: "private-subsonic-token".into(),
                    salt: "salt".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let clone = api.clone();
            for admin in [true, false] {
                api.validate_session().unwrap();
                assert_eq!(clone.session().is_admin, admin);
                assert_eq!(clone.session().token, "private-token");
                assert_eq!(clone.session().subsonic_token, "private-subsonic-token");
                assert!(!clone.is_signed_out());
            }
            for request in worker.join().unwrap() {
                if kind == ServerType::Jellyfin {
                    assert!(
                        request.starts_with("GET /music/Users/account "),
                        "{request}"
                    );
                } else {
                    assert!(
                        request.starts_with("GET /music/rest/getUser.view?"),
                        "{request}"
                    );
                    let url = Url::parse(&format!(
                        "http://localhost{}",
                        request.split_whitespace().nth(1).unwrap()
                    ))
                    .unwrap();
                    assert!(
                        url.query_pairs()
                            .any(|(key, value)| key == "username" && value == api.server.username)
                    );
                }
            }
        }
        for status in [200, 401, 403, 500] {
            let (server, worker) =
                protocol_status_server(ServerType::Jellyfin, vec![(status, json!({}), None, None)]);
            let api = Api::new(
                server,
                Session {
                    token: "private-token".into(),
                    user_id: "account".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let error = api.validate_session().unwrap_err();
            assert!(!is_network_error(&error));
            assert!(!error.to_string().contains("private-token"));
            assert_eq!(api.is_signed_out(), status == 401);
            assert_eq!(api.session().token.is_empty(), status == 401);
            assert_eq!(worker.join().unwrap().len(), 1);
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut server = server(ServerType::Jellyfin);
        server.url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let api = Api::new(
            server,
            Session {
                token: "private-token".into(),
                user_id: "account".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let error = api.validate_session().unwrap_err();
        assert!(is_network_error(&error));
        assert!(!api.is_signed_out());
        assert_eq!(api.session().token, "private-token");
        assert!(!error.to_string().contains("private-token"));
    }

    #[test]
    fn jellyfin_saved_password_renews_account_requests_and_uploads_without_reviving_sign_out() {
        for initial_status in [401, 403] {
            for renewed_user in ["account", "recreated-account"] {
                let (server, worker) = protocol_status_server(
                    ServerType::Jellyfin,
                    vec![
                        (initial_status, json!({}), None, None),
                        (
                            200,
                            json!({"AccessToken":"renewed-token","User":{"Id":renewed_user,"Policy":{"IsAdministrator":true}}}),
                            None,
                            None,
                        ),
                        (
                            200,
                            json!({"Id":renewed_user,"Policy":{"IsAdministrator":true}}),
                            None,
                            None,
                        ),
                    ],
                );
                let api = Api::new(
                    server,
                    Session {
                        token: "expired-token".into(),
                        user_id: "account".into(),
                        refresh_password: Some("fixture-password".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
                let shared = api.clone();
                api.validate_session().unwrap();
                assert_eq!(shared.session().token, "renewed-token");
                assert_eq!(shared.session().user_id, renewed_user);
                assert!(shared.session().is_admin);
                assert_eq!(
                    shared.session().refresh_password.as_deref(),
                    Some("fixture-password")
                );
                let requests = worker.join().unwrap();
                assert!(requests[0].starts_with("GET /music/Users/account "));
                assert!(requests[0].contains("Token=\"expired-token\""));
                assert!(requests[1].starts_with("POST /music/Users/AuthenticateByName "));
                let (headers, body) = requests[1].split_once("\r\n\r\n").unwrap();
                assert!(!headers.contains("Token=") && !headers.contains("fixture-password"));
                assert_eq!(
                    serde_json::from_str::<Value>(body).unwrap(),
                    json!({"Username":api.server.username,"Pw":"fixture-password"})
                );
                assert!(requests[2].starts_with(&format!("GET /music/Users/{renewed_user} ")));
                assert!(requests[2].contains("Token=\"renewed-token\""));
                shared.sign_out();
                assert!(api.validate_session().is_err());
                assert!(api.session().refresh_password.is_none());
            }
        }
        for status in [401, 500] {
            let (server, worker) = protocol_status_server(
                ServerType::Jellyfin,
                vec![
                    (401, json!({}), None, None),
                    (status, json!({}), None, None),
                ],
            );
            let api = Api::new(
                server,
                Session {
                    token: "expired-token".into(),
                    user_id: "account".into(),
                    refresh_password: Some("fixture-password".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            let error = api.validate_session().unwrap_err();
            assert!(!error.to_string().contains("fixture-password"));
            assert_eq!(api.is_signed_out(), status == 401);
            assert_eq!(api.session().refresh_password.is_none(), status == 401);
            assert_eq!(worker.join().unwrap().len(), 2);
        }
        let (server, worker) =
            protocol_status_server(ServerType::Jellyfin, vec![(401, json!({}), None, None)]);
        let api = Api::new(
            server,
            Session {
                token: "expired-token".into(),
                user_id: "account".into(),
                refresh_password: Some("fixture-password".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let error = api.validate_session().unwrap_err();
        assert!(is_network_error(&error));
        assert!(!api.is_signed_out());
        assert_eq!(
            api.session().refresh_password.as_deref(),
            Some("fixture-password")
        );
        assert_eq!(worker.join().unwrap().len(), 1);

        for upload in [false, true] {
            let mut responses = vec![(if upload { 401 } else { 403 }, json!({}), None, None)];
            if upload {
                responses.extend([
                    (
                        200,
                        json!({"AccessToken":"renewed-token","User":{"Id":"account"}}),
                        None,
                        None,
                    ),
                    (204, json!({}), None, None),
                ]);
            }
            let (server, worker) = protocol_status_server(ServerType::Jellyfin, responses);
            let api = Api::new(
                server,
                Session {
                    token: "expired-token".into(),
                    user_id: "account".into(),
                    refresh_password: Some("fixture-password".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            if upload {
                api.upload_image(
                    &Item {
                        id: "artist".into(),
                        kind: ItemKind::Artist,
                        server_id: api.server.id,
                        ..Default::default()
                    },
                    b"GIF89a-fixture".to_vec(),
                )
                .unwrap();
                assert_eq!(api.session().token, "renewed-token");
            } else {
                assert!(api.list(&ListQuery::default()).is_err());
                assert_eq!(api.session().token, "expired-token");
            }
            assert!(!api.is_signed_out());
            let requests = worker.join().unwrap();
            assert_eq!(requests.len(), if upload { 3 } else { 1 });
            if upload {
                assert_eq!(
                    requests[0].split_once("\r\n\r\n").unwrap().1,
                    requests[2].split_once("\r\n\r\n").unwrap().1
                );
            }
        }
    }

    #[test]
    fn authentication_expiry_revokes_clients_but_permission_failures_keep_the_session() {
        for (status, expired) in [(401, true), (403, false)] {
            for image_upload in [false, true] {
                let (server, worker) = protocol_status_server(
                    ServerType::Jellyfin,
                    vec![(status, json!({}), None, None)],
                );
                let api = Api::new(
                    server,
                    Session {
                        token: "private-token".into(),
                        user_id: "user".into(),
                        ..Default::default()
                    },
                )
                .unwrap();
                let clone = api.clone();
                let result = if image_upload {
                    api.upload_image(
                        &Item {
                            id: "artist".into(),
                            kind: ItemKind::Artist,
                            server_id: api.server.id,
                            ..Default::default()
                        },
                        b"GIF89a-test-data".to_vec(),
                    )
                } else {
                    api.request(Method::GET, "Items", &[], None).map(|_| ())
                };
                let error = result.unwrap_err().to_string();
                assert_eq!(clone.is_signed_out(), expired);
                assert!(!error.contains("private-token"));
                if expired {
                    assert!(error.contains("Sign in again"));
                    assert!(clone.session().token.is_empty());
                    assert!(clone.session().user_id.is_empty());
                    assert!(clone.request(Method::GET, "Items", &[], None).is_err());
                } else {
                    assert!(error.contains("Access denied"));
                    assert_eq!(clone.session().token, "private-token");
                }
                assert_eq!(worker.join().unwrap().len(), 1);
            }
        }
        let (server, worker) =
            protocol_status_server(ServerType::Jellyfin, vec![(401, json!({}), None, None)]);
        let anonymous = Api::new(server, Session::default()).unwrap();
        assert!(
            anonymous
                .initiate_quick_connect()
                .err()
                .unwrap()
                .to_string()
                .contains("not active")
        );
        assert!(!anonymous.is_signed_out());
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn authentication_expiry_retains_navidrome_credentials_on_transient_renewal_errors() {
        for renewal_status in [None, Some(401), Some(500)] {
            let mut responses = vec![(401, json!({}), None, None)];
            if let Some(status) = renewal_status {
                responses.push((status, json!({}), None, None));
            }
            let (server, worker) = protocol_status_server(ServerType::Navidrome, responses);
            let api = Api::new(
                server,
                Session {
                    token: "expired-token".into(),
                    salt: "salt".into(),
                    subsonic_token: "subsonic-token".into(),
                    refresh_password: renewal_status.map(|_| "private-password".into()),
                    ..Default::default()
                },
            )
            .unwrap();
            let clone = api.clone();
            let error = api.list(&ListQuery::default()).unwrap_err().to_string();
            assert!(!error.contains("private-password") && !error.contains("expired-token"));
            let expired = renewal_status != Some(500);
            assert_eq!(clone.is_signed_out(), expired);
            if expired {
                assert!(error.contains("Sign in again"));
                assert!(clone.session().token.is_empty());
                assert!(clone.session().refresh_password.is_none());
            } else {
                assert!(error.contains("HTTP 500"));
                assert_eq!(clone.session().token, "expired-token");
                assert_eq!(
                    clone.session().refresh_password.as_deref(),
                    Some("private-password")
                );
            }
            assert_eq!(
                worker.join().unwrap().len(),
                if renewal_status.is_some() { 2 } else { 1 }
            );
        }
    }

    #[test]
    fn authentication_expiry_handles_string_subsonic_codes_and_unlinked_provider_fallbacks() {
        for code in [
            json!(40),
            json!("40"),
            json!(" 40.0 "),
            json!(50),
            json!(0),
            json!("0"),
        ] {
            let (server, worker) = protocol_status_server(
                ServerType::Subsonic,
                vec![(
                    200,
                    json!({"subsonic-response":{
                        "status":"failed", "error":{"code":code,"message":"Server error"}
                    }}),
                    None,
                    None,
                )],
            );
            let api = Api::new(
                server,
                Session {
                    subsonic_token: "private-subsonic-token".into(),
                    salt: "salt".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let clone = api.clone();
            let result = api.artist_info(&Item {
                id: "artist".into(),
                kind: ItemKind::Artist,
                server_id: api.server.id,
                ..Default::default()
            });
            let expired = code == json!(40) || code == json!("40") || code == json!(" 40.0 ");
            assert_eq!(clone.is_signed_out(), expired, "{code}");
            if code == json!(0) {
                let fallback = result.unwrap();
                assert!(fallback.biography.is_empty() && fallback.similar.is_empty());
            } else {
                let error = result.unwrap_err().to_string();
                assert!(error.contains("Subsonic error"));
                assert!(!error.contains("private-subsonic-token"));
            }
            if expired {
                assert!(clone.session().subsonic_token.is_empty());
            }
            assert_eq!(worker.join().unwrap().len(), 1);
        }
    }

    #[test]
    fn navidrome_renews_once_rotates_shared_credentials_and_preserves_mutation_body() {
        let (server, worker) = protocol_status_server(
            ServerType::Navidrome,
            vec![
                (401, json!({}), None, None),
                (
                    200,
                    json!({"token":"renewed", "id":"user", "subsonicSalt":"new-salt", "subsonicToken":"new-ss"}),
                    None,
                    None,
                ),
                (200, json!({}), None, Some("rotated")),
                (200, json!([]), Some(0), None),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "expired".into(),
                salt: "old".into(),
                subsonic_token: "old-ss".into(),
                refresh_password: Some("private-password".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let clone = api.clone();
        api.request(
            Method::PUT,
            "api/song/song",
            &[],
            Some(json!({"starred":true})),
        )
        .unwrap();
        clone.list(&ListQuery::default()).unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("Bearer expired"));
        assert!(requests[1].starts_with("POST /music/auth/login"));
        assert!(requests[1].contains("private-password"));
        assert!(requests[2].contains("Bearer renewed"));
        assert!(requests[3].contains("Bearer rotated"));
        assert_eq!(
            requests[0].split("\r\n\r\n").nth(1),
            requests[2].split("\r\n\r\n").nth(1)
        );
        assert_eq!(clone.session().salt, "new-salt");
        assert_eq!(clone.session().subsonic_token, "new-ss");
        assert!(
            !clone
                .stream_url(
                    &Item {
                        server_id: api.server.id,
                        id: "song".into(),
                        ..Default::default()
                    },
                    &Default::default()
                )
                .unwrap()
                .contains("private-password")
        );

        let (server, worker) = protocol_status_server(
            ServerType::Navidrome,
            vec![
                (401, json!({}), None, None),
                (
                    200,
                    json!({"token":"still-invalid", "id":"user", "subsonicSalt":"s", "subsonicToken":"t"}),
                    None,
                    None,
                ),
                (401, json!({}), None, None),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "old".into(),
                refresh_password: Some("private".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let error = api.list(&ListQuery::default()).unwrap_err().to_string();
        assert!(error.contains("Sign in again"));
        assert!(!error.contains("private"));
        assert!(api.is_signed_out());
        assert_eq!(worker.join().unwrap().len(), 3);

        let (server, worker) =
            protocol_status_server(ServerType::Navidrome, vec![(403, json!({}), None, None)]);
        let api = Api::new(
            server,
            Session {
                refresh_password: Some("private".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(api.list(&ListQuery::default()).is_err());
        assert_eq!(worker.join().unwrap().len(), 1);
    }

    #[test]
    fn jellyfin_track_radio_falls_back_to_instant_mix_after_similar_endpoint_errors() {
        let (server, worker) = protocol_status_server(
            ServerType::Jellyfin,
            vec![
                (
                    500,
                    json!({"Message":"Similar songs unavailable"}),
                    None,
                    None,
                ),
                (
                    200,
                    json!({"Items":[{"Id":"source", "Name":"Source"}, {"Id":"related", "Name":"Related"}]}),
                    None,
                    None,
                ),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "token".into(),
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let songs = api
            .random_songs(
                20,
                Some(&Item {
                    server_id: api.server.id,
                    id: "source".into(),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id, "related");
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("GET /music/Items/source/Similar?"));
        assert!(requests[1].starts_with("GET /music/Items/source/InstantMix?"));
    }

    #[test]
    fn replacing_playlist_tracks_preserves_duplicates_on_every_backend() {
        for kind in [
            ServerType::Navidrome,
            ServerType::Subsonic,
            ServerType::Jellyfin,
        ] {
            let response = if kind == ServerType::Jellyfin {
                json!({})
            } else {
                json!({"subsonic-response":{"status":"ok", "playlist":{"id":"playlist"}}})
            };
            let (server, worker) = protocol_server(kind, vec![(response, None)]);
            let api = Api::new(
                server,
                Session {
                    salt: "salt".into(),
                    subsonic_token: "token".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let playlist = Item {
                id: "playlist".into(),
                server_id: api.server.id,
                kind: ItemKind::Playlist,
                ..Default::default()
            };
            let song = Item {
                id: "song".into(),
                server_id: api.server.id,
                ..Default::default()
            };
            api.replace_playlist_tracks(&playlist, &[song.clone(), song.clone()])
                .unwrap();
            let request = worker.join().unwrap().remove(0);
            if kind == ServerType::Jellyfin {
                assert!(request.starts_with("POST /music/Playlists/playlist"));
                assert_eq!(
                    serde_json::from_str::<Value>(request.split("\r\n\r\n").nth(1).unwrap())
                        .unwrap()["Ids"],
                    json!(["song", "song"])
                );
            } else {
                let url = Url::parse(&format!(
                    "http://test{}",
                    request.split_whitespace().nth(1).unwrap()
                ))
                .unwrap();
                assert!(url.path().ends_with("/rest/createPlaylist.view"));
                assert_eq!(
                    url.query_pairs()
                        .filter(|(key, _)| key == "songId")
                        .map(|(_, value)| value.into_owned())
                        .collect::<Vec<_>>(),
                    vec!["song", "song"]
                );
                assert!(
                    url.query_pairs()
                        .any(|(key, value)| key == "playlistId" && value == "playlist")
                );
            }
            assert!(
                api.replace_playlist_tracks(
                    &playlist,
                    &[Item {
                        server_id: Uuid::new_v4(),
                        ..song
                    }]
                )
                .unwrap_err()
                .to_string()
                .contains("same server")
            );
        }
    }

    #[test]
    fn authenticates_all_backends_and_preserves_duplicate_playlist_entries() {
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (
                    json!({"token":"nd-token","id":"user","subsonicSalt":"salt","subsonicToken":"ss-token","isAdmin":true}),
                    None,
                ),
                (json!({"id":"playlist"}), None),
                (json!({}), None),
                (
                    json!([{"id":"entry1","mediaFileId":"song","title":"First"},{"id":"entry2","mediaFileId":"song","title":"First"}]),
                    Some(2),
                ),
                (json!({}), None),
            ],
        );
        let (api, session) = Api::authenticate(server, "test-password").unwrap();
        assert!(session.is_admin);
        assert_eq!(session.subsonic_token, "ss-token");
        let song = Item {
            id: "song".into(),
            server_id: api.server.id,
            ..Item::default()
        };
        let id = api
            .create_playlist("Duplicates", false, None, &[song.clone(), song])
            .unwrap();
        let playlist = Item {
            id,
            kind: ItemKind::Playlist,
            server_id: api.server.id,
            ..Item::default()
        };
        let tracks = api.all_tracks(&playlist).unwrap();
        assert_eq!(tracks[0].id, tracks[1].id);
        assert_ne!(tracks[0].playlist_entry_id, tracks[1].playlist_entry_id);
        api.remove_playlist_track(&playlist, &tracks[1], 1).unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].starts_with("POST /music/auth/login"));
        assert!(requests[2].ends_with(r#"{"ids":["song","song"]}"#));
        assert!(requests[4].contains("id=entry2"));

        let (server, worker) = protocol_server(
            ServerType::Subsonic,
            vec![(
                json!({"subsonic-response":{"status":"ok","user":{"adminRole":true}}}),
                None,
            )],
        );
        let (_, session) = Api::authenticate(server, "p & +").unwrap();
        let request = worker.join().unwrap().remove(0);
        let target = request.split_whitespace().nth(1).unwrap();
        let url = Url::parse(&format!("http://test{target}")).unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            params["t"],
            format!("{:x}", md5::compute(format!("p & +{}", params["s"])))
        );
        assert_eq!(session.salt, params["s"]);
        assert!(!request.contains("p+%26"));

        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![(
                json!({"AccessToken":"jf-token","User":{"Id":"jf-user","Policy":{"IsAdministrator":true}}}),
                None,
            )],
        );
        let (_, session) = Api::authenticate(server, "test-password").unwrap();
        assert_eq!(session.token, "jf-token");
        assert_eq!(session.user_id, "jf-user");
        assert_eq!(session.refresh_password.as_deref(), Some("test-password"));
        let request = worker.join().unwrap().remove(0);
        assert!(request.starts_with("POST /music/Users/AuthenticateByName"));
        assert!(request.contains("MediaBrowser Client=\"Feishin\""));
    }

    #[test]
    fn fetches_every_page_and_traverses_music_folder_indexes() {
        let first = (0..500)
            .map(|n| json!({"id":format!("s{n}"),"title":"Track"}))
            .collect::<Vec<_>>();
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (json!(first), Some(501)),
                (json!([{"id":"last","title":"Last"}]), Some(501)),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "token".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let items = api
            .all_items(ListQuery {
                kind: ItemKind::Song,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(items.len(), 501);
        assert_eq!(items[500].id, "last");
        assert!(worker.join().unwrap()[1].contains("_start=500"));
        let (server, worker) = protocol_server(
            ServerType::Subsonic,
            vec![
                (
                    json!({"subsonic-response":{"status":"ok","musicFolders":{"musicFolder":[{"id":1,"name":"Library"}]}}}),
                    None,
                ),
                (
                    json!({"subsonic-response":{"status":"ok","indexes":{"index":[{"artist":[{"id":"directory","name":"Artist folder"}]}]}}}),
                    None,
                ),
                (
                    json!({"subsonic-response":{"status":"ok","directory":{"child":[{"id":"song","title":"Song","isDir":false}]}}}),
                    None,
                ),
            ],
        );
        let api = Api::new(server, Session::default()).unwrap();
        let root = api
            .list(&ListQuery {
                kind: ItemKind::Folder,
                ..Default::default()
            })
            .unwrap()
            .items
            .remove(0);
        assert!(root.folder_root);
        let songs = api.all_tracks(&root).unwrap();
        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].id, "song");
        let requests = worker.join().unwrap();
        assert!(requests[1].contains("getIndexes.view"));
        assert!(requests[1].contains("musicFolderId=1"));
        assert!(requests[2].contains("getMusicDirectory.view"));
    }

    #[test]
    fn server_extensions_keep_duplicate_queue_index_and_post_parameters() {
        for kind in [ServerType::Subsonic, ServerType::Navidrome] {
            let native = kind == ServerType::Navidrome;
            let extensions = if native {
                vec![]
            } else {
                vec![
                    json!({"name":"indexBasedQueue","versions":[1]}),
                    json!({"name":"formPost","versions":[1]}),
                    json!({"name":"playbackReport","versions":[1]}),
                ]
            };
            let songs = vec![
                json!({"id":"duplicate","title":"Track"}),
                json!({"id":"duplicate","title":"Track"}),
            ];
            let loaded = if native {
                json!({"items":songs,"current":1,"position":12500})
            } else {
                json!({"subsonic-response":{"status":"ok","playQueueByIndex":{"entry":songs,"currentIndex":1,"position":12500}}})
            };
            let mut responses = vec![
                (
                    json!({"subsonic-response":{"status":"ok","serverVersion":if native {"0.57.0"}else{"1.0.0"}}}),
                    None,
                ),
                (
                    json!({"subsonic-response":{"status":"ok","openSubsonicExtensions":extensions}}),
                    None,
                ),
                (json!({"subsonic-response":{"status":"ok"}}), None),
                (loaded, None),
            ];
            if !native {
                responses.insert(2, (json!({"subsonic-response":{"status":"ok","jukeboxStatus":{"playing":false,"gain":1,"position":0,"currentIndex":-1}}}), None));
                responses
                    .extend((0..3).map(|_| (json!({"subsonic-response":{"status":"ok"}}), None)));
            }
            let (server, worker) = protocol_server(kind, responses);
            let api = Api::new(
                server,
                Session {
                    token: "nd-secret".into(),
                    subsonic_token: "ss-token".into(),
                    salt: "salt".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            api.discover_features();
            assert_eq!(api.features().native_queue, native);
            assert_eq!(api.features().index_queue, !native);
            assert!(api.features().jukebox);
            let song = Item {
                id: "duplicate".into(),
                server_id: api.server.id,
                ..Default::default()
            };
            api.save_queue(&[song.clone(), song.clone()], Some(1), 12.5)
                .unwrap();
            let (items, current, position) = api.load_queue().unwrap();
            assert_eq!(items.len(), 2);
            assert_eq!(current, Some(1));
            assert_eq!(position, 12.5);
            if !native {
                api.scrobble(&song, "start", 0.0, 1.5, &[]).unwrap();
            }
            let requests = worker.join().unwrap();
            let requests = if native {
                requests
            } else {
                assert!(
                    requests[2].contains("jukeboxControl.view")
                        && requests[2].contains("action=status")
                );
                requests
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, request)| (index != 2).then_some(request))
                    .collect::<Vec<_>>()
            };
            if native {
                assert!(requests[2].starts_with("POST /music/api/queue"));
                let body: Value =
                    serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(
                    body,
                    json!({"current":1,"ids":["duplicate","duplicate"],"position":12500})
                );
            } else {
                assert!(requests[2].starts_with("POST /music/rest/savePlayQueueByIndex.view HTTP"));
                let body = requests[2].split_once("\r\n\r\n").unwrap().1;
                let pairs = url::form_urlencoded::parse(body.as_bytes()).collect::<Vec<_>>();
                assert_eq!(pairs.iter().filter(|(key, _)| key == "id").count(), 2);
                assert!(
                    pairs
                        .iter()
                        .any(|(key, value)| key == "currentIndex" && value == "1")
                );
                assert!(
                    pairs
                        .iter()
                        .any(|(key, value)| key == "t" && value == "ss-token")
                );
                assert!(requests[3].starts_with("POST /music/rest/getPlayQueueByIndex.view HTTP"));
                assert!(requests[5].contains("state=starting"));
                assert!(requests[6].contains("state=playing"));
                assert!(requests[6].contains("playbackRate=1.5"));
            }
        }
    }

    #[test]
    fn sharing_artwork_radio_and_playlist_mutation_contracts() {
        use base64::Engine;
        let gif = base64::engine::general_purpose::STANDARD
            .decode("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7")
            .unwrap();
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![
                (json!({"id":"link"}), None),
                (json!({"id":"timed"}), None),
                (json!({"status":"ok"}), None),
                (json!({"status":"ok"}), None),
                (Value::Null, None),
                (Value::Null, None),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "private-token".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let song = Item {
            id: "s1".into(),
            server_id: api.server.id,
            ..Default::default()
        };
        let url = api
            .share(
                &[
                    song.clone(),
                    Item {
                        id: "s2".into(),
                        ..song.clone()
                    },
                ],
                "Description",
                true,
                None,
            )
            .unwrap();
        assert!(url.ends_with("/music/share/link"));
        assert!(!url.contains("private-token"));
        assert!(
            api.share(std::slice::from_ref(&song), "", false, Some(1))
                .is_err()
        );
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            + 3_600_000;
        api.share(&[song], "", false, Some(expires)).unwrap();
        let playlist = Item {
            id: "playlist".into(),
            server_id: api.server.id,
            kind: ItemKind::Playlist,
            ..Default::default()
        };
        api.upload_image(&playlist, gif.clone()).unwrap();
        api.remove_image(&playlist).unwrap();
        let station = Item {
            id: "radio".into(),
            kind: ItemKind::Radio,
            ..playlist.clone()
        };
        api.update_radio(
            &station,
            "Station",
            "https://example.com/live",
            "https://example.com",
        )
        .unwrap();
        api.move_playlist_track(
            &playlist,
            &Item {
                playlist_entry_id: "entry".into(),
                ..playlist.clone()
            },
            2,
            0,
        )
        .unwrap();
        let requests = worker.join().unwrap();
        let share: Value =
            serde_json::from_str(requests[0].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(share["resourceIds"], "s1,s2");
        assert_eq!(share["resourceType"], "song");
        assert_eq!(share["downloadable"], true);
        assert!(share.get("expires").is_none());
        let timed: Value =
            serde_json::from_str(requests[1].split_once("\r\n\r\n").unwrap().1).unwrap();
        assert_eq!(timed["expires"], expires);
        assert!(requests[2].starts_with("POST /music/api/playlist/playlist/image"));
        assert!(requests[2].contains("multipart/form-data"));
        assert!(requests[2].contains("name=\"image\""));
        assert!(requests[2].contains("x-nd-authorization: Bearer private-token"));
        assert!(requests[3].starts_with("DELETE /music/api/playlist/playlist/image"));
        assert!(requests[4].contains("\"homePageUrl\":\"https://example.com\""));
        assert!(requests[5].starts_with("PUT /music/api/playlist/playlist/tracks/3"));
        assert!(requests[5].contains("\"insert_before\":\"1\""));

        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (Value::Null, None),
                (Value::Null, None),
                (
                    json!({"Id":"playlist","Name":"Old","Overview":"Keep","Other":"Untouched"}),
                    None,
                ),
                (Value::Null, None),
                (Value::Null, None),
            ],
        );
        let api = Api::new(
            server,
            Session {
                token: "jf-secret".into(),
                user_id: "user".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let playlist = Item {
            server_id: api.server.id,
            ..playlist
        };
        api.upload_image(&playlist, gif.clone()).unwrap();
        api.update_playlist(&playlist, "New", "Comment", true, None)
            .unwrap();
        api.move_playlist_track(
            &playlist,
            &Item {
                playlist_entry_id: "unique-entry".into(),
                ..playlist.clone()
            },
            1,
            0,
        )
        .unwrap();
        let requests = worker.join().unwrap();
        assert!(requests[0].contains("content-type: image/gif"));
        assert!(requests[0].ends_with(&base64::engine::general_purpose::STANDARD.encode(gif)));
        assert!(requests[1].contains("\"IsPublic\":true"));
        assert!(requests[3].contains("\"Other\":\"Untouched\""));
        assert!(
            requests[4].starts_with("POST /music/Playlists/playlist/Items/unique-entry/Move/0")
        );
        let (server, worker) = protocol_server(
            ServerType::Jellyfin,
            vec![
                (Value::Null, None),
                (
                    json!({"Id":"playlist","Name":"New","Overview":"Keep"}),
                    None,
                ),
            ],
        );
        let api = Api::new(
            server,
            Session {
                user_id: "owner".into(),
                ..Default::default()
            },
        )
        .unwrap();
        api.update_playlist(
            &Item {
                server_id: api.server.id,
                ..playlist
            },
            "New",
            "Keep",
            false,
            None,
        )
        .unwrap();
        let requests = worker.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("POST /music/Playlists/playlist"));
        assert!(requests[1].starts_with("GET /music/Users/owner/Items/playlist"));
    }

    #[test]
    fn authenticated_requests_reject_path_injection_and_cross_origin_redirects() {
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        other.set_nonblocking(true).unwrap();
        let origin = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut server = server(ServerType::Navidrome);
        server.url = format!("http://{}", origin.local_addr().unwrap());
        let destination = format!("http://{}/capture", other.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = origin.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut bytes = [0; 4096];
            let n = stream.read(&mut bytes).unwrap();
            assert!(
                String::from_utf8_lossy(&bytes[..n]).contains("x-nd-authorization: Bearer secret")
            );
            write!(
                stream,
                "HTTP/1.1 302 Found\r\nLocation: {destination}\r\nContent-Length: 0\r\n\r\n"
            )
            .unwrap();
        });
        let api = Api::new(
            server,
            Session {
                token: "secret".into(),
                ..Default::default()
            },
        )
        .unwrap();
        for path in [
            "api/song/../secret",
            "api/song/..%2fsecret",
            "api/song/id?injected=true",
            "api/song/id#hidden",
            "api/song/..\\secret",
        ] {
            assert!(api.url(path, &[]).is_err());
        }
        assert!(
            api.list(&ListQuery::default())
                .unwrap_err()
                .to_string()
                .contains("HTTP 302")
        );
        worker.join().unwrap();
        assert_eq!(
            other.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn validates_urls_and_encodes_stream_credentials() {
        assert_eq!(
            normalize_url(" https://example.com/music/// ").unwrap(),
            "https://example.com/music"
        );
        for url in [
            "file:///tmp",
            "https://user:password@host",
            "https://host?token=x",
            "https://host/#fragment",
            "localhost",
        ] {
            assert!(normalize_url(url).is_err());
        }
        let api = Api::new(
            server(ServerType::Subsonic),
            Session {
                salt: "a&b".into(),
                subsonic_token: "h#t".into(),
                ..Session::default()
            },
        )
        .unwrap();
        let item = Item {
            id: "a&b".into(),
            server_id: api.server.id,
            ..Item::default()
        };
        let url = Url::parse(
            &api.stream_url(&item, &crate::storage::Settings::default())
                .unwrap(),
        )
        .unwrap();
        let params: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.path(), "/music/rest/stream.view");
        assert_eq!(params["u"], "a+b & user");
        assert_eq!(params["id"], "a&b");
        assert_eq!(params["t"], "h#t");
        let foreign = Item {
            server_id: Uuid::new_v4(),
            ..item
        };
        assert!(
            api.stream_url(&foreign, &crate::storage::Settings::default())
                .is_err()
        );
    }

    #[test]
    fn navidrome_pagination_sort_and_normalization() {
        let (api, request) = mock(
            ServerType::Navidrome,
            r#"[{"id":"s1","title":"Song","artist":"Artist","duration":123.5,"trackNumber":2,"starred":true}]"#,
        );
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                offset: 100,
                limit: 1,
                search: "a & b".into(),
                sort: Sort::Newest,
                descending: true,
                ..ListQuery::default()
            })
            .unwrap();
        assert_eq!(page.total, Some(101));
        assert!(!page.has_more);
        assert_eq!(page.items[0].duration, 123.5);
        assert_eq!(page.items[0].track, 2);
        let request = request.join().unwrap();
        assert!(request.starts_with("GET /music/api/song?"));
        assert!(request.contains("_start=100"));
        assert!(request.contains("_end=101"));
        assert!(request.contains("title=a+%26+b"));
        assert!(request.contains("x-nd-authorization: Bearer secret"));
    }

    #[test]
    fn jellyfin_ticks_favorites_and_auth_header() {
        let (api, request) = mock(
            ServerType::Jellyfin,
            r#"{"Items":[{"Id":"s1","Name":"Song","RunTimeTicks":1230000000,"Artists":["Artist"],"AlbumId":"a1","UserData":{"IsFavorite":true}}],"TotalRecordCount":1}"#,
        );
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                favorites: true,
                ..ListQuery::default()
            })
            .unwrap();
        assert_eq!(page.items[0].duration, 123.0);
        assert!(page.items[0].favorite);
        assert!(!page.has_more);
        let request = request.join().unwrap();
        assert!(request.contains("IsFavorite=true"));
        assert!(request.contains("Token=\"secret\""));
        let mut book = serde_json::json!({
            "Id": "book", "Type": "AudioBook", "RunTimeTicks": 6000000000u64,
            "UserData": {"PlaybackPositionTicks": 1234500000u64}
        });
        assert_eq!(api.normalize(&book, ItemKind::Song).resume_position, 123.45);
        assert!(api.normalize(&book, ItemKind::Song).resumable);
        book["Type"] = "Audio".into();
        assert_eq!(api.normalize(&book, ItemKind::Song).resume_position, 0.0);
        assert!(!api.normalize(&book, ItemKind::Song).resumable);
        book["Type"] = "AudioBook".into();
        for position in [serde_json::Value::Null, (-10).into(), 6000000000u64.into()] {
            book["UserData"]["PlaybackPositionTicks"] = position;
            assert_eq!(api.normalize(&book, ItemKind::Song).resume_position, 0.0);
            assert!(api.normalize(&book, ItemKind::Song).resumable);
        }
    }

    #[test]
    fn artist_radio_uses_instant_mix_for_jellyfin_and_similar_songs_elsewhere() {
        for kind in [
            ServerType::Jellyfin,
            ServerType::Navidrome,
            ServerType::Subsonic,
        ] {
            let response = if kind == ServerType::Jellyfin {
                r#"{"Items":[{"Id":"song","Type":"Audio","Name":"Mix song"}]}"#
            } else {
                r#"{"subsonic-response":{"status":"ok","similarSongs2":{"song":[{"id":"song","title":"Mix song"}]}}}"#
            };
            let (api, request) = mock(kind, response);
            let artist = Item {
                id: "artist".into(),
                server_id: api.server.id,
                kind: ItemKind::AlbumArtist,
                ..Default::default()
            };
            let songs = api.random_songs(17, Some(&artist)).unwrap();
            assert_eq!(songs.len(), 1);
            assert_eq!(songs[0].id, "song");
            let request = request.join().unwrap();
            if kind == ServerType::Jellyfin {
                assert!(request.starts_with("GET /music/Items/artist/InstantMix?"));
                assert!(request.contains("Limit=17"));
            } else {
                assert!(request.contains("/rest/getSimilarSongs2.view?"));
                assert!(request.contains("count=17") && request.contains("id=artist"));
            }
        }
    }
    #[test]
    fn album_radio_uses_legacy_subsonic_similar_songs_and_jellyfin_instant_mix() {
        for kind in [
            ServerType::Jellyfin,
            ServerType::Navidrome,
            ServerType::Subsonic,
        ] {
            let response = if kind == ServerType::Jellyfin {
                r#"{"Items":[{"Id":"related","Type":"Audio"}]}"#
            } else {
                r#"{"subsonic-response":{"status":"ok","similarSongs":{"song":[{"id":"related"}]}}}"#
            };
            let (api, worker) = mock(kind, response);
            let album = Item {
                id: "album".into(),
                kind: ItemKind::Album,
                server_id: api.server.id,
                ..Default::default()
            };
            assert_eq!(api.random_songs(23, Some(&album)).unwrap()[0].id, "related");
            let request = worker.join().unwrap();
            if kind == ServerType::Jellyfin {
                assert!(
                    request.contains("/Items/album/InstantMix?") && request.contains("Limit=23")
                );
            } else {
                assert!(
                    request.contains("/rest/getSimilarSongs.view?")
                        && request.contains("id=album")
                        && request.contains("count=23")
                );
            }
        }
    }

    #[test]
    fn subsonic_error_envelope_is_not_an_empty_library() {
        let (api, request) = mock(
            ServerType::Subsonic,
            r#"{"subsonic-response":{"status":"failed","error":{"code":40,"message":"Wrong password"}}}"#,
        );
        assert!(
            api.list(&ListQuery::default())
                .unwrap_err()
                .to_string()
                .contains("Wrong password")
        );
        request.join().unwrap();
    }
    #[test]
    fn classic_subsonic_genre_filters_use_server_membership_without_list_metadata() {
        use crate::filters::Filters;
        let first = json!([
            {"id":"a","name":"Alpha","title":"Alpha","year":2026,"starred":"today","artistId":"primary"},
            {"id":"b","name":"Blue","title":"Blue","year":2020,"artistId":"primary"}
        ]);
        let second = json!([
            {"id":"b","name":"Blue","title":"Blue","year":2020,"artistId":"primary"},
            {"id":"c","name":"Crimson","title":"Crimson","year":2024,"artistId":"other"}
        ]);
        for kind in [ItemKind::Album, ItemKind::Song] {
            let response = |items: Value| {
                if kind == ItemKind::Album {
                    json!({"subsonic-response":{"status":"ok","albumList2":{"album":items}}})
                } else {
                    json!({"subsonic-response":{"status":"ok","songsByGenre":{"song":items}}})
                }
            };
            for all in [false, true] {
                let (server, worker) = protocol_server(
                    ServerType::Subsonic,
                    vec![
                        (response(first.clone()), None),
                        (response(second.clone()), None),
                    ],
                );
                let api = Api::new(server, Session::default()).unwrap();
                let page = api
                    .list(&ListQuery {
                        kind,
                        filters: Filters {
                            genre_ids: vec!["Rock".into(), "Jazz".into()],
                            genre_all: all,
                            ..Default::default()
                        },
                        offset: usize::from(!all),
                        limit: 1,
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(
                    page.items
                        .iter()
                        .map(|item| item.id.as_str())
                        .collect::<Vec<_>>(),
                    ["b"]
                );
                assert_eq!(page.total, Some(if all { 1 } else { 3 }));
                assert_eq!(page.has_more, !all);
                let requests = worker.join().unwrap();
                assert!(requests[0].contains("genre=Rock"));
                assert!(requests[1].contains("genre=Jazz"));
                assert!(requests.iter().all(|request| if kind == ItemKind::Album {
                    request.contains("getAlbumList2.view?") && request.contains("type=byGenre")
                } else {
                    request.contains("getSongsByGenre.view?")
                }));
            }
            let (server, worker) =
                protocol_server(ServerType::Subsonic, vec![(response(first.clone()), None)]);
            let api = Api::new(server, Session::default()).unwrap();
            let page = api
                .list(&ListQuery {
                    kind,
                    favorites: true,
                    search: "Alpha".into(),
                    artist_id: Some("primary".into()),
                    filters: Filters {
                        genre_ids: vec!["R&B / Soul?#".into()],
                        min_year: Some(2025),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(page.items[0].id, "a");
            assert_eq!(page.items.len(), 1);
            let request = worker.join().unwrap().remove(0);
            let url = Url::parse(&format!(
                "http://fixture{}",
                request.split_whitespace().nth(1).unwrap()
            ))
            .unwrap();
            assert!(
                url.query_pairs()
                    .any(|(key, value)| key == "genre" && value == "R&B / Soul?#")
            );
        }
    }

    #[test]
    fn artist_favorites_match_original_membership_and_stable_order() {
        let reference: Value =
            serde_json::from_str(include_str!("../assets/artist-favorites-reference.json"))
                .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let backend = if case["backend"] == "navidrome" {
                ServerType::Navidrome
            } else {
                ServerType::Subsonic
            };
            let songs: Vec<_> = reference["songs"].as_array().unwrap().iter().map(|song| {
                let favorite = song["userFavorite"].as_bool().unwrap();
                json!({"id":song["id"],"title":song["id"],"albumId":song["albumId"],"track":song["trackNumber"],"trackNumber":song["trackNumber"],"playCount":song["playCount"],"rating":song["userRating"],"userRating":song["userRating"],"starred":if backend == ServerType::Navidrome {json!(favorite)} else if favorite {json!("2026-01-01")} else {Value::Null}})
            }).collect();
            let responses = if backend == ServerType::Navidrome {
                vec![(json!(songs), Some(songs.len()))]
            } else {
                vec![
                    (
                        json!({"subsonic-response":{"status":"ok","artist":{"album":[{"id":"album"}]}}}),
                        None,
                    ),
                    (
                        json!({"subsonic-response":{"status":"ok","album":{"song":songs}}}),
                        None,
                    ),
                ]
            };
            let (server, worker) = protocol_server(backend, responses);
            let api = Api::new(server, Session::default()).unwrap();
            let artist = Item {
                id: "artist".into(),
                kind: ItemKind::AlbumArtist,
                server_id: api.server.id,
                ..Default::default()
            };
            let tab = if case["type"] == "favorite" {
                ArtistTab::Favorites
            } else {
                ArtistTab::Rated
            };
            let page = api.artist_page(&artist, tab, 0, 100).unwrap();
            let ids = page.items.iter().map(|item| &item.id).collect::<Vec<_>>();
            assert_eq!(json!(ids), case["expected"], "{case}");
            worker.join().unwrap();
        }
    }

    #[test]
    fn artist_favorite_rated_songs_genre_albums_and_legacy_selection_fallbacks() {
        for backend in [
            ServerType::Navidrome,
            ServerType::Jellyfin,
            ServerType::Subsonic,
        ] {
            let raw = match backend {
                ServerType::Navidrome => {
                    json!([{"id":"first","title":"First","starred":false,"rating":5},{"id":"second","title":"Second","starred":true,"rating":0}])
                }
                ServerType::Jellyfin => {
                    json!({"Items":[{"Id":"first","Name":"First","UserData":{"IsFavorite":false,"Rating":5}},{"Id":"second","Name":"Second","UserData":{"IsFavorite":true}}],"TotalRecordCount":2})
                }
                ServerType::Subsonic => {
                    json!({"subsonic-response":{"status":"ok","album":{"song":[{"id":"first","title":"First","userRating":5},{"id":"second","title":"Second","starred":"2026-01-01"}]}}})
                }
            };
            let albums = match backend {
                ServerType::Navidrome => {
                    json!([{"id":"album","name":"Rock album","genres":[{"id":"rock","name":"Rock"}]}])
                }
                ServerType::Jellyfin => {
                    json!({"Items":[{"Id":"album","Name":"Rock album","Type":"MusicAlbum","GenreItems":[{"Id":"rock","Name":"Rock"}]}],"TotalRecordCount":1})
                }
                ServerType::Subsonic => {
                    json!({"subsonic-response":{"status":"ok","albumList2":{"album":[{"id":"album","name":"Rock album","genre":"Rock"}]}}})
                }
            };
            let mut responses = vec![];
            for _ in 0..2 {
                if backend == ServerType::Subsonic {
                    responses.push((json!({"subsonic-response":{"status":"ok","artist":{"album":[{"id":"album","name":"Album"}]}}}),None));
                }
                responses.push((raw.clone(), Some(2)));
            }
            responses.push((albums, Some(1)));
            let (server, worker) = protocol_server(backend, responses);
            let api = Api::new(
                server,
                Session {
                    user_id: "user".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let artist = Item {
                id: "artist".into(),
                kind: ItemKind::Artist,
                server_id: api.server.id,
                ..Default::default()
            };
            assert_eq!(
                api.artist_page(&artist, ArtistTab::Favorites, 0, 100)
                    .unwrap()
                    .items[0]
                    .id,
                "second"
            );
            let rated = api.artist_page(&artist, ArtistTab::Rated, 0, 100).unwrap();
            if backend == ServerType::Jellyfin {
                // The original Jellyfin normalizer has no user ratings.
                assert!(rated.items.is_empty());
            } else {
                assert_eq!(rated.items[0].id, "first");
            }
            let genre = Item {
                id: if backend == ServerType::Subsonic {
                    "Rock"
                } else {
                    "rock"
                }
                .into(),
                name: "Rock".into(),
                kind: ItemKind::Genre,
                server_id: api.server.id,
                ..Default::default()
            };
            assert_eq!(
                api.all_items(api.genre_query(&genre, ListQuery::default()).unwrap())
                    .unwrap()[0]
                    .id,
                "album"
            );
            let requests = worker.join().unwrap();
            let request = requests.last().unwrap();
            match backend {
                ServerType::Navidrome => {
                    assert!(request.contains("api/album") && request.contains("genre_id=rock"))
                }
                ServerType::Jellyfin => assert!(
                    request.contains("IncludeItemTypes=MusicAlbum")
                        && request.contains("GenreIds=rock")
                ),
                ServerType::Subsonic => {
                    assert!(request.contains("type=byGenre") && request.contains("genre=Rock"))
                }
            }
        }
        let (server, worker) = protocol_server(
            ServerType::Navidrome,
            vec![(
                json!([{"id":"one","title":"One","artistId":"first","genre":"Rock"},{"id":"two","title":"Two","artistId":"second","genre":"Jazz"},{"id":"three","title":"Three","artistId":"other","genre":"Pop"}]),
                Some(3),
            )],
        );
        let api = Api::new(server, Session::default()).unwrap();
        api.features.write().unwrap().version = "0.54.5".into();
        let page = api
            .list(&ListQuery {
                kind: ItemKind::Song,
                filters: crate::filters::Filters {
                    artist_ids: vec!["first".into(), "second".into()],
                    genre_ids: vec!["Rock".into(), "Jazz".into()],
                    ..Default::default()
                },
                offset: 1,
                limit: 1,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.total, Some(2));
        assert_eq!(page.items[0].id, "two");
        let request = &worker.join().unwrap()[0];
        assert!(!request.contains("genre_id="));
        assert!(!request.contains("album_artist_id="));
    }
}
