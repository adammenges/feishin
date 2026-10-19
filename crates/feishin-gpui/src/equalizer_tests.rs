use crate::app::{Feishin, Route};
use ::core::prelude::v1::test;
use feishin_core::{model::Item, queue::Enqueue, settings_search::Tab, storage::State};
use gpui::{prelude::*, *};
use gpui_component::{Root, slider::SliderEvent};
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn equalizer_numbers_drag_release_and_independent_presets_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-equalizer-{}", uuid::Uuid::new_v4()));
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
        state.settings.settings_tab = Tab::Playback;
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
            view.settings_search
                .update(cx, |input, cx| input.set_value("Equalizer", window, cx));
        });
        capture.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1400.), px(2200.)));
    cx.run_until_parked();
    let queue = app.read_with(cx, |view, _| {
        serde_json::to_value(&view.state.queue).unwrap()
    });
    let click = |name, cx: &mut VisualTestContext| {
        let bounds = cx
            .debug_bounds(name)
            .unwrap_or_else(|| panic!("Missing control {name}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    let menu = |selector, name: &str, cx: &mut VisualTestContext| {
        click(selector, cx);
        if selector.ends_with("-presets") {
            cx.simulate_keystrokes(if cfg!(target_os = "macos") {
                "cmd-a"
            } else {
                "ctrl-a"
            });
            cx.simulate_input(name);
            cx.run_until_parked();
            std::thread::sleep(std::time::Duration::from_millis(150));
            cx.run_until_parked();
        }
        cx.update(|window, cx| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
            let id = tree
                .nodes
                .iter()
                .find(|(id, node)| {
                    matches!(
                        node.role(),
                        accesskit::Role::MenuItem | accesskit::Role::ListBoxOption
                    ) && consumer
                        .state()
                        .node_by_tree_local_id(*id, accesskit::TreeId::ROOT)
                        .unwrap()
                        .label()
                        .as_deref()
                        == Some(name)
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Missing menu item {name}: {:?}",
                        tree.nodes
                            .iter()
                            .filter_map(|(_, node)| node.label().map(|label| (node.role(), label)))
                            .collect::<Vec<_>>()
                    )
                })
                .0;
            window.dispatch_accessibility_action_for_test(
                accesskit::ActionRequest {
                    action: accesskit::Action::Click,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: id,
                    data: None,
                },
                cx,
            );
        });
    };
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/equalizer-settings-reference.json")).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        match name {
            "disabled" => {}
            "enabled" | "disabled-again" => click("equalizer-toggle", cx),
            "preamp-preview" | "preamp-commit" => {
                let slider = app.read_with(cx, |view, _| view.preamp_slider.clone());
                slider.update(cx, |_, cx| {
                    cx.emit(if name.ends_with("preview") {
                        SliderEvent::Change(5.5.into())
                    } else {
                        SliderEvent::ChangeEnd(5.5.into())
                    })
                });
            }
            "preamp-draft" | "band-draft" | "band-over" | "band-empty" => {
                let (index, value) = match name {
                    "preamp-draft" => (None, "-20"),
                    "band-draft" => (Some(0), "3.25"),
                    "band-over" => (Some(1), "19"),
                    _ => (Some(1), ""),
                };
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        let input = index.map_or(&view.audio_numbers[0], |index| {
                            &view.equalizer_inputs[index]
                        });
                        input.update(cx, |input, cx| {
                            input.focus(window, cx);
                            input.set_value(value, window, cx);
                        });
                    })
                });
            }
            "preamp-clamp" | "band-clamp" | "band-blur-empty" => {
                cx.update(|window, cx| app.read_with(cx, |view, _| window.focus(&view.focus)));
            }
            "band-enter"
            | "save-eq"
            | "save-compressor"
            | "save-long-eq"
            | "save-long-compressor" => {
                if name.starts_with("save") {
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            let input = if name.ends_with("compressor") {
                                &view.compressor_preset_name
                            } else {
                                &view.preset_name
                            };
                            input.update(cx, |input, cx| input.focus(window, cx));
                        })
                    });
                }
                cx.simulate_keystrokes("enter");
            }
            "band-preview" | "band-release" => {
                let bounds = cx.debug_bounds("equalizer-band-2").unwrap();
                let position = point(bounds.center().x, bounds.top() + bounds.size.height * 0.25);
                if name == "band-preview" {
                    cx.simulate_event(MouseDownEvent {
                        button: MouseButton::Left,
                        position,
                        click_count: 1,
                        modifiers: Modifiers::none(),
                        first_mouse: false,
                    });
                } else {
                    cx.simulate_event(MouseUpEvent {
                        button: MouseButton::Left,
                        position,
                        click_count: 1,
                        modifiers: Modifiers::none(),
                    });
                }
            }
            "rock" => menu("eq-presets", "Rock", cx),
            "custom-eq" => menu("eq-presets", "My EQ", cx),
            "delete-eq" => menu("eq-delete-preset", "My EQ", cx),
            "delete-compressor" => menu("comp-delete-preset", "My Compressor", cx),
            "delete-long-eq" => menu("eq-delete-preset", &"長い名前".repeat(40), cx),
            "delete-long-compressor" => menu(
                "comp-delete-preset",
                "Long Compressor ".repeat(10).trim(),
                cx,
            ),
            "long-names" => cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.preset_name.update(cx, |input, cx| {
                        input.set_value(format!("  {}  ", "長い名前".repeat(40)), window, cx)
                    });
                    view.compressor_preset_name.update(cx, |input, cx| {
                        input.set_value(
                            format!("  {}  ", "Long Compressor ".repeat(10)),
                            window,
                            cx,
                        )
                    });
                })
            }),
            "names" | "independent-name" => {
                if name == "names" {
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.settings_search
                                .update(cx, |input, cx| input.set_value("Compressor", window, cx))
                        })
                    });
                    cx.run_until_parked();
                    click("compressor-toggle", cx);
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.settings_search
                                .update(cx, |input, cx| input.set_value("Equalizer", window, cx))
                        })
                    });
                    cx.run_until_parked();
                }
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        let (input, value) = if name == "names" {
                            (&view.preset_name, "  My EQ  ")
                        } else {
                            (&view.compressor_preset_name, "  My Compressor  ")
                        };
                        input.update(cx, |input, cx| input.set_value(value, window, cx));
                    })
                });
            }
            "reset" => click("equalizer-reset", cx),
            _ => unreachable!("{name}"),
        }
        cx.run_until_parked();
        cx.refresh().unwrap();
        if case["stored"]["enabled"] == true {
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                for frequency in feishin_core::dsp::FREQUENCIES {
                    let label = format!("{frequency} Hz");
                    let node = tree
                        .nodes
                        .iter()
                        .find(|(_, node)| {
                            node.role() == accesskit::Role::TextInput
                                && node.label() == Some(label.as_str())
                        })
                        .unwrap_or_else(|| panic!("{name}: missing visible number input {label}"));
                    assert!(node.1.bounds().unwrap().height() >= 24., "{name}: {label}");
                }
            });
        }
        app.read_with(cx, |view, cx| {
            assert_eq!(
                view.state.settings.equalizer_enabled,
                case["stored"]["enabled"].as_bool().unwrap(),
                "{name}"
            );
            assert_eq!(
                view.state.settings.equalizer_preamp,
                case["stored"]["preamp"].as_f64().unwrap() as f32,
                "{name}"
            );
            for (index, band) in case["stored"]["bands"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
            {
                assert_eq!(
                    view.state.settings.equalizer[index],
                    band["gain"].as_f64().unwrap() as f32,
                    "{name} band {index}"
                );
            }
            if let Some(value) = case["preamp"].as_str() {
                assert_eq!(
                    view.audio_numbers[0].read(cx).value().as_str(),
                    value,
                    "{name}"
                );
            }
            for (input, value) in view
                .equalizer_inputs
                .iter()
                .zip(case["inputs"].as_array().unwrap())
            {
                assert_eq!(
                    input.read(cx).value().as_str(),
                    value.as_str().unwrap(),
                    "{name}"
                );
            }
            for (input, value) in [&view.preset_name, &view.compressor_preset_name]
                .into_iter()
                .zip(case["names"].as_array().unwrap())
            {
                if let Some(value) = value.as_str() {
                    assert_eq!(input.read(cx).value().as_str(), value, "{name}");
                }
            }
            let presets = &case["presets"];
            let eq: std::collections::BTreeMap<String, [f32; 12]> = serde_json::from_value(
                presets
                    .get("feishin_eq_custom_presets")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            )
            .unwrap();
            assert_eq!(view.state.settings.equalizer_presets, eq, "{name}");
            let comp: std::collections::BTreeMap<String, feishin_core::dsp::Compressor> =
                serde_json::from_value(
                    presets
                        .get("feishin_comp_custom_presets")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({})),
                )
                .unwrap();
            let mut actual = view.state.settings.compressor_presets.clone();
            for value in actual.values_mut() {
                value.enabled = false;
            }
            assert_eq!(actual, comp, "{name}");
            if name.starts_with("save-long") {
                feishin_core::migration::preview(
                    &view.state.settings,
                    &serde_json::to_vec(&view.state).unwrap(),
                )
                .unwrap();
            }
            assert_eq!(
                serde_json::to_value(&view.state.queue).unwrap(),
                queue,
                "{name}"
            );
        });
    }
    let persisted = feishin_core::storage::load(&directory.join("state.json")).unwrap();
    assert!(persisted.settings.equalizer_presets.is_empty());
    assert!(persisted.settings.compressor_presets.is_empty());
    assert!(!persisted.settings.equalizer_enabled);
    std::fs::remove_dir_all(directory).unwrap();
}
#[gpui::test]
fn preset_search_keyboard_selection_and_cancellation_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-preset-search-{}", uuid::Uuid::new_v4()));
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
        state.settings.settings_tab = Tab::Playback;
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
            view.settings_search
                .update(cx, |input, cx| input.set_value("Equalizer", window, cx));
        });
        capture.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1400.), px(2200.)));
    cx.run_until_parked();
    let queue = app.read_with(cx, |view, _| {
        serde_json::to_value(&view.state.queue).unwrap()
    });
    let click = |name, cx: &mut VisualTestContext| {
        let bounds = cx
            .debug_bounds(name)
            .unwrap_or_else(|| panic!("Missing control {name}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    };
    let replace_query = |value: &str, cx: &mut VisualTestContext| {
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.simulate_input(value);
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(150));
        cx.run_until_parked();
    };
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("../assets/audio-preset-search-reference.json")).unwrap();
    for case in cases {
        let name = case["name"].as_str().unwrap();
        match name {
            "disabled" => {}
            "enabled" | "disabled-again" | "enabled-again" => click("equalizer-toggle", cx),
            "opened" | "reopen" | "open-again" | "opened-after-enable" => click("eq-presets", cx),
            "search" | "none" | "case-insensitive" | "spaces" | "search-after-enable" => {
                replace_query(case["query"].as_str().unwrap(), cx);
            }
            "next" => cx.simulate_keystrokes("down"),
            "choose" => cx.simulate_keystrokes("enter"),
            "escape" => cx.simulate_keystrokes("escape"),
            "choose-bass" => cx.simulate_keystrokes("down enter"),
            _ => unreachable!("{name}"),
        }
        cx.run_until_parked();
        cx.refresh().unwrap();
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
            let combo = tree.nodes.iter().find(|(_, node)| node.role() == accesskit::Role::ComboBox);
            if name == "disabled" || name == "disabled-again" {
                assert!(combo.is_none());
            } else {
                // The reference's first synthetic focus has not opened its popup yet.
                let initial_open = matches!(name, "opened" | "opened-after-enable");
                let opened = initial_open || case["opened"] == true;
                assert_eq!(combo.unwrap().1.is_expanded(), Some(opened), "{name}");
                if opened {
                    let query = tree.nodes.iter().find(|(_, node)| {
                        node.role() == accesskit::Role::TextInput && node.label() == Some("Search")
                    }).unwrap_or_else(|| panic!("{name}: missing search input"));
                    assert_eq!(query.1.value(), case["query"].as_str(), "{name}");
                    assert_eq!(tree.focus, query.0, "{name}: search owns keyboard focus");
                }
                if !initial_open {
                    let options: Vec<_> = tree.nodes.iter().filter(|(_, node)| node.role() == accesskit::Role::ListBoxOption)
                        .map(|(id, node)| serde_json::json!({
                            "text": consumer.state().node_by_tree_local_id(*id, accesskit::TreeId::ROOT).unwrap().label().unwrap(),
                            "active": node.is_selected().unwrap_or(false),
                        })).collect();
                    assert_eq!(options, *case["options"].as_array().unwrap(), "{name}");
                }
            }
        });
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.state.settings.equalizer_enabled,
                case["stored"]["enabled"].as_bool().unwrap(),
                "{name}"
            );
            assert_eq!(
                view.state.settings.equalizer_preamp,
                case["stored"]["preamp"].as_f64().unwrap() as f32,
                "{name}"
            );
            let gains: Vec<f32> = case["stored"]["bands"]
                .as_array()
                .unwrap()
                .iter()
                .map(|band| band["gain"].as_f64().unwrap() as f32)
                .collect();
            assert_eq!(
                view.state.settings.equalizer.as_slice(),
                gains.as_slice(),
                "{name}"
            );
            assert_eq!(
                serde_json::to_value(&view.state.queue).unwrap(),
                queue,
                "{name}"
            );
        });
    }
    std::fs::remove_dir_all(directory).unwrap();
}
