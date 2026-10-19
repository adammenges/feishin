use super::*;
use ::core::prelude::v1::test;
use feishin_core::{
    api::Api,
    appearance,
    model::{Server, Session},
    storage,
};
use gpui_component::Root;
use std::{cell::RefCell, rc::Rc};

#[gpui::test]
fn playlist_section_headings_and_separator_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    struct Probe {
        parent: Entity<Feishin>,
    }
    impl Render for Probe {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(self.parent.read(cx).state.settings.sidebar.width))
                .h(px(900.))
                .child(self.parent.update(cx, |parent, cx| parent.sidebar_view(cx)))
        }
    }
    #[cfg(target_os = "macos")]
    let source = include_str!("../assets/sidebar-playlist-headings-darwin-reference.json");
    #[cfg(target_os = "linux")]
    let source = include_str!("../assets/sidebar-playlist-headings-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(source).unwrap();
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Fixture".into(),
        url: "http://127.0.0.1:9".into(),
        username: "me".into(),
        kind: ServerType::Navidrome,
        legacy_auth: false,
        options: Default::default(),
    };
    for rem in [16, 20] {
        let directory =
            std::env::temp_dir().join(format!("feishin-section-heading-{}", uuid::Uuid::new_v4()));
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
            state.settings.sidebar.image_shown = false;
            state.settings.sidebar.playlist_list = true;
            state.settings.sidebar.items = ["Home", "Albums", "Tracks", "Playlists", "Settings"]
                .into_iter()
                .map(|id| feishin_core::sidebar::Entry {
                    id: id.into(),
                    disabled: false,
                })
                .collect();
            state.settings.playlist_folders = true;
            state.settings.playlist_separator = "/".into();
            let parent = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            parent.update(cx, |parent, _| {
                parent.state.active_server = Some(server.id);
                parent.sessions.insert(
                    server.id,
                    Api::new(server.clone(), Session::default()).unwrap(),
                );
                parent.playlist_options = [("owned", "me"), ("shared", "other")]
                    .into_iter()
                    .map(|(id, owner)| Item {
                        id: id.into(),
                        name: "Folder/Playlist".into(),
                        owner_name: owner.into(),
                        server_id: server.id,
                        kind: ItemKind::Playlist,
                        ..Default::default()
                    })
                    .collect();
            });
            let probe = cx.new(|_| Probe { parent });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        window_cx.simulate_resize(size(px(1000.), px(1100.)));
        for case in cases.iter().filter(|case| case["config"]["rem"] == rem) {
            let config = &case["config"];
            probe.update(window_cx, |probe, cx| {
                probe.parent.update(cx, |parent, cx| {
                    parent.state.settings.sidebar.width = config["width"].as_f64().unwrap() as f32;
                    parent.state.settings.language = config["language"].as_str().unwrap().into();
                    cx.set_global(crate::appearance::Language(
                        parent.state.settings.language.clone(),
                    ));
                    let navigation = config["navigation"] == true;
                    parent.state.settings.playlist_folder_view =
                        if navigation { "navigation" } else { "tree" }.into();
                    parent.playlist_folder_path = if navigation { "Folder" } else { "" }.into();
                    parent.shared_playlist_folder_path = parent.playlist_folder_path.clone();
                    for shared in [false, true] {
                        if parent
                            .state
                            .playlist_folder_expanded(server.id, "Folder", shared)
                            != (config["expanded"] == true)
                        {
                            parent
                                .state
                                .toggle_playlist_folder(server.id, "Folder", shared);
                        }
                    }
                });
                cx.notify();
            });
            window_cx.run_until_parked();
            window_cx
                .executor()
                .advance_clock(std::time::Duration::from_millis(200));
            window_cx.update(|window, _| window.refresh());
            window_cx.run_until_parked();
            let divider = window_cx
                .debug_bounds("sidebar-playlist-separator")
                .unwrap();
            let expected = &case["divider"]["bounds"];
            for (actual, expected) in [
                divider.origin.x,
                divider.origin.y,
                divider.size.width,
                divider.size.height,
            ]
            .into_iter()
            .zip([
                rem as f32 * 0.75,
                expected[1].as_f64().unwrap() as f32,
                expected[2].as_f64().unwrap() as f32,
                expected[3].as_f64().unwrap() as f32,
            ]) {
                assert!(
                    (f32::from(actual) - expected).abs() <= 1.,
                    "separator: {divider:?}, {case}"
                );
            }
            for (index, section) in ["playlists", "shared-playlists"].into_iter().enumerate() {
                let reference = &case["headings"][index];
                let heading = window_cx
                    .debug_bounds(&format!("sidebar-section-{section}"))
                    .unwrap();
                assert!(
                    (f32::from(heading.size.width)
                        - reference["bounds"][2].as_f64().unwrap() as f32)
                        .abs()
                        <= 1.,
                    "width: {heading:?}, {case}"
                );
                assert!(
                    (f32::from(heading.size.height)
                        - reference["bounds"][3].as_f64().unwrap() as f32)
                        .abs()
                        <= 1.,
                    "height: {heading:?}, {case}"
                );
                if index == 0 {
                    assert!(
                        (f32::from(heading.top())
                            - reference["bounds"][1].as_f64().unwrap() as f32)
                            .abs()
                            <= 1.,
                        "heading top: {heading:?}, {case}"
                    );
                }
                let mut actions = vec![];
                if config["navigation"] == true {
                    actions.push(if index == 0 {
                        "playlist-folder-back"
                    } else {
                        "shared-playlist-folder-back"
                    });
                }
                if index == 0 {
                    actions.push("sidebar-create-playlist");
                    if config["navigation"] != true {
                        actions.push("sidebar-toggle-all-folders");
                    }
                    actions.push("sidebar-view-playlists");
                }
                let mut checks = actions
                    .into_iter()
                    .zip(reference["actions"].as_array().unwrap())
                    .map(|(id, value)| (id.to_string(), value["bounds"].clone()))
                    .collect::<Vec<_>>();
                checks.push((
                    format!("sidebar-section-chevron-{section}"),
                    reference["chevron"].clone(),
                ));
                for (id, expected) in checks {
                    let bounds = window_cx.debug_bounds(&id).unwrap();
                    for (actual, expected) in [
                        bounds.left() - heading.left(),
                        bounds.top() - heading.top(),
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .zip(expected.as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{id}: {bounds:?} in {heading:?}, expected {expected}; {case}"
                        );
                    }
                }
            }
        }
        window_cx.update(|window, _| window.remove_window());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
