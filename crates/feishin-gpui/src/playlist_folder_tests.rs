use super::*;
use ::core::prelude::v1::test;
use feishin_core::{appearance, storage};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn playlist_folder_headers_match_original_layout_and_navigation(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe {
        parent: Entity<Feishin>,
        folder: Node,
        open: bool,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(self.parent.read(cx).state.settings.sidebar.width))
                .child(self.parent.update(cx, |view, cx| {
                    view.playlist_folder_header(&self.folder, self.open, false, cx)
                }))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/playlist-folder-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/playlist-folder-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let server = uuid::Uuid::new_v4();
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-folder-header-{}", uuid::Uuid::new_v4()));
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
            parent.update(cx, |view, _| view.state.active_server = Some(server));
            let probe = cx.new(|_| Probe {
                parent: parent.clone(),
                folder: Node::Folder {
                    name: "Saved folder".into(),
                    path: "saved".into(),
                    children: (0..123)
                        .map(|_| Node::Playlist {
                            name: "Track".into(),
                            item: Box::new(Item::default()),
                        })
                        .collect(),
                },
                open: false,
            });
            captured.replace(Some((parent, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (parent, probe) = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(1100.)));
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            let open = config["open"] == true;
            let navigation = config["navigation"] == true;
            window_cx.update(|window, cx| {
                parent.update(cx, |view, cx| {
                    view.state.settings.sidebar.width = config["width"].as_f64().unwrap() as f32;
                    view.state.settings.language = config["language"].as_str().unwrap().into();
                    cx.set_global(crate::appearance::Language(
                        view.state.settings.language.clone(),
                    ));
                    view.state.settings.playlist_folder_view =
                        if navigation { "navigation" } else { "tree" }.into();
                    view.playlist_folder_path.clear();
                    view.state
                        .expanded_playlist_folders
                        .as_mut()
                        .unwrap()
                        .insert(
                            server,
                            if open {
                                ["saved".into()].into()
                            } else {
                                Default::default()
                            },
                        );
                    cx.notify();
                });
                probe.update(cx, |probe, cx| {
                    let Node::Folder { name, .. } = &mut probe.folder else {
                        unreachable!()
                    };
                    *name = if config["long"] == true {
                        "A very long playlist folder title that must truncate"
                    } else {
                        "Saved folder"
                    }
                    .into();
                    probe.open = open;
                    cx.notify();
                });
                window.refresh();
            });
            window_cx.simulate_mouse_move(point(px(500.), px(900.)), None, Modifiers::none());
            window_cx.run_until_parked();
            for (part, key) in [
                ("header", "row"),
                ("icon", "icon"),
                ("name", "name"),
                ("count", "count"),
                ("chevron", "chevron"),
                ("icon-frame", "iconFrame"),
            ] {
                if case[key].is_null() {
                    continue;
                }
                let bounds = window_cx
                    .debug_bounds(&format!("playlist-folder-{part}-saved"))
                    .unwrap();
                for (actual, expected) in [
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ]
                .into_iter()
                .zip(case[key]["bounds"].as_array().unwrap())
                {
                    assert!(
                        (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{config}: {part}: {bounds:?}, expected {expected}"
                    );
                }
            }
            let header = window_cx
                .debug_bounds("playlist-folder-header-saved")
                .unwrap();
            window_cx.simulate_click(header.center(), Modifiers::none());
            window_cx.run_until_parked();
            parent.read_with(window_cx, |view, _| {
                if navigation {
                    assert_eq!(view.playlist_folder_path, "saved");
                } else {
                    assert_eq!(
                        view.state.playlist_folder_expanded(server, "saved", false),
                        !open
                    );
                }
                assert!(matches!(view.route, Route::Home | Route::Servers));
            });
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[gpui::test]
fn playlist_folder_trees_match_original_indentation_and_navigation(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |view, cx| {
                let settings = &view.state.settings;
                let nodes = playlist_folders::tree(
                    &view.playlist_options,
                    "/",
                    settings.playlist_folder_view == "single",
                );
                let display = if settings.playlist_folder_view == "navigation"
                    && !view.playlist_folder_path.is_empty()
                {
                    let Node::Folder { children, .. } = &nodes[0] else {
                        unreachable!()
                    };
                    children
                } else {
                    &nodes
                };
                div()
                    .debug_selector(|| "folder-tree".into())
                    .v_flex()
                    .w(px(settings.sidebar.width))
                    .children(
                        display
                            .iter()
                            .map(|node| view.playlist_node(node, false, cx)),
                    )
            })
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/playlist-folder-tree-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/playlist-folder-tree-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let server = uuid::Uuid::new_v4();
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-folder-tree-{}", uuid::Uuid::new_v4()));
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
            let mut state = storage::State::default();
            state.settings.theme = "reference".into();
            state.settings.follow_system_theme = false;
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            parent.update(cx, |view, _| {
                view.state.active_server = Some(server);
                view.state.settings.sidebar.playlist_mode = "default".into();
                view.playlist_options = [
                    ("one", "Rock/Classic/One"),
                    ("two", "Rock/Classic/Two"),
                    ("three", "Rock/Three"),
                    ("root", "Root"),
                ]
                .into_iter()
                .map(|(id, name)| Item {
                    id: id.into(),
                    name: name.into(),
                    server_id: server,
                    kind: ItemKind::Playlist,
                    ..Default::default()
                })
                .collect();
            });
            captured.replace(Some(parent.clone()));
            let probe = cx.new(|_| Probe(parent));
            Root::new(probe, window, cx)
        });
        let parent = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(1100.)));
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            window_cx.update(|window, cx| {
                parent.update(cx, |view, cx| {
                    view.state.settings.sidebar.width = config["width"].as_f64().unwrap() as f32;
                    view.state.settings.language = config["language"].as_str().unwrap().into();
                    cx.set_global(crate::appearance::Language(
                        view.state.settings.language.clone(),
                    ));
                    view.state.settings.playlist_folder_view =
                        config["mode"].as_str().unwrap().into();
                    view.state.settings.playlist_folder_indent =
                        config["indent"].as_u64().unwrap() as u32;
                    view.playlist_folder_path =
                        if config["open"] == true { "Rock" } else { "" }.into();
                    view.state
                        .expanded_playlist_folders
                        .as_mut()
                        .unwrap()
                        .insert(
                            server,
                            if config["open"] == true {
                                ["Rock".into(), "Rock/Classic".into()].into()
                            } else {
                                Default::default()
                            },
                        );
                    cx.notify();
                });
                window.refresh();
            });
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(200));
            window_cx.update(|window, _| window.refresh());
            window_cx.run_until_parked();
            let mut boxes = vec![("folder-tree".to_owned(), &case["bounds"])];
            for header in case["headers"].as_array().unwrap() {
                boxes.push((
                    format!(
                        "playlist-folder-header-{}",
                        if header["name"] == "Rock" {
                            "Rock"
                        } else {
                            "Rock/Classic"
                        }
                    ),
                    &header["bounds"],
                ));
            }
            for leaf in case["leaves"].as_array().unwrap() {
                boxes.push((
                    format!("sidebar-playlist-{}", leaf["id"].as_str().unwrap()),
                    &leaf["bounds"],
                ));
            }
            for (selector, expected) in boxes {
                let bounds = window_cx
                    .debug_bounds(&selector)
                    .unwrap_or_else(|| panic!("Missing {selector}: {config}"));
                for (actual, expected) in [
                    bounds.origin.x,
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
            }
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[gpui::test]
fn playlist_folder_drag_opens_nested_targets_and_preserves_only_completed_drops(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let directory =
        std::env::temp_dir().join(format!("feishin-folder-drag-{}", uuid::Uuid::new_v4()));
    let server = uuid::Uuid::new_v4();
    let captured = Rc::new(RefCell::new(None));
    let (_, window_cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = storage::State::default();
        state.settings.sidebar.image_shown = false;
        state.settings.sidebar.width = 400.;
        state.settings.playlist_folders = true;
        state.settings.playlist_folder_view = "tree".into();
        let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
        parent.update(cx, |view, _| {
            view.state.active_server = Some(server);
            view.route = Route::Settings;
            view.sidebar_closed_sections.insert("library");
            view.playlist_options = [("one", "Rock/Classic/One"), ("root", "Root")]
                .into_iter()
                .map(|(id, name)| Item {
                    id: id.into(),
                    name: name.into(),
                    server_id: server,
                    kind: ItemKind::Playlist,
                    ..Default::default()
                })
                .collect();
        });
        captured.replace(Some(parent.clone()));
        Root::new(parent, window, cx)
    });
    let parent = captured.borrow_mut().take().unwrap();
    window_cx.simulate_resize(size(px(1400.), px(1000.)));
    window_cx.run_until_parked();
    window_cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    window_cx.update(|window, _| window.refresh());
    window_cx.run_until_parked();
    let start = |cx: &mut VisualTestContext| {
        let source =
            cx.debug_bounds("sidebar-playlist-root").unwrap().origin + point(px(3.), px(2.));
        cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            source + point(px(12.), px(5.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(cx.update(|_, cx| cx.active_drag::<Dragged>().is_some()));
    };
    let hover = |path: &str, cx: &mut VisualTestContext| {
        let target = cx
            .debug_bounds(&format!("playlist-folder-drop-{path}"))
            .unwrap()
            .center();
        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        target
    };
    let visible = |label: &str, cx: &mut VisualTestContext| {
        cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button && node.label() == Some(label))
        })
    };
    for outside in [true, false] {
        start(window_cx);
        hover("Rock", window_cx);
        assert!(visible("Classic", window_cx));
        hover("Rock/Classic", window_cx);
        assert!(visible("One", window_cx));
        parent.read_with(window_cx, |view, _| {
            assert!(!view.state.playlist_folder_expanded(server, "Rock", false));
            assert!(
                !view
                    .state
                    .playlist_folder_expanded(server, "Rock/Classic", false)
            );
        });
        if outside {
            window_cx.simulate_mouse_move(
                point(px(1200.), px(900.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(200));
            window_cx.update(|window, _| window.refresh());
            window_cx.run_until_parked();
            assert!(!visible("Classic", window_cx));
        }
        window_cx.update(|window, cx| {
            cx.stop_active_drag(window);
            window.refresh();
        });
        window_cx.simulate_mouse_up(
            point(px(1200.), px(900.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        window_cx.run_until_parked();
        window_cx
            .executor()
            .advance_clock(std::time::Duration::from_millis(200));
        window_cx.update(|window, _| window.refresh());
        window_cx.run_until_parked();
        assert!(!visible("Classic", window_cx));
        parent.read_with(window_cx, |view, _| {
            assert!(view.sidebar_folder_drag_expanded.is_empty())
        });
    }
    // Cancellation keeps folders the user had already opened.
    window_cx.update(|_, cx| {
        parent.update(cx, |view, cx| {
            view.state.toggle_playlist_folder(server, "Rock", false);
            view.save();
            cx.notify();
        })
    });
    window_cx.run_until_parked();
    window_cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    window_cx.update(|window, _| window.refresh());
    window_cx.run_until_parked();
    start(window_cx);
    hover("Rock/Classic", window_cx);
    window_cx.update(|window, cx| {
        cx.stop_active_drag(window);
        window.refresh();
    });
    window_cx.simulate_mouse_up(
        point(px(1200.), px(900.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    window_cx.run_until_parked();
    window_cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    window_cx.update(|window, _| window.refresh());
    window_cx.run_until_parked();
    assert!(visible("Classic", window_cx));
    assert!(!visible("One", window_cx));
    start(window_cx);
    let target = hover("Rock/Classic", window_cx);
    window_cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    window_cx.run_until_parked();
    window_cx
        .executor()
        .advance_clock(std::time::Duration::from_millis(200));
    window_cx.update(|window, _| window.refresh());
    window_cx.run_until_parked();
    assert!(visible("One", window_cx));
    let saved = storage::load(&directory.join("state.json")).unwrap();
    assert!(saved.playlist_folder_expanded(server, "Rock", false));
    assert!(saved.playlist_folder_expanded(server, "Rock/Classic", false));
    parent.read_with(window_cx, |view, _| {
        assert!(view.sidebar_folder_drag_expanded.is_empty())
    });
    window_cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(directory).unwrap();
}
