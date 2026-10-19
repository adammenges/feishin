use super::*;
use ::core::prelude::v1::test;
use feishin_core::{
    api::Api,
    appearance,
    model::{Server, ServerType, Session},
    storage,
};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[gpui::test]
fn palette_frames_match_original_composition(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    cx.update(crate::styles::install_dialog_styles);
    struct Probe;
    impl Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .children(Root::render_dialog_layer(window, cx))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/command-layout-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/command-layout-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    for case in cases {
        let config = &case["config"];
        let directory =
            std::env::temp_dir().join(format!("feishin-palette-layout-{}", uuid::Uuid::new_v4()));
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
            let mut state = storage::State::default();
            state.settings.theme = "reference".into();
            state.settings.follow_system_theme = false;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(parent));
            let probe = cx.new(|_| Probe);
            Root::new(probe, window, cx)
        });
        let parent = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(
            px(config["width"].as_f64().unwrap() as f32),
            px(config["height"].as_f64().unwrap() as f32),
        ));
        window_cx.update(|window, cx| {
            parent.update(cx, |parent, cx| {
                let server = Server {
                    id: uuid::Uuid::from_u128(1),
                    name: "Music server".into(),
                    url: "http://127.0.0.1:1".into(),
                    kind: ServerType::Navidrome,
                    username: "fixture".into(),
                    legacy_auth: false,
                    options: Default::default(),
                };
                parent.state.servers = vec![
                    server.clone(),
                    Server {
                        id: uuid::Uuid::from_u128(2),
                        name: "Other server".into(),
                        ..server.clone()
                    },
                ];
                parent.state.active_server = Some(server.id);
                parent.sessions.insert(
                    server.id,
                    Api::new(
                        server,
                        Session {
                            token: "fixture".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                parent
                    .state
                    .command_palette_sections
                    .insert("albums".into(), config["collapsed"] != true);
                parent.open_command_palette(window, cx);
            })
        });
        window_cx.run_until_parked();
        let palette = parent.read_with(window_cx, |parent, _| {
            parent.command_palette.clone().unwrap()
        });
        window_cx.update(|window, cx| {
            palette.read(cx).input.clone().update(cx, |input, cx| {
                input.set_value(config["query"].as_str().unwrap().to_owned(), window, cx)
            });
        });
        window_cx.run_until_parked();
        window_cx.update(|_, cx| {
            palette.update(cx, |palette, cx| {
                palette.page = match config["page"].as_str().unwrap() {
                    "go" => Page::Navigation,
                    "servers" => Page::Servers,
                    _ => Page::Home,
                };
                if !palette.query.is_empty() {
                    for (index, kind) in KINDS.into_iter().enumerate() {
                        let resource = ["albums", "albumArtists", "songs"][index];
                        palette.sections[index] = Section {
                            items: (0..2)
                                .map(|item| Item {
                                    id: format!("{resource}-{item}"),
                                    name: format!("Music {resource} {item}"),
                                    kind,
                                    server_id: uuid::Uuid::from_u128(1),
                                    artist: if index == 0 {
                                        "Album artist"
                                    } else {
                                        "Track artist"
                                    }
                                    .into(),
                                    album_id: "albums-0".into(),
                                    column_data: std::collections::BTreeMap::from([(
                                        "albumCount".into(),
                                        "1".into(),
                                    )]),
                                    ..Default::default()
                                })
                                .collect(),
                            fetched: true,
                            ..Default::default()
                        };
                    }
                }
                palette.selected = config["selected"]
                    .as_str()
                    .map(|_| "tracks-songs-0".into())
                    .or_else(|| (palette.page == Page::Home).then(|| "search".into()));
                cx.notify();
            })
        });
        window_cx
            .executor()
            .advance_clock(Duration::from_millis(310));
        window_cx.refresh().unwrap();
        window_cx.run_until_parked();
        let commands = palette.read_with(window_cx, |palette, cx| palette.commands(cx));
        let mut assert_bounds = |id: &str, expected: &serde_json::Value| {
            let actual = window_cx
                .debug_bounds(id)
                .unwrap_or_else(|| panic!("Missing {id}"));
            for ((name, actual), expected) in ["x", "y", "width", "height"]
                .into_iter()
                .zip([
                    actual.left(),
                    actual.top(),
                    actual.size.width,
                    actual.size.height,
                ])
                .zip(expected["bounds"].as_array().unwrap())
            {
                let actual = f32::from(actual);
                let expected = expected.as_f64().unwrap() as f32;
                assert!(
                    (actual - expected).abs() <= 1.,
                    "{config}: {id} {name}: native {actual}, original {expected}"
                );
            }
        };
        assert_bounds("native-modal-content-0", &case["parts"]["dialog"]);
        assert_bounds("palette-input", &case["parts"]["input"]);
        assert_bounds("palette-results", &case["parts"]["list"]);
        assert_bounds("palette-breadcrumbs", &case["footer"][0]);
        for index in 0..4 {
            assert_bounds(&format!("palette-kbd-{index}"), &case["footer"][index + 1]);
        }
        let items = commands
            .iter()
            .enumerate()
            .filter(|(_, c)| !matches!(c.operation, Operation::All(_)))
            .collect::<Vec<_>>();
        assert_eq!(items.len(), case["items"].as_array().unwrap().len());
        for ((index, command), expected) in items.into_iter().zip(case["items"].as_array().unwrap())
        {
            assert_bounds(&format!("palette-{}", command.id), expected);
            if !expected["artwork"].is_null() {
                assert_bounds(&format!("palette-artwork-{index}"), &expected["artwork"]);
                for (part, expected) in ["title", "subtitle"]
                    .into_iter()
                    .zip(expected["metadata"].as_array().unwrap())
                {
                    assert_bounds(&format!("palette-{part}-{index}"), expected);
                }
            }
            for (action, expected) in expected["actions"].as_array().unwrap().iter().enumerate() {
                let id = if action == 3 {
                    format!("palette-playlist-{index}")
                } else {
                    format!("palette-play-{index}-{action}")
                };
                assert_bounds(&id, expected);
            }
        }
        for (index, expected) in case["headings"].as_array().unwrap().iter().enumerate() {
            assert_bounds(&format!("palette-all-{}", SECTION_KEYS[index]), expected);
        }
        window_cx.update(|window, _| window.remove_window());
        drop(parent);
        std::fs::remove_dir_all(directory).ok();
    }
}
