use super::*;
use ::core::prelude::v1::test;
use feishin_core::{appearance, storage};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn sidebar_menu_positions_match_original_edge_fitting(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    struct Probe(serde_json::Value);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let config = &self.0;
            div().size_full().child(
                div()
                    .absolute()
                    .left(px(config["x"].as_f64().unwrap() as f32))
                    .top(px(config["y"].as_f64().unwrap() as f32))
                    .child(
                        gpui_component::popover::Popover::new("sidebar-menu-position")
                            .appearance(false)
                            .default_open(true)
                            .side(
                                match (config["rtl"] == true, config["position"] == "right-end") {
                                    (false, false) => Corner::TopLeft,
                                    (false, true) => Corner::BottomLeft,
                                    (true, false) => Corner::TopRight,
                                    (true, true) => Corner::BottomRight,
                                },
                            )
                            .offset(px(config["offset"].as_f64().unwrap() as f32))
                            .viewport_margin(px(5.))
                            .trigger(
                                Button::new("position-trigger")
                                    .w(px(80.))
                                    .h(px(62.))
                                    .label("Menu"),
                            )
                            .content(|_, _, _| {
                                div()
                                    .debug_selector(|| "positioned-sidebar-menu".into())
                                    .w(px(200.))
                                    .h(px(160.))
                            }),
                    ),
            )
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-menu-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-menu-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases {
        let config = &case["config"];
        let (_, window_cx) = cx.add_window_view(|window, cx| {
            cx.set_global(crate::appearance::Language(
                if config["rtl"] == true { "ar" } else { "en" }.into(),
            ));
            let probe = cx.new(|_| Probe(config.clone()));
            Root::new(probe, window, cx)
        });
        window_cx.simulate_resize(size(px(1024.), px(768.)));
        window_cx.run_until_parked();
        let bounds = window_cx.debug_bounds("positioned-sidebar-menu").unwrap();
        for (actual, expected) in [
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height,
        ]
        .into_iter()
        .zip(case["bounds"].as_array().unwrap())
        {
            assert!(
                (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                "{config}: {bounds:?}, expected {expected}"
            );
        }
        window_cx.update(|window, _| window.remove_window());
    }
}

#[gpui::test]
fn compact_sidebar_tiles_history_and_scroll_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(80.))
                .h(px(250.))
                .child(self.0.update(cx, |view, cx| view.sidebar_view(cx)))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-compact-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-compact-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases {
        let config = &case["config"];
        let directory =
            std::env::temp_dir().join(format!("feishin-compact-layout-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(directory.join("Themes")).unwrap();
        let mut theme = appearance::builtins()["defaultTheme"].clone();
        theme
            .app
            .insert("root-font-size".into(), format!("{}px", config["rem"]));
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
            state.settings.language = config["language"].as_str().unwrap().into();
            state.settings.sidebar.collapsed = true;
            state.settings.sidebar.collapsed_navigation = config["navigation"] == true;
            state.settings.sidebar.playlist_list = false;
            state.settings.sidebar.items = ["Home", "Collections", "Settings"]
                .into_iter()
                .map(|id| feishin_core::sidebar::Entry {
                    id: id.into(),
                    disabled: false,
                })
                .collect();
            state.settings.collections = vec![feishin_core::collections::Collection {
                id: "saved".into(),
                name: "Saved".into(),
                kind: ItemKind::Album,
                view: Default::default(),
            }];
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            parent.update(cx, |view, _| {
                view.route = Route::Servers;
                view.history = vec![Route::Home.into()];
                view.forward_history = vec![Route::Queue.into()];
                view.library_scan.status.scanning = config["scanning"] == true;
            });
            captured.replace(Some(parent.clone()));
            let probe = cx.new(|_| Probe(parent));
            Root::new(probe, window, cx)
        });
        let parent = captured.borrow_mut().take().unwrap();
        window_cx.run_until_parked();
        for (index, id) in ["Menu", "Home", "Collections", "Settings"]
            .into_iter()
            .enumerate()
        {
            for (suffix, key) in [
                ("control", "bounds"),
                ("icon", "iconBounds"),
                ("label", "labelBounds"),
            ] {
                let selector = match (suffix, id) {
                    ("control", "Menu") => "application-menu".to_owned(),
                    ("control", "Collections") => "sidebar-nav-Collections".to_owned(),
                    _ => format!("sidebar-navigation-{suffix}-{id}"),
                };
                let bounds = window_cx.debug_bounds(&selector).unwrap();
                let expected = &case["tiles"][index][key];
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
        for (index, button) in case["buttons"].as_array().unwrap().iter().enumerate() {
            let bounds = window_cx
                .debug_bounds(if index == 0 {
                    "sidebar-back"
                } else {
                    "sidebar-forward"
                })
                .unwrap();
            for (actual, expected) in [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ]
            .into_iter()
            .zip(button["bounds"].as_array().unwrap())
            {
                assert!(
                    (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "History: {config}: {bounds:?}, expected {expected}"
                );
            }
        }
        let click = |selector: &str, cx: &mut gpui::VisualTestContext| {
            let point = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(point, Modifiers::none());
            cx.run_until_parked();
        };
        if config["navigation"] == true {
            click("sidebar-back", window_cx);
            assert!(parent.read_with(window_cx, |view, _| view.route == Route::Home));
            click("sidebar-forward", window_cx);
            assert!(parent.read_with(window_cx, |view, _| view.route == Route::Servers));
        }
        click("application-menu", window_cx);
        let tree =
            window_cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        let menu = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Menu)
            .unwrap()
            .1
            .bounds()
            .unwrap();
        assert!(
            menu.x0 >= 90.,
            "Application menu must open beside its tile: {config}: {menu:?}"
        );
        window_cx.simulate_keystrokes("escape");
        window_cx.run_until_parked();
        let before = window_cx.debug_bounds("application-menu").unwrap().origin.y;
        let area = window_cx.debug_bounds("sidebar-scroll").unwrap();
        window_cx.simulate_event(ScrollWheelEvent {
            position: area.center(),
            delta: ScrollDelta::Pixels(point(px(0.), px(-100.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        window_cx.run_until_parked();
        let after = window_cx.debug_bounds("application-menu").unwrap().origin.y;
        assert!(after < before, "Menu must scroll with the column: {config}");
        let expected = case["scrolledMenuY"].as_f64().unwrap() as f32;
        assert!(
            (f32::from(after) - expected).abs() <= 1.,
            "Scrolled menu: {config}: {after:?}, expected {expected}"
        );
        click("sidebar-nav-Collections", window_cx);
        let tree =
            window_cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        let menu = tree
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Menu)
            .unwrap()
            .1
            .bounds()
            .unwrap();
        assert!(
            menu.x0 >= 80.,
            "Collections must open beside its tile: {config}: {menu:?}"
        );
        window_cx.simulate_keystrokes("down enter");
        window_cx.run_until_parked();
        assert!(parent.read_with(window_cx, |view, _| matches!(&view.route, Route::Collection(collection) if collection.id == "saved")));
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[gpui::test]
fn sidebar_navigation_rows_match_original_labels_geometry_and_active_icons(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe {
        parent: Entity<Feishin>,
        id: String,
        active: bool,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let width = if self.parent.read(cx).state.settings.sidebar.collapsed {
                80.
            } else {
                260.
            };
            div().w(px(width)).child(self.parent.update(cx, |view, cx| {
                view.sidebar_navigation_control(&self.id, self.active, cx)
            }))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-navigation-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-navigation-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-sidebar-rows-{}", uuid::Uuid::new_v4()));
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
            let probe = cx.new(|_| Probe {
                parent,
                id: "Home".into(),
                active: false,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            let id = config["id"].as_str().unwrap();
            let active = config["active"] == true;
            assert_eq!(navigation_icon(id, active), case["icon"]["name"].as_str());
            window_cx.update(|window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.id = id.into();
                    probe.active = active;
                    probe.parent.update(cx, |view, cx| {
                        view.state.settings.sidebar.collapsed = config["collapsed"] == true;
                        view.state.settings.language = config["language"].as_str().unwrap().into();
                        cx.set_global(crate::appearance::Language(
                            view.state.settings.language.clone(),
                        ));
                    });
                    cx.notify();
                });
                window.refresh();
            });
            window_cx.run_until_parked();
            for (suffix, expected) in [
                ("control", &case["item"]),
                ("icon", &case["icon"]),
                ("label", &case["label"]),
                ("marker", &case["indicator"]),
            ] {
                if expected.is_null() {
                    continue;
                }
                let selector = format!("sidebar-navigation-{suffix}-{id}");
                let bounds = window_cx.debug_bounds(&selector).unwrap();
                for (actual, expected) in [
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ]
                .into_iter()
                .zip(expected["bounds"].as_array().unwrap())
                {
                    assert!(
                        (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{config}: {suffix}: {bounds:?}, expected {}",
                        expected
                    );
                }
            }
            let icon = window_cx
                .debug_bounds(&format!("sidebar-navigation-icon-{id}"))
                .unwrap();
            window_cx.update(|window, cx| {
                let marker_x = if config["language"] == "ar" { 78. } else { 3. };
                let markers = window
                    .painted_quad_opacities_for_test()
                    .into_iter()
                    .filter(|(bounds, color)| {
                        bounds.origin.x == px(marker_x)
                            && bounds.size.width == px(2.)
                            && color.a > 0.5
                    })
                    .count();
                assert_eq!(
                    markers,
                    usize::from(config["collapsed"] == true && active),
                    "{config}"
                );
                let collapsed = config["collapsed"] == true;
                let icon_color = if active {
                    cx.theme().primary
                } else if collapsed {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().foreground
                };
                let text_color = if collapsed && !active {
                    cx.theme().muted_foreground
                } else if !collapsed && active {
                    cx.theme().primary
                } else {
                    cx.theme().foreground
                };
                let glyphs = window.painted_glyphs_for_test();
                assert!(!glyphs.is_empty());
                for (bounds, _, color) in glyphs {
                    assert_eq!(
                        color,
                        if icon.contains(&bounds.center()) {
                            icon_color
                        } else {
                            text_color
                        },
                        "Painted color: {config}: {bounds:?}"
                    );
                }
                let tree = window.accessibility_tree_for_test().unwrap();
                assert!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.value() == case["label"]["text"].as_str()),
                    "Missing label: {config}"
                );
                assert!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == Role::Button
                            && node.is_selected() == Some(active)),
                    "Selection: {config}"
                );
            });
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[gpui::test]
fn sidebar_action_row_matches_original_layout_and_input(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    cx.update(crate::styles::install_dialog_styles);
    struct Probe(Entity<Feishin>);
    impl Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .key_context("Feishin")
                .on_action(|_: &crate::shortcuts::RunShortcut, _, _| {
                    panic!("Search keys must not trigger playback shortcuts")
                })
                .child(self.0.update(cx, |view, cx| view.sidebar_action_bar(cx)))
                .children(Root::render_dialog_layer(window, cx))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-action-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-action-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases {
        let config = &case["config"];
        let directory = std::env::temp_dir().join(format!(
            "feishin-sidebar-action-layout-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(directory.join("Themes")).unwrap();
        let mut theme = appearance::builtins()["defaultTheme"].clone();
        theme
            .app
            .insert("root-font-size".into(), format!("{}px", config["rem"]));
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
            state.settings.language = config["language"].as_str().unwrap().into();
            state.settings.sidebar.width = config["width"].as_f64().unwrap() as f32;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            parent.update(cx, |view, _| {
                view.route = Route::Servers;
                view.history = vec![Route::Home.into()];
                view.forward_history = vec![Route::Queue.into()];
                view.library_scan.watching = true;
                view.library_scan.status.scanning = config["scanning"] == true;
            });
            captured.replace(Some(parent.clone()));
            let probe = cx.new(|_| Probe(parent));
            Root::new(probe, window, cx)
        });
        let parent = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(800.)));
        window_cx.run_until_parked();
        assert_eq!(
            window_cx
                .debug_bounds("application-menu-scanning")
                .is_some(),
            config["scanning"] == true
        );
        for (selector, expected) in [
            ("sidebar-action-bar", &case["bar"]),
            ("sidebar-command-palette", &case["input"]),
            ("sidebar-search-icon", &case["searchIcon"]),
            ("application-menu", &case["buttons"][0]),
            ("sidebar-back", &case["buttons"][1]),
            ("sidebar-forward", &case["buttons"][2]),
        ] {
            let bounds = window_cx.debug_bounds(selector).unwrap();
            let actual = [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ];
            for (actual, expected) in actual
                .into_iter()
                .zip(expected["bounds"].as_array().unwrap())
            {
                assert!(
                    (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "{config}: {selector}: {bounds:?}, expected {}",
                    expected
                );
            }
        }
        let tree =
            window_cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button
                    && node.label() == case["placeholder"].as_str())
        );

        // Exercise the same input callbacks in each locale, including while a scan is running.
        if config["width"] == 260 && config["rem"] == 16 {
            let click = |selector: &str, cx: &mut gpui::VisualTestContext| {
                let point = cx.debug_bounds(selector).unwrap().center();
                cx.simulate_click(point, Modifiers::none());
                cx.run_until_parked();
            };
            click("sidebar-back", window_cx);
            assert_eq!(
                parent.read_with(window_cx, |view, _| view.route.clone()),
                Route::Home
            );
            window_cx.refresh().unwrap();
            click("sidebar-forward", window_cx);
            assert_eq!(
                parent.read_with(window_cx, |view, _| view.route.clone()),
                Route::Servers
            );
            click("sidebar-command-palette", window_cx);
            for key in ["enter", "space"] {
                assert!(
                    parent.read_with(window_cx, |view, _| view.command_palette.is_some()),
                    "Before {key}: {config}"
                );
                window_cx.simulate_keystrokes("escape");
                window_cx
                    .executor()
                    .advance_clock(std::time::Duration::from_millis(400));
                window_cx.run_until_parked();
                assert!(parent.read_with(window_cx, |view, _| view.command_palette.is_none()));
                assert!(
                    window_cx.update(|window, cx| parent
                        .read(cx)
                        .sidebar_search_focus
                        .is_focused(window)),
                    "Search focus was not restored: {config}"
                );
                window_cx.simulate_keystrokes(key);
                window_cx.simulate_event(KeyUpEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                });
                window_cx.run_until_parked();
            }
            assert!(parent.read_with(window_cx, |view, _| view.command_palette.is_some()));
            window_cx.simulate_keystrokes("escape");
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(400));
            window_cx.run_until_parked();
            click("application-menu", window_cx);
            assert!(window_cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Menu)
            }));
            window_cx.simulate_keystrokes("escape");
            window_cx.run_until_parked();
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[gpui::test]
fn expanded_sidebar_composition_headings_and_artwork_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe {
        parent: Entity<Feishin>,
        height: f32,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .h(px(self.height))
                .child(self.parent.update(cx, |view, cx| view.sidebar_view(cx)))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-expanded-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-expanded-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-expanded-sidebar-{}", uuid::Uuid::new_v4()));
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
            state.settings.sidebar.collapsed = false;
            state.settings.sidebar.playlist_list = false;
            state.settings.sidebar.items = ["Home", "Albums", "Tracks", "Playlists", "Settings"]
                .into_iter()
                .map(|id| feishin_core::sidebar::Entry {
                    id: id.into(),
                    disabled: false,
                })
                .collect();
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            let probe = cx.new(|_| Probe {
                parent,
                height: 480.,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(1100.)));
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            window_cx.update(|window, cx| {
                probe.update(cx, |probe, cx| {
                    probe.height = config["height"].as_f64().unwrap() as f32;
                    probe.parent.update(cx, |view, cx| {
                        view.state.settings.sidebar.width =
                            config["width"].as_f64().unwrap() as f32;
                        view.state.settings.sidebar.image_shown = config["image"] == true;
                        view.state.settings.sidebar.image_enabled = true;
                        view.state.settings.language = config["language"].as_str().unwrap().into();
                        view.sidebar_closed_sections.clear();
                        cx.set_global(crate::appearance::Language(
                            view.state.settings.language.clone(),
                        ));
                    });
                    cx.notify();
                });
                window.refresh();
            });
            window_cx.simulate_mouse_move(point(px(800.), px(1050.)), None, Modifiers::none());
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(300));
            window_cx.update(|window, _| window.refresh());
            window_cx.run_until_parked();
            let check =
                |selector: &str, expected: &serde_json::Value, cx: &mut VisualTestContext| {
                    let bounds = cx
                        .debug_bounds(selector)
                        .unwrap_or_else(|| panic!("Missing {selector}: {config}"));
                    for (actual, expected) in [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .zip(expected["bounds"].as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{config}: {selector}: {bounds:?}, expected {}",
                            expected
                        );
                    }
                };
            for (selector, part) in [
                ("sidebar", "sidebar"),
                ("sidebar-action-bar", "action"),
                ("sidebar-scroll", "scroll"),
                ("sidebar-section-library", "heading"),
                ("sidebar-section-label-library", "headingLabel"),
                ("sidebar-section-chevron-library", "chevron"),
            ] {
                check(selector, &case[part], window_cx);
            }
            for (id, expected) in ["Home", "Albums", "Tracks", "Playlists", "Settings"]
                .into_iter()
                .zip(case["rows"].as_array().unwrap())
            {
                check(
                    &format!("sidebar-navigation-control-{id}"),
                    expected,
                    window_cx,
                );
            }
            let collapse = probe.read_with(window_cx, |probe, cx| {
                probe.parent.read(cx).tr("common.collapse")
            });
            window_cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert!(
                    !tree
                        .nodes
                        .iter()
                        .any(|(_, node)| node.role() == Role::Button
                            && node.label() == Some(collapse.as_ref())),
                    "Collapse must wait for artwork hover: {config}"
                );
            });
            if config["image"] == true {
                check("sidebar-artwork", &case["artwork"], window_cx);
                check("sidebar-artwork-picture", &case["picture"], window_cx);
                let artwork = window_cx.debug_bounds("sidebar-artwork").unwrap();
                window_cx.simulate_mouse_move(artwork.center(), None, Modifiers::none());
                window_cx.run_until_parked();
                check("sidebar-hide-artwork", &case["hide"], window_cx);
                let hide = window_cx.debug_bounds("sidebar-hide-artwork").unwrap();
                window_cx.simulate_click(hide.center(), Modifiers::none());
                window_cx.run_until_parked();
                probe.read_with(window_cx, |probe, cx| {
                    let view = probe.parent.read(cx);
                    assert!(!view.state.settings.sidebar.image_shown);
                    assert!(!view.full_player);
                });
                // In a short sidebar, the exiting image still covers the heading.
                window_cx
                    .executor()
                    .advance_clock(std::time::Duration::from_millis(300));
                window_cx.update(|window, _| window.refresh());
                window_cx.run_until_parked();
            }
            let heading = window_cx.debug_bounds("sidebar-section-library").unwrap();
            window_cx.simulate_click(heading.center(), Modifiers::none());
            window_cx.run_until_parked();
            probe.read_with(window_cx, |probe, cx| {
                assert!(
                    probe
                        .parent
                        .read(cx)
                        .sidebar_closed_sections
                        .contains("library")
                )
            });
            window_cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert!(
                    !tree.nodes.iter().any(
                        |(_, node)| node.role() == Role::Button && node.label() == Some("Home")
                    )
                );
            });
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
