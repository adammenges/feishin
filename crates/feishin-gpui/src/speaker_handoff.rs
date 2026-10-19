use crate::app::Feishin;
use feishin_core::{
    dlna::{Controller, Device, matching_queue_entry},
    playback::Control,
};
use gpui::Context;

impl Feishin {
    pub(crate) fn connect_speaker(&mut self, device: Device, cx: &mut Context<Self>) {
        self.connect_speaker_with_group(device, None, cx);
    }
    pub(crate) fn connect_speaker_with_group(
        &mut self,
        device: Device,
        group: Option<Vec<Device>>,
        cx: &mut Context<Self>,
    ) {
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        self.cast_connecting = Some((request, device.id.clone()));
        let was_playing = self.playing;
        let probe = device.clone();
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || Controller::new()?.connection_status(&probe)).await;
            this.update(cx, |this, cx| {
                if this
                    .cast_connecting
                    .as_ref()
                    .is_none_or(|(id, _)| *id != request)
                {
                    return;
                }
                this.cast_connecting = None;
                if this.play_request_generation != request {
                    cx.notify();
                    return;
                }
                let status = match result {
                    Ok(status) => status,
                    Err(error) => {
                        log::warn!("Speaker connection failed: {error}");
                        this.message = error.to_string();
                        cx.notify();
                        return;
                    }
                };
                let urls = this
                    .state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| {
                        this.sessions
                            .get(&entry.item.server_id)
                            .and_then(|api| api.stream_url(&entry.item, &this.state.settings).ok())
                            .or_else(|| {
                                (!entry.item.stream_url.is_empty())
                                    .then(|| entry.item.stream_url.clone())
                            })
                    })
                    .collect::<Vec<_>>();
                let adopt = (!was_playing
                    && !this.playing
                    && !status.uri.is_empty()
                    && status.uri != "NOT_IMPLEMENTED"
                    && matches!(
                        status.transport.as_str(),
                        "PLAYING" | "PAUSED_PLAYBACK" | "TRANSITIONING"
                    ))
                .then(|| matching_queue_entry(&urls, &status.uri, &status.next_uri))
                .flatten();
                if this.cast_selected.is_none()
                    && this.state.settings.previous_local_volume.is_none()
                {
                    this.state.settings.previous_local_volume = Some(this.state.settings.volume);
                }
                this.state.settings.volume = status.volume.unwrap_or(50) as f32;
                this.volume_sync_pending = true;
                this.select_speaker(Some(device.clone()), cx);
                if let Some(index) = adopt {
                    let entry = this.state.queue.entries[index].clone();
                    this.state.queue.current = Some(entry.key);
                    this.state.queue.position = status.position;
                    this.radio_item = None;
                    this.begin_track_reports(cx);
                    this.player_generation = this.player.send(Control::AdoptCast {
                        status: Box::new(status),
                        item: Box::new(entry.item),
                        url: urls[index].clone().unwrap_or_default(),
                        key: entry.key.to_string(),
                    });
                    this.fetch_covers(cx);
                    this.load_lyrics(false, cx);
                }
                if let Some(group) = group {
                    this.add_speaker_group(device, group, cx);
                } else if device.sonos {
                    this.load_speaker_properties(device, cx);
                }
                this.save();
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn disconnect_speaker(&mut self, passive: bool, cx: &mut Context<Self>) {
        if self.cast_selected.take().is_none() {
            self.select_speaker(None, cx);
            return;
        }
        self.cancel_speaker_group();
        self.cast_builder = None;
        self.cast_connecting = None;
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let resume = self.playing && !passive;
        self.report("stop", cx);
        let (position, result) = std::sync::mpsc::channel();
        self.player_generation = self
            .player
            .send(Control::DisconnectCast { passive, position });
        self.loaded = false;
        self.buffering = false;
        self.playing = false;
        self.cast_topology_generation += 1;
        self.cast_groups.clear();
        self.cast_members.clear();
        self.cast_member_volumes.clear();
        self.cast_member_mutes.clear();
        self.cast_volume_pending.clear();
        if let Some(volume) = self.state.settings.previous_local_volume.take() {
            self.state.settings.volume = volume;
            self.state.settings.muted = false;
            // The disconnect command runs first, so this cannot change the detached speaker.
            self.player.send(Control::Volume(volume));
            self.volume_sync_pending = true;
        }
        cx.spawn(async move |this, cx| {
            let position = smol::unblock(move || result.recv().ok().flatten()).await;
            this.update(cx, |this, cx| {
                if this.play_request_generation != request || this.cast_selected.is_some() {
                    return;
                }
                if let Some(position) = position.filter(|position| *position > 0.) {
                    this.state.queue.position = position;
                }
                if resume {
                    this.play_active(cx);
                }
                this.cast_devices.clear();
                this.cast_output_open = false;
                this.discover_speakers(cx);
                this.save();
                cx.notify();
            })
            .ok();
        })
        .detach();
        self.save();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn speaker_queue_matching_uses_original_ids_paths_and_next_track() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/network-queue-reference.json")).unwrap();
        for case in cases {
            let urls = if case["empty"] == true {
                Vec::new()
            } else {
                case["urls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|url| url.as_str().map(str::to_owned))
                    .collect()
            };
            let actual = super::matching_queue_entry(
                &urls,
                case["uri"].as_str().unwrap(),
                case["nextUri"].as_str().unwrap(),
            );
            assert_eq!(
                actual,
                case["match"]["index"].as_u64().map(|index| index as usize),
                "{case}"
            );
        }
    }
}

#[cfg(all(test, unix))]
mod ui_tests {
    use super::*;
    use crate::test_speaker::Speaker;
    use ::core::prelude::v1::test;
    use feishin_core::{
        dlna::Status,
        model::{Item, ItemKind},
        queue::Enqueue,
        storage::State,
    };
    use gpui::{prelude::*, *};
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        rc::Rc,
        sync::atomic::Ordering,
        time::{Duration, Instant},
    };

    struct Outputs(Entity<Feishin>);
    impl Render for Outputs {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |app, cx| app.output_view(cx))
        }
    }
    #[track_caller]
    fn wait(cx: &mut VisualTestContext, app: &Entity<Feishin>, ready: impl Fn(&Feishin) -> bool) {
        // The full suite runs multiple native text/layout tests on shared CI CPUs.
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            cx.update(|window, cx| app.update(cx, |app, cx| app.tick(window, cx)));
            if app.read_with(cx, |app, _| ready(app)) {
                break;
            }
            assert!(Instant::now() < deadline, "Speaker handoff timed out: {}", app.read_with(cx, |app, _| format!("selected={:?}, connecting={:?}, picker={}, loaded={}, playing={}, position={}, request={}, player={}, message={:?}", app.cast_selected.as_ref().map(|device| &device.id), app.cast_connecting, app.cast_output_open, app.loaded, app.playing, app.state.queue.position, app.play_request_generation, app.player_generation, app.message)));
            std::thread::sleep(Duration::from_millis(3));
        }
    }
    fn detach(cx: &mut VisualTestContext, app: &Entity<Feishin>) {
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.playing = false;
                app.cast_output_open = app.cast_selected.is_some();
                app.disconnect_speaker(true, cx);
            })
        });
        wait(cx, app, |app| !app.cast_output_open);
    }
    #[gpui::test]
    fn speaker_connections_and_disconnects_follow_original_handoff_choices(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let speaker = Speaker::new();
        let mut device = speaker.device("speaker");
        device.sonos = false;
        let dir =
            std::env::temp_dir().join(format!("feishin-speaker-handoff-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.scrobble = false;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.queue.add(
                ["local", "remote"]
                    .map(|id| Item {
                        id: id.into(),
                        name: id.into(),
                        kind: ItemKind::Song,
                        duration: 120.,
                        container: "mp3".into(),
                        stream_url: format!("http://fixture/stream?id={id}"),
                        ..Default::default()
                    })
                    .to_vec(),
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| app.cast_scanning = true);
            captured.replace(Some(app.clone()));
            let view = cx.new(|_| Outputs(app));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1200.), px(700.)));
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/network-handoff-reference.json")).unwrap();
        for case in reference["connect"].as_array().unwrap() {
            detach(cx, &app);
            speaker.calls.lock().unwrap().clear();
            let success = case["success"].as_bool().unwrap();
            let playing = case["playing"].as_bool().unwrap();
            let kind = case["type"].as_str().unwrap();
            let transport = case["transport"].as_str().unwrap();
            *speaker.snapshot.lock().unwrap() = Status {
                position: 23.,
                duration: 120.,
                uri: if transport == "STOPPED" {
                    String::new()
                } else {
                    "http://speaker/stream?id=remote&token=new".into()
                },
                transport: transport.into(),
                volume: Some(37),
                ..Default::default()
            };
            speaker.fail.store(!success, Ordering::SeqCst);
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.state.settings.volume = 50.;
                    app.state.settings.previous_local_volume = (kind == "dlna").then_some(50.);
                    app.state.settings.mpv_path = if kind == "local" {
                        "/missing-mpv-test-fixture"
                    } else {
                        ""
                    }
                    .into();
                    app.cast_selected = (kind == "dlna").then(|| speaker.device("old"));
                    app.state.queue.current = Some(app.state.queue.entries[0].key);
                    app.state.queue.position = 83.;
                    app.playing = playing;
                    app.loaded = false;
                    app.buffering = false;
                    app.connect_speaker(device.clone(), cx);
                })
            });
            let adopt = success && !playing && transport != "STOPPED";
            wait(cx, &app, |app| {
                app.cast_connecting.is_none()
                    && (!adopt || (app.loaded && app.playing == (transport == "PLAYING")))
            });
            app.read_with(cx, |app, _| {
                assert_eq!(
                    app.state.settings.mpv_path,
                    if kind == "local" {
                        "/missing-mpv-test-fixture"
                    } else {
                        ""
                    },
                    "{case}"
                );
                if success {
                    assert_eq!(app.cast_selected.as_ref().unwrap().id, device.id, "{case}");
                    assert_eq!(app.state.settings.volume, 37.);
                    assert_eq!(app.state.settings.previous_local_volume, Some(50.));
                    assert_eq!(
                        app.state.queue.current,
                        Some(app.state.queue.entries[usize::from(adopt)].key),
                        "{case}"
                    );
                    if adopt {
                        assert_eq!(app.state.queue.position, 23.);
                        assert_eq!(app.playing, transport == "PLAYING");
                    }
                } else {
                    assert_eq!(
                        app.cast_selected.as_ref().map(|d| d.id.as_str()),
                        (kind == "dlna").then_some("uuid:old")
                    );
                    assert_eq!(app.state.settings.volume, 50.);
                    assert!(!app.message.is_empty());
                }
            });
            if adopt {
                assert!(
                    speaker
                        .calls
                        .lock()
                        .unwrap()
                        .iter()
                        .all(|call| call[0] != "Play" && call[0] != "SetAVTransportURI")
                );
            }
            speaker.fail.store(false, Ordering::SeqCst);
        }
        for case in reference["disconnect"].as_array().unwrap() {
            detach(cx, &app);
            *speaker.snapshot.lock().unwrap() = Status {
                position: 23.,
                duration: 120.,
                uri: "http://speaker/stream?id=remote".into(),
                transport: "PLAYING".into(),
                volume: Some(37),
                ..Default::default()
            };
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.playing = false;
                    app.connect_speaker(device.clone(), cx);
                })
            });
            wait(cx, &app, |app| app.cast_connecting.is_none() && app.loaded);
            // Establish the fallback through the actor and await its position event.
            // Started can arrive before the adopted position, so directly assigning
            // queue.position here races that still-pending event.
            speaker.snapshot.lock().unwrap().position = 11.;
            app.update(cx, |app, _| {
                app.player.send(Control::Seek(11.));
            });
            wait(cx, &app, |app| app.state.queue.position == 11.);
            let passive = case["shift"].as_bool().unwrap();
            let position = case["position"].as_f64().unwrap();
            speaker.snapshot.lock().unwrap().position = position;
            speaker.calls.lock().unwrap().clear();
            cx.update(|_, cx| {
                app.update(cx, |app, cx| {
                    app.state.settings.mpv_path = if case["previous"] == "local" {
                        "/missing-mpv-test-fixture"
                    } else {
                        ""
                    }
                    .into();
                    app.state.settings.previous_local_volume = Some(24.);
                    app.message.clear();
                    // The completion callback closes this picker after receiving the position.
                    app.cast_output_open = true;
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let point = cx.debug_bounds("speaker-disconnect").unwrap().center();
            let generation = app.read_with(cx, |app, _| app.play_request_generation);
            cx.simulate_click(
                point,
                Modifiers {
                    shift: passive,
                    ..Default::default()
                },
            );
            wait(cx, &app, |app| {
                app.cast_selected.is_none() && app.state.settings.volume == 24.
            });
            // A second DisconnectCast is not a barrier: it advances the player's
            // generation and can cancel the real disconnect before the actor handles it.
            wait(cx, &app, |app| !app.cast_output_open);
            app.read_with(cx, |app, _| {
                assert_eq!(
                    app.state.queue.position,
                    if position > 0. { position } else { 11. },
                    "{case}"
                );
                if !passive {
                    assert_eq!(
                        app.message, "Sign in to the server for this track",
                        "{case}"
                    );
                }
            });
            let calls = speaker.calls.lock().unwrap().clone();
            assert_eq!(
                calls.iter().filter(|call| call[0] == "Stop").count(),
                usize::from(!passive),
                "{case}: {calls:?}"
            );
            assert!(
                !calls
                    .iter()
                    .any(|call| call[0] == "volume" || call[0] == "mute"),
                "Restoring local volume touched the speaker: {calls:?}"
            );
            app.read_with(cx, |app, _| {
                assert_eq!(app.play_request_generation - generation, 1, "{case}");
                assert!(!app.playing);
                assert!(app.state.settings.previous_local_volume.is_none());
                assert_eq!(
                    app.state.settings.mpv_path,
                    if case["previous"] == "local" {
                        "/missing-mpv-test-fixture"
                    } else {
                        ""
                    }
                );
            });
        }
        // Cancelling a pending connection leaves the current output and volume untouched.
        speaker.hold.store(true, Ordering::SeqCst);
        speaker.calls.lock().unwrap().clear();
        cx.update(|_, cx| app.update(cx, |app, cx| app.connect_speaker(device, cx)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while speaker.calls.lock().unwrap().is_empty() {
            cx.run_until_parked();
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(3));
        }
        cx.update(|_, cx| app.update(cx, |app, cx| app.select_speaker(None, cx)));
        speaker.hold.store(false, Ordering::SeqCst);
        for _ in 0..25 {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(3));
        }
        app.read_with(cx, |app, _| {
            assert!(app.cast_selected.is_none());
            assert_eq!(app.state.settings.volume, 24.);
        });
        std::fs::remove_dir_all(dir).ok();
    }
}
