use crate::{
    model::{Item, Page, Server, Session},
    queue::Queue,
};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub settings_tab: crate::settings_search::Tab,
    pub ignore_ssl: bool,
    pub jukebox: bool,
    pub playback_filters: Vec<crate::playback_filters::Filter>,
    pub volume: f32,
    pub muted: bool,
    pub speed: f32,
    pub microtonal_pitch_controls: bool,
    pub transition: crate::transition::Mode,
    pub crossfade_seconds: f64,
    pub crossfade_style: crate::transition::Curve,
    pub audio_device: String,
    pub mpv_audio_device: String,
    pub ffmpeg_path: String,
    pub preserve_pitch: bool,
    pub fade_on_status: bool,
    pub mpv_path: String,
    pub saved_mpv_path: String,
    pub mpv_extra_args: Vec<String>,
    pub mpv_properties: std::collections::BTreeMap<String, serde_json::Value>,
    pub theme: String,
    pub css: crate::styles::Config,
    pub follow_system_theme: bool,
    pub theme_dark: String,
    pub theme_light: String,
    pub accent: String,
    pub use_theme_accent_color: bool,
    pub use_theme_primary_shade: bool,
    pub primary_shade: u8,
    pub language: String,
    pub zoom: f32,
    pub log_level: String,
    pub password_store: crate::credentials::Backend,
    pub tray: bool,
    pub exit_to_tray: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub window_bar_style: String,
    pub window_title_track_info: bool,
    pub media_session: bool,
    pub global_media_hotkeys: bool,
    pub transcode: bool,
    pub bitrate: Option<u32>,
    pub max_sample_rate: Option<u32>,
    pub format: String,
    pub scrobble: bool,
    pub notify_tracks: bool,
    pub confirm_large_fetch: bool,
    pub previous_local_volume: Option<f32>,
    pub scrobble_mode: ScrobbleMode,
    pub scrobble_percentage: f32,
    pub scrobble_seconds: f32,
    pub confirm_queue_changes: bool,
    pub follow_current_song: bool,
    pub resume_queue: bool,
    pub queue_autosave: bool,
    pub queue_autosave_count: u32,
    pub external_links: crate::external_links::Config,
    pub show_favorites: bool,
    pub show_ratings: bool,
    pub enable_grid_multiselect: bool,
    pub blur_explicit_images: bool,
    pub image_placeholder: crate::images::Placeholder,
    pub image_resolution: crate::images::Resolution,
    pub native_image_aspect_ratio: bool,
    pub album_background: bool,
    pub album_background_blur: f32,
    pub artist_background: bool,
    pub artist_background_blur: f32,
    pub fullscreen_auto_open_minutes: f64,
    pub playerbar_open_drawer: bool,
    pub show_time_remaining: bool,
    pub volume_wheel_step: f32,
    pub volume_width: f32,
    pub button_size: f32,
    pub play_button_behavior: crate::queue::PlayBehavior,
    pub artist_radio_count: usize,
    pub album_group: crate::album_metadata::Config,
    pub artist_items: Vec<crate::releases::Setting>,
    pub artist_top_personal: bool,
    pub artist_favorites_rating: bool,
    pub lyric_offset: f64,
    pub lyric_fetch: bool,
    pub lyric_prefer_local: bool,
    pub lyric_alignment: String,
    pub lyric_follow_alignment: f32,
    pub lyric_lead_time: f64,
    pub lyric_show_match: bool,
    pub lyric_show_provider: bool,
    pub lyric_display: crate::lyrics::Display,
    pub lyric_displays: std::collections::BTreeMap<String, crate::lyrics::Display>,
    pub lyric_cloud_translation: crate::translation::Config,
    pub lyric_sources: Vec<crate::lyric_sources::Provider>,
    pub lyric_translation: bool,
    pub lyric_scroll: bool,
    pub lyric_words: bool,
    pub lyric_overlay_layers: Vec<String>,
    pub lyric_furigana: bool,
    pub lyric_romaji: bool,
    pub tag_configs: std::collections::BTreeMap<String, crate::tags::Config>,
    pub tag_rescan: bool,
    pub single_select_filters: Vec<String>,
    pub pinned_filter_views: Vec<String>,
    pub page_sidebars: std::collections::BTreeMap<String, bool>,
    pub views: std::collections::BTreeMap<String, crate::filters::View>,
    pub lists: std::collections::BTreeMap<String, crate::lists::Config>,
    pub collections: Vec<crate::collections::Collection>,
    pub query_fields: Vec<crate::smart::Field>,
    pub visualizer: crate::visualizer::Config,
    pub player_display: crate::player_display::Config,
    pub playerbar: crate::waveform::Config,
    pub home: crate::home::Config,
    pub font: crate::fonts::Config,
    pub sidebar: crate::sidebar::Config,
    pub panels: crate::panels::Config,
    pub updates: crate::updates::Config,
    pub last_release_notes_version: String,
    pub genre_albums: bool,
    pub playlist_albums: bool,
    pub playlist_header_collapsed: bool,
    pub playlist_skip_duplicates: bool,
    pub artist_grouping: String,
    pub artist_release_types: Vec<crate::releases::Setting>,
    pub auto_dj: bool,
    pub auto_dj_config: crate::auto_dj::Config,
    pub shuffle: crate::shuffle::Config,
    pub replay_gain: String,
    #[serde(deserialize_with = "deserialize_equalizer")]
    pub equalizer: [f32; 12],
    pub equalizer_enabled: bool,
    pub equalizer_preamp: f32,
    pub compressor: crate::dsp::Compressor,
    pub equalizer_presets: std::collections::BTreeMap<String, [f32; 12]>,
    pub compressor_presets: std::collections::BTreeMap<String, crate::dsp::Compressor>,
    pub remote_port: u16,
    pub remote_enabled: bool,
    pub remote_username: String,
    pub path_replace: String,
    pub path_replace_with: String,
    pub discord_enabled: bool,
    pub discord_client_id: String,
    pub discord_config: crate::presence::Config,
    pub private_mode: bool,
    pub shortcuts: crate::shortcuts::Bindings,
    pub prevent_sleep: bool,
    pub prevent_suspend: bool,
    pub playlist_folders: bool,
    pub playlist_separator: String,
    pub playlist_folder_view: String,
    pub playlist_folder_indent: u32,
    pub playlist_folder_color: String,
    pub share_days: Option<u32>,
    pub share_expiration: Option<crate::sharing::Expiration>,
    pub share_downloadable: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_tab: Default::default(),
            ignore_ssl: false,
            jukebox: false,
            playback_filters: Vec::new(),
            volume: 50.0,
            muted: false,
            speed: 1.0,
            microtonal_pitch_controls: false,
            transition: crate::transition::Mode::Gapless,
            crossfade_seconds: 5.0,
            crossfade_style: crate::transition::Curve::EqualPower,
            audio_device: String::new(),
            mpv_audio_device: "auto".into(),
            ffmpeg_path: "ffmpeg".into(),
            preserve_pitch: true,
            fade_on_status: true,
            mpv_path: String::new(),
            saved_mpv_path: String::new(),
            mpv_extra_args: vec![],
            mpv_properties: std::collections::BTreeMap::from([
                ("gapless-audio".into(), "weak".into()),
                ("audio-exclusive".into(), false.into()),
                ("replaygain-clip".into(), true.into()),
                ("replaygain-preamp".into(), 0.into()),
            ]),
            theme: "dark".into(),
            css: Default::default(),
            follow_system_theme: false,
            theme_dark: "defaultDark".into(),
            theme_light: "defaultLight".into(),
            accent: "rgb(53, 116, 252)".into(),
            use_theme_accent_color: false,
            use_theme_primary_shade: true,
            primary_shade: 6,
            language: "en".into(),
            zoom: 1.0,
            log_level: "info".into(),
            password_store: Default::default(),
            tray: true,
            exit_to_tray: false,
            minimize_to_tray: false,
            start_minimized: false,
            window_bar_style: "linux".into(),
            window_title_track_info: true,
            media_session: false,
            global_media_hotkeys: true,
            transcode: false,
            bitrate: None,
            max_sample_rate: None,
            format: String::new(),
            scrobble: true,
            notify_tracks: false,
            confirm_large_fetch: true,
            previous_local_volume: None,
            scrobble_mode: ScrobbleMode::Both,
            scrobble_percentage: 75.0,
            scrobble_seconds: 240.0,
            confirm_queue_changes: true,
            follow_current_song: true,
            resume_queue: true,
            queue_autosave: false,
            queue_autosave_count: 10,
            external_links: Default::default(),
            show_favorites: true,
            show_ratings: true,
            enable_grid_multiselect: false,
            blur_explicit_images: false,
            image_placeholder: Default::default(),
            image_resolution: Default::default(),
            native_image_aspect_ratio: false,
            album_background: false,
            album_background_blur: 6.,
            artist_background: true,
            artist_background_blur: 6.,
            fullscreen_auto_open_minutes: 0.,
            playerbar_open_drawer: false,
            show_time_remaining: false,
            volume_wheel_step: 5.,
            volume_width: 70.,
            button_size: 15.,
            play_button_behavior: Default::default(),
            artist_radio_count: 20,
            album_group: Default::default(),
            artist_items: crate::artist_sections::defaults(),
            artist_top_personal: false,
            artist_favorites_rating: false,
            lyric_offset: 0.0,
            lyric_fetch: true,
            lyric_prefer_local: true,
            lyric_alignment: "left".into(),
            lyric_follow_alignment: 0.0,
            lyric_lead_time: 0.8,
            lyric_show_match: false,
            lyric_show_provider: false,
            lyric_display: Default::default(),
            lyric_displays: Default::default(),
            lyric_cloud_translation: Default::default(),
            lyric_sources: vec![
                crate::lyric_sources::Provider::NetEase,
                crate::lyric_sources::Provider::Lrclib,
            ],
            lyric_translation: false,
            lyric_scroll: true,
            lyric_words: true,
            lyric_overlay_layers: vec![],
            lyric_furigana: false,
            lyric_romaji: false,
            tag_configs: crate::tags::default_configs(),
            tag_rescan: true,
            single_select_filters: vec![],
            pinned_filter_views: vec![],
            page_sidebars: [("album".into(), true), ("song".into(), true)].into(),
            views: Default::default(),
            lists: Default::default(),
            collections: vec![],
            query_fields: vec![],
            visualizer: Default::default(),
            player_display: Default::default(),
            playerbar: Default::default(),
            home: Default::default(),
            font: Default::default(),
            sidebar: Default::default(),
            panels: Default::default(),
            updates: Default::default(),
            last_release_notes_version: String::new(),
            genre_albums: false,
            playlist_albums: false,
            playlist_header_collapsed: false,
            playlist_skip_duplicates: true,
            artist_grouping: "primary".into(),
            artist_release_types: crate::releases::defaults(),
            auto_dj: false,
            auto_dj_config: crate::auto_dj::Config::default(),
            shuffle: crate::shuffle::Config::default(),
            replay_gain: "no".into(),
            equalizer: [0.0; 12],
            equalizer_enabled: false,
            equalizer_preamp: 0.0,
            compressor: crate::dsp::Compressor::default(),
            equalizer_presets: Default::default(),
            compressor_presets: Default::default(),
            remote_port: 4333,
            remote_enabled: false,
            remote_username: String::new(),
            path_replace: String::new(),
            path_replace_with: String::new(),
            discord_enabled: false,
            discord_client_id: "1165957668758900787".into(),
            discord_config: Default::default(),
            private_mode: false,
            shortcuts: crate::shortcuts::defaults(),
            prevent_sleep: false,
            prevent_suspend: false,
            playlist_folders: false,
            playlist_separator: "/".into(),
            playlist_folder_view: "tree".into(),
            playlist_folder_indent: 16,
            playlist_folder_color: String::new(),
            share_days: Some(365),
            share_expiration: None,
            share_downloadable: false,
        }
    }
}

impl Settings {
    pub fn set_mpv_path(&mut self, path: String) {
        if !path.is_empty() {
            self.saved_mpv_path = path.clone();
        } else if !self.mpv_path.is_empty() {
            self.saved_mpv_path.clone_from(&self.mpv_path);
        }
        self.mpv_path = path;
    }

    pub fn select_mpv(&mut self, enabled: bool) {
        let path = if !enabled {
            String::new()
        } else if !self.mpv_path.is_empty() {
            self.mpv_path.clone()
        } else if !self.saved_mpv_path.is_empty() {
            self.saved_mpv_path.clone()
        } else {
            "mpv".into()
        };
        self.set_mpv_path(path);
    }

    pub fn validate_transcoding(&self) -> Result<()> {
        anyhow::ensure!(
            self.format.len() <= 64
                && self
                    .format
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')),
            "Use an audio format name containing letters, numbers, underscores or dashes"
        );
        Ok(())
    }

    pub fn resolve_song_path(&self, path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }
        format!(
            "{}{}",
            self.path_replace_with,
            path.strip_prefix(&self.path_replace).unwrap_or(path)
        )
    }
    pub fn lyric_display_for(&self, key: &str) -> &crate::lyrics::Display {
        if key == "default" {
            &self.lyric_display
        } else {
            self.lyric_displays.get(key).unwrap_or(&self.lyric_display)
        }
    }
    pub fn should_scrobble(&self, played: f64, duration: f64) -> bool {
        if !self.scrobble
            || self.private_mode
            || !played.is_finite()
            || played < 0.0
            || !duration.is_finite()
            || duration < 0.0
        {
            return false;
        }
        let percentage = (if duration > 0. {
            played / duration * 100.
        } else {
            0.
        }) >= self.scrobble_percentage as f64;
        let seconds = played >= self.scrobble_seconds as f64;
        match self.scrobble_mode {
            ScrobbleMode::Both => percentage || seconds,
            ScrobbleMode::Percentage => percentage,
            ScrobbleMode::Seconds => seconds,
        }
    }
    pub fn apply_mpv_options(&mut self, text: &str) -> Result<()> {
        anyhow::ensure!(text.len() <= 65_536, "MPV options are too large");
        let value: serde_json::Value =
            serde_json::from_str(text).context("Enter valid MPV options JSON")?;
        let arguments: Vec<String> = serde_json::from_value(value["arguments"].clone())
            .context("MPV arguments must be an array of strings")?;
        let properties: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_value(value["properties"].clone())
                .context("MPV properties must be an object")?;
        anyhow::ensure!(
            arguments.len() <= 128
                && arguments.iter().all(|arg| arg.starts_with("--")
                    && arg.len() <= 4096
                    && !arg.contains(['\0', '\n', '\r'])),
            "Use one complete --option=value per MPV argument"
        );
        anyhow::ensure!(
            properties.len() <= 128
                && properties.iter().all(|(key, value)| !key.is_empty()
                    && key.len() <= 128
                    && key
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-/_.".contains(&byte))
                    && (value.is_string() || value.is_boolean() || value.is_number())),
            "MPV properties need valid names and string, number, or boolean values"
        );
        self.mpv_extra_args = arguments;
        self.mpv_properties = properties;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ScrobbleMode {
    #[default]
    Both,
    Percentage,
    Seconds,
}

fn deserialize_equalizer<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<[f32; 12], D::Error> {
    let values = Vec::<f32>::deserialize(deserializer)?;
    if values.len() == 12 {
        return Ok(values.try_into().unwrap());
    }
    if values.len() == 10 {
        let mut bands = [0.0; 12];
        for (old, new) in [0, 1, 2, 3, 4, 5, 6, 8, 9, 11].into_iter().enumerate() {
            bands[new] = values[old];
        }
        return Ok(bands);
    }
    Err(serde::de::Error::custom("Equalizer needs twelve bands"))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct State {
    pub cache_revision: u64,
    pub servers: Vec<Server>,
    pub active_server: Option<Uuid>,
    #[serde(skip_serializing_if = "std::collections::HashSet::is_empty")]
    pub signed_out_servers: std::collections::HashSet<Uuid>,
    pub settings: Settings,
    pub queue: Queue,
    pub radio: Vec<Item>,
    pub window: Option<WindowPlacement>,
    pub collapsed_playlist_folders:
        std::collections::BTreeMap<Uuid, std::collections::BTreeSet<String>>,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shared_collapsed_playlist_folders:
        std::collections::BTreeMap<Uuid, std::collections::BTreeSet<String>>,
    // Missing fields identify older native profiles, whose folders started open.
    #[serde(default)]
    pub expanded_playlist_folders:
        Option<std::collections::BTreeMap<Uuid, std::collections::BTreeSet<String>>>,
    #[serde(default)]
    pub shared_expanded_playlist_folders:
        Option<std::collections::BTreeMap<Uuid, std::collections::BTreeSet<String>>>,
    pub sidebar_playlist_order: std::collections::BTreeMap<Uuid, Vec<String>>,
    pub command_palette_sections: std::collections::BTreeMap<String, bool>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            cache_revision: 0,
            servers: vec![],
            active_server: None,
            signed_out_servers: Default::default(),
            settings: Settings::default(),
            queue: Queue::default(),
            radio: vec![],
            window: None,
            collapsed_playlist_folders: Default::default(),
            shared_collapsed_playlist_folders: Default::default(),
            expanded_playlist_folders: Some(Default::default()),
            shared_expanded_playlist_folders: Some(Default::default()),
            sidebar_playlist_order: Default::default(),
            command_palette_sections: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct WindowPlacement {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fullscreen: bool,
    pub maximized: bool,
}

impl State {
    pub fn playlist_folder_expanded(&self, server: Uuid, path: &str, shared: bool) -> bool {
        let (expanded, collapsed) = if shared {
            (
                &self.shared_expanded_playlist_folders,
                &self.shared_collapsed_playlist_folders,
            )
        } else {
            (
                &self.expanded_playlist_folders,
                &self.collapsed_playlist_folders,
            )
        };
        match expanded {
            Some(expanded) => expanded
                .get(&server)
                .is_some_and(|paths| paths.contains(path)),
            None => !collapsed
                .get(&server)
                .is_some_and(|paths| paths.contains(path)),
        }
    }
    pub fn toggle_playlist_folder(&mut self, server: Uuid, path: &str, shared: bool) {
        let (expanded, collapsed) = if shared {
            (
                &mut self.shared_expanded_playlist_folders,
                &mut self.shared_collapsed_playlist_folders,
            )
        } else {
            (
                &mut self.expanded_playlist_folders,
                &mut self.collapsed_playlist_folders,
            )
        };
        let paths = expanded
            .as_mut()
            .unwrap_or(collapsed)
            .entry(server)
            .or_default();
        if !paths.remove(path) {
            paths.insert(path.into());
        }
    }
    pub fn radio_page(&self, server_id: Uuid, search: &str, offset: usize, limit: usize) -> Page {
        let items = crate::lists::radio_items(
            self.radio
                .iter()
                .filter(|item| item.server_id == server_id)
                .cloned()
                .collect(),
            search,
            crate::model::Sort::Name,
            false,
        );
        let total = items.len();
        Page {
            has_more: offset.saturating_add(limit) < total,
            total: Some(total),
            items: items.into_iter().skip(offset).take(limit).collect(),
        }
    }
}

pub fn data_dir() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("FEISHIN_DATA_DIR") {
        return Ok(path.into());
    }
    Ok(ProjectDirs::from("org", "jeffvli", "FeishinRust")
        .context("Cannot locate the application data directory")?
        .data_dir()
        .to_owned())
}

pub fn load(path: &Path) -> Result<State> {
    match fs::read(path) {
        Ok(data) => {
            let mut state: State = serde_json::from_slice(&data)
                .context("Settings file is invalid; the original file was preserved")?;
            state.queue.repair();
            state.settings.volume = state.settings.volume.clamp(0.0, 100.0);
            state.settings.speed = state.settings.speed.clamp(0.25, 4.0);
            Ok(state)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(path: &Path, state: &State) -> Result<()> {
    let mut value = serde_json::to_value(state)?;
    if !state.settings.resume_queue {
        value["queue"] = serde_json::to_value(Queue {
            shuffle: state.queue.shuffle,
            repeat: state.queue.repeat,
            ..Default::default()
        })?;
    }
    atomic_write(path, &serde_json::to_vec_pretty(&value)?)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("File must have a parent directory")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[derive(Deserialize, Serialize, PartialEq, Eq)]
struct CredentialAccount {
    kind: crate::model::ServerType,
    username: String,
    url: String,
    remote_url: String,
}
impl CredentialAccount {
    fn for_server(server: &Server) -> Result<Self> {
        Ok(Self {
            kind: server.kind,
            username: server.username.clone(),
            url: crate::api::normalize_url(&server.url)?,
            remote_url: if server.options.remote_url.is_empty() {
                String::new()
            } else {
                crate::api::normalize_url(&server.options.remote_url)?
            },
        })
    }
}

pub fn same_account(left: &Server, right: &Server) -> bool {
    left.id == right.id
        && CredentialAccount::for_server(left)
            .ok()
            .zip(CredentialAccount::for_server(right).ok())
            .is_some_and(|(left, right)| left == right)
}

#[derive(Deserialize, Serialize)]
struct StoredSession {
    account: CredentialAccount,
    session: Session,
}
fn encode_session(server: &Server, session: &Session) -> Result<String> {
    Ok(serde_json::to_string(&StoredSession {
        account: CredentialAccount::for_server(server)?,
        session: session.clone(),
    })?)
}
fn decode_session(server: &Server, value: &str) -> Result<Session> {
    let mut record: StoredSession = serde_json::from_str(value)
        .context("Sign in again to bind stored credentials to this server account")?;
    anyhow::ensure!(
        record.account == CredentialAccount::for_server(server)?,
        "Stored credentials belong to a different server address or account; sign in again"
    );
    if server.kind == crate::model::ServerType::Navidrome && !server.options.save_password {
        record.session.refresh_password = None;
    }
    Ok(record.session)
}

pub fn store_session(server: &Server, session: &Session) -> Result<()> {
    keyring::Entry::new("org.jeffvli.feishin.rust", &server.id.to_string())?
        .set_password(&encode_session(server, session)?)
        .context("Could not save credentials in the OS password store")
}

pub fn load_session(server: &Server) -> Result<Session> {
    let value = keyring::Entry::new("org.jeffvli.feishin.rust", &server.id.to_string())?
        .get_password()
        .context("Sign in to this server again")?;
    decode_session(server, &value)
}

pub fn delete_session(server: &Server) -> Result<()> {
    match keyring::Entry::new("org.jeffvli.feishin.rust", &server.id.to_string())?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn store_remote_credentials(username: &str, password: &str) -> Result<()> {
    keyring::Entry::new("org.jeffvli.feishin.rust", "remote-control")?
        .set_password(&serde_json::to_string(&(username, password))?)
        .context("Could not save remote credentials in the OS password store")
}
pub fn load_remote_credentials() -> Result<(String, String)> {
    let value = keyring::Entry::new("org.jeffvli.feishin.rust", "remote-control")?
        .get_password()
        .context("Enter remote credentials to enable remote control again")?;
    Ok(serde_json::from_str(&value)?)
}

pub fn store_lastfm_key(key: &str) -> Result<()> {
    anyhow::ensure!(key.len() <= 1024, "Last.fm API key is too long");
    let entry = keyring::Entry::new("Feishin Native", "lastfm-api-key")?;
    if key.is_empty() {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(error.into()),
        }
    } else {
        entry.set_password(key).map_err(Into::into)
    }
}
pub fn load_lastfm_key() -> Result<String> {
    match keyring::Entry::new("Feishin Native", "lastfm-api-key")?.get_password() {
        Ok(key) => Ok(key),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(error) => Err(error.into()),
    }
}

pub fn store_translation_key(provider: &str, key: &str) -> Result<()> {
    anyhow::ensure!(
        matches!(provider, "Microsoft Azure" | "Google Cloud")
            && !key.is_empty()
            && key.len() <= 4096
            && !key.chars().any(char::is_control),
        "Invalid translation provider or key"
    );
    keyring::Entry::new(
        "org.jeffvli.feishin.rust",
        &format!("translation-{provider}"),
    )?
    .set_password(key)
    .context("Could not save translation credentials in the OS password store")
}
pub fn load_translation_key(provider: &str) -> Result<String> {
    anyhow::ensure!(
        matches!(provider, "Microsoft Azure" | "Google Cloud"),
        "Invalid translation provider"
    );
    keyring::Entry::new(
        "org.jeffvli.feishin.rust",
        &format!("translation-{provider}"),
    )?
    .get_password()
    .context("Enter a translation API key in Settings")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn song_paths_apply_prefixes_with_and_without_a_matching_replacement() {
        let mut settings = Settings {
            path_replace: "/music/".into(),
            path_replace_with: "/mnt/".into(),
            ..Default::default()
        };
        assert_eq!(
            settings.resolve_song_path("/music/track.flac"),
            "/mnt/track.flac"
        );
        assert_eq!(
            settings.resolve_song_path("relative.flac"),
            "/mnt/relative.flac"
        );
        assert_eq!(settings.resolve_song_path(""), "");
        settings.path_replace.clear();
        assert_eq!(settings.resolve_song_path("track.flac"), "/mnt/track.flac");
        settings.path_replace_with.clear();
        assert_eq!(
            settings.resolve_song_path("/music/track.flac"),
            "/music/track.flac"
        );
    }
    #[test]
    fn stored_credentials_are_bound_to_server_addresses_type_and_account() {
        let mut server = Server {
            id: Uuid::new_v4(),
            name: "Music".into(),
            url: "https://local.example.com/music/".into(),
            username: "user".into(),
            kind: crate::model::ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        server.options.remote_url = "https://remote.example.com/music".into();
        let session = Session {
            token: "private-token".into(),
            ..Default::default()
        };
        let value = encode_session(&server, &session).unwrap();
        let original = server.clone();
        server.name = "Renamed".into();
        server.options.prefer_remote_url = true;
        server.options.ignore_ssl = true;
        server.options.music_folder_ids = vec!["library".into()];
        server.url.pop();
        assert!(same_account(&original, &server));
        assert_eq!(
            decode_session(&server, &value).unwrap().token,
            "private-token"
        );
        for change in 0..4 {
            let mut changed = server.clone();
            match change {
                0 => changed.url = "https://different.example.com".into(),
                1 => changed.options.remote_url = "https://different.example.com".into(),
                2 => changed.username = "different".into(),
                _ => changed.kind = crate::model::ServerType::Jellyfin,
            }
            assert!(!same_account(&original, &changed));
            assert!(decode_session(&changed, &value).is_err());
        }
        let mut changed = server.clone();
        changed.id = Uuid::new_v4();
        assert!(!same_account(&original, &changed));
        changed = server.clone();
        changed.url = "invalid".into();
        assert!(!same_account(&changed, &changed));
        assert!(decode_session(&server, &serde_json::to_string(&session).unwrap()).is_err());
    }

    #[test]
    fn durable_settings_and_corrupt_file_preservation() {
        let dir = std::env::temp_dir().join(format!("feishin-test-{}", Uuid::new_v4()));
        let path = dir.join("state.json");
        assert_eq!(load(&path).unwrap().settings.volume, 50.0);
        let mut state = State::default();
        state.settings.volume = 73.0;
        save(&path, &state).unwrap();
        assert_eq!(load(&path).unwrap().settings.volume, 73.0);
        state.settings.volume = 25.0;
        save(&path, &state).unwrap();
        assert_eq!(load(&path).unwrap().settings.volume, 25.0);
        state.queue.add(
            vec![Item {
                id: "book".into(),
                ..Default::default()
            }],
            crate::queue::Enqueue::Now,
        );
        state.queue.position = 123.0;
        state.queue.shuffle = true;
        state.queue.repeat = crate::queue::Repeat::All;
        save(&path, &state).unwrap();
        assert_eq!(load(&path).unwrap().queue.position, 123.0);
        state.settings.resume_queue = false;
        save(&path, &state).unwrap();
        let saved = load(&path).unwrap();
        assert!(saved.queue.entries.is_empty() && saved.queue.current.is_none());
        assert!(saved.queue.shuffle);
        assert_eq!(saved.queue.repeat, crate::queue::Repeat::All);
        assert_eq!(state.queue.entries.len(), 1);
        assert_eq!(state.queue.position, 123.0);
        fs::write(&path, "broken").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
        state.settings.discord_config.last_fm_api_key = "export-secret-must-be-absent".into();
        let exported = serde_json::to_string(&state).unwrap();
        assert!(!exported.contains("export-secret-must-be-absent"));
        let exported: serde_json::Value = serde_json::from_str(&exported).unwrap();
        fn check_credentials(value: &serde_json::Value) {
            match value {
                serde_json::Value::Object(object) => {
                    for (key, value) in object {
                        assert!(!matches!(
                            key.as_str(),
                            "password"
                                | "remote_password"
                                | "legacy_password"
                                | "refresh_password"
                                | "token"
                                | "session"
                                | "last_fm_api_key"
                        ));
                        check_credentials(value);
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        check_credentials(value);
                    }
                }
                _ => {}
            }
        }
        check_credentials(&exported);
        assert_eq!(exported["settings"]["password_store"], "gnome_libsecret");
        let old = serde_json::json!({"settings":{"equalizer":[1,2,3,4,5,6,7,8,9,10]}});
        let old: State = serde_json::from_value(old).unwrap();
        assert_eq!(
            old.settings.equalizer,
            [1., 2., 3., 4., 5., 6., 7., 0., 8., 9., 0., 10.]
        );
        let old_item: Item =
            serde_json::from_value(serde_json::json!({"id":"old-song","name":"Old song"})).unwrap();
        assert_eq!(old_item.album_artist, "");
        assert_eq!(old_item.play_count, 0);
        let text = r#"{"arguments":["--no-config","--ao=null"],"properties":{"audio-exclusive":false,"audio-samplerate":48000,"replaygain-preamp":-3.5}}"#;
        state.settings.apply_mpv_options(text).unwrap();
        assert_eq!(state.settings.mpv_extra_args, ["--no-config", "--ao=null"]);
        assert_eq!(state.settings.mpv_properties["audio-samplerate"], 48000);
        let properties = state.settings.mpv_properties.clone();
        assert!(
            state
                .settings
                .apply_mpv_options(r#"{"arguments":["not-an-option"],"properties":{}}"#)
                .is_err()
        );
        assert!(
            state
                .settings
                .apply_mpv_options(r#"{"arguments":[],"properties":{"volume":null}}"#)
                .is_err()
        );
        assert_eq!(state.settings.mpv_properties, properties);
        fs::remove_dir_all(dir).unwrap();
    }
}

/// Restore previous keychain entries if the settings import does not finish.
pub struct SecretTransaction {
    records: Vec<(std::sync::Arc<keyring::Entry>, Option<String>)>,
    committed: bool,
}
impl SecretTransaction {
    pub fn import(
        data: &crate::migration::DataImport,
        backend: crate::credentials::Backend,
    ) -> Result<Self> {
        let mut entries = vec![];
        if let Some(key) = &data.lastfm_api_key {
            entries.push((
                std::sync::Arc::new(crate::credentials::entry_for_backend(
                    backend,
                    "Feishin Native",
                    "lastfm-api-key",
                )?),
                key.clone(),
            ));
        }
        if let Some(credentials) = &data.remote_credentials {
            entries.push((
                std::sync::Arc::new(crate::credentials::entry_for_backend(
                    backend,
                    "org.jeffvli.feishin.rust",
                    "remote-control",
                )?),
                serde_json::to_string(credentials)?,
            ));
        }
        if let Some((provider, key)) = &data.translation_key {
            entries.push((
                std::sync::Arc::new(crate::credentials::entry_for_backend(
                    backend,
                    "org.jeffvli.feishin.rust",
                    &format!("translation-{provider}"),
                )?),
                key.clone(),
            ));
        }
        Self::apply(entries)
    }
    fn apply(entries: Vec<(std::sync::Arc<keyring::Entry>, String)>) -> Result<Self> {
        let mut transaction = Self {
            records: vec![],
            committed: false,
        };
        for (entry, value) in entries {
            let previous = match entry.get_password() {
                Ok(value) => Some(value),
                Err(keyring::Error::NoEntry) => None,
                Err(error) => return Err(error.into()),
            };
            transaction.records.push((entry.clone(), previous));
            entry
                .set_password(&value)
                .context("Could not securely store imported credentials")?;
        }
        Ok(transaction)
    }
    pub fn commit(mut self) {
        self.committed = true;
    }
    fn rollback(&self) {
        for (entry, previous) in self.records.iter().rev() {
            let result = if let Some(previous) = previous {
                entry.set_password(previous)
            } else {
                entry.delete_credential()
            };
            if let Err(error) = result
                && !matches!(error, keyring::Error::NoEntry)
            {
                log::warn!("Import credential rollback failed: {error}");
            }
        }
    }
}
impl Drop for SecretTransaction {
    fn drop(&mut self) {
        if !self.committed {
            self.rollback();
        }
    }
}
#[cfg(test)]
mod secret_import_tests {
    use super::*;
    #[test]
    fn imported_secrets_commit_or_restore_existing_and_missing_entries_without_the_os_store() {
        let old = std::sync::Arc::new(keyring::Entry::new_with_credential(Box::<
            keyring::mock::MockCredential,
        >::default()));
        let missing = std::sync::Arc::new(keyring::Entry::new_with_credential(Box::<
            keyring::mock::MockCredential,
        >::default(
        )));
        old.set_password("original").unwrap();
        {
            let transaction = SecretTransaction::apply(vec![
                (old.clone(), "changed".into()),
                (missing.clone(), "added".into()),
            ])
            .unwrap();
            assert_eq!(old.get_password().unwrap(), "changed");
            assert_eq!(missing.get_password().unwrap(), "added");
            drop(transaction);
        }
        assert_eq!(old.get_password().unwrap(), "original");
        assert!(matches!(
            missing.get_password(),
            Err(keyring::Error::NoEntry)
        ));
        SecretTransaction::apply(vec![
            (old.clone(), "committed".into()),
            (missing.clone(), "committed".into()),
        ])
        .unwrap()
        .commit();
        assert_eq!(old.get_password().unwrap(), "committed");
        assert_eq!(missing.get_password().unwrap(), "committed");
        missing
            .get_credential()
            .downcast_ref::<keyring::mock::MockCredential>()
            .unwrap()
            .set_error(keyring::Error::NoStorageAccess(Box::new(
                std::io::Error::other("fixture failure"),
            )));
        assert!(
            SecretTransaction::apply(vec![
                (old.clone(), "temporary".into()),
                (missing.clone(), "rejected".into())
            ])
            .is_err()
        );
        assert_eq!(old.get_password().unwrap(), "committed");
    }
}

#[cfg(test)]
mod scrobble_tests {
    use super::*;
    #[test]
    fn thresholds_match_original_zero_seconds_and_unknown_duration() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/scrobble-conditions-reference.json"))
                .unwrap();
        for case in cases {
            let mut settings = Settings {
                scrobble: true,
                scrobble_mode: serde_json::from_value(case["mode"].clone()).unwrap(),
                scrobble_seconds: case["seconds"].as_f64().unwrap() as f32,
                scrobble_percentage: 50.,
                ..Default::default()
            };
            let played = case["listened"].as_f64().unwrap();
            let duration = case["duration"].as_f64().unwrap();
            assert_eq!(
                settings.should_scrobble(played, duration),
                case["submit"] == true,
                "{case}"
            );
            settings.private_mode = true;
            assert!(!settings.should_scrobble(played, duration));
            settings.private_mode = false;
            settings.scrobble = false;
            assert!(!settings.should_scrobble(played, duration));
        }
        let settings = Settings {
            scrobble: true,
            scrobble_seconds: 0.,
            ..Default::default()
        };
        for invalid in [f64::NAN, f64::INFINITY, -1.] {
            assert!(!settings.should_scrobble(invalid, 100.));
            assert!(!settings.should_scrobble(100., invalid));
        }
    }
}

#[test]
fn playlist_folder_defaults_preserve_legacy_profiles_and_independent_scopes() {
    let server = Uuid::new_v4();
    let mut fresh = State::default();
    let mut legacy: State = serde_json::from_str("{}").unwrap();
    assert!(!fresh.playlist_folder_expanded(server, "Rock", false));
    assert!(legacy.playlist_folder_expanded(server, "Rock", false));
    for (state, starts_open) in [(&mut fresh, false), (&mut legacy, true)] {
        state.toggle_playlist_folder(server, "Rock", true);
        assert_eq!(
            state.playlist_folder_expanded(server, "Rock", false),
            starts_open
        );
        assert_eq!(
            state.playlist_folder_expanded(server, "Rock", true),
            !starts_open
        );
        let mut restored: State =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        assert_eq!(
            restored.playlist_folder_expanded(server, "Rock", true),
            !starts_open
        );
        restored.toggle_playlist_folder(server, "Rock", true);
        assert_eq!(
            restored.playlist_folder_expanded(server, "Rock", true),
            starts_open
        );
    }
}
