use crate::app::{Feishin, Route};
use feishin_core::{model::*, storage};
use gpui::*;
use uuid::Uuid;
pub const AUDIO_NUMBERS: [(&str, f32, f32, f32); 9] = [
    ("setting.equalizerPreamp", -12.0, 12.0, 0.5),
    ("setting.compressorThreshold", -60.0, 0.0, 1.0),
    ("setting.compressorRatio", 1.0, 20.0, 0.5),
    ("setting.compressorAttack", 0.1, 2000.0, 1.0),
    ("setting.compressorRelease", 1.0, 9000.0, 10.0),
    ("setting.compressorMakeupGain", 0.0, 30.0, 0.5),
    ("setting.compressorKnee", 1.0, 10.0, 0.5),
    ("setting.minimumScrobblePercentage", 25.0, 90.0, 1.0),
    ("setting.minimumScrobbleSeconds", 0.0, 1200.0, 1.0),
];

impl Feishin {
    pub fn play_detail(
        &mut self,
        mode: feishin_core::queue::Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Route::Detail(item) = &self.route else {
            return;
        };
        if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
            self.enqueue_artist_header(item.as_ref().clone(), mode, shuffle, cx);
            return;
        }
        let items = if item.kind == ItemKind::Playlist {
            self.items.iter().flat_map(Item::action_items).collect()
        } else {
            vec![item.as_ref().clone()]
        };
        self.enqueue_items(items, mode, shuffle, cx);
    }

    fn enqueue_artist_header(
        &mut self,
        artist: Item,
        mode: feishin_core::queue::Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.sessions.get(&artist.server_id).cloned() else {
            return;
        };
        let saved = self
            .state
            .settings
            .views
            .get(&format!("{}:detail:albumArtistAlbum", artist.server_id));
        let (sort, descending) = if self.artist_tab == ArtistTab::Albums {
            (self.sort, self.descending)
        } else {
            saved.map_or((Sort::ReleaseDate, true), |view| {
                (view.sort, view.descending)
            })
        };
        let combined = self.state.settings.artist_grouping == "all";
        let groups = self.state.settings.artist_release_types.clone();
        self.play_request_generation += 1;
        let generation = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(generation, cancelled.clone());
        let api = api.with_cancellation(cancelled.clone());
        let task = cx.background_executor().spawn(async move {
            let mut albums = api.all_artist_items(&artist, ArtistTab::Albums)?;
            feishin_core::lists::sort_albums(&mut albums, sort, descending);
            let mut songs = vec![];
            for (_, indices) in
                feishin_core::releases::groups(&albums, &artist.id, combined, &groups)
            {
                for index in indices {
                    songs.extend(api.all_tracks(&albums[index])?.into_iter().filter(|song| {
                        feishin_core::artists::contains(song, &artist.id, false)
                            || feishin_core::artists::contains(song, &artist.id, true)
                    }));
                }
            }
            if shuffle {
                use rand::seq::SliceRandom;
                songs.shuffle(&mut rand::thread_rng());
            }
            Ok::<_, anyhow::Error>(songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.library_fetches.remove(&generation);
                if !cancelled.load(std::sync::atomic::Ordering::Relaxed)
                    && (!matches!(mode, feishin_core::queue::Enqueue::Now)
                        || generation == this.play_request_generation)
                {
                    match result {
                        Ok(songs) if !songs.is_empty() => {
                            this.add_tracks_in_context(songs, mode, None, None, cx)
                        }
                        Ok(_) => {}
                        Err(error) => {
                            log::warn!("Artist header playback failed: {error}");
                            this.message = error.to_string();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn open_similar(&mut self, item: Item, cx: &mut Context<Self>) {
        if item.kind != ItemKind::Song {
            return;
        }
        if self.full_player
            && self.current_entry().is_some_and(|entry| {
                entry.item.id == item.id && entry.item.server_id == item.server_id
            })
        {
            self.state.settings.player_display.active_tab = "related".into();
            self.sync_player_related(cx);
            self.save();
            cx.notify();
        } else {
            self.navigate(Route::Similar(Box::new(item)), cx);
        }
    }
    pub fn replace_playlist(
        &mut self,
        playlist: Item,
        from_queue: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.sessions.get(&playlist.server_id).cloned() else {
            return;
        };
        let songs = if from_queue {
            self.state
                .queue
                .ordered()
                .iter()
                .map(|entry| entry.item.clone())
                .collect::<Vec<_>>()
        } else {
            self.items
                .iter()
                .enumerate()
                .filter(|(index, _)| self.selected.is_empty() || self.selected.contains(index))
                .flat_map(|(_, item)| item.action_items())
                .collect()
        };
        let question = format!(
            "Replace all tracks in '{}' with {} tracks?",
            playlist.name,
            songs.len()
        );
        let confirmation = window.prompt(
            PromptLevel::Warning,
            "Replace playlist tracks",
            Some(&question),
            &["Replace", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if confirmation.await.ok() == Some(0) {
                this.update(cx, |this, cx| {
                    this.library_mutation(
                        move || api.replace_playlist_tracks_checked(&playlist, &songs),
                        "Playlist tracks replaced",
                        None,
                        cx,
                    );
                })
                .ok();
            }
        })
        .detach();
    }
    pub fn export_diagnostics(&self, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_new_path(&self.data_dir, Some("feishin-diagnostics.zip"));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = picker.await else { return; };
            this.update(cx, |this, cx| {
                let state = this.state.clone(); let dir = this.data_dir.clone();
                let servers = this.state.servers.iter().map(|server| serde_json::json!({"type":server.kind,"connected":this.sessions.contains_key(&server.id),"active":this.state.active_server == Some(server.id)})).collect();
                let playback = serde_json::json!({"engine":if this.state.settings.jukebox { "jukebox" } else if this.cast_selected.is_some() { "network" } else if this.state.settings.mpv_path.is_empty() { "native" } else { "mpv" },"playing":this.playing,"loaded":this.loaded,"buffering":this.buffering});
                let task = cx.background_executor().spawn(async move { feishin_core::diagnostics::export(&state, &dir, &path, playback, servers) });
                cx.spawn(async move |this, cx| { let result = task.await; this.update(cx, |this, cx| { this.message = match result { Ok(()) => "Diagnostics exported".into(), Err(error) => format!("Could not export diagnostics: {error}") }; cx.notify(); }).ok(); }).detach();
            }).ok();
        }).detach();
    }
    pub fn change_log_level(&mut self, cx: &mut Context<Self>) {
        self.state.settings.log_level = if self.state.settings.log_level == "info" {
            "debug"
        } else {
            "info"
        }
        .into();
        log::set_max_level(if self.state.settings.log_level == "debug" {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        });
        self.save();
        cx.notify();
    }

    pub fn open_settings_import(&mut self, cx: &mut Context<Self>) {
        self.settings_import_generation += 1;
        self.settings_import_loading = false;
        self.settings_import_error.clear();
        self.settings_import = None;
        self.navigate(Route::SettingsImport, cx);
    }

    pub fn cancel_settings_import(&mut self, cx: &mut Context<Self>) {
        self.settings_import_generation += 1;
        self.settings_import_loading = false;
        self.settings_import_error.clear();
        self.settings_import = None;
        self.route = Route::Settings;
        cx.notify();
    }

    pub fn choose_settings_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.settings_import_generation;
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(
                self.tr("setting.exportImportSettings_importModalTitle")
                    .into(),
            ),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picker.await else {
                return;
            };
            this.update_in(cx, |this, _, cx| {
                if this.settings_import_generation == generation
                    && this.route == Route::SettingsImport
                {
                    this.read_settings_import(&paths, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn read_settings_import(&mut self, paths: &[std::path::PathBuf], cx: &mut Context<Self>) {
        if paths.is_empty() || self.route != Route::SettingsImport {
            return;
        }
        self.settings_import_generation += 1;
        self.settings_import_loading = false;
        if paths.len() != 1 {
            self.settings_import_error = self.tr("dragDropZone.error_oneFileOnly");
            cx.notify();
            return;
        }
        self.settings_import_error.clear();
        self.settings_import_loading = true;
        let generation = self.settings_import_generation;
        let path = paths[0].clone();
        let current = self.state.settings.clone();
        let task = cx.background_executor().spawn(async move {
            use std::io::Read;
            let mut bytes = Vec::new();
            std::fs::File::open(path)?
                .take(16_000_001)
                .read_to_end(&mut bytes)?;
            feishin_core::migration::preview(&current, &bytes)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.settings_import_generation != generation {
                    return;
                }
                this.settings_import_loading = false;
                if this.route != Route::SettingsImport {
                    return;
                }
                match result {
                    Ok(preview) => this.settings_import = Some(preview),
                    Err(error) => {
                        this.settings_import_error =
                            if let Some(error) = error.downcast_ref::<std::io::Error>() {
                                log::warn!("Settings import could not read the file: {error}");
                                feishin_core::i18n::translate_with(
                                    &this.state.settings.language,
                                    "dragDropZone.error_readingFile",
                                    &serde_json::json!({"errorMessage":error.to_string()}),
                                )
                                .unwrap_or_else(|| error.to_string())
                            } else if error
                                .downcast_ref::<serde_json::Error>()
                                .is_some_and(|error| error.is_syntax() || error.is_eof())
                            {
                                this.tr("setting.exportImportSettings_notValidJSON")
                            } else {
                                error.to_string()
                            };
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn apply_settings_import(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(preview) = self.settings_import.take() else {
            return;
        };
        let backup = self
            .data_dir
            .join(format!("state-before-import-{}.json", Uuid::new_v4()));
        let current_path = self.data_dir.join("state.json");
        let backed_up = if current_path.exists() {
            std::fs::read(&current_path)
                .map_err(anyhow::Error::from)
                .and_then(|bytes| storage::atomic_write(&backup, &bytes))
        } else {
            storage::save(&backup, &self.state)
        };
        if let Err(error) = backed_up {
            self.settings_import = Some(preview);
            self.message = format!("Could not save the import backup: {error}");
            cx.notify();
            return;
        }
        let css_path = self.data_dir.join("custom.css");
        let previous_css = match std::fs::read(&css_path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                self.settings_import = Some(preview);
                self.message = format!("Could not read existing custom CSS: {error}");
                cx.notify();
                return;
            }
        };
        if let Err(error) =
            storage::atomic_write(&css_path, preview.settings.css.content.as_bytes())
        {
            self.settings_import = Some(preview);
            self.message = format!("Could not import custom CSS: {error}");
            cx.notify();
            return;
        }
        let previous_key = self.state.settings.discord_config.last_fm_api_key.clone();
        let restart_keychain = cfg!(target_os = "linux")
            && preview.settings.password_store != feishin_core::credentials::active_backend();
        let secrets = match storage::SecretTransaction::import(
            &preview.data,
            preview.settings.password_store,
        ) {
            Ok(transaction) => transaction,
            Err(error) => {
                if let Some(bytes) = &previous_css {
                    if let Err(error) = storage::atomic_write(&css_path, bytes) {
                        log::warn!("CSS import rollback failed: {error}");
                    }
                } else if let Err(error) = std::fs::remove_file(&css_path) {
                    log::warn!("CSS import rollback failed: {error}");
                }
                self.settings_import = Some(preview);
                self.message = format!("Could not securely import credentials: {error}");
                cx.notify();
                return;
            }
        };
        let mut imported = self.state.clone();
        imported.settings = preview.settings;
        imported.settings.discord_config.last_fm_api_key = preview
            .data
            .lastfm_api_key
            .clone()
            .unwrap_or_else(|| previous_key.clone());
        if preview.data.lastfm_api_key.is_some() {
            imported.settings.discord_config.last_fm_key_enabled = Some(true);
        } else if imported.settings.discord_config.last_fm_key_enabled == Some(false) {
            imported.settings.discord_config.last_fm_api_key.clear();
        }
        preview.data.apply(&mut imported);
        if let Err(error) = storage::save(&current_path, &imported) {
            let rollback = if let Some(bytes) = &previous_css {
                storage::atomic_write(&css_path, bytes)
            } else {
                std::fs::remove_file(&css_path).map_err(anyhow::Error::from)
            };
            if let Err(error) = rollback {
                log::warn!("CSS import rollback failed: {error}");
            }
            self.settings_import = Some(feishin_core::migration::Preview {
                settings: imported.settings,
                changes: preview.changes,
                omitted: preview.omitted,
                data: preview.data,
            });
            self.message = format!("Could not save imported settings: {error}");
            cx.notify();
            return;
        }
        secrets.commit();
        let playing_account_changed = self.current_metadata().is_some_and(|item| {
            self.sessions.get(&item.server_id).is_some_and(|api| {
                imported.signed_out_servers.contains(&item.server_id)
                    || !imported
                        .servers
                        .iter()
                        .any(|server| storage::same_account(server, &api.server))
            })
        });
        if preview.data.replaces_playback() || playing_account_changed {
            self.stop_transport(cx);
        }
        self.server_check.cancel();
        self.state = imported;
        self.restore_player_modes();
        self.sessions = std::mem::take(&mut self.sessions)
            .into_iter()
            .filter_map(|(id, api)| {
                let configured = self
                    .state
                    .servers
                    .iter()
                    .find(|server| server.id == id)
                    .filter(|server| {
                        !self.state.signed_out_servers.contains(&id)
                            && storage::same_account(server, &api.server)
                    })
                    .and_then(|server| api.reconfigure(server.clone()).ok());
                if configured.is_none() {
                    api.sign_out();
                }
                configured.map(|api| (id, api))
            })
            .collect();
        self.sync_tls_settings(cx);
        self.reset_playlists();
        self.reset_auto_dj();
        self.save_enabled = true;
        self.refresh_setting_inputs(window, cx);
        if let Some((username, password)) = &preview.data.remote_credentials {
            self.remote_username.update(cx, |input, cx| {
                input.set_value(username.clone(), window, cx)
            });
            self.remote_password.update(cx, |input, cx| {
                input.set_value(password.clone(), window, cx)
            });
        }
        self.install_shortcuts(cx);
        log::set_max_level(if self.state.settings.log_level == "debug" {
            log::LevelFilter::Debug
        } else {
            log::LevelFilter::Info
        });
        self.presence.publish(
            self.current_metadata().as_ref(),
            self.playing,
            self.current_position(),
            &self.state.settings,
        );
        self.route = Route::Settings;
        self.playback_filter_inputs.clear();
        self.annotate_lyrics(cx);
        self.remote_generation += 1;
        self.remote = None;
        let remote_message = if self.state.settings.remote_enabled && !restart_keychain {
            self.restore_remote(window, cx);
            ". Remote settings updated"
        } else {
            ""
        };
        self.message = format!(
            "Imported {} preferences. Previous state saved as {}{}",
            preview.changes.len(),
            backup.file_name().unwrap().to_string_lossy(),
            remote_message
        );
        if restart_keychain {
            self.message
                .push_str(&format!(". {}", self.tr("Restart required")));
        }
        cx.notify();
    }
    pub(crate) fn refresh_setting_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_image_inputs(window, cx);
        self.sync_presence_inputs(window, cx);
        self.init_player_numbers(window, cx);
        self.presence_artwork.clear();
        let s = &self.state.settings;
        self.sample_rate_input.update(cx, |input, cx| {
            input.set_value(
                s.max_sample_rate
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.bitrate_input.update(cx, |input, cx| {
            input.set_value(
                s.bitrate.map(|value| value.to_string()).unwrap_or_default(),
                window,
                cx,
            )
        });
        self.transcode_format_input.update(cx, |input, cx| {
            input.set_value(s.format.clone(), window, cx)
        });
        self.remote_port.update(cx, |input, cx| {
            input.set_value(s.remote_port.to_string(), window, cx)
        });
        self.remote_username.update(cx, |input, cx| {
            input.set_value(s.remote_username.clone(), window, cx)
        });
        for (input, value) in [
            (&self.mpv_path, s.mpv_path.clone()),
            (&self.mpv_device, s.mpv_audio_device.clone()),
            (&self.ffmpeg_path, s.ffmpeg_path.clone()),
            (&self.path_replace, s.path_replace.clone()),
            (&self.path_replace_with, s.path_replace_with.clone()),
            (&self.playlist_separator, s.playlist_separator.clone()),
            (&self.playlist_folder_color, s.playlist_folder_color.clone()),
            (
                &self.playlist_folder_indent_input,
                s.playlist_folder_indent.to_string(),
            ),
            (&self.speed_input, s.speed.to_string()),
            (&self.album_group_size, s.album_group.image_size.to_string()),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.mpv_options.update(cx, |input, cx| input.set_value(serde_json::to_string_pretty(&serde_json::json!({"arguments":s.mpv_extra_args,"properties":s.mpv_properties})).unwrap(), window, cx));
        for (input, value) in self
            .auto_dj_numbers
            .iter()
            .zip([s.auto_dj_config.item_count, s.auto_dj_config.timing])
        {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.auto_dj_folder.update(cx, |input, cx| {
            input.set_value(
                s.auto_dj_config.music_folder.clone().unwrap_or_default(),
                window,
                cx,
            )
        });
        self.accent_input.update(cx, |input, cx| {
            input.set_value(s.accent.clone(), window, cx)
        });
        self.fullscreen_idle_input.update(cx, |input, cx| {
            input.set_value(s.fullscreen_auto_open_minutes.to_string(), window, cx)
        });
        self.queue_autosave_input.update(cx, |input, cx| {
            input.set_value(s.queue_autosave_count.to_string(), window, cx)
        });
        for (input, value) in self.volume_settings_inputs.iter().zip([
            s.volume_width,
            s.artist_radio_count as f32,
            s.button_size,
        ]) {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.volume_wheel_slider.update(cx, |slider, cx| {
            slider.set_value(s.volume_wheel_step, window, cx)
        });
        let c = s.compressor;
        for (input, value) in self.audio_numbers.iter().zip([
            s.equalizer_preamp,
            c.threshold,
            c.ratio,
            c.attack,
            c.release,
            c.makeup,
            c.knee,
            s.scrobble_percentage,
            s.scrobble_seconds,
        ]) {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        for (slider, gain) in self.equalizer.iter().zip(s.equalizer) {
            slider.update(cx, |slider, cx| slider.set_value(gain, window, cx));
        }
        for (input, gain) in self.equalizer_inputs.iter().zip(s.equalizer) {
            input.update(cx, |input, cx| {
                input.set_value(gain.to_string(), window, cx)
            });
        }
        self.preamp_slider.update(cx, |slider, cx| {
            slider.set_value(s.equalizer_preamp, window, cx)
        });
        for (slider, value) in self.compressor_sliders.iter().zip([
            c.threshold,
            c.ratio,
            c.attack,
            c.release,
            c.makeup,
            c.knee,
        ]) {
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        self.scrobble_percentage_slider.update(cx, |slider, cx| {
            slider.set_value(s.scrobble_percentage, window, cx)
        });
        self.volume
            .update(cx, |slider, cx| slider.set_value(s.volume, window, cx));
        self.apply_audio_settings(cx);
        self.apply_theme(window, cx);
        self.set_lyric_settings_key("default", window, cx);
        self.sidebar_width.update(cx, |input, cx| {
            input.set_value(self.state.settings.sidebar.width.to_string(), window, cx)
        });
        self.sidebar_regex.update(cx, |input, cx| {
            input.set_value(
                self.state.settings.sidebar.playlist_filter_regex.clone(),
                window,
                cx,
            )
        });
    }
    pub fn apply_eq_preset(
        &mut self,
        gains: [f32; 12],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.state.settings.equalizer;
        self.state.settings.equalizer = gains;
        self.state.settings.equalizer_preamp = 0.0;
        self.audio_numbers[0].update(cx, |input, cx| input.set_value("0", window, cx));
        self.preamp_slider
            .update(cx, |slider, cx| slider.set_value(0., window, cx));
        for ((input, gain), old) in self.equalizer_inputs.iter().zip(gains).zip(previous) {
            if gain != old {
                input.update(cx, |input, cx| {
                    input.set_value(gain.to_string(), window, cx)
                });
            }
        }
        for (slider, gain) in self.equalizer.iter().zip(gains) {
            slider.update(cx, |slider, cx| slider.set_value(gain, window, cx));
        }
        self.apply_audio_settings(cx);
    }
    pub fn apply_compressor_preset(
        &mut self,
        mut preset: feishin_core::dsp::Compressor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        preset.enabled = self.state.settings.compressor.enabled;
        let previous = self.state.settings.compressor;
        self.state.settings.compressor = preset;
        for ((input, value), old) in self
            .audio_numbers
            .iter()
            .skip(1)
            .zip([
                preset.threshold,
                preset.ratio,
                preset.attack,
                preset.release,
                preset.makeup,
                preset.knee,
            ])
            .zip([
                previous.threshold,
                previous.ratio,
                previous.attack,
                previous.release,
                previous.makeup,
                previous.knee,
            ])
        {
            if value != old {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            }
        }
        for (slider, value) in self.compressor_sliders.iter().zip([
            preset.threshold,
            preset.ratio,
            preset.attack,
            preset.release,
            preset.makeup,
            preset.knee,
        ]) {
            slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        }
        self.apply_audio_settings(cx);
    }
    pub fn save_audio_preset(
        &mut self,
        compressor: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = if compressor {
            &self.compressor_preset_name
        } else {
            &self.preset_name
        }
        .clone();
        let name = input.read(cx).value().trim().to_owned();
        if !name.is_empty() {
            if compressor {
                self.state
                    .settings
                    .compressor_presets
                    .insert(name.clone(), self.state.settings.compressor);
            } else {
                self.state
                    .settings
                    .equalizer_presets
                    .insert(name.clone(), self.state.settings.equalizer);
            }
            self.save();
            input.update(cx, |input, cx| input.set_value("", window, cx));
            self.refresh_audio_preset_options(window, cx);
            self.message = format!("Saved audio preset: {name}");
        }
        cx.notify();
    }
    pub fn load_artist_info(&mut self, cx: &mut Context<Self>) {
        let Some((item, api)) = (match &self.route {
            Route::Detail(item)
                if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) =>
            {
                self.sessions
                    .get(&item.server_id)
                    .map(|api| (item.as_ref().clone(), api.clone()))
            }
            _ => None,
        }) else {
            self.artist_info = None;
            self.artist_info_target = None;
            self.artist_info_error = if self.artist_overview_active() {
                "Sign in to this artist's server".into()
            } else {
                String::new()
            };
            return;
        };
        let target = (item.server_id, item.id.clone());
        if self.artist_info_target.as_ref() == Some(&target) {
            return;
        }
        self.artist_info = None;
        self.artist_info_error.clear();
        self.artist_info_target = Some(target.clone());
        let generation = self.query_cache_generation;
        let task = cx
            .background_executor()
            .spawn(async move { api.artist_info(&item) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.artist_info_target.as_ref() != Some(&target)
                    || this.query_cache_generation != generation
                {
                    return;
                }
                match result {
                    Ok(info) => {
                        this.artist_info_error.clear();
                        if let Some(section) = this.artist_sections.get_mut("similarArtists") {
                            section.items = info.similar.clone();
                            section.loading = false;
                        }
                        this.artist_info = Some(info);
                        this.fetch_covers(cx);
                    }
                    Err(error) => {
                        log::debug!("Artist information unavailable: {error}");
                        this.artist_info_error = error.to_string();
                        if let Some(section) = this.artist_sections.get_mut("similarArtists") {
                            section.error = error.to_string();
                            section.loading = false;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn apply_audio_number(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (label, min, max, _) = AUDIO_NUMBERS[index];
        let input = self.audio_numbers[index].clone();
        if index == 0 {
            if let Ok(value) = input.read(cx).value().parse::<f32>()
                && value.is_finite()
            {
                self.set_preamp(value.clamp(min, max), true, window, cx);
            }
            return;
        }
        if (1..=6).contains(&index) {
            if let Ok(value) = input.read(cx).value().parse::<f32>()
                && value.is_finite()
            {
                self.set_compressor_value(index - 1, value.clamp(min, max), true, window, cx);
            }
            return;
        }
        if index >= 7 && input.read(cx).value().trim().is_empty() {
            return;
        }
        let value = input
            .read(cx)
            .value()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite() && *value >= min && *value <= max);
        let Some(value) = value else {
            self.message = format!("{} must be between {min} and {max}", self.tr(label));
            cx.notify();
            return;
        };
        let settings = &mut self.state.settings;
        match index {
            0 => settings.equalizer_preamp = value,
            1 => settings.compressor.threshold = value,
            2 => settings.compressor.ratio = value,
            3 => settings.compressor.attack = value,
            4 => settings.compressor.release = value,
            5 => settings.compressor.makeup = value,
            6 => settings.compressor.knee = value,
            7 => settings.scrobble_percentage = value,
            _ => settings.scrobble_seconds = value,
        }
        input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
        if index >= 7 {
            self.scrobble_percentage_slider.update(cx, |slider, cx| {
                slider.set_value(self.state.settings.scrobble_percentage, window, cx)
            });
            self.save();
            cx.notify();
        } else {
            self.apply_audio_settings(cx);
        }
    }
    pub fn set_playback_speed(&mut self, speed: f32, window: &mut Window, cx: &mut Context<Self>) {
        if !speed.is_finite() || speed <= 0. {
            self.message = "Enter a playback speed from 0.5 to 2".into();
            cx.notify();
            return;
        }
        self.state.settings.speed = speed.clamp(0.5, 2.);
        self.speed_input.update(cx, |input, cx| {
            input.set_value(self.state.settings.speed.to_string(), window, cx)
        });
        self.player.send(feishin_core::playback::Control::Speed(
            self.state.settings.speed,
        ));
        self.save();
        cx.notify();
    }

    pub fn adjust_musical_speed(
        &mut self,
        semitones: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_playback_speed(
            self.state.settings.speed * 2_f32.powf(semitones / 12.),
            window,
            cx,
        );
    }

    pub fn apply_audio_settings(&mut self, cx: &mut Context<Self>) {
        let gain = self.current_entry().map_or(1.0, |entry| {
            feishin_core::dsp::replay_gain(&entry.item, &self.state.settings)
        });
        self.player
            .send(feishin_core::playback::Control::Configure {
                settings: Box::new(self.state.settings.clone()),
                gain,
            });
        self.save();
        cx.notify();
    }
    pub fn library_mutation(
        &mut self,
        job: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
        success: &'static str,
        destination: Option<Route>,
        cx: &mut Context<Self>,
    ) {
        let route = self.route.clone();
        let task = cx.background_executor().spawn(async move { job() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.refresh_playlists(cx);
                        if this.route == route {
                            if let Some(destination) = destination {
                                this.navigate(destination, cx);
                            } else {
                                this.reload(false, cx);
                            }
                        }
                        this.message = success.into();
                    }
                    Err(error) => {
                        log::warn!("Library mutation failed: {error}");
                        this.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn playlist_index(&self, index: usize) -> usize {
        let config = self.list_config();
        self.items
            .get(index)
            .and_then(|item| item.playlist_position)
            .unwrap_or(
                index
                    + if config.pagination == "paginated" {
                        self.library_page * config.items_per_page
                    } else {
                        0
                    },
            )
    }
    pub(crate) fn playlist_boundary_position(&self, index: usize) -> usize {
        if let Some(item) = self.items.get(index) {
            item.playlist_songs
                .first()
                .and_then(|song| song.playlist_position)
                .unwrap_or_else(|| self.playlist_index(index))
        } else if self.items.is_empty() {
            0
        } else {
            self.items
                .last()
                .and_then(|item| item.playlist_songs.last())
                .and_then(|song| song.playlist_position)
                .unwrap_or_else(|| self.playlist_index(self.items.len() - 1))
                + 1
        }
    }
    pub fn playlist_selection_action(
        &mut self,
        action: feishin_core::api::PlaylistSelectionAction,
        cx: &mut Context<Self>,
    ) {
        if self.playlist_tracks_editing() {
            self.edit_playlist_draft_selection(action, cx);
            return;
        }
        let Route::Detail(playlist) = self.route.clone() else {
            return;
        };
        let Some(api) = self.sessions.get(&playlist.server_id).cloned() else {
            return;
        };
        if playlist.kind != ItemKind::Playlist || !self.can_edit_playlist(&playlist) {
            return;
        }
        let expected = self
            .items
            .iter()
            .enumerate()
            .flat_map(|(index, item)| {
                if item.playlist_songs.is_empty() {
                    let mut song = item.clone();
                    song.playlist_position = Some(self.playlist_index(index));
                    vec![song]
                } else {
                    item.playlist_songs.clone()
                }
            })
            .collect::<Vec<_>>();
        let selected = self
            .selected
            .iter()
            .flat_map(|index| {
                self.items
                    .get(*index)
                    .map(|item| {
                        if item.playlist_songs.is_empty() {
                            vec![self.playlist_index(*index)]
                        } else {
                            item.playlist_songs
                                .iter()
                                .filter_map(|song| song.playlist_position)
                                .collect()
                        }
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();
        self.library_mutation(
            move || api.edit_playlist_selection_checked(&playlist, &expected, &selected, action),
            "Playlist updated",
            None,
            cx,
        );
    }
    pub fn remove_playlist_track(&mut self, song: Item, index: usize, cx: &mut Context<Self>) {
        if self.items.get(index).is_none_or(|item| {
            item.id != song.id || item.playlist_entry_id != song.playlist_entry_id
        }) {
            return;
        }
        self.selected.clear();
        self.selected.insert(index);
        self.playlist_selection_action(feishin_core::api::PlaylistSelectionAction::Remove, cx);
    }

    pub fn edit_radio(
        &mut self,
        station: Option<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if station
            .as_ref()
            .is_some_and(|item| !self.can_edit_radio(item))
        {
            return;
        }
        self.form_name.update(cx, |input, cx| {
            input.set_value(
                station
                    .as_ref()
                    .map_or("", |item| item.name.as_str())
                    .to_owned(),
                window,
                cx,
            )
        });
        self.form_comment.update(cx, |input, cx| {
            input.set_value(
                station
                    .as_ref()
                    .map_or("", |item| item.stream_url.as_str())
                    .to_owned(),
                window,
                cx,
            )
        });
        self.form_homepage.update(cx, |input, cx| {
            input.set_value(
                station
                    .as_ref()
                    .map(|item| item.homepage.clone())
                    .unwrap_or_default(),
                window,
                cx,
            )
        });
        self.form_target = station;
        self.navigate(Route::RadioEditor, cx);
    }

    pub fn save_radio(&mut self, cx: &mut Context<Self>) {
        if self
            .form_target
            .as_ref()
            .is_some_and(|item| !self.can_edit_radio(item))
        {
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        let mut station = self.form_target.clone().unwrap_or_else(|| Item {
            id: Uuid::new_v4().to_string(),
            server_id: api.server.id,
            kind: ItemKind::Radio,
            ..Default::default()
        });
        station.name = self.form_name.read(cx).value().trim().to_owned();
        station.stream_url = self.form_comment.read(cx).value().trim().to_owned();
        station.homepage = self.form_homepage.read(cx).value().trim().to_owned();
        if station.name.is_empty() {
            self.message = "Enter a station name".into();
            cx.notify();
            return;
        }
        if let Err(error) = api.stream_url(&station, &self.state.settings) {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        if api.server.kind == ServerType::Jellyfin {
            self.state
                .radio
                .retain(|item| item.server_id != station.server_id || item.id != station.id);
            self.state.radio.push(station);
            self.save();
            self.navigate(Route::Library(ItemKind::Radio), cx);
        } else {
            let target = self.form_target.clone();
            self.library_mutation(
                move || {
                    if let Some(target) = target {
                        api.update_radio(
                            &target,
                            &station.name,
                            &station.stream_url,
                            &station.homepage,
                        )
                    } else {
                        api.create_radio(&station.name, &station.stream_url, &station.homepage)
                    }
                },
                "Radio station saved",
                Some(Route::Library(ItemKind::Radio)),
                cx,
            );
        }
    }

    pub fn delete_library_item(&mut self, item: Item, window: &mut Window, cx: &mut Context<Self>) {
        if item.kind == ItemKind::Playlist {
            self.delete_playlists(vec![item], window, cx);
            return;
        }
        if !self.can_edit_radio(&item) {
            return;
        }
        let confirmation = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {}?", item.name),
            Some("This removes the playlist or radio station from your library."),
            &["Cancel", "Delete"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if confirmation.await != Ok(1) {
                return;
            }
            this.update(cx, |this, cx| {
                if !this.can_edit_radio(&item) {
                    return;
                }
                let Some(api) = this.sessions.get(&item.server_id).cloned() else {
                    return;
                };
                if item.kind == ItemKind::Radio && api.server.kind == ServerType::Jellyfin {
                    this.state.radio.retain(|station| {
                        station.server_id != item.server_id || station.id != item.id
                    });
                    this.save();
                    this.reload(false, cx);
                } else {
                    let destination = Some(Route::Library(item.kind));
                    this.library_mutation(
                        move || api.delete_radio(&item),
                        "Deleted",
                        destination,
                        cx,
                    );
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn remove_server(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(server) = self
            .state
            .servers
            .iter()
            .find(|server| server.id == id)
            .cloned()
        else {
            return;
        };
        let confirmation = window.prompt(
            PromptLevel::Warning,
            &format!("Remove {}?", server.name),
            Some("Saved credentials and this server's queued tracks will be removed."),
            &["Cancel", "Remove"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if confirmation.await != Ok(1) {
                return;
            }
            this.update(cx, |this, cx| {
                let current = this
                    .state
                    .queue
                    .current()
                    .is_some_and(|entry| entry.item.server_id == id);
                if current {
                    this.report("stop", cx);
                }
                let keys = this
                    .state
                    .queue
                    .entries
                    .iter()
                    .filter(|entry| entry.item.server_id == id)
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>();
                for key in keys {
                    this.state.queue.remove(key);
                }
                let api = this.sessions.remove(&id);
                this.lyric_selections.retain(|(server, _), _| *server != id);
                if let Some(api) = &api {
                    api.sign_out();
                }
                if this.server_check.busy == Some(id) {
                    this.server_check.cancel();
                }
                this.state.signed_out_servers.remove(&id);
                this.state.servers.retain(|server| server.id != id);
                this.state.radio.retain(|station| station.server_id != id);
                if this.state.active_server == Some(id) {
                    this.reset_library_scan();
                    this.state.active_server = this.state.servers.first().map(|server| server.id);
                }
                if current {
                    this.play_current(cx);
                }
                this.save();
                this.perform(
                    move || {
                        api.map_or_else(
                            || storage::delete_session(&server),
                            |api| api.forget_saved_session(),
                        )
                    },
                    "Server removed",
                    cx,
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
