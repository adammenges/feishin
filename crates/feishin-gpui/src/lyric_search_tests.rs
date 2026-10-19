use super::*;
use ::core::prelude::v1::test;
use feishin_core::{queue::Enqueue, storage};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

fn setup(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
}
fn selected(id: &str) -> Match {
    Match {
        id: id.into(),
        provider: Provider::Lrclib,
        name: "Chosen title".into(),
        artist: "Chosen artist".into(),
        synchronized: true,
        score: 0.1,
    }
}
fn record(raw: &str, id: &str) -> Record {
    Record {
        raw: raw.into(),
        matched: Some(selected(id)),
        variants: vec![],
        selected: 0,
    }
}
fn click(cx: &mut VisualTestContext, selector: &str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn lyric_search_modal_keeps_the_player_and_cancels_without_changing_lyrics(
    cx: &mut TestAppContext,
) {
    setup(cx);
    let dir = std::env::temp_dir().join(format!("feishin-search-entry-{}", uuid::Uuid::new_v4()));
    let saved = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = storage::State::default();
        state.settings.private_mode = true;
        state.queue.add(
            vec![Item {
                id: "playing".into(),
                name: "Original song".into(),
                artist: "Original artist".into(),
                ..Default::default()
            }],
            Enqueue::Now,
        );
        let app = cx.new(|cx| {
            let mut app = Feishin::new(dir.clone(), Ok(state), window, cx);
            app.route = Route::Lyrics;
            app.lyrics = lyrics::parse_lrc("[00:01]Original line");
            app
        });
        saved.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = saved.borrow_mut().take().unwrap();
    cx.run_until_parked();
    let queue = app.read_with(cx, |app, _| app.state.queue.clone());
    click(cx, "lyric-search");
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
    for label in ["Name", "Artist", "Lyric search"] {
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some(label)),
            "missing {label}"
        );
    }
    assert!(cx.debug_bounds("lyrics-frame-false").is_some());
    let body = cx.debug_bounds("native-modal-body-0").unwrap();
    assert!((body.size.height - px(600.)).abs() < px(1.), "{body:?}");
    app.read_with(cx, |app, _| {
        assert_eq!(app.route, Route::Lyrics);
        assert_eq!(app.state.queue.order, queue.order);
        assert_eq!(app.lyrics[0].text, "Original line");
        assert!(!app.lyric_busy);
    });
    click(cx, "lyric-search-cancel");
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    app.read_with(cx, |app, _| assert_eq!(app.lyrics[0].text, "Original line"));
    click(cx, "lyric-search");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn lyric_search_preview_export_debounce_and_confirm_match_original_flow(cx: &mut TestAppContext) {
    setup(cx);
    let dir = std::env::temp_dir().join(format!("feishin-search-flow-{}", uuid::Uuid::new_v4()));
    let target = Item {
        id: "original".into(),
        name: "Original song".into(),
        artist: "Original artist".into(),
        artist_names: vec!["Old first artist".into()],
        ..Default::default()
    };
    let saved = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = storage::State::default();
        state.settings.private_mode = true;
        state.settings.lyric_offset = 0.25;
        state.queue.add(
            vec![
                target.clone(),
                Item {
                    id: "next".into(),
                    name: "Next song".into(),
                    ..Default::default()
                },
            ],
            Enqueue::Now,
        );
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        saved.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = saved.borrow_mut().take().unwrap();
    let dialog = cx.update(|window, cx| open(app.clone(), target.clone(), 1., window, cx));
    let route = app.read_with(cx, |app, _| app.route.clone());
    cx.run_until_parked();
    let frames: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../assets/lyric-search-debounce-reference.json"
    ))
    .unwrap();
    let mut previous = 0;
    for frame in frames {
        let elapsed = frame["event"]["at"].as_u64().unwrap();
        cx.executor()
            .advance_clock(Duration::from_millis(elapsed - previous));
        cx.run_until_parked();
        cx.update(|window, cx| {
            dialog.update(cx, |dialog, cx| {
                for (input, field) in [(&dialog.name, "name"), (&dialog.artist, "artist")] {
                    if let Some(value) = frame["event"][field].as_str() {
                        input.update(cx, |input, cx| {
                            input.set_value(value.to_owned(), window, cx)
                        });
                    }
                }
            })
        });
        cx.run_until_parked();
        dialog.read_with(cx, |dialog, _| {
            let query = dialog.query();
            assert_eq!(query.name, frame["query"][0], "{frame}");
            assert_eq!(query.primary_artist(), frame["query"][1], "{frame}");
        });
        previous = elapsed;
    }
    let references: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/lyric-search-reference.json")).unwrap();
    for reference in references {
        cx.update(|_, cx| {
            dialog.update(cx, |dialog, cx| {
                let results = reference["input"]
                    .as_object()
                    .unwrap()
                    .values()
                    .flat_map(|rows| rows.as_array().unwrap())
                    .map(|row| Match {
                        id: row["id"].as_str().unwrap().into(),
                        provider: Provider::ALL
                            .into_iter()
                            .find(|provider| row["source"] == provider.name())
                            .unwrap(),
                        name: row["name"].as_str().unwrap().into(),
                        artist: row["artist"].as_str().unwrap().into(),
                        synchronized: row["isSync"] == true,
                        score: row["score"].as_f64().unwrap(),
                    })
                    .collect::<Vec<_>>();
                dialog.search_result(dialog.search_generation, Ok(results), cx);
                let expected = reference["order"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|id| id.as_str().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    dialog
                        .results
                        .iter()
                        .map(|result| format!("{} - {}", result.provider.name(), result.id))
                        .collect::<Vec<_>>(),
                    expected
                );
                dialog.search_result(dialog.search_generation.wrapping_sub(1), Ok(vec![]), cx);
                assert_eq!(dialog.results.len(), expected.len());
                dialog.selected = (reference["mode"] != "none").then(|| selected("42"));
                dialog.preview = match reference["mode"].as_str().unwrap() {
                    "plain" => Some(record("Plain lyrics", "42")),
                    "timed" => Some(record("[00:01]Timed lyrics", "42")),
                    _ => None,
                };
                let exported = dialog.export_metadata();
                let expected = reference["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|call| call[0] == "export");
                assert_eq!(exported.is_some(), expected.is_some());
                if let Some((metadata, synchronized)) = exported {
                    let expected = &expected.unwrap()[1];
                    assert_eq!(metadata.artist, expected["lyrics"]["artist"]);
                    assert_eq!(metadata.name, expected["lyrics"]["name"]);
                    if let Some(lines) = metadata.lyrics.as_array() {
                        let expected = expected["lyrics"]["lyrics"].as_array().unwrap();
                        assert_eq!(lines.len(), expected.len());
                        for (line, expected) in lines.iter().zip(expected) {
                            assert_eq!(line["startMs"].as_f64(), expected["startMs"].as_f64());
                            assert_eq!(line["text"], expected["text"]);
                        }
                    } else {
                        assert_eq!(metadata.lyrics, expected["lyrics"]["lyrics"]);
                    }
                    assert_eq!(synchronized, expected["synced"] == true);
                    assert!(metadata.offset_ms.is_none());
                }
                cx.notify();
            })
        });
        cx.run_until_parked();
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        for (id, key) in reference["statuses"].as_object().unwrap() {
            let status = cx.update(|_, cx| crate::appearance::text(key.as_str().unwrap(), cx));
            assert!(
                tree.nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Button
                        && node
                            .label()
                            .is_some_and(|label| label.contains(&format!("- {id} - {status} -")))),
                "{id}: {status}"
            );
        }
        for (label, disabled) in [
            ("Confirm", reference["mode"] == "none"),
            (
                "Export lyrics",
                matches!(reference["mode"].as_str(), Some("none" | "loading")),
            ),
        ] {
            let node = tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == Role::Button && node.label() == Some(label))
                .unwrap();
            assert_eq!(node.1.is_disabled(), disabled, "{reference}");
        }
    }
    for (index, provider) in [(1, Provider::SimpMusic), (0, Provider::Lrclib)] {
        click(cx, &format!("lyric-search-result-{index}"));
        dialog.read_with(cx, |dialog, _| {
            let selected = dialog.selected.as_ref().unwrap();
            assert_eq!(selected.id, "42");
            assert_eq!(selected.provider, provider);
        });
    }
    cx.update(|_, cx| {
        dialog.update(cx, |dialog, cx| {
            let mut second = selected("42");
            second.name = "Second result".into();
            dialog.results = vec![selected("old"), second];
            dialog.selected = None;
            dialog.preview = None;
            cx.notify();
        })
    });
    cx.run_until_parked();
    click(cx, "lyric-search-result-0");
    assert_eq!(
        dialog.read_with(cx, |dialog, _| dialog.selected.as_ref().unwrap().id.clone()),
        "old"
    );
    let focused_before = cx.update(|window, _| {
        let tree = window.accessibility_tree_for_test().unwrap();
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .map(|(_, node)| (node.role(), node.label().map(str::to_owned)))
    });
    cx.simulate_keystrokes("tab");
    cx.run_until_parked();
    let focused_after = cx.update(|window, _| {
        let tree = window.accessibility_tree_for_test().unwrap();
        tree.nodes
            .iter()
            .find(|(id, _)| *id == tree.focus)
            .map(|(_, node)| (node.role(), node.label().map(str::to_owned)))
    });
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
    cx.run_until_parked();
    assert_eq!(
        dialog.read_with(cx, |dialog, _| dialog.selected.as_ref().unwrap().id.clone()),
        "42",
        "focus: {focused_before:?} -> {focused_after:?}"
    );
    // Preview requests are independent from active playback, and a late result
    // cannot replace a newer selection.
    cx.update(|_, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.select(selected("old"), cx);
            let stale = dialog.preview_generation;
            dialog.select(selected("42"), cx);
            dialog.preview_result(stale, Ok(record("Old result", "old")), cx);
            assert!(dialog.preview.is_none());
            dialog.preview_result(
                dialog.preview_generation,
                Ok(record("[00:01]Timed lyrics", "42")),
                cx,
            );
        })
    });
    cx.run_until_parked();
    // The preview retains word timing and background-singer typography without
    // replacing the currently installed lyrics or applying their saved offset.
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.lyrics = lyrics::parse_lrc("[00:01]Installed lyrics");
            app.state.settings.lyric_words = true;
            app.state.settings.lyric_display.font_size = 64.;
            cx.notify();
        });
        dialog.update(cx, |dialog, cx| {
            let mut preview = record("Backing", "42");
            preview.variants = vec![lyrics::Variant {
                synchronized: true,
                agents: vec![lyrics::Agent {
                    id: "background".into(),
                    role: "bg".into(),
                    ..Default::default()
                }],
                lines: vec![lyrics::Line {
                    time: Some(1.),
                    text: "Backing".into(),
                    words: vec![],
                    cue_lines: vec![lyrics::CueLine {
                        start: 3.,
                        end: Some(4.),
                        text: "Backing".into(),
                        agent_id: Some("background".into()),
                        words: vec![lyrics::Word {
                            time: 3.,
                            end: Some(4.),
                            text: "Backing".into(),
                        }],
                    }],
                }],
                ..Default::default()
            }];
            dialog.preview = Some(preview);
            cx.notify();
        });
    });
    cx.run_until_parked();
    let cue = cx.debug_bounds("lyric-cue-0-0").unwrap();
    let word = cx
        .debug_bounds("lyric-word-progress-0-3.000-Backing")
        .unwrap();
    assert!((cue.size.height - px(24.)).abs() < px(1.), "{cue:?}");
    assert!((word.size.height - px(16.2)).abs() < px(1.), "{word:?}");
    click(cx, "lyric-cue-0-0");
    app.read_with(cx, |app, _| {
        assert_eq!(app.state.queue.position, 3.);
        assert_eq!(app.lyrics[0].text, "Installed lyrics");
    });
    cx.update(|_, cx| {
        dialog.update(cx, |dialog, cx| {
            dialog.preview = Some(record("[00:01]Timed lyrics", "42"));
            cx.notify();
        });
    });
    cx.run_until_parked();
    click(cx, "lyric-search-export");
    click(cx, "lyric-export-copy");
    let copied = cx.update(|_, cx| cx.read_from_clipboard().unwrap().text().unwrap());
    assert_eq!(
        copied,
        "[ar:Chosen artist]\n[ti:Chosen title]\n[offset:250]\n[00:01.000]Timed lyrics\n"
    );
    cx.update(|window, cx| window.close_dialog(cx));
    cx.run_until_parked();
    assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
    // Keep the dialog's original target while another song is now current.
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.state.queue.current = Some(app.state.queue.order[1]);
            app.lyrics = lyrics::parse_lrc("[00:02]Next song lyrics");
            app.lyric_busy = true;
            cx.notify();
        })
    });
    click(cx, "lyric-search-confirm");
    assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    let saved = lyric_sources::read_record(&lyric_sources::cache_path(&dir, &target, true))
        .unwrap()
        .unwrap();
    assert_eq!(saved.raw, "[00:01]Timed lyrics");
    app.read_with(cx, |app, _| {
        assert_eq!(app.lyrics[0].text, "Next song lyrics");
        assert!(app.lyric_busy);
        assert_eq!(app.route, route);
    });
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn lyric_search_late_confirmations_cannot_restore_cleared_or_newer_choices(
    cx: &mut TestAppContext,
) {
    setup(cx);
    let dir = std::env::temp_dir().join(format!("feishin-search-races-{}", uuid::Uuid::new_v4()));
    let saved = Rc::new(RefCell::new(None));
    let target = Item {
        id: "original".into(),
        ..Default::default()
    };
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut state = storage::State::default();
        state.settings.private_mode = true;
        state.queue.add(vec![target.clone()], Enqueue::Now);
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        saved.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = saved.borrow_mut().take().unwrap();
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            let key = (target.server_id, target.id.clone());
            let old = uuid::Uuid::new_v4();
            app.lyric_selections
                .entry(key.clone())
                .or_default()
                .override_generation = Some(old);
            app.clear_lyrics(cx);
            app.finish_lyric_match(&target, old, Ok(record("Cleared request", "old")), cx);
            let path = lyric_sources::cache_path(&dir, &target, true);
            assert!(!path.exists());
            let next = uuid::Uuid::new_v4();
            app.lyric_selections
                .entry(key.clone())
                .or_default()
                .override_generation = Some(next);
            app.finish_lyric_match(&target, old, Ok(record("Stale request", "old")), cx);
            assert!(!path.exists());
            app.finish_lyric_match(&target, next, Ok(record("Current request", "new")), cx);
            assert_eq!(app.lyrics[0].text, "Current request");
            assert!(!app.lyric_busy);
            let late = uuid::Uuid::new_v4();
            app.lyric_selections
                .entry(key)
                .or_default()
                .override_generation = Some(late);
            app.lyric_selections.clear();
            app.finish_lyric_match(&target, late, Ok(record("Pre-clear request", "late")), cx);
            assert_eq!(
                lyric_sources::read_record(&path).unwrap().unwrap().raw,
                "Current request"
            );
        })
    });
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(dir).ok();
}
