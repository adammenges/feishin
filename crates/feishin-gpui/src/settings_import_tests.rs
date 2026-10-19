use crate::app::{Feishin, Route};
use feishin_core::{model::Item, queue::Enqueue, settings_search::Tab, storage::State};
use gpui::{AppContext, ExternalPaths, FileDropEvent, Modifiers, TestAppContext};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc, time::Duration};

#[gpui::test]
fn import_drop_matches_original_errors_and_preserves_preferences_until_apply(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-import-drop-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).unwrap();
    let valid = directory.join("settings.json");
    let invalid = directory.join("invalid.json");
    let unreadable = directory.join("missing.json");
    std::fs::write(&valid, r#"{"general":{"theme":"dracula"}}"#).unwrap();
    std::fs::write(&invalid, "bad json").unwrap();
    let captured = Rc::new(RefCell::new(None));
    let capture = captured.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        window.activate_accessibility_for_test();
        let mut state = State::default();
        state.settings.home.items.clear();
        state.settings.home.feature = false;
        state.settings.lyric_fetch = false;
        state.settings.scrobble = false;
        state.settings.settings_tab = Tab::Advanced;
        state.settings.mpv_path = directory.join("absent-player").to_string_lossy().into();
        state.queue.add(
            vec![
                Item {
                    id: "keep".into(),
                    ..Default::default()
                };
                2
            ],
            Enqueue::End,
        );
        let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        app.update(cx, |view, cx| {
            view.route = Route::Settings;
            view.settings_search.update(cx, |input, cx| {
                input.set_value("Import settings", window, cx)
            });
        });
        capture.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.run_until_parked();
    let button = cx.debug_bounds("import-settings").unwrap();
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        app.read_with(cx, |view, _| view.route.clone()),
        Route::SettingsImport
    );
    let (theme, queue) = app.read_with(cx, |view, _| {
        (
            view.state.settings.theme.clone(),
            serde_json::to_value(&view.state.queue).unwrap(),
        )
    });
    let wait = |cx: &mut gpui::VisualTestContext| {
        for _ in 0..200 {
            cx.run_until_parked();
            if app.read_with(cx, |view, _| !view.settings_import_loading) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("Settings file read did not finish");
    };
    let drop = |paths, cx: &mut gpui::VisualTestContext| {
        let position = cx.debug_bounds("settings-import-drop").unwrap().center();
        cx.simulate_event(FileDropEvent::Entered {
            position,
            paths: ExternalPaths::new(paths),
        });
        cx.simulate_event(FileDropEvent::Submit { position });
    };
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../assets/settings-import-drop-reference.json"
    ))
    .unwrap();
    for case in cases
        .iter()
        .filter(|case| !case["name"].as_str().unwrap().starts_with("choose"))
    {
        let name = case["name"].as_str().unwrap();
        app.update(cx, |view, cx| view.open_settings_import(cx));
        cx.run_until_parked();
        let paths = match name {
            "initial" => None,
            "drop-one" => Some(vec![valid.clone()]),
            "drop-many" => Some(vec![valid.clone(), invalid.clone()]),
            "drop-invalid" => Some(vec![invalid.clone()]),
            "drop-nonfile" => Some(vec![]),
            "drop-unreadable" => Some(vec![unreadable.clone()]),
            _ => unreachable!(),
        };
        if let Some(paths) = paths {
            drop(paths, cx);
        }
        wait(cx);
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.settings_import.is_some(),
                !case["selected"].as_array().unwrap().is_empty(),
                "{name}"
            );
            if let Some(preview) = &view.settings_import {
                assert_eq!(preview.settings.theme, "dracula");
            }
            let expected = case["text"]
                .as_array()
                .unwrap()
                .get(1)
                .and_then(|value| value.as_str())
                .unwrap_or_default();
            if name == "drop-unreadable" {
                assert!(
                    view.settings_import_error
                        .starts_with(expected.strip_suffix("Permission denied").unwrap()),
                    "{}",
                    view.settings_import_error
                );
            } else {
                assert_eq!(view.settings_import_error, expected, "{name}");
            }
            assert_eq!(view.state.settings.theme, theme, "{name}");
            assert_eq!(
                serde_json::to_value(&view.state.queue).unwrap(),
                queue,
                "{name}"
            );
        });
        if name == "drop-one" {
            cx.dispatch_action(crate::app::LeaveFullPlayer);
        } else {
            let cancel = cx.debug_bounds("cancel-settings-import").unwrap();
            cx.simulate_click(cancel.center(), Modifiers::none());
        }
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Settings);
            assert!(view.settings_import.is_none());
        });
    }
    // Cancel before an asynchronous read completes; its response must not reopen the preview.
    app.update(cx, |view, cx| {
        view.open_settings_import(cx);
        view.read_settings_import(std::slice::from_ref(&valid), cx);
        view.cancel_settings_import(cx);
    });
    cx.run_until_parked();
    app.read_with(cx, |view, _| {
        assert_eq!(view.route, Route::Settings);
        assert!(view.settings_import.is_none() && !view.settings_import_loading);
    });
    app.update(cx, |view, cx| view.open_settings_import(cx));
    cx.run_until_parked();
    drop(vec![valid], cx);
    wait(cx);
    let apply = cx.debug_bounds("apply-settings-import").unwrap();
    cx.simulate_click(apply.center(), Modifiers::none());
    cx.run_until_parked();
    app.read_with(cx, |view, _| {
        assert_eq!(view.state.settings.theme, "dracula");
        assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
    });
    let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
    assert_eq!(saved.settings.theme, "dracula");
    assert!(std::fs::read_dir(&directory).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("state-before-import-")
    }));
    std::fs::remove_dir_all(directory).unwrap();
}
