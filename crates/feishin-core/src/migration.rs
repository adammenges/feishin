//! Reviewable preference imports from native exports and Electron settings.
use crate::{lyric_sources::Provider, storage::Settings};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub struct Preview {
    pub settings: Settings,
    pub changes: Vec<(String, String, String)>,
    pub omitted: Vec<String>,
    pub data: DataImport,
}

#[derive(Clone, Default)]
pub struct DataImport {
    pub servers: Option<Vec<crate::model::Server>>,
    pub active_server: Option<Option<uuid::Uuid>>,
    pub signed_out_servers: Option<std::collections::HashSet<uuid::Uuid>>,
    pub queue: Option<crate::queue::Queue>,
    pub radio: Option<Vec<crate::model::Item>>,
    pub collapsed_playlist_folders:
        Option<std::collections::BTreeMap<uuid::Uuid, BTreeSet<String>>>,
    pub shared_collapsed_playlist_folders:
        Option<std::collections::BTreeMap<uuid::Uuid, BTreeSet<String>>>,
    pub expanded_playlist_folders:
        Option<Option<std::collections::BTreeMap<uuid::Uuid, BTreeSet<String>>>>,
    pub shared_expanded_playlist_folders:
        Option<Option<std::collections::BTreeMap<uuid::Uuid, BTreeSet<String>>>>,
    pub sidebar_playlist_order: Option<std::collections::BTreeMap<uuid::Uuid, Vec<String>>>,
    pub command_palette_sections: Option<std::collections::BTreeMap<String, bool>>,
    pub merge_servers: bool,
    pub lastfm_api_key: Option<String>,
    pub remote_credentials: Option<(String, String)>,
    pub translation_key: Option<(String, String)>,
}
impl DataImport {
    pub fn apply(&self, state: &mut crate::storage::State) {
        if let Some(servers) = &self.servers {
            if self.merge_servers {
                for server in servers {
                    if let Some(old) = state.servers.iter_mut().find(|old| old.id == server.id) {
                        *old = server.clone();
                    } else {
                        state.servers.push(server.clone());
                    }
                }
            } else {
                state.servers = servers.clone();
            }
        }
        if let Some(active) = self.active_server {
            state.active_server = active;
        }
        if let Some(ids) = &self.signed_out_servers {
            state.signed_out_servers = ids.clone();
        }
        state
            .signed_out_servers
            .retain(|id| state.servers.iter().any(|server| server.id == *id));
        if let Some(queue) = &self.queue {
            state.queue = queue.clone();
        }
        if let Some(radio) = &self.radio {
            state.radio = radio.clone();
        }
        if let Some(folders) = &self.collapsed_playlist_folders {
            state.collapsed_playlist_folders = folders.clone();
        }
        if let Some(folders) = &self.shared_collapsed_playlist_folders {
            state.shared_collapsed_playlist_folders = folders.clone();
        }
        if let Some(folders) = &self.expanded_playlist_folders {
            state.expanded_playlist_folders = folders.clone();
        }
        if let Some(folders) = &self.shared_expanded_playlist_folders {
            state.shared_expanded_playlist_folders = folders.clone();
        }
        if let Some(order) = &self.sidebar_playlist_order {
            state.sidebar_playlist_order = order.clone();
        }
        if let Some(sections) = &self.command_palette_sections {
            state.command_palette_sections = sections.clone();
        }
        if state
            .active_server
            .is_some_and(|id| !state.servers.iter().any(|server| server.id == id))
        {
            state.active_server = None;
        }
    }
    pub fn replaces_playback(&self) -> bool {
        self.queue.is_some() || (self.servers.is_some() && !self.merge_servers)
    }
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if self.command_palette_sections.is_some() {
            parts.push("Command palette section preferences".into());
        }
        if let Some(servers) = &self.servers {
            parts.push(format!("{} server definitions", servers.len()));
        }
        if let Some(queue) = &self.queue {
            parts.push(format!("{} queue entries", queue.entries.len()));
        }
        if let Some(radio) = &self.radio {
            parts.push(format!("{} radio stations", radio.len()));
        }
        if self.remote_credentials.is_some() {
            parts.push("remote-control credentials (stored securely)".into());
        }
        if self.translation_key.is_some() {
            parts.push("translation API key (stored securely)".into());
        }
        if self.lastfm_api_key.is_some() {
            parts.push("Last.fm API key (stored securely)".into());
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!(
                "Data to restore: {}. Playback will remain stopped when the queue is restored.",
                parts.join(", ")
            )
        }
    }
}

fn data_import(value: &Value) -> Result<DataImport> {
    use crate::model::{Server, ServerOptions};
    let mut data = DataImport::default();
    if let Some(servers) = value.get("servers") {
        data.servers =
            Some(serde_json::from_value(servers.clone()).context("Invalid server definitions")?);
        data.signed_out_servers = Some(serde_json::from_value(
            value
                .get("signed_out_servers")
                .cloned()
                .unwrap_or_else(|| json!([])),
        )?);
    }
    if let Some(active) = value.get("active_server") {
        data.active_server = Some(serde_json::from_value(active.clone())?);
    }
    let auth = value.get("state").unwrap_or(value);
    if let Some(servers) = auth.get("serverList").and_then(Value::as_object) {
        let id_for = |id: &str| {
            uuid::Uuid::parse_str(id).unwrap_or_else(|_| {
                uuid::Uuid::from_bytes(md5::compute(format!("electron-server:{id}")).0)
            })
        };
        let mut imported = Vec::new();
        for (key, value) in servers {
            let field = |name: &str| value[name].as_str().unwrap_or_default().to_owned();
            let enabled = |name: &str| value[name].as_bool().unwrap_or(false);
            let folders = match &value["musicFolderId"] {
                Value::String(id) => vec![id.clone()],
                Value::Array(ids) => ids
                    .iter()
                    .map(|id| {
                        id.as_str()
                            .context("Library IDs must be strings")
                            .map(str::to_owned)
                    })
                    .collect::<Result<Vec<_>>>()?,
                _ => Vec::new(),
            };
            imported.push(Server {
                id: id_for(value["id"].as_str().unwrap_or(key)),
                name: field("name"),
                url: field("url"),
                username: field("username"),
                kind: serde_json::from_value(value["type"].clone())
                    .context("Invalid Electron server type")?,
                legacy_auth: enabled("legacyAuth"),
                options: ServerOptions {
                    ignore_ssl: false,
                    save_password: enabled("savePassword"),
                    remote_url: field("remoteUrl"),
                    prefer_remote_url: enabled("preferRemoteUrl"),
                    prefer_instant_mix: enabled("preferInstantMix"),
                    enable_audiobooks: enabled("enableAudiobooks"),
                    music_folder_ids: folders,
                },
            });
        }
        data.servers = Some(imported);
        data.merge_servers = true;
        if let Some(current) = auth.get("currentServer") {
            data.active_server = Some(current["id"].as_str().map(id_for));
        }
    }
    if let Some(servers) = &mut data.servers {
        ensure!(servers.len() <= 100, "Too many imported servers");
        let mut ids = BTreeSet::new();
        for server in servers {
            ensure!(ids.insert(server.id), "Duplicate imported server identity");
            *server = crate::api::Api::new(server.clone(), Default::default())?.server;
        }
    }
    if let Some(queue) = value.get("queue") {
        let mut queue: crate::queue::Queue =
            serde_json::from_value(queue.clone()).context("Invalid imported queue")?;
        let mut keys = BTreeSet::new();
        ensure!(
            queue.entries.iter().all(|entry| keys.insert(entry.key)),
            "Duplicate imported queue identity"
        );
        queue.repair();
        data.queue = Some(queue);
    }
    if let Some(radio) = value.get("radio") {
        let radio: Vec<crate::model::Item> = serde_json::from_value(radio.clone())?;
        ensure!(
            radio
                .iter()
                .all(|item| item.kind == crate::model::ItemKind::Radio
                    && url::Url::parse(&item.stream_url).is_ok_and(|url| matches!(
                        url.scheme(),
                        "http" | "https"
                    ) && url
                        .host_str()
                        .is_some())),
            "Invalid imported radio station"
        );
        data.radio = Some(radio);
    }
    if let Some(folders) = value.get("collapsed_playlist_folders") {
        data.collapsed_playlist_folders = Some(serde_json::from_value(folders.clone())?);
    }
    if value.get("collapsed_playlist_folders").is_some() {
        data.shared_collapsed_playlist_folders = Some(serde_json::from_value(
            value
                .get("shared_collapsed_playlist_folders")
                .cloned()
                .unwrap_or_else(|| json!({})),
        )?);
        data.expanded_playlist_folders = Some(serde_json::from_value(
            value
                .get("expanded_playlist_folders")
                .cloned()
                .unwrap_or(Value::Null),
        )?);
        data.shared_expanded_playlist_folders = Some(serde_json::from_value(
            value
                .get("shared_expanded_playlist_folders")
                .cloned()
                .unwrap_or(Value::Null),
        )?);
    }
    if let Some(order) = value.get("sidebar_playlist_order") {
        let order: std::collections::BTreeMap<uuid::Uuid, Vec<String>> =
            serde_json::from_value(order.clone())?;
        ensure!(
            order.len() <= 100
                && order.values().all(|ids| ids.len() <= 100_000
                    && ids.iter().all(|id| id.len() <= 4096 && !id.contains('\0'))),
            "Invalid sidebar playlist order"
        );
        data.sidebar_playlist_order = Some(order);
    }
    if let Some(sections) = value
        .get("command_palette_sections")
        .or_else(|| auth.get("commandPaletteSearchSectionsExpanded"))
    {
        let sections: std::collections::BTreeMap<String, bool> =
            serde_json::from_value(sections.clone())?;
        ensure!(
            sections
                .keys()
                .all(|key| matches!(key.as_str(), "albums" | "artists" | "tracks")),
            "Invalid command palette sections"
        );
        data.command_palette_sections = Some(sections);
    }
    Ok(data)
}

pub fn preview(current: &Settings, bytes: &[u8]) -> Result<Preview> {
    ensure!(bytes.len() <= 16_000_000, "Settings file is too large");
    let value: Value =
        serde_json::from_slice(bytes).context("Settings file must contain valid JSON")?;
    let mut data = data_import(&value)?;
    if let Some(key) = at(&value, "general.lastfmApiKey")
        .and_then(Value::as_str)
        .filter(|key| !key.is_empty())
    {
        ensure!(key.len() <= 1024, "Last.fm API key is too long");
        data.lastfm_api_key = Some(key.into());
    }
    let mut settings = serde_json::to_value(current)?;
    let mut imported = BTreeSet::new();
    for (original, native) in [
        ("playlist-skip-duplicate", "playlist_skip_duplicates"),
        ("playlist-header-collapsed", "playlist_header_collapsed"),
    ] {
        if let Some(value) = value.get(original) {
            let enabled = value
                .as_bool()
                .or_else(|| value.as_str().and_then(|value| value.parse::<bool>().ok()))
                .with_context(|| format!("Invalid {original} preference"))?;
            settings[native] = json!(enabled);
            imported.insert(original.into());
        }
    }

    let stored_app = value
        .get("store_app")
        .map(|app| {
            if let Some(text) = app.as_str() {
                serde_json::from_str::<Value>(text).context("Invalid saved app preferences")
            } else {
                Ok(app.clone())
            }
        })
        .transpose()?;
    let app = stored_app.as_ref().unwrap_or(&value);
    let app = app.get("state").unwrap_or(app);
    if let Some(sidebars) = app.get("pageSidebar") {
        let sidebars: std::collections::BTreeMap<String, bool> =
            serde_json::from_value(sidebars.clone()).context("Invalid page sidebar preferences")?;
        for (key, visible) in sidebars {
            settings["page_sidebars"][key] = json!(visible);
        }
        imported.insert(
            if stored_app.is_some() {
                "store_app"
            } else {
                "pageSidebar"
            }
            .into(),
        );
    }
    if let Some(remaining) = app.get("showTimeRemaining") {
        settings["show_time_remaining"] = json!(
            remaining
                .as_bool()
                .context("Invalid remaining-time preference")?
        );
        imported.insert(
            if stored_app.is_some() {
                "store_app"
            } else {
                "showTimeRemaining"
            }
            .into(),
        );
    }

    if let Some(shuffle) = value.get("store_shuffle_all") {
        let stored = if let Some(text) = shuffle.as_str() {
            serde_json::from_str(text).context("Invalid saved Play random preferences")?
        } else {
            shuffle.clone()
        };
        let mut shuffle = stored.get("state").unwrap_or(&stored).clone();
        ensure!(
            shuffle.is_object(),
            "Play random preferences must be an object"
        );
        if shuffle.get("genre").is_some_and(Value::is_null) {
            shuffle["genre"] = json!("");
        }
        if shuffle.get("played").is_some_and(Value::is_null) {
            shuffle["played"] = json!("all");
        }
        if let Some(folder) = shuffle.get("musicFolderId") {
            ensure!(
                folder.is_null()
                    || folder.is_string()
                    || folder
                        .as_array()
                        .is_some_and(|ids| ids.len() <= 100 && ids.iter().all(Value::is_string)),
                "Invalid random-play library selection"
            );
            let folder = folder
                .as_str()
                .or_else(|| {
                    folder
                        .as_array()
                        .and_then(|ids| ids.first())
                        .and_then(Value::as_str)
                })
                .unwrap_or_default();
            shuffle["music_folder"] = json!(folder);
        }
        let shuffle: crate::shuffle::Config =
            serde_json::from_value(shuffle).context("Invalid Play random preferences")?;
        shuffle.validate()?;
        settings["shuffle"] = serde_json::to_value(shuffle)?;
        imported.insert("shuffle".into());
    }
    let mut omitted = vec![];
    if data.lastfm_api_key.is_some() {
        imported.insert("general.lastfmApiKey".into());
    }
    if let Some(native) = value.get("settings").and_then(Value::as_object) {
        for (key, value) in native {
            if settings.get(key).is_some() {
                settings[key] = value.clone();
            } else {
                omitted.push(key.clone());
            }
        }
    } else {
        if let Some(value) = value.get("ignore_ssl") {
            let enabled = value
                .as_bool()
                .or_else(|| value.as_str().and_then(|value| value.parse::<bool>().ok()))
                .context("Invalid ignore_ssl setting")?;
            settings["ignore_ssl"] = json!(enabled);
            imported.insert("ignore_ssl".into());
        }
        ensure!(
            value.get("general").is_some_and(Value::is_object)
                || value.get("playback").is_some_and(Value::is_object)
                || data.servers.is_some()
                || data.queue.is_some()
                || data.command_palette_sections.is_some()
                || value.get("ignore_ssl").is_some()
                || imported.contains("showTimeRemaining")
                || imported.contains("pageSidebar")
                || imported.contains("store_app")
                || value.get("store_shuffle_all").is_some()
                || value.get("playlist-skip-duplicate").is_some()
                || value.get("playlist-header-collapsed").is_some(),
            "This is not a Feishin settings export"
        );
        let mappings = [
            ("tab", "settings_tab"),
            ("general.theme", "theme"),
            ("css", "css"),
            ("general.followSystemTheme", "follow_system_theme"),
            ("general.themeDark", "theme_dark"),
            ("general.themeLight", "theme_light"),
            ("tagEditor.triggerRescan", "tag_rescan"),
            ("general.language", "language"),
            ("general.logLevel", "log_level"),
            ("general.passwordStore", "password_store"),
            ("general.pathReplace", "path_replace"),
            ("general.pathReplaceWith", "path_replace_with"),
            ("general.sidebarPlaylistFolders", "playlist_folders"),
            (
                "general.sidebarPlaylistFolderSeparator",
                "playlist_separator",
            ),
            ("general.sidebarPlaylistFolderView", "playlist_folder_view"),
            (
                "general.sidebarPlaylistFolderTreeIndent",
                "playlist_folder_indent",
            ),
            (
                "general.sidebarPlaylistFolderTreeLineColor",
                "playlist_folder_color",
            ),
            ("general.confirmQueueChanges", "confirm_queue_changes"),
            ("general.followCurrentSong", "follow_current_song"),
            ("general.accent", "accent"),
            ("general.useThemeAccentColor", "use_theme_accent_color"),
            ("general.useThemePrimaryShade", "use_theme_primary_shade"),
            ("general.primaryShade", "primary_shade"),
            ("general.showFavorites", "show_favorites"),
            ("general.showRatings", "show_ratings"),
            ("general.enableGridMultiSelect", "enable_grid_multiselect"),
            ("general.blurExplicitImages", "blur_explicit_images"),
            ("general.imagePlaceholderPriority", "image_placeholder"),
            ("general.imageRes", "image_resolution"),
            ("general.nativeAspectRatio", "native_image_aspect_ratio"),
            ("general.buttonSize", "button_size"),
            ("general.albumBackground", "album_background"),
            ("general.albumBackgroundBlur", "album_background_blur"),
            ("general.artistBackground", "artist_background"),
            ("general.artistBackgroundBlur", "artist_background_blur"),
            (
                "general.fullscreenAutoOpenTimeout",
                "fullscreen_auto_open_minutes",
            ),
            ("general.playerbarOpenDrawer", "playerbar_open_drawer"),
            ("general.volumeWheelStep", "volume_wheel_step"),
            ("general.volumeWidth", "volume_width"),
            ("general.playButtonBehavior", "play_button_behavior"),
            ("general.upNextQueue", "up_next_queue"),
            ("general.artistRadioCount", "artist_radio_count"),
            (
                "general.microtonalPitchControls",
                "microtonal_pitch_controls",
            ),
            ("general.shareExpiration", "share_expiration"),
            ("general.artistItems", "artist_items"),
            ("queryBuilder.tag", "query_fields"),
            ("general.resume", "resume_queue"),
            ("general.autoSave.enabled", "queue_autosave"),
            ("general.autoSave.count", "queue_autosave_count"),
            ("playback.preservePitch", "preserve_pitch"),
            ("playback.filters", "playback_filters"),
            ("playback.audioFadeOnStatusChange", "fade_on_status"),
            ("playback.mpvAudioDeviceId", "mpv_audio_device"),
            ("playback.mpvExtraParameters", "mpv_extra_args"),
            ("playback.scrobble.enabled", "scrobble"),
            ("playback.scrobble.notify", "notify_tracks"),
            ("playback.previousLocalVolume", "previous_local_volume"),
            ("playback.scrobble.minimumMode", "scrobble_mode"),
            (
                "playback.scrobble.scrobbleAtPercentage",
                "scrobble_percentage",
            ),
            ("playback.scrobble.scrobbleAtDuration", "scrobble_seconds"),
            ("playback.transcode.enabled", "transcode"),
            ("playback.transcode.bitrate", "bitrate"),
            ("playback.transcode.maxSampleRate", "max_sample_rate"),
            ("playback.transcode.format", "format"),
            ("playback.compressor", "compressor"),
            ("playback.equalizer.enabled", "equalizer_enabled"),
            ("playback.equalizer.preamp", "equalizer_preamp"),
            ("playback.mpvProperties.replayGainMode", "replay_gain"),
            ("autoDJ.enabled", "auto_dj"),
            ("discord.enabled", "discord_enabled"),
            ("discord.clientId", "discord_client_id"),
            ("remote.port", "remote_port"),
            ("remote.enabled", "remote_enabled"),
            ("remote.username", "remote_username"),
            ("window.preventSleepOnPlayback", "prevent_sleep"),
            ("window.preventSuspendOnPlayback", "prevent_suspend"),
            ("window.tray", "tray"),
            ("window.exitToTray", "exit_to_tray"),
            ("window.minimizeToTray", "minimize_to_tray"),
            ("window.startMinimized", "start_minimized"),
            ("window.windowBarStyle", "window_bar_style"),
            ("window.windowBarTrackinfo", "window_title_track_info"),
            ("playback.mediaSession", "media_session"),
            ("hotkeys.globalMediaHotkeys", "global_media_hotkeys"),
            ("lyrics.enableNeteaseTranslation", "lyric_translation"),
            ("lyrics.follow", "lyric_scroll"),
            ("lyrics.fetch", "lyric_fetch"),
            ("lyrics.preferLocalLyrics", "lyric_prefer_local"),
            ("lyrics.alignment", "lyric_alignment"),
            ("lyrics.followScrollAlignment", "lyric_follow_alignment"),
            ("lyrics.showMatch", "lyric_show_match"),
            ("lyrics.showProvider", "lyric_show_provider"),
            ("lyrics.enableFurigana", "lyric_furigana"),
            ("lyrics.enableRomaji", "lyric_romaji"),
        ];
        for (from, to) in mappings {
            if let Some(value) = at(&value, from).filter(|value| !value.is_null()) {
                settings[to] = value.clone();
                imported.insert(from.to_owned());
            }
        }
        if let Some(kind) = at(&value, "playback.type")
            .and_then(Value::as_str)
            .filter(|kind| matches!(*kind, "jukebox" | "web" | "local"))
        {
            settings["jukebox"] = (kind == "jukebox").into();
            imported.insert("playback.type".into());
        }
        if let Some(config) = value.get("discord") {
            settings["discord_config"] = config.clone();
            imported.insert("discord".into());
        }
        if let Some(zoom) = at(&value, "general.zoomFactor").and_then(Value::as_f64) {
            settings["zoom"] = (zoom / 100.).into();
            imported.insert("general.zoomFactor".into());
        }
        for (from, to) in [
            ("albumGroupImageSize", "imageSize"),
            ("albumGroupItems", "items"),
            ("albumGroupShowFavoriteRating", "showFavoriteRating"),
            ("albumGroupVerticalLayout", "verticalLayout"),
        ] {
            let path = format!("general.{from}");
            if let Some(value) = at(&value, &path) {
                settings["album_group"][to] = value.clone();
                imported.insert(path);
            }
        }
        for (from, to) in [
            ("externalLinks", "enabled"),
            ("lastFM", "last_fm"),
            ("listenBrainz", "listen_brainz"),
            ("musicBrainz", "music_brainz"),
            ("qobuz", "qobuz"),
            ("spotify", "spotify"),
            ("nativeSpotify", "native_spotify"),
        ] {
            let key = format!("general.{from}");
            if let Some(value) = at(&value, &key) {
                settings["external_links"][to] = value.clone();
                imported.insert(key);
            }
        }
        if let Some(fields) = at(&value, "general.playerItems") {
            settings["player_display"]["fields"] = fields.clone();
            imported.insert("general.playerItems".into());
        }
        if let Some(font) = value.get("font").filter(|font| font.is_object()) {
            settings["font"] = font.clone();
            imported.insert("font".into());
        }
        if let Some(channel) = at(&value, "window.releaseChannel") {
            settings["updates"]["channel"] = channel.clone();
            imported.insert("window.releaseChannel".into());
        }
        if let Some(disabled) = at(&value, "window.disableAutoUpdate").and_then(Value::as_bool) {
            settings["updates"]["automatic"] = Value::Bool(!disabled);
            imported.insert("window.disableAutoUpdate".into());
        }
        for (from, to) in [
            ("sidebarItems", "items"),
            ("sidebarCollapsedNavigation", "collapsedNavigation"),
            ("sidebarPlaylistList", "playlistList"),
            ("sidebarPlaylistMode", "playlistMode"),
            ("sidebarPlaylistListFilterRegex", "playlistFilterRegex"),
            ("sidebarPlaylistSorting", "playlistSorting"),
            ("sidebarCollapseShared", "collapseShared"),
            ("sidebarImageEnabled", "imageEnabled"),
        ] {
            let key = format!("general.{from}");
            if let Some(value) = at(&value, &key) {
                settings["sidebar"][to] = value.clone();
                imported.insert(key);
            }
        }
        for (from, to) in [
            ("general.homeItems", "items"),
            ("general.homeFeature", "feature"),
            ("general.homeFeatureStyle", "featureStyle"),
        ] {
            if let Some(value) = at(&value, from) {
                settings["home"][to] = value.clone();
                imported.insert(from.into());
            }
        }
        if let Some(waveform) = at(&value, "general.playerbarSlider").and_then(Value::as_object) {
            settings["playerbar"]
                .as_object_mut()
                .unwrap()
                .extend(waveform.clone());
            imported.insert("general.playerbarSlider".into());
        }
        for (from, to) in [
            ("sideQueueType", "kind"),
            ("sideQueueLayout", "layout"),
            ("showQueueInSidebar", "queue"),
            ("showLyricsInSidebar", "lyrics"),
            ("showVisualizerInSidebar", "visualizer"),
            ("combinedLyricsAndVisualizer", "combined"),
            ("sidebarPanelOrder", "order"),
        ] {
            if let Some(value) = at(&value, &format!("general.{from}")) {
                settings["panels"][to] = value.clone();
                imported.insert(format!("general.{from}"));
            }
        }
        for (from, to) in [
            ("enabled", "skipEnabled"),
            ("skipBackwardSeconds", "skipBackwardSeconds"),
            ("skipForwardSeconds", "skipForwardSeconds"),
        ] {
            if let Some(value) = at(&value, &format!("general.skipButtons.{from}")) {
                settings["playerbar"][to] = value.clone();
                imported.insert(format!("general.skipButtons.{from}"));
            }
        }
        if let Some(display) = value.get("fullScreenPlayer") {
            let target = settings["player_display"].as_object_mut().unwrap();
            for (key, value) in display
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid player display settings"))?
            {
                if target.contains_key(key) {
                    target.insert(key.clone(), value.clone());
                    imported.insert(format!("fullScreenPlayer.{key}"));
                }
            }
        }
        if let Some(visualizer) = value.get("visualizer") {
            let mut config = serde_json::to_value(crate::visualizer::Config::default())?;
            for (key, value) in visualizer
                .as_object()
                .ok_or_else(|| anyhow::anyhow!("Invalid visualizer settings"))?
            {
                if let (Some(target), Some(source)) =
                    (config[key].as_object_mut(), value.as_object())
                {
                    target.extend(source.clone());
                } else {
                    config[key] = value.clone();
                }
            }
            settings["visualizer"] = config;
            imported.insert("visualizer".into());
        }
        for (path, key) in [
            ("general.genreTarget", "genre_albums"),
            ("general.playlistTarget", "playlist_albums"),
        ] {
            if let Some(target) = at(&value, path).and_then(Value::as_str) {
                ensure!(
                    matches!(target, "album" | "track" | "song"),
                    "Invalid library target"
                );
                settings[key] = (target == "album").into();
                imported.insert(path.into());
            }
        }
        if let Some(collections) = at(&value, "general.collections") {
            settings["collections"] =
                serde_json::to_value(crate::collections::import_electron(collections)?)?;
            imported.insert("general.collections".into());
        }
        if let Some(configs) = at(&value, "lists").filter(|value| value.is_object()) {
            let mapped: std::collections::BTreeMap<String, crate::lists::Config> =
                serde_json::from_value(configs.clone())?;
            settings["lists"] = serde_json::to_value(mapped)?;
            imported.insert("lists".into());
        }
        if let Some(configs) = at(&value, "tagEditor.tagConfigs").and_then(Value::as_object) {
            let mut mapped = std::collections::BTreeMap::new();
            for (name, config) in configs {
                mapped.insert(
                    crate::tags::canonical_name(name),
                    crate::tags::Config {
                        multi_value: config["multiValue"].as_bool().unwrap_or(false),
                        autocomplete_source: config["autocompleteSource"]
                            .as_str()
                            .unwrap_or_default()
                            .into(),
                        custom_values: serde_json::from_value(config["customValues"].clone())?,
                    },
                );
            }
            settings["tag_configs"] = serde_json::to_value(mapped)?;
            imported.insert("tagEditor.tagConfigs".into());
        }
        if let Some(groups) = at(&value, "general.artistReleaseTypeItems") {
            let groups: Vec<crate::releases::Setting> = serde_json::from_value(groups.clone())?;
            settings["artist_release_types"] =
                serde_json::to_value(crate::releases::from_electron(&groups))?;
            imported.insert("general.artistReleaseTypeItems".into());
        }
        if let Some(config) = at(&value, "autoDJ") {
            for (from, to) in [
                ("mode", "mode"),
                ("songStrategy", "song_strategy"),
                ("albumStrategy", "album_strategy"),
                ("itemCount", "item_count"),
                ("timing", "timing"),
                ("allowDuplicates", "allow_duplicates"),
                ("onlySimilar", "only_similar"),
            ] {
                if let Some(value) = config.get(from) {
                    settings["auto_dj_config"][to] = value.clone();
                    imported.insert(format!("autoDJ.{from}"));
                }
            }
        }
        if let Some(bindings) = at(&value, "hotkeys.bindings") {
            settings["shortcuts"] = serde_json::to_value(crate::shortcuts::from_electron(
                bindings,
                &current.shortcuts,
            )?)?;
            imported.insert("hotkeys.bindings".into());
        }
        for (from, to) in [
            ("enableAutoTranslation", "automatic"),
            ("translationApiProvider", "provider"),
            ("translationTargetLanguage", "language"),
        ] {
            if let Some(setting) =
                at(&value, &format!("lyrics.{from}")).filter(|value| !value.is_null())
            {
                settings["lyric_cloud_translation"][to] = setting.clone();
                imported.insert(format!("lyrics.{from}"));
            }
        }
        if let (Some(username), Some(password)) = (
            at(&value, "remote.username").and_then(Value::as_str),
            at(&value, "remote.password").and_then(Value::as_str),
        ) && (!username.is_empty() || !password.is_empty())
        {
            crate::remote::Remote::validate_credentials(username, password)?;
            data.remote_credentials = Some((username.into(), password.into()));
            imported.insert("remote.password".into());
        }
        if let Some(key) = at(&value, "lyrics.translationApiKey")
            .and_then(Value::as_str)
            .filter(|key| !key.is_empty())
            && let Some(provider @ ("Microsoft Azure" | "Google Cloud")) =
                settings["lyric_cloud_translation"]["provider"].as_str()
        {
            ensure!(
                key.len() <= 4096 && !key.chars().any(char::is_control),
                "Invalid translation API key"
            );
            data.translation_key = Some((provider.into(), key.into()));
            imported.insert("lyrics.translationApiKey".into());
        }
        if let Some(lead) = at(&value, "lyrics.lineLeadTimeMs") {
            settings["lyric_lead_time"] =
                json!(lead.as_f64().context("Lyrics lead time must be a number")? / 1000.0);
            imported.insert("lyrics.lineLeadTimeMs".into());
        }
        if let Some(display) = at(&value, "lyricsDisplay.default") {
            for (from, to) in [
                ("fontSize", "font_size"),
                ("fontSizeUnsync", "font_size_unsync"),
                ("gap", "gap"),
                ("gapUnsync", "gap_unsync"),
                ("opacityNonActive", "opacity_non_active"),
                ("scaleNonActive", "scale_non_active"),
                ("paddingLeft", "padding_left"),
                ("paddingRight", "padding_right"),
            ] {
                if let Some(number) = display.get(from) {
                    settings["lyric_display"][to] = number.clone();
                    imported.insert(format!("lyricsDisplay.default.{from}"));
                }
            }
        }
        if let Some(displays) = at(&value, "lyricsDisplay").and_then(Value::as_object) {
            for (key, display) in displays.iter().filter(|(key, _)| key.as_str() != "default") {
                ensure!(
                    key.len() <= 128 && !key.chars().any(char::is_control),
                    "Invalid lyric display key"
                );
                let mut native = settings["lyric_display"].clone();
                for (from, to) in [
                    ("fontSize", "font_size"),
                    ("fontSizeUnsync", "font_size_unsync"),
                    ("gap", "gap"),
                    ("gapUnsync", "gap_unsync"),
                    ("opacityNonActive", "opacity_non_active"),
                    ("scaleNonActive", "scale_non_active"),
                    ("paddingLeft", "padding_left"),
                    ("paddingRight", "padding_right"),
                ] {
                    if let Some(value) = display.get(from) {
                        native[to] = value.clone();
                        imported.insert(format!("lyricsDisplay.{key}.{from}"));
                    }
                }
                settings["lyric_displays"][key] = native;
            }
        }
        if let Some(delay) = at(&value, "lyrics.delayMs") {
            settings["lyric_offset"] =
                json!(delay.as_f64().context("Lyrics delay must be a number")? / 1000.0);
            imported.insert("lyrics.delayMs".into());
        }
        if let Some(bands) = at(&value, "playback.equalizer.bands") {
            let bands = bands
                .as_array()
                .context("Equalizer bands must be an array")?;
            let gains = bands
                .iter()
                .map(|band| {
                    band.get("gain")
                        .unwrap_or(band)
                        .as_f64()
                        .context("Equalizer gains must be numbers")
                })
                .collect::<Result<Vec<_>>>()?;
            settings["equalizer"] = json!(gains);
            imported.insert("playback.equalizer.bands".into());
        }
        if let Some(sources) = at(&value, "lyrics.sources") {
            let mut providers = vec![];
            for source in sources
                .as_array()
                .context("Lyrics sources must be an array")?
            {
                let provider = match source.as_str() {
                    Some("lrclib.net" | "Lrclib") => Some(Provider::Lrclib),
                    Some("NetEase") => Some(Provider::NetEase),
                    Some("Genius") => Some(Provider::Genius),
                    Some("SimpMusic") => Some(Provider::SimpMusic),
                    _ => None,
                };
                if let Some(provider) = provider {
                    providers.push(provider);
                } else {
                    omitted.push("lyrics.sources (unrecognized provider)".into());
                }
            }
            settings["lyric_sources"] = serde_json::to_value(providers)?;
            imported.insert("lyrics.sources".into());
        }
        let properties = settings["mpv_properties"].as_object_mut().unwrap();
        for (from, to) in [
            ("audioExclusiveMode", "audio-exclusive"),
            ("audioFormat", "audio-format"),
            ("audioSampleRateHz", "audio-samplerate"),
            ("gaplessAudio", "gapless-audio"),
            ("replayGainClip", "replaygain-clip"),
            ("replayGainFallbackDB", "replaygain-fallback"),
            ("replayGainPreampDB", "replaygain-preamp"),
        ] {
            let path = format!("playback.mpvProperties.{from}");
            if let Some(value) = at(&value, &path).filter(|value| !value.is_null()) {
                if from != "audioSampleRateHz" || value.as_u64() != Some(0) {
                    properties.insert(to.into(), value.clone());
                }
                imported.insert(path);
            }
        }
        if data.servers.is_some() {
            imported.insert("servers".into());
            imported.insert("state.serverList".into());
            imported.insert("serverList".into());
            omitted.push("Server credentials (sign in again)".into());
        }
        for key in [
            "queue",
            "active_server",
            "radio",
            "collapsed_playlist_folders",
            "shared_collapsed_playlist_folders",
            "expanded_playlist_folders",
            "shared_expanded_playlist_folders",
            "sidebar_playlist_order",
            "command_palette_sections",
            "state.commandPaletteSearchSectionsExpanded",
            "commandPaletteSearchSectionsExpanded",
            "state.currentServer",
            "currentServer",
        ] {
            imported.insert(key.into());
        }
        collect_omitted(&value, "", &imported, &mut omitted);
    }
    let mut parsed: Settings =
        serde_json::from_value(settings).context("Imported preference values are invalid")?;
    if !value.get("settings").is_some_and(Value::is_object)
        && let Some(kind @ ("local" | "web")) = at(&value, "playback.type").and_then(Value::as_str)
    {
        parsed.select_mpv(kind == "local");
    }
    validate(&mut parsed)?;
    let before = serde_json::to_value(current)?;
    let after = serde_json::to_value(&parsed)?;
    let changes = before
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(key, value)| {
            let updated = &after[key];
            (value != updated).then(|| (key.clone(), value.to_string(), updated.to_string()))
        })
        .collect();
    omitted.sort();
    omitted.dedup();
    Ok(Preview {
        settings: parsed,
        changes,
        omitted,
        data,
    })
}
fn at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(value, |value, key| value.get(key))
}
fn collect_omitted(
    value: &Value,
    path: &str,
    imported: &BTreeSet<String>,
    omitted: &mut Vec<String>,
) {
    if path == "version" || imported.contains(path) {
        return;
    }
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            collect_omitted(
                value,
                &if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                },
                imported,
                omitted,
            );
        }
    } else if !path.is_empty() {
        omitted.push(path.to_owned());
    }
}
pub fn validate(settings: &mut Settings) -> Result<()> {
    crate::playback_filters::validate(&settings.playback_filters)?;
    settings.css.validate()?;
    ensure!(
        settings
            .previous_local_volume
            .is_none_or(|n| n.is_finite() && (0. ..=100.).contains(&n)),
        "Invalid previous local volume"
    );
    ensure!(
        matches!(
            settings.window_bar_style.as_str(),
            "linux" | "macos" | "windows" | "web"
        ),
        "Invalid window bar style"
    );
    settings.validate_transcoding()?;
    settings.discord_config.validate()?;
    ensure!(
        !settings.discord_client_id.is_empty()
            && settings.discord_client_id.len() <= 32
            && settings
                .discord_client_id
                .bytes()
                .all(|n| n.is_ascii_digit()),
        "Invalid Discord client ID"
    );
    ensure!(
        (15. ..=30.).contains(&settings.button_size),
        "Player control size must be between 15 and 30 pixels"
    );
    ensure!(
        crate::images::Placement::ALL
            .into_iter()
            .all(|placement| settings.image_resolution.get(placement).is_finite()),
        "Artwork resolution must be finite"
    );
    settings.album_group.validate()?;
    crate::smart::validate_fields(&settings.query_fields)?;
    crate::artist_sections::validate(&mut settings.artist_items)?;
    if let Some(expiration) = &settings.share_expiration {
        expiration.deadline(chrono::Local::now())?;
    }
    ensure!(
        (1..=500).contains(&settings.artist_radio_count),
        "Artist radio needs between 1 and 500 songs"
    );
    crate::appearance::color(&settings.accent)?;
    ensure!(
        settings.primary_shade <= 9,
        "Primary shade must be between 0 and 9"
    );
    ensure!(
        (0. ..=120.).contains(&settings.fullscreen_auto_open_minutes),
        "Fullscreen idle timeout must be between 0 and 120 minutes"
    );
    ensure!(
        (0. ..=6.).contains(&settings.album_background_blur)
            && (0. ..=6.).contains(&settings.artist_background_blur),
        "Artwork background blur must be between 0 and 6 rem"
    );
    ensure!(
        (1. ..=20.).contains(&settings.volume_wheel_step)
            && (0. ..=180.).contains(&settings.volume_width),
        "Invalid volume step or width"
    );
    crate::tags::validate_configs(&settings.tag_configs)?;
    crate::lists::validate(&settings.lists)?;
    crate::filters::validate_views(&settings.views)?;
    ensure!(
        settings.single_select_filters.len() <= 3
            && settings
                .single_select_filters
                .iter()
                .all(|kind| matches!(kind.as_str(), "artist" | "albumArtist" | "genre")),
        "Invalid filter selection modes"
    );
    ensure!(
        settings.pinned_filter_views.len() <= 5000
            && settings
                .pinned_filter_views
                .iter()
                .all(|key| key.len() <= 512 && !key.contains(['\0', '\n'])),
        "Invalid pinned filter views"
    );
    crate::collections::validate(&settings.collections)?;
    crate::visualizer::validate(&settings.visualizer)?;
    ensure!(
        matches!(settings.artist_grouping.as_str(), "primary" | "all"),
        "Invalid artist album grouping"
    );
    crate::releases::validate(&settings.artist_release_types)?;
    settings.auto_dj_config.validate()?;
    settings.shuffle.validate()?;
    ensure!(
        !settings.playlist_separator.is_empty()
            && settings.playlist_separator.len() <= 32
            && !settings.playlist_separator.chars().any(char::is_control),
        "Choose a folder separator of 1 to 32 bytes"
    );
    ensure!(
        matches!(
            settings.playlist_folder_view.as_str(),
            "tree" | "single" | "navigation"
        ) && settings.playlist_folder_indent <= 64,
        "Invalid playlist folder layout"
    );
    if !settings.playlist_folder_color.is_empty() {
        crate::appearance::color(&settings.playlist_folder_color)?;
    }
    ensure!(
        (0.5..=2.0).contains(&settings.zoom),
        "Invalid interface zoom"
    );
    ensure!(
        (0.0..=100.0).contains(&settings.volume) && (0.25..=4.0).contains(&settings.speed),
        "Volume or speed is outside its allowed range"
    );
    ensure!(
        (0.0..=100.0).contains(&settings.scrobble_percentage)
            && (0.0..=36000.0).contains(&settings.scrobble_seconds),
        "Invalid scrobble thresholds"
    );
    ensure!(
        settings
            .equalizer
            .iter()
            .all(|gain| (-24.0..=24.0).contains(gain))
            && (-12.0..=12.0).contains(&settings.equalizer_preamp),
        "Invalid equalizer gain"
    );
    ensure!(
        settings.remote_port > 0
            && settings.remote_username.len() <= 128
            && !settings.remote_username.contains(':'),
        "Invalid remote port or username"
    );
    ensure!(
        settings
            .equalizer_presets
            .iter()
            .all(|(name, gains)| !name.trim().is_empty()
                && gains.iter().all(|gain| (-24.0..=24.0).contains(gain))),
        "Invalid saved equalizer preset"
    );
    for (name, compressor) in &settings.compressor_presets {
        ensure!(
            !name.trim().is_empty(),
            "Invalid saved compressor preset name"
        );
        validate_compressor(*compressor)?;
    }
    validate_compressor(settings.compressor)?;
    settings.lyric_display.validate()?;
    ensure!(
        settings.lyric_displays.len() <= 100,
        "Too many lyric display overrides"
    );
    for (key, display) in &settings.lyric_displays {
        ensure!(
            key.len() <= 128 && !key.chars().any(char::is_control),
            "Invalid lyric display key"
        );
        display.validate()?;
    }
    settings.player_display.validate()?;
    settings.playerbar.validate()?;
    settings.home.validate()?;
    settings.font.validate()?;
    settings.sidebar.validate()?;
    settings.panels.validate()?;
    ensure!(
        settings.lyric_overlay_layers.len() <= 100
            && settings
                .lyric_overlay_layers
                .iter()
                .all(|key| !key.is_empty() && key.len() <= 256),
        "Invalid lyric overlay layers"
    );
    settings.lyric_cloud_translation.validate()?;
    ensure!(
        matches!(
            settings.lyric_alignment.as_str(),
            "left" | "center" | "right"
        ) && (-50.0..=50.0).contains(&settings.lyric_follow_alignment)
            && (0.0..=3.0).contains(&settings.lyric_lead_time),
        "Invalid lyric follow settings"
    );
    ensure!(
        (0.0..=30.0).contains(&settings.crossfade_seconds)
            && settings.lyric_offset.is_finite()
            && settings.lyric_offset.abs() <= 9_007_199_254_740_991. / 1000.,
        "Invalid playback timing"
    );
    ensure!(
        matches!(settings.replay_gain.as_str(), "no" | "track" | "album"),
        "Invalid ReplayGain mode"
    );
    ensure!(
        settings
            .shortcuts
            .values()
            .all(|binding| binding.key.len() <= 128 && !binding.key.chars().any(char::is_control)),
        "Invalid shortcuts"
    );
    let options = json!({"arguments":settings.mpv_extra_args,"properties":settings.mpv_properties});
    settings.apply_mpv_options(&options.to_string())?;
    Ok(())
}

fn validate_compressor(c: crate::dsp::Compressor) -> Result<()> {
    ensure!(
        (-60.0..=0.0).contains(&c.threshold)
            && (1.0..=20.0).contains(&c.ratio)
            && (0.01..=2000.0).contains(&c.attack)
            && (1.0..=9000.0).contains(&c.release)
            && (0.0..=30.0).contains(&c.makeup)
            && (1.0..=10.0).contains(&c.knee),
        "Invalid compressor values"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn imports_remaining_time_from_original_app_store_and_native_export() {
        for value in [
            json!({"showTimeRemaining":true}),
            json!({"state":{"showTimeRemaining":true}}),
            json!({"store_app":"{\"state\":{\"showTimeRemaining\":true}}"}),
            json!({"settings":{"show_time_remaining":true}}),
        ] {
            assert!(
                preview(&Settings::default(), &serde_json::to_vec(&value).unwrap())
                    .unwrap()
                    .settings
                    .show_time_remaining
            );
        }
        assert!(preview(&Settings::default(), br#"{"showTimeRemaining":"true"}"#).is_err());
    }

    #[test]
    fn filter_sidebar_defaults_and_imports_preserve_explicit_visibility() {
        let defaults = Settings::default();
        assert_eq!(
            defaults.page_sidebars,
            [("album".into(), true), ("song".into(), true)].into()
        );
        for value in [
            json!({"pageSidebar":{"album":false,"genreAlbum":true}}),
            json!({"store_app":{"state":{"pageSidebar":{"album":false,"genreAlbum":true}}}}),
            json!({"store_app":"{\"state\":{\"pageSidebar\":{\"album\":false,\"genreAlbum\":true}}}"}),
            json!({"settings":{"page_sidebars":{"album":false,"song":true,"genreAlbum":true}}}),
        ] {
            let imported = preview(&defaults, &serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(!imported.settings.page_sidebars["album"]);
            assert!(imported.settings.page_sidebars["song"]);
            assert!(imported.settings.page_sidebars["genreAlbum"]);
        }
        assert!(preview(&defaults, br#"{"pageSidebar":{"album":"false"}}"#).is_err());
    }

    #[test]
    fn playlist_preferences_import_boolean_and_local_storage_text() {
        for value in [serde_json::json!(false), serde_json::json!("false")] {
            let imported = super::preview(
                &crate::storage::Settings::default(),
                &serde_json::to_vec(&serde_json::json!({"playlist-skip-duplicate":value})).unwrap(),
            )
            .unwrap();
            assert!(!imported.settings.playlist_skip_duplicates);
        }
        assert!(crate::storage::Settings::default().playlist_skip_duplicates);
        assert!(
            super::preview(
                &crate::storage::Settings::default(),
                br#"{"playlist-skip-duplicate":"sometimes"}"#
            )
            .is_err()
        );
        for value in [serde_json::json!(true), serde_json::json!("true")] {
            let imported = super::preview(
                &crate::storage::Settings::default(),
                &serde_json::to_vec(&serde_json::json!({"playlist-header-collapsed":value}))
                    .unwrap(),
            )
            .unwrap();
            assert!(imported.settings.playlist_header_collapsed);
            assert!(imported.omitted.is_empty());
            assert!(
                imported
                    .changes
                    .iter()
                    .any(|(key, _, _)| key == "playlist_header_collapsed")
            );
        }
        assert!(!crate::storage::Settings::default().playlist_header_collapsed);
        assert!(
            super::preview(
                &crate::storage::Settings::default(),
                br#"{"playlist-header-collapsed":"sometimes"}"#
            )
            .is_err()
        );
    }

    #[test]
    fn playback_engine_import_preserves_custom_paths_and_native_preferences() {
        let current = Settings {
            mpv_path: "/custom/mpv".into(),
            saved_mpv_path: "/older/mpv".into(),
            jukebox: true,
            ..Settings::default()
        };
        let imported = preview(&current, br#"{"playback":{"type":"web"}}"#).unwrap();
        assert!(!imported.settings.jukebox);
        assert!(imported.settings.mpv_path.is_empty());
        assert_eq!(imported.settings.saved_mpv_path, "/custom/mpv");
        assert!(imported.omitted.is_empty());
        assert!(imported.changes.iter().any(|(key, _, _)| key == "mpv_path"));
        let mut settings = imported.settings;
        for _ in 0..2 {
            settings = preview(&settings, br#"{"playback":{"type":"local"}}"#)
                .unwrap()
                .settings;
            assert_eq!(settings.mpv_path, "/custom/mpv");
            settings = preview(&settings, br#"{"playback":{"type":"web"}}"#)
                .unwrap()
                .settings;
            assert!(settings.mpv_path.is_empty());
            assert_eq!(settings.saved_mpv_path, "/custom/mpv");
        }
        let jukebox = preview(&current, br#"{"playback":{"type":"jukebox"}}"#).unwrap();
        assert!(jukebox.settings.jukebox);
        assert_eq!(jukebox.settings.mpv_path, current.mpv_path);
        assert_eq!(jukebox.settings.saved_mpv_path, current.saved_mpv_path);
        let local = preview(&Settings::default(), br#"{"playback":{"type":"local"}}"#).unwrap();
        assert_eq!(local.settings.mpv_path, "mpv");
        assert_eq!(local.settings.saved_mpv_path, "mpv");
        let general = preview(&current, br#"{"general":{"language":"fr"}}"#).unwrap();
        assert_eq!(general.settings.mpv_path, current.mpv_path);
        assert_eq!(general.settings.saved_mpv_path, current.saved_mpv_path);
        let native = preview(
            &settings,
            &serde_json::to_vec(&json!({"settings": current})).unwrap(),
        )
        .unwrap();
        assert_eq!(native.settings.mpv_path, current.mpv_path);
        assert_eq!(native.settings.saved_mpv_path, current.saved_mpv_path);
        assert!(native.settings.jukebox);
    }

    #[test]
    fn complete_original_default_preferences_import_with_explicit_platform_omissions() {
        let imported = super::preview(
            &Settings::default(),
            include_bytes!("../assets/electron-default-settings.json"),
        )
        .unwrap();
        // Empty secret slots contain no credential to migrate. Web Audio selects
        // the browser's processing pipeline.
        assert_eq!(
            imported.omitted,
            [
                "general.lastfmApiKey",
                "lyrics.translationApiKey",
                "playback.webAudio"
            ]
        );
        assert_eq!(imported.settings.language, "en");
        assert_eq!(imported.settings.home.items.len(), 7);
        assert_eq!(
            imported.settings.visualizer.audiomotionanalyzer["presets"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(imported.settings.font.built_in, "Inter");
        assert_eq!(imported.settings.player_display.fields.len(), 16);
        let native = super::preview(
            &Settings::default(),
            &serde_json::to_vec(&json!({"settings":imported.settings})).unwrap(),
        )
        .unwrap();
        assert!(native.omitted.is_empty());
        assert_eq!(
            serde_json::to_value(native.settings).unwrap(),
            serde_json::to_value(imported.settings).unwrap()
        );
    }

    #[test]
    fn random_play_store_imports_reviewable_options_and_rejects_invalid_values() {
        let options = json!({"state":{"playbackKind":"albums","limit":17,"minYear":1991,"enableMinYear":true,"maxYear":1980,"enableMaxYear":true,"genre":"Jazz","played":"never","musicFolderId":["music", "other"]},"version":2});
        for stored in [options.clone(), json!(options.to_string())] {
            let result = preview(
                &Settings::default(),
                &serde_json::to_vec(&json!({"store_shuffle_all":stored})).unwrap(),
            )
            .unwrap();
            assert_eq!(result.settings.shuffle.limit, 17);
            assert_eq!(result.settings.shuffle.kind, crate::auto_dj::Mode::Albums);
            assert_eq!(result.settings.shuffle.min(), Some(1991));
            assert_eq!(result.settings.shuffle.max(), None);
            assert_eq!(result.settings.shuffle.music_folder, "music");
            assert!(result.changes.iter().any(|(key, _, _)| key == "shuffle"));
            let native = json!({"settings":result.settings});
            let restored =
                preview(&Settings::default(), &serde_json::to_vec(&native).unwrap()).unwrap();
            assert_eq!(restored.settings.shuffle.genre, "Jazz");
        }
        let cleared = preview(
            &Settings::default(),
            br#"{"store_shuffle_all":{"state":{"genre":null,"played":null}}}"#,
        )
        .unwrap();
        assert!(cleared.settings.shuffle.genre.is_empty());
        assert_eq!(cleared.settings.shuffle.played, crate::shuffle::Played::All);
        for invalid in [
            json!({"limit":0}),
            json!({"limit":501}),
            json!({"musicFolderId":["music",17]}),
            json!({"playbackKind":"anything"}),
        ] {
            assert!(
                preview(
                    &Settings::default(),
                    &serde_json::to_vec(&json!({"store_shuffle_all":{"state":invalid}})).unwrap()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn electron_home_and_waveform_preferences_import_without_losing_order() {
        use super::*;
        let imported = preview(&Settings::default(), br#"{"general":{"homeFeature":false,"homeFeatureStyle":"multiple","homeItems":[{"id":"playlists","disabled":false},{"id":"genres","disabled":true}],"playerbarSlider":{"type":"waveform","barWidth":4.5,"stretched":true},"skipButtons":{"enabled":true,"skipBackwardSeconds":7,"skipForwardSeconds":23}}}"#).unwrap();
        assert!(!imported.settings.home.feature);
        assert_eq!(imported.settings.home.feature_style, "multiple");
        assert_eq!(
            imported.settings.home.items[0].id,
            crate::home::Kind::Playlists
        );
        assert!(imported.settings.home.items[1].disabled);
        assert_eq!(imported.settings.playerbar.kind, "waveform");
        assert_eq!(imported.settings.playerbar.bar_width, 4.5);
        assert!(imported.settings.playerbar.stretched && imported.settings.playerbar.skip_enabled);
        assert_eq!(imported.settings.playerbar.skip_backward_seconds, 7.);
        assert_eq!(imported.settings.playerbar.skip_forward_seconds, 23.);
        let native = preview(
            &Settings::default(),
            &serde_json::to_vec(&serde_json::json!({"settings": imported.settings})).unwrap(),
        )
        .unwrap();
        assert_eq!(native.settings.home, imported.settings.home);
        let font = preview(&Settings::default(), br#"{"general":{},"font":{"type":"system","builtIn":"Poppins","system":"Helvetica Neue","custom":null}}"#).unwrap();
        assert_eq!(font.settings.font.kind, "system");
        assert_eq!(font.settings.font.system.as_deref(), Some("Helvetica Neue"));
        let panels = preview(&Settings::default(), br#"{"general":{"sideQueueType":"sideDrawerQueue","sideQueueLayout":"vertical","showQueueInSidebar":false,"showLyricsInSidebar":true,"showVisualizerInSidebar":true,"combinedLyricsAndVisualizer":true,"sidebarPanelOrder":["visualizer","lyrics","queue"]}}"#).unwrap();
        assert_eq!(panels.settings.panels.kind, "sideDrawerQueue");
        assert_eq!(panels.settings.panels.layout, "vertical");
        assert!(panels.settings.panels.combined && !panels.settings.panels.queue);
        assert_eq!(
            panels.settings.panels.order[0],
            crate::panels::Panel::Visualizer
        );
    }
    use super::*;
    #[test]
    fn electron_player_fields_and_fullscreen_display_preserve_visibility_and_order() {
        let value = json!({"general":{"playerItems":[{"id":"title","disabled":true},{"id":"artist","disabled":false},{"id":"album","disabled":false},{"id":"sample_rate","disabled":false},{"id":"bit_rate","disabled":false}]},"fullScreenPlayer":{"coverArtSize":90,"dynamicIsImage":true,"dynamicImageBlur":2.5,"opacity":40,"playerItemAlignment":"right","titleDisplayType":"multiLine","titleLineCount":3,"useImageAspectRatio":true,"activeTab":"queue","expanded":true}});
        let imported = preview(&Settings::default(), &serde_json::to_vec(&value).unwrap()).unwrap();
        let display = imported.settings.player_display.clone();
        assert_eq!(display.cover_art_size, 90.);
        assert_eq!(display.fields[3].id, "sample_rate");
        assert!(!display.visible("title"));
        assert!(display.visible("artist") && display.visible("bit_rate"));
        assert_eq!(display.title_line_count, 3);
        assert_eq!(display.active_tab, "queue");
        assert!(display.dynamic_is_image && display.use_image_aspect_ratio);
        assert!(display.expanded);
        assert!(
            !imported
                .omitted
                .contains(&"fullScreenPlayer.expanded".into())
        );
        let native = preview(
            &Settings::default(),
            &serde_json::to_vec(&json!({"settings": imported.settings})).unwrap(),
        )
        .unwrap();
        assert_eq!(native.settings.player_display, display);
    }
    #[test]
    fn certificate_exception_imports_the_original_boolean_and_local_storage_strings() {
        assert!(!Settings::default().ignore_ssl);
        for value in [json!(true), json!("true"), json!(false), json!("false")] {
            let imported = preview(
                &Settings::default(),
                &serde_json::to_vec(&json!({"ignore_ssl":value})).unwrap(),
            )
            .unwrap();
            assert_eq!(
                imported.settings.ignore_ssl,
                value == json!(true) || value == json!("true")
            );
            assert!(imported.omitted.is_empty());
            let native = preview(
                &Settings::default(),
                &serde_json::to_vec(&json!({"settings":imported.settings})).unwrap(),
            )
            .unwrap();
            assert_eq!(native.settings.ignore_ssl, imported.settings.ignore_ssl);
        }
        assert!(preview(&Settings::default(), br#"{"ignore_ssl":"invalid"}"#).is_err());
    }

    #[test]
    fn command_palette_sections_import_original_persistence_and_reject_unknown_groups() {
        let original =
            br#"{"state":{"commandPaletteSearchSectionsExpanded":{"albums":false,"tracks":true}}}"#;
        let imported = preview(&Settings::default(), original).unwrap();
        let mut state = crate::storage::State::default();
        imported.data.apply(&mut state);
        assert_eq!(state.command_palette_sections.get("albums"), Some(&false));
        assert_eq!(state.command_palette_sections.get("tracks"), Some(&true));
        assert!(!state.command_palette_sections.contains_key("artists"));
        assert!(
            !imported
                .omitted
                .iter()
                .any(|field| field.contains("commandPalette"))
        );
        assert!(
            preview(
                &Settings::default(),
                br#"{"state":{"commandPaletteSearchSectionsExpanded":{"unknown":false}}}"#
            )
            .is_err()
        );
    }

    #[test]
    fn lyric_delay_import_keeps_milliseconds_and_rejects_unrepresentable_values() {
        for milliseconds in [
            -9_007_199_254_740_991_f64,
            -125.,
            0.,
            250.,
            9_007_199_254_740_991.,
        ] {
            let bytes =
                serde_json::to_vec(&json!({"general":{},"lyrics":{"delayMs":milliseconds}}))
                    .unwrap();
            let imported = preview(&Settings::default(), &bytes).unwrap();
            assert_eq!(imported.settings.lyric_offset, milliseconds / 1000.);
            let native = serde_json::to_vec(&json!({"settings":imported.settings})).unwrap();
            assert_eq!(
                preview(&Settings::default(), &native)
                    .unwrap()
                    .settings
                    .lyric_offset,
                imported.settings.lyric_offset
            );
        }
        let mut settings = Settings::default();
        for invalid in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9_007_199_254_741.,
        ] {
            settings.lyric_offset = invalid;
            assert!(validate(&mut settings).is_err());
        }
    }

    #[test]
    fn native_data_restore_keeps_duplicate_queue_entries_position_and_finished_state() {
        use crate::{
            model::{Item, Server, ServerType},
            queue::Enqueue,
            storage::State,
        };
        let mut exported = State::default();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Music".into(),
            url: "https://music.example.com".into(),
            username: "user".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        exported.active_server = Some(server.id);
        exported.servers.push(server.clone());
        exported.signed_out_servers.insert(server.id);
        exported
            .command_palette_sections
            .insert("tracks".into(), false);
        exported
            .sidebar_playlist_order
            .insert(server.id, vec!["last".into(), "first".into()]);
        exported.queue.add(
            vec![
                Item {
                    id: "same".into(),
                    server_id: server.id,
                    ..Default::default()
                };
                2
            ],
            Enqueue::Now,
        );
        exported.toggle_playlist_folder(server.id, "Rock", false);
        exported.toggle_playlist_folder(server.id, "Jazz", true);
        exported.queue.position = 42.0;
        let bytes = serde_json::to_vec(&exported).unwrap();
        let preview = preview(&Settings::default(), &bytes).unwrap();
        assert!(preview.data.summary().contains("2 queue entries"));
        let mut current = State::default();
        preview.data.apply(&mut current);
        assert_eq!(current.servers[0].id, server.id);
        assert!(current.playlist_folder_expanded(server.id, "Rock", false));
        assert!(!current.playlist_folder_expanded(server.id, "Rock", true));
        assert!(current.playlist_folder_expanded(server.id, "Jazz", true));
        assert!(!current.playlist_folder_expanded(server.id, "Jazz", false));

        assert_eq!(current.active_server, Some(server.id));
        assert_eq!(current.signed_out_servers, exported.signed_out_servers);
        assert_eq!(
            current.command_palette_sections,
            exported.command_palette_sections
        );
        assert_eq!(
            current.sidebar_playlist_order,
            exported.sidebar_playlist_order
        );
        assert_eq!(
            serde_json::to_value(&current.queue).unwrap(),
            serde_json::to_value(&exported.queue).unwrap()
        );
        assert_ne!(current.queue.entries[0].key, current.queue.entries[1].key);
        let mut old_export = serde_json::to_value(&exported).unwrap();
        old_export
            .as_object_mut()
            .unwrap()
            .remove("expanded_playlist_folders");
        old_export
            .as_object_mut()
            .unwrap()
            .remove("shared_expanded_playlist_folders");
        old_export["shared_collapsed_playlist_folders"] =
            json!({server.id.to_string(): ["Old shared"]});

        old_export
            .as_object_mut()
            .unwrap()
            .remove("signed_out_servers");
        super::preview(
            &Settings::default(),
            &serde_json::to_vec(&old_export).unwrap(),
        )
        .unwrap()
        .data
        .apply(&mut current);
        assert!(current.playlist_folder_expanded(server.id, "Old owned", false));
        assert!(!current.playlist_folder_expanded(server.id, "Old shared", true));
        assert!(current.signed_out_servers.is_empty());
        exported.queue.current = None;
        let preview = super::preview(
            &Settings::default(),
            &serde_json::to_vec(&exported).unwrap(),
        )
        .unwrap();
        assert!(preview.data.queue.as_ref().unwrap().current.is_none());
        exported.queue.entries[1].key = exported.queue.entries[0].key;
        assert!(
            super::preview(
                &Settings::default(),
                &serde_json::to_vec(&exported).unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn electron_account_definitions_map_ids_consistently_and_leave_credentials_out() {
        let bytes = br#"{"state":{"currentServer":{"id":"electron-id"},"serverList":{"electron-id":{"id":"electron-id","name":"Books","url":"https://local.example.com/music","remoteUrl":"https://remote.example.com/music","preferRemoteUrl":true,"type":"jellyfin","username":"user","musicFolderId":["books"],"enableAudiobooks":true,"preferInstantMix":true,"credential":"must-not-import","ndCredential":"must-not-import"}}},"version":2}"#;
        let first = preview(&Settings::default(), bytes).unwrap();
        let second = preview(&Settings::default(), bytes).unwrap();
        let server = &first.data.servers.as_ref().unwrap()[0];
        assert_eq!(server.id, second.data.servers.as_ref().unwrap()[0].id);
        assert_eq!(first.data.active_server, Some(Some(server.id)));
        assert!(
            server.options.enable_audiobooks
                && server.options.prefer_remote_url
                && server.options.prefer_instant_mix
        );
        assert_eq!(server.options.music_folder_ids, ["books"]);
        assert!(!server.options.save_password);
        let serialized = serde_json::to_string(server).unwrap();
        assert!(!serialized.contains("must-not-import"));
        assert!(first.data.merge_servers);
        let mut state = crate::storage::State::default();
        first.data.apply(&mut state);
        second.data.apply(&mut state);
        assert_eq!(state.servers.len(), 1);
        assert!(
            first
                .omitted
                .iter()
                .any(|value| value.contains("credentials"))
        );
        for choice in [false, true] {
            let mut source: serde_json::Value = serde_json::from_slice(bytes).unwrap();
            source["state"]["serverList"]["electron-id"]["savePassword"] = choice.into();
            let imported =
                preview(&Settings::default(), &serde_json::to_vec(&source).unwrap()).unwrap();
            assert_eq!(
                imported.data.servers.unwrap()[0].options.save_password,
                choice
            );
        }
    }

    #[test]
    fn previews_native_and_electron_preferences_with_secrets_kept_out_of_settings() {
        let current = Settings::default();
        let mut value = json!({"version":42,"general":{"theme":"light","language":"ja","pathReplace":"/music","confirmQueueChanges":false},"playback":{"preservePitch":false,"scrobble":{"enabled":true,"minimumMode":"percentage","scrobbleAtPercentage":60,"scrobbleAtDuration":180},"mpvExtraParameters":["--no-config"],"mpvProperties":{"audioExclusiveMode":"yes","audioSampleRateHz":48000,"gaplessAudio":"yes","replayGainClip":true,"replayGainMode":"album","replayGainPreampDB":-2},"equalizer":{"enabled":true,"preamp":-3,"bands":(0..12).map(|_|json!({"freq":100,"gain":2})).collect::<Vec<_>>()}},"lyrics":{"delayMs":250,"sources":["lrclib.net","NetEase"],"translationApiKey":"secret-key"},"remote":{"port":4334,"username":"phone","password":"remote-secret"},"window":{"tray":true}});
        value["general"]["zoomFactor"] = 125.into();
        value["general"]["lastfmApiKey"] = "fixture-lastfm-private-key".into();
        value["discord"] = json!({"enabled":false,"clientId":"123456789012345678","displayType":"artist","linkType":"musicbrainz_last_fm","showAsListening":true,"showPaused":false,"showServerImage":true,"showStateIcon":false});
        value["lyrics"]["fetch"] = false.into();
        value["lyrics"]["preferLocalLyrics"] = false.into();
        value["lyrics"]["alignment"] = "center".into();
        value["lyrics"]["followScrollAlignment"] = (-20).into();
        value["lyrics"]["lineLeadTimeMs"] = 1200.into();
        value["lyrics"]["showMatch"] = true.into();
        value["lyricsDisplay"] = json!({"default":{"fontSize":32,"fontSizeUnsync":18,"gap":12,"gapUnsync":16,"opacityNonActive":0.3,"scaleNonActive":0.9,"paddingLeft":20,"paddingRight":25}, "sidebar":{"fontSize":14,"gap":6}});
        value["general"]["genreTarget"] = "album".into();
        value["general"]["followCurrentSong"] = false.into();
        value["general"]["resume"] = false.into();
        value["general"]["autoSave"] = json!({"enabled":true,"count":3});
        value["general"]["lastFM"] = false.into();
        value["general"]["nativeSpotify"] = true.into();
        value["general"]["accent"] = "#aabbcc".into();
        value["general"]["buttonSize"] = 24.into();
        value["general"]["useThemeAccentColor"] = true.into();
        value["general"]["useThemePrimaryShade"] = false.into();
        value["general"]["primaryShade"] = 8.into();
        value["general"]["showFavorites"] = false.into();
        value["general"]["showRatings"] = false.into();
        value["general"]["enableGridMultiSelect"] = true.into();
        value["general"]["blurExplicitImages"] = true.into();
        value["general"]["albumBackground"] = true.into();
        value["general"]["albumBackgroundBlur"] = 2.5.into();
        value["general"]["artistBackground"] = false.into();
        value["general"]["artistBackgroundBlur"] = 1.5.into();
        value["general"]["fullscreenAutoOpenTimeout"] = 22.5.into();
        value["general"]["playerbarOpenDrawer"] = true.into();
        value["general"]["volumeWheelStep"] = 7.into();
        value["general"]["volumeWidth"] = 90.into();
        value["general"]["playButtonBehavior"] = "shuffle".into();
        value["general"]["artistRadioCount"] = 17.into();
        value["general"]["artistItems"] = serde_json::json!([{"id":"topSongs","disabled":false},{"id":"biography","disabled":true}]);
        value["general"]["albumGroupImageSize"] = 145.into();
        value["general"]["albumGroupVerticalLayout"] = false.into();
        value["general"]["albumGroupShowFavoriteRating"] = false.into();
        value["general"]["albumGroupItems"] = serde_json::json!([{"id":"duration","disabled":false},{"id":"albumArtists","disabled":true}]);
        value["general"]["microtonalPitchControls"] = true.into();
        value["general"]["shareExpiration"] =
            serde_json::json!({"amount": 2, "unit": "month", "useServerDefault": false});
        value["general"]["followSystemTheme"] = true.into();
        value["general"]["themeDark"] = "nord".into();
        value["general"]["themeLight"] = "ayuLight".into();
        value["general"]["collections"] = json!([{"id":"saved","name":"Imported","type":"album","filterQueryString":"favorite=false&sortBy=year&sortOrder=DESC"}]);
        value["autoDJ"] = json!({"enabled":true,"mode":"albums","songStrategy":"library_random","albumStrategy":"similar","allowDuplicates":true,"onlySimilar":true,"timing":3,"itemCount":8});
        value["playback"]["audioFadeOnStatusChange"] = false.into();
        value["window"] = json!({"tray":false,"exitToTray":true,"minimizeToTray":true,"startMinimized":true,"releaseChannel":"beta","disableAutoUpdate":true});
        let mut album_layout = crate::lists::default_for("album");
        album_layout.items_per_page = 25;
        album_layout.table.columns[0].width = 333.;
        value["lists"] = json!({"album":album_layout});

        value["tagEditor"] = json!({"triggerRescan": false, "tagConfigs": {"my_tag":{"multiValue":true,"autocompleteSource":"tag:mood","customValues":["Happy", "Calm"]}}});
        value["tagEditor"]["tagConfigs"]["musicbrainzReleaseId"] = json!({"multiValue": false, "autocompleteSource": "none", "customValues": ["release-id"]});
        let result = preview(&current, &serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(!result.settings.tray);
        assert_eq!(
            result.settings.updates.channel,
            crate::updates::Channel::Beta
        );
        assert!(!result.settings.updates.automatic);
        assert!(
            result.settings.exit_to_tray
                && result.settings.minimize_to_tray
                && result.settings.start_minimized
        );
        assert!(!result.settings.lyric_fetch && !result.settings.lyric_prefer_local);
        assert_eq!(result.settings.lyric_alignment, "center");
        assert_eq!(result.settings.lyric_follow_alignment, -20.0);
        assert_eq!(result.settings.lyric_lead_time, 1.2);
        assert_eq!(result.settings.lyric_display.font_size, 32.0);
        assert_eq!(result.settings.lyric_display.font_size_unsync, 18.0);
        assert!(result.settings.lyric_show_match);
        assert_eq!(result.settings.zoom, 1.25);
        assert_eq!(result.settings.discord_config.display_type, "artist");
        assert_eq!(
            result.settings.discord_config.link_type,
            "musicbrainz_last_fm"
        );
        assert!(
            result.settings.discord_config.show_as_listening
                && result.settings.discord_config.show_server_image
        );
        assert!(
            !result.settings.discord_config.show_paused
                && !result.settings.discord_config.show_state_icon
        );
        assert_eq!(
            result.data.lastfm_api_key.as_deref(),
            Some("fixture-lastfm-private-key")
        );
        assert!(
            !serde_json::to_string(&result.settings)
                .unwrap()
                .contains("fixture-lastfm-private-key")
        );
        assert_eq!(result.settings.theme, "light");
        assert!(result.settings.follow_system_theme);
        assert!(!result.settings.follow_current_song);
        assert!(!result.settings.resume_queue);
        assert!(result.settings.queue_autosave);
        assert_eq!(result.settings.queue_autosave_count, 3);
        assert!(!result.settings.external_links.last_fm);
        assert!(result.settings.external_links.native_spotify);
        assert_eq!(result.settings.accent, "#aabbcc");
        assert!(result.settings.use_theme_accent_color);
        assert!(!result.settings.use_theme_primary_shade);
        assert_eq!(result.settings.button_size, 24.);
        assert_eq!(result.settings.primary_shade, 8);
        assert!(!result.settings.show_favorites && !result.settings.show_ratings);
        assert!(result.settings.enable_grid_multiselect && result.settings.blur_explicit_images);
        assert!(result.settings.album_background && !result.settings.artist_background);
        assert_eq!(result.settings.album_background_blur, 2.5);
        assert_eq!(result.settings.artist_background_blur, 1.5);
        assert_eq!(result.settings.fullscreen_auto_open_minutes, 22.5);
        assert!(result.settings.playerbar_open_drawer);
        assert_eq!(result.settings.volume_wheel_step, 7.);
        assert_eq!(result.settings.volume_width, 90.);
        assert_eq!(
            result.settings.play_button_behavior,
            crate::queue::PlayBehavior::Shuffle
        );
        assert_eq!(result.settings.artist_radio_count, 17);
        assert_eq!(result.settings.artist_items[0].id, "topSongs");
        assert!(result.settings.artist_items[1].disabled);
        assert_eq!(result.settings.album_group.image_size, 145.);
        assert!(
            !result.settings.album_group.vertical_layout
                && !result.settings.album_group.show_favorite_rating
        );
        assert_eq!(result.settings.album_group.items[0].id, "duration");
        assert!(result.settings.microtonal_pitch_controls);
        assert_eq!(
            result.settings.share_expiration,
            Some(crate::sharing::Expiration {
                amount: 2,
                unit: crate::sharing::Unit::Month,
                use_server_default: false,
            })
        );
        assert_eq!(result.settings.theme_dark, "nord");
        assert_eq!(result.settings.theme_light, "ayuLight");
        assert!(result.settings.genre_albums);
        assert_eq!(
            result.settings.collections[0].view.filters.favorite,
            Some(false)
        );
        assert!(result.settings.auto_dj);
        assert_eq!(
            result.settings.auto_dj_config.mode,
            crate::auto_dj::Mode::Albums
        );
        assert_eq!(result.settings.auto_dj_config.item_count, 8);
        assert_eq!(result.settings.auto_dj_config.timing, 3);
        assert!(
            result.settings.auto_dj_config.allow_duplicates
                && result.settings.auto_dj_config.only_similar
        );
        assert!(!result.settings.fade_on_status);
        assert_eq!(result.settings.lists["album"].items_per_page, 25);
        assert_eq!(result.settings.lists["album"].table.columns[0].width, 333.);
        assert!(!result.settings.tag_rescan);
        assert!(result.settings.tag_configs["MY_TAG"].multi_value);
        assert_eq!(
            result.settings.tag_configs["MUSICBRAINZ_ALBUMID"].custom_values,
            ["release-id"]
        );
        assert_eq!(
            result.settings.tag_configs["MY_TAG"].autocomplete_source,
            "tag:mood"
        );
        assert_eq!(
            result.settings.tag_configs["MY_TAG"].custom_values,
            ["Happy", "Calm"]
        );
        assert_eq!(result.settings.lyric_offset, 0.25);
        assert_eq!(
            result.settings.lyric_sources,
            [Provider::Lrclib, Provider::NetEase]
        );
        assert_eq!(result.settings.equalizer, [2.; 12]);
        assert_eq!(result.settings.mpv_properties["audio-samplerate"], 48000);
        assert_eq!(result.data.remote_credentials.as_ref().unwrap().0, "phone");
        for (username, password) in [("phone", ""), ("", "remote-secret"), ("", "")] {
            let imported = preview(
                &current,
                &serde_json::to_vec(&json!({
                    "general": {},
                    "remote": {"username": username, "password": password}
                }))
                .unwrap(),
            )
            .unwrap();
            assert_eq!(
                imported.data.remote_credentials,
                (!username.is_empty() || !password.is_empty())
                    .then(|| (username.into(), password.into()))
            );
            assert!(
                !serde_json::to_string(&imported.settings)
                    .unwrap()
                    .contains("remote-secret")
            );
        }
        assert_eq!(result.settings.lyric_display_for("sidebar").font_size, 14.);
        assert_eq!(result.settings.lyric_display_for("sidebar").gap, 6.);
        assert_eq!(
            result.settings.lyric_display_for("sidebar").padding_left,
            20.
        );
        assert!(
            !serde_json::to_string(&result.settings)
                .unwrap()
                .contains("remote-secret")
        );
        assert!(!result.omitted.contains(&"window.tray".into()));
        let encoded = serde_json::to_string(&result.settings).unwrap();
        assert!(!encoded.contains("secret-key") && !encoded.contains("remote-secret"));
        assert!(result.settings.should_scrobble(120.0, 200.0));
        assert!(!result.settings.should_scrobble(119.0, 200.0));
        let imported = preview(
            &current,
            &serde_json::to_vec(&json!({"settings":result.settings})).unwrap(),
        )
        .unwrap();
        assert_eq!(imported.settings.remote_port, 4334);
        assert_eq!(
            imported.settings.lyric_display_for("sidebar").font_size,
            14.
        );
        assert!(imported.omitted.is_empty());
        assert!(preview(&current, b"{}").is_err());
        assert!(preview(&current, br#"{"settings":{"speed":999}}"#).is_err());
        assert_eq!(current.speed, 1.0);
        let mut settings = Settings::default();
        assert!(!settings.should_scrobble(149.0, 200.0));
        assert!(settings.should_scrobble(150.0, 200.0));
        settings.scrobble_mode = crate::storage::ScrobbleMode::Seconds;
        assert!(!settings.should_scrobble(200.0, 200.0));
        assert!(settings.should_scrobble(240.0, 400.0));
        settings.private_mode = true;
        assert!(!settings.should_scrobble(1000.0, 400.0));
    }
    #[test]
    fn custom_and_optional_transcoding_preferences_survive_import_and_old_native_values() {
        for (format, bitrate, rate) in [
            ("alac", Some(257), Some(192000)),
            ("pcm_s16le", Some(0), Some(4000)),
            ("", None, None),
            ("custom-codec", Some(u32::MAX), Some(u32::MAX)),
        ] {
            let input = serde_json::json!({"playback":{"transcode":{"enabled":true,"format":format,"bitrate":bitrate,"maxSampleRate":rate}}});
            let migrated = preview(&Settings::default(), &serde_json::to_vec(&input).unwrap())
                .unwrap()
                .settings;
            assert_eq!(migrated.format, format);
            assert_eq!(migrated.bitrate, bitrate);
            assert_eq!(migrated.max_sample_rate, rate);
            let native = serde_json::to_vec(&migrated).unwrap();
            let restored: Settings = serde_json::from_slice(&native).unwrap();
            assert_eq!(restored.bitrate, bitrate);
        }
        let old: Settings = serde_json::from_str(r#"{"bitrate":192,"format":"mp3"}"#).unwrap();
        assert_eq!(old.bitrate, Some(192));
        for format in [
            "../wav",
            "mp3?token=oops",
            "flac#other",
            "wav\\other",
            "m p3",
        ] {
            let input = serde_json::json!({"playback":{"transcode":{"format":format}}});
            assert!(preview(&Settings::default(), &serde_json::to_vec(&input).unwrap()).is_err());
        }
    }
}
