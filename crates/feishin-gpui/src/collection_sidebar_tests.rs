use super::*;
use ::core::prelude::v1::test;
use feishin_core::{appearance, storage};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn sidebar_collection_rows_and_inline_rename_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let width = self.0.read(cx).state.settings.sidebar.width;
            div().size_full().child(
                div()
                    .absolute()
                    .top_0()
                    .w(px(width))
                    .when(crate::appearance::rtl(cx), |div| div.right_0())
                    .when(!crate::appearance::rtl(cx), |div| div.left_0())
                    .child(self.0.update(cx, |view, cx| view.sidebar_collections(cx))),
            )
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-collection-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-collection-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-collection-rows-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("Themes")).unwrap();
        let mut theme = appearance::builtins()["defaultTheme"].clone();
        theme
            .app
            .insert("root-font-size".into(), format!("{rem}px"));
        std::fs::write(
            directory.join("Themes/reference.json"),
            serde_json::to_vec(&theme).unwrap(),
        )
        .unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, window_cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.settings.theme = "reference".into();
            state.settings.follow_system_theme = false;
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(parent.clone()));
            let probe = cx.new(|_| Probe(parent));
            Root::new(probe, window, cx)
        });
        let parent = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(1100.)));
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            let width = config["width"].as_f64().unwrap() as f32;
            let rtl = config["language"] == "ar";
            let origin = if rtl { 1000. - width } else { 0. };
            let kind = if config["kind"] == "album" {
                ItemKind::Album
            } else {
                ItemKind::Song
            };
            let name = if config["long"] == true {
                "A very long saved collection title that must truncate at the row edge"
            } else {
                "Saved"
            };
            window_cx.update(|window, cx| {
                parent.update(cx, |view, cx| {
                    view.state.settings.sidebar.width = width;
                    view.state.settings.language = config["language"].as_str().unwrap().into();
                    cx.set_global(crate::appearance::Language(
                        view.state.settings.language.clone(),
                    ));
                    let saved = Collection {
                        id: "saved".into(),
                        name: name.into(),
                        kind,
                        view: Default::default(),
                    };
                    view.active_filters = saved.view.filters.clone();
                    view.local_filter = saved.view.text.clone();
                    view.sort = saved.view.sort;
                    view.descending = saved.view.descending;
                    view.artist_role = saved.view.artist_role.clone();
                    view.route = if config["active"] == true {
                        Route::Library(kind)
                    } else {
                        Route::Home
                    };
                    view.state.settings.collections = vec![saved];
                    view.collection_rename = None;
                    cx.notify();
                });
                window.refresh();
            });
            window_cx.simulate_mouse_move(point(px(500.), px(900.)), None, Modifiers::none());
            window_cx.run_until_parked();
            let check = |selector: &str,
                         expected: &serde_json::Value,
                         offset: f32,
                         cx: &mut VisualTestContext| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("Missing {selector}: {config}"));
                for (actual, expected) in [
                    bounds.origin.x - px(offset),
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ]
                .into_iter()
                .zip(expected.as_array().unwrap())
                {
                    assert!(
                        (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{config}: {selector}: {bounds:?}, expected {expected}"
                    );
                }
            };
            for (selector, part) in [
                ("sidebar-collection-saved", "row"),
                ("sidebar-collection-icon-saved", "icon"),
                ("sidebar-collection-name-saved", "name"),
                ("sidebar-collection-menu-saved", "more"),
            ] {
                check(selector, &case[part]["bounds"], origin, window_cx);
            }
            let row = window_cx.debug_bounds("sidebar-collection-saved").unwrap();
            window_cx.simulate_mouse_move(row.center(), None, Modifiers::none());
            window_cx.run_until_parked();
            if case.get("rename").is_some() {
                let route = parent.read_with(window_cx, |view, _| view.route.clone());
                let more = window_cx
                    .debug_bounds("sidebar-collection-menu-saved")
                    .unwrap();
                window_cx.simulate_click(more.center(), Modifiers::none());
                window_cx.simulate_keystrokes("down enter");
                window_cx.run_until_parked();
                parent.read_with(window_cx, |view, cx| {
                    assert_eq!(view.route, route);
                    let rename = view
                        .collection_rename
                        .as_ref()
                        .expect("Rename stays beside the row");
                    assert_eq!(rename.input.read(cx).value().as_str(), "Saved");
                });
                for (selector, expected) in [
                    ("sidebar-collection-rename", &case["rename"]["bounds"]),
                    ("sidebar-collection-name", &case["rename"]["inputBounds"]),
                    (
                        "sidebar-collection-rename-cancel",
                        &case["rename"]["buttons"][0]["bounds"],
                    ),
                    (
                        "sidebar-collection-rename-save",
                        &case["rename"]["buttons"][1]["bounds"],
                    ),
                ] {
                    check(selector, expected, 0., window_cx);
                }
                for name in ["   ", "  Renamed  "] {
                    let input = parent.read_with(window_cx, |view, _| {
                        view.collection_rename.as_ref().unwrap().input.clone()
                    });
                    window_cx.update(|window, cx| {
                        input.update(cx, |input, cx| {
                            input.set_value(name, window, cx);
                            input.focus(window, cx);
                        })
                    });
                    window_cx.simulate_keystrokes("enter");
                    window_cx.run_until_parked();
                    parent.read_with(window_cx, |view, _| {
                        assert_eq!(view.route, route);
                        assert_eq!(view.collection_rename.is_some(), name.trim().is_empty());
                        assert_eq!(
                            view.state.settings.collections[0].name,
                            if name.trim().is_empty() {
                                "Saved"
                            } else {
                                "Renamed"
                            }
                        );
                    });
                }
                let saved = storage::load(&directory.join("state.json")).unwrap();
                assert_eq!(saved.settings.collections[0].name, "Renamed");
            }
        }
        if rem == 16 {
            window_cx.update(|_, cx| {
                parent.update(cx, |view, cx| {
                    view.state.settings.collections[0].name = "Saved".into();
                    view.state.settings.collections[0].view.filters.favorite = Some(true);
                    view.state.settings.collections[0].view.text = "saved filter".into();
                    let saved = view.state.settings.collections[0].clone();
                    view.route = Route::Collection(Box::new(saved.clone()));
                    view.history = vec![view.route.clone().into()];
                    view.forward_history = view.history.clone();
                    view.state.queue.position = 37.;
                    cx.notify();
                })
            });
            for dismissal in ["cancel", "escape", "outside"] {
                window_cx.update(|window, cx| {
                    parent.update(cx, |view, cx| {
                        view.rename_sidebar_collection("saved", window, cx)
                    })
                });
                window_cx.run_until_parked();
                window_cx.simulate_input("discarded");
                match dismissal {
                    "cancel" => {
                        let button = window_cx
                            .debug_bounds("sidebar-collection-rename-cancel")
                            .unwrap();
                        window_cx.simulate_click(button.center(), Modifiers::none());
                    }
                    "escape" => window_cx.simulate_keystrokes("escape"),
                    _ => window_cx.simulate_click(point(px(500.), px(900.)), Modifiers::none()),
                }
                window_cx.run_until_parked();
                parent.read_with(window_cx, |view, _| {
                    assert!(view.collection_rename.is_none(), "{dismissal}");
                    assert_eq!(view.state.settings.collections[0].name, "Saved");
                });
            }
            window_cx.update(|window, cx| {
                parent.update(cx, |view, cx| {
                    view.rename_sidebar_collection("saved", window, cx);
                    view.collection_rename
                        .as_ref()
                        .unwrap()
                        .input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                })
            });
            window_cx.run_until_parked();
            window_cx.simulate_input(&"界".repeat(129));
            window_cx.run_until_parked();
            let unicode = "界".repeat(128);
            parent.read_with(window_cx, |view, cx| {
                assert_eq!(
                    view.collection_rename
                        .as_ref()
                        .unwrap()
                        .input
                        .read(cx)
                        .value()
                        .as_str(),
                    unicode,
                );
            });
            let button = window_cx
                .debug_bounds("sidebar-collection-rename-save")
                .unwrap();
            window_cx.simulate_click(button.center(), Modifiers::none());
            window_cx.run_until_parked();
            parent.read_with(window_cx, |view, _| {
                assert!(view.collection_rename.is_none());
                assert_eq!(view.state.settings.collections[0].name, unicode);
                for route in [
                    &view.route,
                    &view.history[0].route,
                    &view.forward_history[0].route,
                ] {
                    assert!(matches!(route, Route::Collection(saved) if saved.name == unicode));
                }
                assert_eq!(view.state.queue.position, 37.);
            });
            let saved = storage::load(&directory.join("state.json")).unwrap();
            assert_eq!(saved.settings.collections[0].name, unicode);
            window_cx.update(|_, cx| {
                parent.update(cx, |view, _| {
                    view.active_filters.favorite = Some(true);
                    view.local_filter = "kept filter".into();
                    view.remember_view();
                    view.state
                        .settings
                        .pinned_filter_views
                        .push(view.view_key());
                });
            });
            let more = window_cx
                .debug_bounds("sidebar-collection-menu-saved")
                .unwrap();
            window_cx.simulate_click(more.center(), Modifiers::none());
            window_cx.simulate_keystrokes("down down enter");
            window_cx.run_until_parked();
            parent.read_with(window_cx, |view, _| {
                assert!(view.state.settings.collections.is_empty());
                assert_eq!(view.history.len(), 1);
                assert_eq!(view.forward_history.len(), 1);
                assert!(
                    view.state
                        .settings
                        .pinned_filter_views
                        .contains(&view.view_key())
                );
                assert_eq!(view.state.queue.position, 37.);
            });
            window_cx.update(|_, cx| {
                parent.update(cx, |view, cx| {
                    view.route = Route::Settings;
                    view.back(cx);
                    assert!(
                        matches!(&view.route, Route::Collection(saved) if saved.name == unicode)
                    );
                    assert_eq!(view.active_filters.favorite, Some(true));
                    assert_eq!(view.local_filter, "kept filter");
                    view.forward(cx);
                    assert!(matches!(view.route, Route::Settings));
                    view.forward(cx);
                    assert!(
                        matches!(&view.route, Route::Collection(saved) if saved.name == unicode)
                    );
                    assert_eq!(view.active_filters.favorite, Some(true));
                    assert_eq!(view.local_filter, "kept filter");
                    assert_eq!(view.state.queue.position, 37.);
                });
            });
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[gpui::test]
fn collection_name_paste_replacement_and_composition_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    let directory =
        std::env::temp_dir().join(format!("feishin-collection-input-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, window_cx) = cx.add_window_view(|window, cx| {
        let parent =
            cx.new(|cx| Feishin::new(directory.clone(), Ok(storage::State::default()), window, cx));
        captured.replace(Some(parent.clone()));
        Root::new(parent, window, cx)
    });
    let parent = captured.borrow_mut().take().unwrap();
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../assets/sidebar-collection-input-reference.json"
    ))
    .unwrap();
    for (index, case) in cases.iter().enumerate() {
        window_cx.update(|window, cx| {
            parent.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.state.settings.collections = vec![Collection {
                    id: "saved".into(),
                    name: case["initial"].as_str().unwrap().into(),
                    kind: ItemKind::Album,
                    view: Default::default(),
                }];
                view.rename_sidebar_collection("saved", window, cx);
                let input = view.collection_rename.as_ref().unwrap().input.clone();
                input.update(cx, |input, cx| {
                    let range = case["selection"][0].as_u64().unwrap() as usize
                        ..case["selection"][1].as_u64().unwrap() as usize;
                    let text = case["text"].as_str().unwrap();
                    if case["composition"] == true {
                        input.replace_and_mark_text_in_range(Some(range), text, None, window, cx);
                        assert_eq!(
                            input.value().as_str(),
                            case["marked"].as_str().unwrap(),
                            "composition {index}"
                        );
                        input.replace_text_in_range(None, text, window, cx);
                    } else {
                        input.replace_text_in_range(Some(range), text, window, cx);
                    }
                    assert_eq!(
                        input.value().as_str(),
                        case["value"].as_str().unwrap(),
                        "value {index}"
                    );
                    assert_eq!(
                        input.selected_text_range(false, window, cx).unwrap().range,
                        case["selectionAfter"][0].as_u64().unwrap() as usize
                            ..case["selectionAfter"][1].as_u64().unwrap() as usize,
                        "selection {index}"
                    );
                });
                view.save_sidebar_collection_name(cx);
                assert!(view.collection_rename.is_none(), "save {index}");
                assert_eq!(
                    view.state.settings.collections[0].name,
                    case["calls"][0][2].as_str().unwrap(),
                    "saved {index}"
                );
            });
        });
        let saved = storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(
            saved.settings.collections[0].name,
            case["calls"][0][2].as_str().unwrap()
        );
    }
    let legacy = "界".repeat(200);
    let input = window_cx.update(|window, cx| {
        parent.update(cx, |view, cx| {
            view.state.settings.collections[0].name.clone_from(&legacy);
            view.rename_sidebar_collection("saved", window, cx);
            view.collection_rename.as_ref().unwrap().input.clone()
        })
    });
    window_cx.run_until_parked();
    window_cx.dispatch_action(gpui_component::input::SelectAll);
    // TestAppContext owns an in-memory clipboard, independent of the user's clipboard.
    window_cx.update(|_, cx| cx.write_to_clipboard(ClipboardItem::new_string("😀".repeat(65))));
    window_cx.dispatch_action(gpui_component::input::Paste);
    assert_eq!(
        input.read_with(window_cx, |input, _| input.value()),
        "😀".repeat(64)
    );
    window_cx.dispatch_action(gpui_component::input::Undo);
    assert_eq!(input.read_with(window_cx, |input, _| input.value()), legacy);
    window_cx.dispatch_action(gpui_component::input::Redo);
    assert_eq!(
        input.read_with(window_cx, |input, _| input.value()),
        "😀".repeat(64)
    );
    window_cx.update(|window, cx| {
        input.update(cx, |input, cx| input.set_value(legacy.clone(), window, cx))
    });
    assert_eq!(input.read_with(window_cx, |input, _| input.value()), legacy);
    window_cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(directory).unwrap();
}
