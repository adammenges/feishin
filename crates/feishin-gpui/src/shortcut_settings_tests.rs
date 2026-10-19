use super::*;
use ::core::prelude::v1::test;
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn duplicate_shortcuts_dispatch_once_and_preserve_input_and_menu_scope(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-duplicate-keys-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let mut state = feishin_core::storage::State::default();
        state.settings.home.feature = false;
        state.settings.home.items.clear();
        state.settings.shortcuts = feishin_core::shortcuts::from_electron(
            &serde_json::json!({
                "zoomIn": {"hotkey":"mod+j"},
                "toggleRepeat": {"hotkey":"mod+j"}
            }),
            &state.settings.shortcuts,
        )
        .unwrap();
        let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        captured.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_keystrokes("secondary-j");
    app.read_with(cx, |view, _| {
        assert_eq!(
            view.state.queue.repeat,
            Repeat::All,
            "Every duplicate local action must run"
        );
        assert!((view.state.settings.zoom - 1.1).abs() < 0.001);
    });
    cx.dispatch_action(RunShortcut {
        action: Action::ToggleRepeat,
    });
    app.read_with(cx, |view, _| {
        assert_eq!(view.state.queue.repeat, Repeat::One);
        assert!(
            (view.state.settings.zoom - 1.1).abs() < 0.001,
            "A menu action must not invoke another shortcut"
        );
    });
    cx.update(|window, cx| {
        app.update(cx, |view, cx| {
            view.search.update(cx, |input, cx| input.focus(window, cx));
        })
    });
    cx.simulate_keystrokes("secondary-j");
    app.read_with(cx, |view, _| {
        assert_eq!(view.state.queue.repeat, Repeat::One);
        assert!((view.state.settings.zoom - 1.1).abs() < 0.001);
    });
    cx.update(|window, cx| {
        app.update(cx, |view, cx| {
            window.focus(&view.focus);
            view.change_shortcut(
                Action::ZoomOut,
                Binding {
                    key: "secondary-j".into(),
                    physical: true,
                    global: false,
                },
                cx,
            );
            assert_eq!(
                view.state.settings.shortcuts[&Action::ZoomOut].key,
                "secondary-j"
            );
            view.change_shortcut(
                Action::Next,
                Binding {
                    key: "ctrl-n".into(),
                    physical: true,
                    global: false,
                },
                cx,
            );
            assert_eq!(
                view.state.settings.shortcuts[&Action::Next].key,
                "ctrl-n",
                "Imported duplicates must not block unrelated edits"
            );
        })
    });
    cx.simulate_keystrokes("secondary-j");
    app.read_with(cx, |view, _| {
        assert_eq!(view.state.queue.repeat, Repeat::Off);
        assert!(
            (view.state.settings.zoom - 1.1).abs() < 0.001,
            "Both zoom actions must run exactly once"
        );
    });
    let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
    assert_eq!(
        saved.settings.shortcuts[&Action::ZoomOut].key,
        "secondary-j"
    );
    cx.update(|window, cx| {
        app.update(cx, |view, cx| {
            view.state.settings.shortcuts = feishin_core::shortcuts::from_electron(
                &serde_json::json!({
                    "zoomIn": {"hotkey":"ctrl+j"},
                    "listPlayLast": {"hotkey":"ctrl+j"}
                }),
                &Bindings::default(),
            )
            .unwrap();
            view.install_shortcuts(cx);
            view.route = Route::Library(ItemKind::Song);
            view.items = vec![feishin_core::model::Item {
                id: "selected".into(),
                name: "Selected".into(),
                ..Default::default()
            }];
            view.selected.insert(0);
            view.state.queue.add(view.items.clone(), Enqueue::End);
            window.focus(&view.focus);
            cx.notify();
        })
    });
    cx.simulate_keystrokes("ctrl-j");
    assert_eq!(
        app.read_with(cx, |view, _| view.state.queue.entries.len()),
        1,
        "List shortcuts must remain inactive outside their list"
    );
    cx.update(|window, cx| app.read_with(cx, |view, _| window.focus(&view.library_focus)));
    cx.simulate_keystrokes("ctrl-j");
    app.read_with(cx, |view, _| {
        assert_eq!(view.state.queue.entries.len(), 2);
        assert!(
            (view.state.settings.zoom - 1.3).abs() < 0.001,
            "The global and focused-list actions must both run"
        );
    });
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn physical_shortcuts_keep_original_codes_modifiers_and_legacy_matching() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../feishin-core/assets/shortcut-keys-reference.json"
    ))
    .unwrap();
    for (code, original) in reference.as_object().unwrap() {
        for (prefix, modifiers) in [
            ("ctrl+", Modifiers::control()),
            ("ctrl+shift+", Modifiers::control_shift()),
        ] {
            let imported = feishin_core::shortcuts::from_electron(&serde_json::json!({
                "zoomIn": {"hotkey": format!("{prefix}{}", original.as_str().unwrap()), "isGlobal": false}
            }), &Bindings::default()).unwrap();
            let saved = &imported[&Action::ZoomIn];
            assert!(saved.physical);
            let binding = KeyBinding::new(
                &saved.key,
                RunShortcut {
                    action: Action::ZoomIn,
                },
                None,
            )
            .with_physical_keys();
            assert_eq!(
                binding.keystrokes()[0].inner().unparse(),
                Keystroke::parse(&saved.key).unwrap().unparse(),
                "Readable shortcut {code}"
            );
            let mut event = Keystroke::parse("ctrl-y").unwrap();
            event.physical_key = Some(PhysicalKeystroke {
                code: code.clone(),
                modifiers,
            });
            assert_eq!(
                binding.match_keystrokes(&[event.clone()]),
                Some(false),
                "{prefix}{code}"
            );
            event.physical_key.as_mut().unwrap().modifiers.control = false;
            assert_eq!(
                binding.match_keystrokes(&[event]),
                None,
                "Modifier isolation for {code}"
            );
        }
    }
    // Original typed-character bindings still work; key position is opt-in.
    let mut event = Keystroke::parse("ctrl-y").unwrap();
    event.physical_key = Some(PhysicalKeystroke {
        code: "KeyZ".into(),
        modifiers: Modifiers::control(),
    });
    let logical = KeyBinding::new(
        "ctrl-y",
        RunShortcut {
            action: Action::ZoomIn,
        },
        None,
    );
    assert_eq!(logical.match_keystrokes(&[event.clone()]), Some(false));
    assert_eq!(
        logical.with_physical_keys().match_keystrokes(&[event]),
        None
    );
    for (typed, unshifted) in "~!@#$%^&*()_+{}|:\"<>?"
        .chars()
        .zip("`1234567890-=[]\\;',./".chars())
    {
        let event = Keystroke::parse(&format!("ctrl-{typed}")).unwrap();
        let binding = KeyBinding::new(
            &format!("ctrl-shift-{unshifted}"),
            RunShortcut {
                action: Action::ZoomIn,
            },
            None,
        )
        .with_physical_keys();
        assert_eq!(
            binding.match_keystrokes(&[event]),
            Some(false),
            "Shifted {typed}"
        );
    }
    // Independent scan-code positions exercise all three platform converters.
    for (code, mac, win, xkb) in [
        ("KeyZ", 0x06, 0x2c, "AB01"),
        ("KeyY", 0x10, 0x15, "AD06"),
        ("Digit1", 0x12, 0x02, "AE01"),
        ("Equal", 0x18, 0x0d, "AE12"),
        ("Minus", 0x1b, 0x0c, "AE11"),
        ("Backslash", 0x2a, 0x2b, "BKSL"),
        ("Enter", 0x24, 0x1c, "RTRN"),
        ("NumpadEnter", 0x4c, 0x11c, "KPEN"),
        ("Numpad1", 0x53, 0x4f, "KP1"),
        ("NumpadAdd", 0x45, 0x4e, "KPAD"),
        ("NumpadDivide", 0x4b, 0x135, "KPDV"),
        ("ArrowDown", 0x7d, 0x150, "DOWN"),
        ("Home", 0x73, 0x147, "HOME"),
    ] {
        assert_eq!(macos_physical_key(mac), Some(code));
        assert_eq!(windows_physical_key(win, 0), Some(code));
        assert_eq!(xkb_physical_key(xkb), Some(code));
    }
    for number in 1..=24 {
        let code = format!("F{number}");
        assert_eq!(windows_physical_key(0, 0x6f + number), Some(code.as_str()));
        assert_eq!(
            xkb_physical_key(&format!("FK{number:02}")),
            Some(code.as_str())
        );
    }
    assert_eq!(macos_physical_key(u16::MAX), None);
    assert_eq!(windows_physical_key(u16::MAX, 0), None);
    assert_eq!(xkb_physical_key("AE00"), None);
}

#[gpui::test]
fn physical_shortcut_recording_preserves_shift_layout_and_keypad(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-physical-keys-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            Feishin::new(
                directory.clone(),
                Ok(feishin_core::storage::State::default()),
                window,
                cx,
            )
        });
        captured.replace(Some(view.clone()));
        Root::new(view, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    for (logical, code, modifiers, expected, other) in [
        (
            "ctrl-+",
            "Equal",
            Modifiers::control_shift(),
            "ctrl-shift-=",
            "ctrl-=",
        ),
        ("ctrl-y", "KeyZ", Modifiers::control(), "ctrl-z", "ctrl-y"),
        (
            "ctrl-+",
            "NumpadAdd",
            Modifiers::control(),
            "ctrl-numpadadd",
            "ctrl-+",
        ),
        (
            "ctrl-enter",
            "NumpadEnter",
            Modifiers::control(),
            "ctrl-numpadenter",
            "ctrl-enter",
        ),
        (
            "ctrl-1",
            "Numpad1",
            Modifiers::control(),
            "ctrl-numpad1",
            "ctrl-1",
        ),
    ] {
        let mut stroke = Keystroke::parse(logical).unwrap();
        stroke.physical_key = Some(PhysicalKeystroke {
            code: code.into(),
            modifiers,
        });
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.shortcuts = Bindings::default();
                view.state.settings.zoom = 1.;
                view.install_shortcuts(cx);
                view.record_shortcut = Some(Action::ZoomIn);
                window.focus(&view.focus);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.simulate_event(KeyDownEvent {
            keystroke: stroke.clone(),
            is_held: false,
        });
        cx.simulate_event(KeyUpEvent {
            keystroke: stroke.clone(),
        });
        cx.run_until_parked();
        let saved = app.read_with(cx, |view, _| {
            view.state.settings.shortcuts[&Action::ZoomIn].clone()
        });
        assert_eq!(
            saved,
            Binding {
                key: expected.into(),
                global: false,
                physical: true
            }
        );
        assert_eq!(
            feishin_core::storage::load(&directory.join("state.json"))
                .unwrap()
                .settings
                .shortcuts[&Action::ZoomIn],
            saved
        );
        cx.simulate_keystrokes(other);
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.zoom),
            1.,
            "{other} must not match {code}"
        );
        cx.simulate_event(KeyDownEvent {
            keystroke: stroke.clone(),
            is_held: false,
        });
        cx.simulate_event(KeyUpEvent { keystroke: stroke });
        cx.run_until_parked();
        assert!(
            (app.read_with(cx, |view, _| view.state.settings.zoom) - 1.1).abs() < 0.001,
            "Recorded {code} must dispatch"
        );
    }
    app.update(cx, |view, cx| {
        view.record_shortcut = Some(Action::ZoomIn);
        cx.notify();
    });
    cx.run_until_parked();
    let mut unsupported = Keystroke::parse("ctrl-a").unwrap();
    unsupported.physical_key = Some(PhysicalKeystroke {
        code: "Unidentified".into(),
        modifiers: Modifiers::control(),
    });
    cx.simulate_event(KeyDownEvent {
        keystroke: unsupported,
        is_held: false,
    });
    cx.run_until_parked();
    assert_eq!(
        app.read_with(cx, |view, _| view.record_shortcut),
        Some(Action::ZoomIn)
    );
    assert_eq!(
        app.read_with(cx, |view, _| view.state.settings.shortcuts[&Action::ZoomIn]
            .key
            .clone()),
        "ctrl-numpad1"
    );
    cx.simulate_keystrokes("escape");
    std::fs::remove_dir_all(directory).unwrap();
}

#[gpui::test]
fn imported_browser_named_shortcuts_dispatch_native_actions(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-imported-keys-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            Feishin::new(
                directory.clone(),
                Ok(feishin_core::storage::State::default()),
                window,
                cx,
            )
        });
        captured.replace(Some(view.clone()));
        Root::new(view, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../feishin-core/assets/shortcut-keys-reference.json"
    ))
    .unwrap();
    for (code, key) in [
        ("Equal", "="),
        ("Minus", "-"),
        ("ArrowDown", "down"),
        ("ArrowLeft", "left"),
        ("ArrowRight", "right"),
        ("ArrowUp", "up"),
    ] {
        for (original_modifier, native_modifier) in [
            ("mod+", "secondary-"),
            ("alt+", "alt-"),
            ("mod+shift+", "secondary-shift-"),
        ] {
            let imported = feishin_core::shortcuts::from_electron(&serde_json::json!({
                "zoomIn": {"hotkey": format!("{original_modifier}{}", reference[code].as_str().unwrap()), "isGlobal": false}
            }), &Bindings::default()).unwrap();
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.shortcuts = imported;
                    view.state.settings.zoom = 1.;
                    view.install_shortcuts(cx);
                    window.focus(&view.focus);
                })
            });
            cx.run_until_parked();
            let stroke = match (native_modifier, key) {
                ("secondary-shift-", "=") => "secondary-+".into(),
                ("secondary-shift-", "-") => "secondary-_".into(),
                _ => format!("{native_modifier}{key}"),
            };
            cx.simulate_keystrokes(&stroke);
            cx.run_until_parked();
            assert!(
                (app.read_with(cx, |view, _| view.state.settings.zoom) - 1.1).abs() < 0.001,
                "Imported {original_modifier}{code} did not dispatch Zoom in"
            );
            // Check OS registration parsing without registering an actual hotkey.
            let binding = app.read_with(cx, |view, _| {
                view.state.settings.shortcuts[&Action::ZoomIn].clone()
            });
            let parsed = super::global_key(&binding.key).unwrap();
            let expected = super::global_key(&format!("{native_modifier}{key}")).unwrap();
            assert_eq!(parsed, expected, "Global {original_modifier}{code}");
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[gpui::test]
fn shortcut_settings_use_original_localized_labels_and_keep_bindings(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-shortcut-labels-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = feishin_core::storage::State::default();
        state.settings.home.feature = false;
        state.settings.home.items.clear();
        let view = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        view.update(cx, |view, _| view.route = Route::Shortcuts);
        captured.replace(Some(view.clone()));
        Root::new(view, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1600.), px(4000.)));
    cx.update(|window, _| window.activate_window());
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../feishin-core/assets/shortcut-labels-reference.json"
    ))
    .unwrap();
    let bindings = app.read_with(cx, |view, _| view.state.settings.shortcuts.clone());
    for language in ["en", "fr", "ja", "ar"] {
        app.update(cx, |view, cx| {
            view.state.settings.language = language.into();
            cx.notify();
        });
        cx.run_until_parked();
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        for (action, label) in reference["languages"][language]["actions"]
            .as_object()
            .unwrap()
        {
            assert!(
                tree.nodes
                    .iter()
                    .any(|(_, node)| node.label().or_else(|| node.value()) == label.as_str()),
                "{language}: {action} must render {label}"
            );
        }
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.shortcuts.clone()),
            bindings
        );
    }
    let labels = &reference["languages"]["ar"];
    let next = labels["actions"]["next"].as_str().unwrap();
    let edit = labels["controls"]["common.edit"].as_str().unwrap();
    let clear = labels["controls"]["common.clear"].as_str().unwrap();
    let description = labels["controls"]["setting.applicationHotkeys_description"]
        .as_str()
        .unwrap();
    cx.update(|window, cx| {
        app.update(cx, |view, cx| {
            view.settings_search
                .update(cx, |input, cx| input.set_value(next.to_owned(), window, cx));
            window.focus(&view.focus);
        })
    });
    cx.run_until_parked();
    let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
    let edits = tree
        .nodes
        .iter()
        .filter(|(_, node)| {
            node.role() == Role::Button
                && node
                    .label()
                    .is_some_and(|label| label.ends_with(&format!(": {edit}")))
        })
        .count();
    assert_eq!(edits, 1, "Search must match translated action titles");
    cx.simulate_keystrokes("secondary-f");
    cx.run_until_parked();
    cx.update(|window, cx| {
        app.read_with(cx, |view, cx| {
            assert!(
                view.settings_search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        })
    });

    click(cx, Role::Button, &format!("{next}: {edit}"));
    assert_eq!(
        app.read_with(cx, |view, _| view.record_shortcut),
        Some(Action::Next)
    );
    cx.simulate_keystrokes("ctrl-j");
    cx.run_until_parked();
    assert_eq!(
        app.read_with(cx, |view, _| view.state.settings.shortcuts[&Action::Next]
            .key
            .clone()),
        "ctrl-j"
    );
    click(cx, Role::CheckBox, &format!("{next}: {description}"));
    assert!(app.read_with(cx, |view, _| {
        view.state.settings.shortcuts[&Action::Next].global
    }));
    assert!(
        feishin_core::storage::load(&directory.join("state.json"))
            .unwrap()
            .settings
            .shortcuts[&Action::Next]
            .global
    );
    click(cx, Role::Button, &format!("{next}: {clear}"));
    assert_eq!(
        app.read_with(cx, |view, _| view.state.settings.shortcuts[&Action::Next]
            .clone()),
        Binding::default()
    );
    let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
    assert!(
        tree.nodes
            .iter()
            .any(|(_, node)| node.role() == Role::CheckBox
                && node.label() == Some(format!("{next}: {description}").as_str())
                && node.is_disabled())
    );

    click(cx, Role::Button, &format!("{next}: {edit}"));
    let search_center = cx.debug_bounds("shortcut-search").unwrap().center();
    cx.simulate_click(search_center, Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        app.read_with(cx, |view, cx| {
            assert!(
                view.settings_search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window),
                "Click must focus shortcut search; app focus: {}",
                view.focus.is_focused(window)
            );
        })
    });
    assert!(
        app.read_with(cx, |view, _| view.record_shortcut.is_none()),
        "Leaving the recording control must cancel capture"
    );
    cx.simulate_input("x");
    assert_eq!(
        app.read_with(cx, |view, _| view.state.settings.shortcuts[&Action::Next]
            .clone()),
        Binding::default()
    );
    assert!(app.read_with(cx, |view, cx| {
        view.settings_search.read(cx).value().contains('x')
    }));
    std::fs::remove_dir_all(directory).unwrap();
}

fn click(cx: &mut VisualTestContext, role: Role, label: &str) {
    let (bounds, scale) = cx.update(|window, _| {
        let node = window
            .accessibility_tree_for_test()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.role() == role && node.label() == Some(label))
            .unwrap_or_else(|| panic!("Missing {role:?}: {label}"));
        (node.1.bounds().unwrap(), f64::from(window.scale_factor()))
    });
    cx.simulate_click(
        point(
            px(((bounds.x0 + bounds.x1) / 2. / scale) as f32),
            px(((bounds.y0 + bounds.y1) / 2. / scale) as f32),
        ),
        Modifiers::none(),
    );
    cx.run_until_parked();
}
