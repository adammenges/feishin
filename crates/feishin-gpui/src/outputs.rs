use crate::app::Feishin;
use crate::scroll::VerticalScroll;
use feishin_core::{
    dlna::{self, Controller, Device, Property},
    model::Item,
    playback::Control,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, button::ButtonVariants,
    popover::Popover,
};
use std::time::Duration;

impl Feishin {
    pub(crate) fn output_button(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let app = cx.weak_entity();
        Popover::new("player-output-popover")
            .open(self.cast_output_open)
            .appearance(false)
            .anchor(Corner::BottomRight)
            .centered(Axis::Horizontal)
            .offset(px(10. * self.state.settings.zoom))
            .viewport_margin(px(5. * self.state.settings.zoom))
            .transition_offset(gpui::point(px(0.), px(30. * self.state.settings.zoom)))
            .trigger(
                crate::playerbar::button("player-output", false, window, cx)
                    .debug_selector(|| "player-output".into())
                    .icon(
                        gpui_component::Icon::default()
                            .path("native-icons/cast.svg")
                            .with_size(window.rem_size()),
                    )
                    .tooltip(self.tr("dlna.castToDevice"))
                    .when(self.cast_selected.is_some(), |button| {
                        button.text_color(cx.theme().primary)
                    }),
            )
            .on_open_change(cx.listener(|app, open, _, cx| {
                app.cast_output_open = *open;
                cx.notify();
                if *open {
                    if app.cast_selected.is_none() {
                        app.cast_builder = None;
                    }
                    if let Some(device) = app.cast_selected.clone() {
                        app.load_speaker_properties(device, cx);
                    } else {
                        app.discover_speakers(cx);
                    }
                }
            }))
            .content(move |_, window, cx| {
                window.use_keyed_state("player-output-panel", cx, |_, cx| OutputPanel {
                    _subscription: app
                        .upgrade()
                        .map(|app| cx.observe(&app, |_, _, cx| cx.notify())),
                    app: app.clone(),
                })
            })
            .into_any_element()
    }
    pub fn sync_jukebox(&mut self, cx: &mut Context<Self>) {
        let api = self.api().filter(|api| {
            self.state.settings.jukebox
                && (api.server.kind == feishin_core::model::ServerType::Navidrome
                    || api.features().jukebox)
        });
        let server = api.as_ref().map(|api| api.server.id);
        if server != self.jukebox_server {
            self.jukebox_server = server;
            self.player_generation = self.player.send(Control::Jukebox(api.map(Box::new)));
            self.loaded = false;
            self.buffering = server.is_some();
            self.playing = false;
            self.cast_selected = None;
            cx.notify();
        }
    }
    pub fn select_jukebox(&mut self, cx: &mut Context<Self>) {
        if self.api().is_none_or(|api| {
            api.server.kind != feishin_core::model::ServerType::Navidrome && !api.features().jukebox
        }) {
            self.message = "The selected server does not support Jukebox playback".into();
            cx.notify();
            return;
        }
        self.play_request_generation += 1;
        self.report("stop", cx);
        if self.cast_selected.is_none() && self.jukebox_server.is_none() {
            self.state.settings.previous_local_volume = Some(self.state.settings.volume);
        }
        self.state.settings.jukebox = true;
        if !self.loaded && !self.buffering && !self.playing {
            self.jukebox_server = None;
        }
        self.sync_jukebox(cx);
        self.save();
        cx.notify();
    }
    pub fn apply_jukebox_state(
        &mut self,
        state: feishin_core::jukebox::Snapshot,
        cx: &mut Context<Self>,
    ) {
        let key = state.current().and_then(|track| {
            self.state
                .queue
                .current()
                .filter(|entry| {
                    entry.item.id == track.id && entry.item.server_id == track.server_id
                })
                .or_else(|| {
                    self.state.queue.entries.iter().find(|entry| {
                        entry.item.id == track.id && entry.item.server_id == track.server_id
                    })
                })
                .map(|entry| entry.key)
        });
        if let Some(key) = key {
            self.state.queue.current = Some(key);
            self.state.queue.position = state.position;
            self.radio_item = None;
            self.begin_track_reports(cx);
            self.loaded = true;
            self.arm_next();
            self.fetch_covers(cx);
            self.load_lyrics(false, cx);
            if state.playing {
                self.report("start", cx);
            }
        }
        self.playing = state.playing;
        self.buffering = false;
        self.state.settings.volume = (state.gain * 100.).round();
        if self.state.settings.muted {
            self.apply_audio_settings(cx);
        }
        self.volume_sync_pending = true;
        self.lyric_position_at = std::time::Instant::now();
        self.save();
        cx.notify();
    }
    pub fn current_metadata(&self) -> Option<Item> {
        let mut item = self.current_entry()?.item.clone();
        if item.kind == feishin_core::model::ItemKind::Radio {
            item.album = item.name.clone();
            if !self.radio_metadata.title.is_empty() {
                item.name.clone_from(&self.radio_metadata.title);
            }
            if !self.radio_metadata.artist.is_empty() {
                item.artist.clone_from(&self.radio_metadata.artist);
            } else if item.artist.is_empty() {
                item.artist.clone_from(&item.album);
            }
        }
        Some(item)
    }
    pub fn stream_metadata(&self, item: &Item) -> Item {
        let mut item = item.clone();
        if self.state.settings.transcode {
            if !self.state.settings.format.is_empty() {
                item.container = self.state.settings.format.clone();
            } else if self
                .sessions
                .get(&item.server_id)
                .is_some_and(|api| api.server.kind == feishin_core::model::ServerType::Jellyfin)
            {
                item.container = "mp3".into();
            }
        }
        item
    }
    pub fn arm_next(&self) {
        if self.state.settings.jukebox {
            let next = self.state.queue.next_entry(true).filter(|entry| {
                self.radio_item.is_none()
                    && !self.sleep_stops_at_end()
                    && Some(entry.item.server_id) == self.jukebox_server
                    && entry.item.kind == feishin_core::model::ItemKind::Song
            });
            self.player.send(Control::NextUri {
                url: String::new(),
                item: Box::new(next.map_or_else(Item::default, |entry| entry.item.clone())),
                key: next.map_or_else(String::new, |entry| entry.key.to_string()),
                gain: 1.,
            });
            return;
        }
        let current_radio = self
            .current_entry()
            .is_some_and(|entry| entry.item.kind == feishin_core::model::ItemKind::Radio);
        let next = self
            .state
            .queue
            .next_entry(true)
            .filter(|entry| {
                !current_radio
                    && !self.sleep_stops_at_end()
                    && entry.item.kind != feishin_core::model::ItemKind::Radio
                    // Resumed books load at their checkpoint instead of advancing at zero.
                    && (entry.item.resume_position <= 0.0
                        || self.resumed_entry == Some(entry.key))
            })
            .and_then(|entry| {
                self.sessions
                    .get(&entry.item.server_id)
                    .and_then(|api| api.stream_url(&entry.item, &self.state.settings).ok())
                    .map(|url| {
                        (
                            url,
                            self.stream_metadata(&entry.item),
                            entry.key.to_string(),
                            feishin_core::dsp::replay_gain(&entry.item, &self.state.settings),
                        )
                    })
            });
        let (url, item, key, gain) =
            next.unwrap_or_else(|| (String::new(), Item::default(), String::new(), 1.0));
        self.player.send(Control::NextUri {
            url,
            item: Box::new(item),
            key,
            gain,
        });
    }
    pub(crate) fn discover_speakers(&mut self, cx: &mut Context<Self>) {
        if self.cast_scanning {
            return;
        }
        self.cast_scanning = true;
        self.cast_discovery_generation += 1;
        self.cast_devices.clear();
        self.cast_discovery_groups.clear();
        let task = cx
            .background_executor()
            .spawn(async move { smol::unblock(|| dlna::discover(Duration::from_secs(4))).await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.cast_scanning = false;
                match result {
                    Ok(devices) => {
                        this.cast_devices = devices;
                        this.load_discovery_topology(cx);
                        if this.cast_devices.is_empty() {
                            this.message = "No network speakers found".into();
                        }
                    }
                    Err(error) => this.message = error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn refresh_audio_devices(&mut self, cx: &mut Context<Self>) {
        self.audio_devices_generation += 1;
        let generation = self.audio_devices_generation;
        let settings = self.state.settings.clone();
        let source = settings.mpv_path.clone();
        self.audio_devices_source = Some(source.clone());
        self.audio_devices.clear();
        let task = cx.background_executor().spawn(async move {
            smol::unblock(move || feishin_core::playback::output_devices(&settings)).await
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if source != this.state.settings.mpv_path
                    || generation != this.audio_devices_generation
                {
                    return;
                }
                match result {
                    Ok(devices) => this.audio_devices = devices,
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn select_audio_device(&mut self, device: String, cx: &mut Context<Self>) {
        self.state.settings.audio_device = device;
        let was_playing = self.playing;
        self.select_speaker(None, cx);
        if !was_playing && self.loaded {
            self.player_generation = self.player.send(Control::Stop);
            self.loaded = false;
        }
        self.save();
        cx.notify();
    }
    pub(crate) fn select_playback_device(
        &mut self,
        device: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.settings.mpv_path.is_empty() {
            self.select_audio_device(device, cx);
        } else {
            self.state.settings.mpv_audio_device = device.clone();
            self.mpv_device
                .update(cx, |input, cx| input.set_value(device, window, cx));
            if self.cast_selected.is_some() || self.state.settings.jukebox {
                self.select_speaker(None, cx);
            }
            self.apply_audio_settings(cx);
        }
    }
    pub fn select_speaker(&mut self, device: Option<Device>, cx: &mut Context<Self>) {
        if device.is_none() && self.cast_selected.is_some() {
            self.disconnect_speaker(false, cx);
            return;
        }
        self.cancel_speaker_group();
        self.cast_connecting = None;
        let was_playing = self.playing;
        self.state.settings.jukebox = false;
        self.sync_jukebox(cx);
        self.play_request_generation += 1;
        self.report("stop", cx);
        if device.is_some()
            && self.cast_selected.is_none()
            && self.state.settings.previous_local_volume.is_none()
        {
            self.state.settings.previous_local_volume = Some(self.state.settings.volume);
        } else if device.is_none()
            && let Some(volume) = self.state.settings.previous_local_volume.take()
        {
            self.state.settings.volume = volume;
            self.state.settings.muted = false;
            self.volume_sync_pending = true;
        }
        self.player_generation = self
            .player
            .send(Control::Cast(device.clone().map(Box::new)));
        self.cast_selected = device;
        if self.cast_selected.is_none() {
            self.player
                .send(Control::Volume(self.state.settings.volume));
        }
        self.cast_topology_generation += 1;
        self.cast_groups.clear();
        self.cast_members.clear();
        self.cast_member_volumes.clear();
        self.cast_member_mutes.clear();
        self.cast_volume_pending.clear();
        self.loaded = false;
        self.buffering = false;
        self.playing = false;
        if was_playing {
            self.play_active(cx);
        }
        self.save();
        cx.notify();
    }
    pub fn load_speaker_properties(&mut self, device: Device, cx: &mut Context<Self>) {
        let id = device.id.clone();
        let version = self.cast_property_versions.entry(id.clone()).or_default();
        *version += 1;
        let property_version = *version;
        let read_properties = !self.cast_property_pending.contains_key(&id);
        if read_properties {
            self.cast_property_loading.insert(id.clone());
        }
        let requested_device = device.clone();
        self.cast_topology_generation += 1;
        let generation = self.cast_topology_generation;
        let known = self.cast_devices.clone();
        let task = cx.background_executor().spawn(async move {
            smol::unblock(move || {
                let controller = Controller::new()?;
                let properties = read_properties.then(|| controller.properties(&device));
                let groups = match controller.groups(&device) {
                    Ok(groups) => Some(groups),
                    Err(error) => {
                        log::debug!("Speaker topology unavailable: {error}");
                        None
                    }
                };
                let mut members = std::collections::HashMap::new();
                let mut volumes = std::collections::HashMap::new();
                for member in groups
                    .iter()
                    .flatten()
                    .flat_map(|group| &group.members)
                    .filter(|member| !member.satellite)
                {
                    if members.contains_key(&member.id) {
                        continue;
                    }
                    let candidate = known
                        .iter()
                        .chain(std::iter::once(&device))
                        .find(|device| {
                            device.id.trim_start_matches("uuid:")
                                == member.id.trim_start_matches("uuid:")
                        })
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| controller.describe(&member.location));
                    match candidate {
                        Ok(device) => {
                            if let Ok(volume) = controller.get_volume(&device) {
                                volumes.insert(member.id.clone(), volume);
                            }
                            members.insert(member.id.clone(), device);
                        }
                        Err(error) => log::debug!("Speaker group member unavailable: {error}"),
                    }
                }
                Ok::<_, anyhow::Error>((properties, groups, members, volumes))
            })
            .await
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if !this.speaker_endpoint_current(&requested_device) {
                    return;
                }
                if this.cast_property_versions.get(&id) == Some(&property_version) {
                    this.cast_property_loading.remove(&id);
                    if let Ok((Some(properties), _, _, _)) = &result {
                        this.cast_properties.insert(id, properties.clone());
                    }
                    cx.notify();
                }
                if generation != this.cast_topology_generation {
                    return;
                }
                match result {
                    Ok((_, groups, members, volumes)) => {
                        let Some(groups) = groups else {
                            return;
                        };
                        this.cast_groups = groups;
                        this.cast_members = members;
                        for (id, volume) in volumes {
                            if !this.cast_volume_pending.contains_key(&id) {
                                this.cast_member_volumes.insert(id, volume);
                            }
                        }
                        this.cast_member_volumes
                            .retain(|id, _| this.cast_members.contains_key(id));
                        this.cast_member_mutes
                            .retain(|id, _| this.cast_members.contains_key(id));
                        this.cast_volume_pending.retain(|id, pending| {
                            this.cast_members.get(id) == Some(&pending.device)
                        });
                    }
                    Err(error) => this.message = error.to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn output_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut page = gpui::div()
            .id("outputs")
            .v_flex()
            .gap_4()
            .size_full()
            .native_vertical_scroll()
            .when_some(self.cast_selected.as_ref(), |page, device| {
                page.child(
                    div()
                        .v_flex()
                        .gap_2()
                        .p_3()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded_md()
                        .child(self.tr("dlna.nowCasting"))
                        .child(device.name.clone())
                        .child(
                            crate::styles::button("speaker-disconnect", cx)
                                .debug_selector(|| "speaker-disconnect".into())
                                .label(self.tr("dlna.disconnect"))
                                .on_click(cx.listener(|app, event: &ClickEvent, _, cx| {
                                    app.disconnect_speaker(event.modifiers().shift, cx)
                                })),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(self.tr("dlna.shiftDisconnectHint")),
                        ),
                )
            })
            .child(
                gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(
                        crate::styles::button("local-output", cx)
                            .debug_selector(|| "local-output".into())
                            .label(self.tr("Local audio"))
                            .selected(self.cast_selected.is_none() && !self.state.settings.jukebox)
                            .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                if this.cast_selected.is_some() {
                                    this.disconnect_speaker(event.modifiers().shift, cx);
                                } else {
                                    this.select_speaker(None, cx);
                                }
                            })),
                    )
                    .when(
                        self.api().is_some_and(|api| {
                            api.server.kind == feishin_core::model::ServerType::Navidrome
                                || api.features().jukebox
                        }),
                        |row| {
                            row.child(
                                crate::styles::button("jukebox-output", cx)
                                    .debug_selector(|| "jukebox-output".into())
                                    .label(self.tr("Jukebox"))
                                    .selected(self.state.settings.jukebox)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.select_jukebox(cx)),
                                    ),
                            )
                        },
                    )
                    .child(
                        crate::styles::button("discover-speakers", cx)
                            .primary()
                            .label(if self.cast_scanning {
                                "Discovering..."
                            } else {
                                "Discover network speakers"
                            })
                            .disabled(self.cast_scanning)
                            .on_click(cx.listener(|this, _, _, cx| this.discover_speakers(cx))),
                    ),
            );
        page = page.child(
            gpui::div()
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .gap_3()
                .child(crate::appearance::text("Local audio device", cx))
                .child(
                    crate::styles::button("refresh-audio-devices", cx)
                        .debug_selector(|| "refresh-audio-devices".into())
                        .label(crate::appearance::text("Refresh devices", cx))
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_audio_devices(cx))),
                ),
        );
        let default = if self.state.settings.mpv_path.is_empty() {
            ""
        } else {
            "auto"
        };
        let current = if self.state.settings.mpv_path.is_empty() {
            &self.state.settings.audio_device
        } else {
            &self.state.settings.mpv_audio_device
        };
        for (i, (device, label)) in
            std::iter::once((default.to_owned(), self.tr("setting.audioDeviceDefault")))
                .chain(
                    self.audio_devices
                        .iter()
                        .filter(|(id, _)| {
                            id != default
                                && self.audio_devices_source.as_ref()
                                    == Some(&self.state.settings.mpv_path)
                        })
                        .cloned(),
                )
                .enumerate()
        {
            let selected = *current == device;
            page = page.child(
                crate::styles::button(("audio-device", i), cx)
                    .label(label)
                    .selected(selected)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_playback_device(device.clone(), window, cx)
                    })),
            );
        }
        for (i, device) in self.cast_devices.iter().enumerate() {
            let connect = device.clone();
            let selected = self
                .cast_selected
                .as_ref()
                .is_some_and(|selected| selected.id == device.id);
            let mut actions = gpui::div()
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .gap_3()
                .child(gpui::div().flex_1().child(device.name.clone()))
                .child(
                    crate::styles::button(("speaker-connect", i), cx)
                        .label(
                            if self
                                .cast_connecting
                                .as_ref()
                                .is_some_and(|(_, id)| id == &device.id)
                            {
                                self.tr("dlna.connecting")
                            } else if selected {
                                self.tr("Connected")
                            } else {
                                self.tr("Connect")
                            },
                        )
                        .selected(selected)
                        .disabled(selected || self.cast_connecting.is_some())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.connect_speaker(connect.clone(), cx)
                        })),
                );
            if device.sonos {
                let details = device.clone();
                let join = device.clone();
                let leave = device.clone();
                actions = actions.child(
                    crate::styles::button(("speaker-properties", i), cx)
                        .label(self.tr("Properties and groups"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.load_speaker_properties(details.clone(), cx)
                        })),
                );
                if self
                    .cast_selected
                    .as_ref()
                    .is_some_and(|coordinator| coordinator.sonos)
                    && !selected
                {
                    actions = actions.child(
                        crate::styles::button(("speaker-join", i), cx)
                            .label(self.tr("Join group"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.speaker_group(join.clone(), false, cx)
                            })),
                    );
                }
                actions = actions.child(
                    crate::styles::button(("speaker-leave", i), cx)
                        .label(self.tr("Leave group"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.speaker_group(leave.clone(), true, cx)
                        })),
                );
            }
            let mut card = gpui::div()
                .v_flex()
                .gap_3()
                .p_4()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .child(actions);
            if let Some(properties) = self.cast_properties.get(&device.id) {
                for (j, (value, label)) in
                    [(properties.bass, "Bass"), (properties.treble, "Treble")]
                        .into_iter()
                        .enumerate()
                {
                    if let Some(value) = value {
                        let down = device.clone();
                        let up = device.clone();
                        let minus = if j == 0 {
                            Property::Bass(value - 1)
                        } else {
                            Property::Treble(value - 1)
                        };
                        let plus = if j == 0 {
                            Property::Bass(value + 1)
                        } else {
                            Property::Treble(value + 1)
                        };
                        card = card.child(
                            gpui::div()
                                .h_flex()
                                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                .gap_3()
                                .child(format!("{label}: {value}"))
                                .child(
                                    crate::styles::button(("speaker-minus", i * 2 + j), cx)
                                        .label(crate::appearance::text("-", cx))
                                        .disabled(value <= -10)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.speaker_property(down.clone(), minus, cx)
                                        })),
                                )
                                .child(
                                    crate::styles::button(("speaker-plus", i * 2 + j), cx)
                                        .label(crate::appearance::text("+", cx))
                                        .disabled(value >= 10)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.speaker_property(up.clone(), plus, cx)
                                        })),
                                ),
                        );
                    }
                }
                let mut toggles = gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2();
                for (j, (value, label)) in [
                    (properties.loudness, "Loudness"),
                    (properties.crossfade, "Crossfade"),
                    (properties.led, "LED"),
                    (properties.touch, "Touch controls"),
                ]
                .into_iter()
                .enumerate()
                {
                    if let Some(value) = value {
                        let device = device.clone();
                        let property = match j {
                            0 => Property::Loudness(!value),
                            1 => Property::Crossfade(!value),
                            2 => Property::Led(!value),
                            _ => Property::Touch(!value),
                        };
                        toggles = toggles.child(
                            crate::styles::button(("speaker-property", i * 4 + j), cx)
                                .label(format!("{label}: {value}"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.speaker_property(device.clone(), property, cx)
                                })),
                        );
                    }
                }
                card = card.child(toggles);
            }
            page = page.child(card);
        }
        for group in &self.cast_groups {
            page = page.child(
                gpui::div()
                    .v_flex()
                    .gap_2()
                    .child(format!(
                        "Group {} (coordinator {})",
                        group.id, group.coordinator
                    ))
                    .children(group.members.iter().enumerate().map(|(index, member)| {
                        let id = member.id.clone();
                        let minus_id = id.clone();
                        let plus_id = id.clone();
                        let volume = self.cast_member_volumes.get(&id).copied();
                        gpui::div()
                            .debug_selector(move || format!("speaker-member-{index}"))
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_3()
                            .child(format!(
                                "{}{}",
                                member.name,
                                if member.satellite {
                                    " (paired speaker)"
                                } else {
                                    ""
                                }
                            ))
                            .when_some(volume, |div, volume| {
                                div.child(format!("Volume: {volume}%"))
                                    .child(
                                        crate::styles::button(
                                            SharedString::from(format!("member-minus-{id}")),
                                            cx,
                                        )
                                        .label(crate::appearance::text("-", cx))
                                        .disabled(volume == 0)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.speaker_member_volume(
                                                    minus_id.clone(),
                                                    volume.saturating_sub(5),
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                                    .child(
                                        crate::styles::button(
                                            SharedString::from(format!("member-plus-{id}")),
                                            cx,
                                        )
                                        .label(crate::appearance::text("+", cx))
                                        .disabled(volume == 100)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.speaker_member_volume(
                                                    plus_id.clone(),
                                                    volume.saturating_add(5).min(100),
                                                    cx,
                                                )
                                            }),
                                        ),
                                    )
                            })
                    })),
            );
        }
        page.into_any_element()
    }
}

struct OutputPanel {
    app: WeakEntity<Feishin>,
    _subscription: Option<Subscription>,
}
impl Render for OutputPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().children(
            self.app
                .update(cx, |app, cx| app.speaker_picker_view(window, cx))
                .ok(),
        )
    }
}
