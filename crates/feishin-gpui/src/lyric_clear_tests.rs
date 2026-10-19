use super::*;
use ::core::prelude::v1::test;
use feishin_core::{
    api::Api,
    model::{Server, ServerType, Session},
    queue::Enqueue,
    storage::State,
};
use gpui_component::Root;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

fn variant_record(offsets: &[Option<f64>]) -> Record {
    Record {
        raw: "[00:01]Line 0".into(),
        matched: None,
        selected: 0,
        variants: offsets
            .iter()
            .enumerate()
            .map(|(index, offset)| lyrics::Variant {
                language: ["en", "fr", "de"][index].into(),
                kind: "main".into(),
                synchronized: true,
                offset: *offset,
                lines: lyrics::parse_lrc(&format!("[00:01]Line {index}")),
                agents: vec![],
            })
            .collect(),
    }
}

#[gpui::test]
fn lyric_selections_and_offsets_match_original_callbacks(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-lyric-selections-{}", uuid::Uuid::new_v4()));
    let (root, cx) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(State::default()), window, cx));
        Root::new(app, window, cx)
    });
    let app = root.read_with(cx, |root, _| {
        root.view().clone().downcast::<Feishin>().unwrap()
    });
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/lyric-selection-reference.json")).unwrap();
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            for (case_index, case) in cases.iter().enumerate() {
                let item = Item {
                    id: format!("song-{case_index}"),
                    ..Default::default()
                };
                app.state.queue = Default::default();
                app.state.queue.add(vec![item.clone()], Enqueue::Now);
                app.state.settings.lyric_offset = case["defaultOffset"].as_f64().unwrap() / 1000.;
                let offsets: Vec<_> = case["originalOffsets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_f64().map(|value| value / 1000.))
                    .collect();
                let original = variant_record(&offsets);
                app.lyric_record = Some(original.clone());
                app.lyrics = original.lines();
                for frame in case["frames"].as_array().unwrap() {
                    match frame["action"]["type"].as_str().unwrap() {
                        "initial" => {}
                        "offset" => app
                            .set_song_lyric_offset(frame["action"]["value"].as_f64().unwrap(), cx),
                        "select" => app.select_lyric_variant(
                            frame["action"]["value"].as_u64().unwrap() as usize,
                            cx,
                        ),
                        "refresh" => {
                            let mut record = original.clone();
                            app.restore_lyric_selection(&item, &mut record, true);
                            app.lyrics = record.lines();
                            app.lyric_record = Some(record);
                        }
                        _ => unreachable!(),
                    }
                    let record = app.lyric_record.as_ref().unwrap();
                    assert_eq!(
                        record.selected,
                        frame["index"].as_u64().unwrap() as usize,
                        "{frame}"
                    );
                    assert!(
                        (app.lyric_offset() * 1000. - frame["offset"].as_f64().unwrap()).abs()
                            < 0.0001,
                        "{frame}"
                    );
                    assert_eq!(
                        app.lyric_selections
                            .get(&(item.server_id, item.id.clone()))
                            .and_then(|value| value.offset),
                        frame["stored"].as_f64().map(|value| value / 1000.),
                        "{frame}"
                    );
                    for (index, variant) in record.variants.iter().enumerate() {
                        assert_eq!(
                            variant.offset,
                            frame["offsets"][index].as_f64().map(|value| value / 1000.),
                            "{frame}"
                        );
                    }
                    assert_eq!(
                        app.state.settings.lyric_offset,
                        case["defaultOffset"].as_f64().unwrap() / 1000.
                    );
                }
                let before = app.lyric_offset();
                for value in [f64::NAN, f64::INFINITY, -f64::INFINITY, 1e20] {
                    app.set_song_lyric_offset(value, cx);
                }
                assert_eq!(app.lyric_offset(), before);
            }
        })
    });
    cx.update(|window, _| window.remove_window());
    let _ = std::fs::remove_dir_all(directory);
}

#[gpui::test]
fn lyric_offset_mouse_typing_keyboard_and_track_return_preserve_global_preferences(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-lyric-offset-ui-{}", uuid::Uuid::new_v4()));
    let (root, cx) = cx.add_window_view(|window, cx| {
        let mut state = State::default();
        state.settings.sidebar.playlist_list = false;
        state.settings.lyric_sources.clear();
        state.settings.lyric_offset = 0.25;
        let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        Root::new(app, window, cx)
    });
    let app = root.read_with(cx, |root, _| {
        root.view().clone().downcast::<Feishin>().unwrap()
    });
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Offline lyric files".into(),
        url: "http://127.0.0.1:9".into(),
        username: "fixture".into(),
        kind: ServerType::Subsonic,
        legacy_auth: false,
        options: Default::default(),
    };
    let items: Vec<_> = ["one", "two"]
        .into_iter()
        .map(|id| Item {
            id: id.into(),
            server_id: server.id,
            name: id.into(),
            ..Default::default()
        })
        .collect();
    for item in &items {
        lyric_sources::save_record(
            &lyric_sources::cache_path(&directory, item, true),
            &variant_record(&[None, Some(0.125), None]),
        )
        .unwrap();
    }
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.sessions
                .insert(server.id, Api::new(server, Session::default()).unwrap());
            app.state.queue.add(items, Enqueue::Now);
            app.route = Route::Lyrics;
            app.load_lyrics(false, cx);
            cx.notify();
        })
    });
    let wait = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if !app.read_with(cx, |app, _| app.lyric_busy) {
                break;
            }
            assert!(Instant::now() < deadline, "Saved lyric load did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    wait(cx);
    for (id, expected) in [
        ("lyric-offset-increase", 0.3),
        ("lyric-offset-decrease", 0.25),
    ] {
        let button = cx.debug_bounds(id).unwrap();
        cx.simulate_click(button.center(), Modifiers::none());
        cx.run_until_parked();
        assert_eq!(app.read_with(cx, |app, _| app.lyric_offset()), expected);
    }
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.lyric_song_offset.update(cx, |input, cx| {
                input.focus(window, cx);
                input.set_value("-125", window, cx);
            })
        })
    });
    cx.run_until_parked();
    assert_eq!(app.read_with(cx, |app, _| app.lyric_offset()), -0.125);
    cx.simulate_keystrokes("up");
    cx.run_until_parked();
    assert_eq!(app.read_with(cx, |app, _| app.lyric_offset()), -0.124);
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.select_lyric_variant(2, cx);
            app.set_song_lyric_offset(175., cx);
        })
    });
    for (index, expected, selected) in [(1, 0.25, 0), (0, 0.175, 2)] {
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.queue.current = Some(app.state.queue.order[index]);
                app.load_lyrics(false, cx);
                cx.notify();
            })
        });
        wait(cx);
        app.read_with(cx, |app, cx| {
            assert_eq!(app.lyric_offset(), expected);
            assert_eq!(app.lyric_record.as_ref().unwrap().selected, selected);
            assert_eq!(
                app.lyric_song_offset
                    .read(cx)
                    .value()
                    .parse::<f64>()
                    .unwrap(),
                expected * 1000.
            );
            assert_eq!(app.state.settings.lyric_offset, 0.25);
        });
    }
    cx.update(|_, cx| app.update(cx, |app, cx| app.clear_cache(false, cx)));
    wait(cx);
    app.read_with(cx, |app, _| {
        assert!(app.lyric_selections.is_empty());
        assert_eq!(app.lyric_record.as_ref().unwrap().selected, 0);
        assert_eq!(app.lyric_offset(), 0.25);
    });
    cx.update(|window, _| window.remove_window());
    let _ = std::fs::remove_dir_all(directory);
}

#[gpui::test]
fn clear_lyrics_preserves_local_data_and_suppresses_only_the_selected_song(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-lyric-clear-{}", uuid::Uuid::new_v4()));
    let (root, cx) = cx.add_window_view(|window, cx| {
        let mut state = State::default();
        state.settings.sidebar.playlist_list = false;
        state.settings.lyric_sources = vec![Provider::Lrclib];
        state.settings.private_mode = true; // Providers are visible; requests stay on the local fixture.
        let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        Root::new(app, window, cx)
    });
    let app = root.read_with(cx, |root, _| {
        root.view().clone().downcast::<Feishin>().unwrap()
    });
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Lyrics fixture".into(),
        url: format!("http://{}", listener.local_addr().unwrap()),
        username: "fixture".into(),
        kind: ServerType::Subsonic,
        legacy_auth: false,
        options: Default::default(),
    };
    let item = Item {
        id: "song".into(),
        server_id: server.id,
        name: "Song".into(),
        ..Default::default()
    };
    let requests = Arc::new(AtomicUsize::new(0));
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let received = requests.clone();
    let worker = std::thread::spawn(move || {
        while !worker_stop.load(Ordering::SeqCst) {
            let Ok((mut stream, _)) = listener.accept() else {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            };
            // Accepted sockets inherit the nonblocking listener mode on macOS.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = [0; 8192];
            let count = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..count]);
            assert!(request.contains("/rest/getLyricsBySongId"), "{request}");
            received.fetch_add(1, Ordering::SeqCst);
            let body = r#"{"subsonic-response":{"status":"ok","version":"1.16.1","lyricsList":{"structuredLyrics":[{"synced":true,"line":[{"start":1000,"value":"Server lyrics"}]}]}}}"#;
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        }
    });
    // Always stop this owned fixture, including when a check panics.
    struct Fixture(Arc<AtomicBool>, Option<std::thread::JoinHandle<()>>);
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            let _ = self.1.take().unwrap().join();
        }
    }
    let fixture = Fixture(stop, Some(worker));
    let selected = Record {
        raw: "[00:01]Chosen lyrics".into(),
        matched: Some(Match {
            provider: Provider::Lrclib,
            id: "selected".into(),
            name: "Song".into(),
            artist: "Artist".into(),
            synchronized: true,
            score: 0.,
        }),
        variants: vec![],
        selected: 0,
    };
    let path = lyric_sources::cache_path(&directory, &item, true);
    lyric_sources::save_record(&path, &selected).unwrap();
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.sessions
                .insert(server.id, Api::new(server, Session::default()).unwrap());
            app.state
                .queue
                .add(vec![item.clone(), item.clone()], Enqueue::Now);
            app.state.queue.position = 7.;
            app.state.settings.lyric_offset = 0.25;
            app.lyrics = selected.lines();
            app.lyric_record = Some(selected.clone());
            app.route = Route::Lyrics;
            cx.notify();
        })
    });
    cx.run_until_parked();
    let queue = app.read_with(cx, |app, _| app.state.queue.clone());
    let wait = |cx: &mut gpui::VisualTestContext| {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if !app.read_with(cx, |app, _| app.lyric_busy) {
                break;
            }
            assert!(Instant::now() < deadline, "Lyric request did not complete");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    let button = cx.debug_bounds("lyric-clear").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    wait(cx);
    assert!(!path.exists());
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    app.read_with(cx, |app, _| {
        assert_eq!(app.lyrics[0].text, "Server lyrics");
        assert!(
            app.lyric_remote_suppressed
                .contains(&(item.server_id, item.id.clone()))
        );
        assert!(
            !app.lyric_remote_suppressed
                .contains(&(uuid::Uuid::new_v4(), item.id.clone()))
        );
        assert!(
            !app.lyric_remote_suppressed
                .contains(&(item.server_id, "another-song".into()))
        );
        assert_eq!(app.state.settings.lyric_offset, 0.25);
        assert_eq!(app.state.queue.order, queue.order);
        assert_eq!(app.state.queue.position, queue.position);
    });
    let cached: serde_json::Value = serde_json::from_slice(
        &std::fs::read(lyric_sources::cache_path_revision(&directory, &item, 0)).unwrap(),
    )
    .unwrap();
    assert_eq!(cached["options"]["fetch"], false);
    // A later manual provider choice still wins over the automatic-fetch gate.
    lyric_sources::save_record(&path, &selected).unwrap();
    cx.update(|_, cx| app.update(cx, |app, cx| app.load_lyrics(true, cx)));
    wait(cx);
    assert_eq!(
        app.read_with(cx, |app, _| app.lyrics[0].text.clone()),
        "Chosen lyrics"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    // Imported local text survives Clear, just like server-provided local lyrics.
    let imported = Record {
        raw: "[00:01]Imported lyrics".into(),
        matched: None,
        ..selected
    };
    lyric_sources::save_record(&path, &imported).unwrap();
    cx.update(|_, cx| app.update(cx, |app, cx| app.clear_lyrics(cx)));
    wait(cx);
    assert_eq!(
        app.read_with(cx, |app, _| app.lyrics[0].text.clone()),
        "Imported lyrics"
    );
    assert_eq!(
        lyric_sources::read_record(&path).unwrap().unwrap().raw,
        imported.raw
    );
    // Query-cache clearing also resets the session's automatic-provider choices.
    cx.update(|_, cx| app.update(cx, |app, cx| app.clear_cache(false, cx)));
    wait(cx);
    assert!(app.read_with(cx, |app, _| app.lyric_remote_suppressed.is_empty()));
    drop(fixture);
    cx.update(|window, _| window.remove_window());
    let _ = std::fs::remove_dir_all(directory);
}
