use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, StyledExt,
    input::{InputEvent, InputState},
};
use std::time::Duration;
use uuid::Uuid;

pub fn view(app: WeakEntity<Feishin>, preview: Option<String>) -> AnyElement {
    Section { app, preview }.into_any_element()
}

#[derive(IntoElement)]
struct Section {
    app: WeakEntity<Feishin>,
    preview: Option<String>,
}

impl RenderOnce for Section {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let id = if self.preview.is_some() {
            "tag-path-settings"
        } else {
            "path-settings"
        };
        let preview = self.preview;
        let fields = window.use_keyed_state(id, cx, |window, cx| {
            Fields::new(self.app, preview.clone(), window, cx)
        });
        fields.update(cx, |fields, _| fields.preview = preview);
        fields
    }
}

struct Fields {
    app: WeakEntity<Feishin>,
    inputs: [Entity<InputState>; 2],
    saved: [String; 2],
    preview: Option<String>,
    sample: String,
    server: Option<Uuid>,
    loading: bool,
    generation: u64,
    pending: [Option<Task<()>>; 2],
    _subscriptions: Vec<Subscription>,
}

impl Fields {
    fn new(
        app: WeakEntity<Feishin>,
        preview: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (inputs, saved) = app
            .read_with(cx, |view, _| {
                (
                    [view.path_replace.clone(), view.path_replace_with.clone()],
                    [
                        view.state.settings.path_replace.clone(),
                        view.state.settings.path_replace_with.clone(),
                    ],
                )
            })
            .unwrap();
        for (input, value) in inputs.iter().zip(&saved) {
            input.update(cx, |input, cx| input.set_value(value.clone(), window, cx));
        }
        let subscriptions = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| {
                cx.subscribe_in(
                    input,
                    window,
                    move |fields: &mut Self, input, event: &InputEvent, _, cx| {
                        if !matches!(event, InputEvent::Change) {
                            return;
                        }
                        fields.pending[index] = None;
                        let value = input.read(cx).value().to_string();
                        if fields.preview.is_some() {
                            fields.commit(index, value, cx);
                        } else {
                            let timer = cx.background_executor().timer(Duration::from_millis(500));
                            fields.pending[index] = Some(cx.spawn(async move |fields, cx| {
                                timer.await;
                                fields
                                    .update(cx, |fields, cx| fields.commit(index, value, cx))
                                    .ok();
                            }));
                        }
                    },
                )
            })
            .collect();
        Self {
            app,
            inputs,
            saved,
            preview,
            sample: String::new(),
            server: None,
            loading: false,
            generation: 0,
            pending: [None, None],
            _subscriptions: subscriptions,
        }
    }

    fn commit(&mut self, index: usize, value: String, cx: &mut Context<Self>) {
        self.app
            .update(cx, |view, cx| {
                let setting = if index == 0 {
                    &mut view.state.settings.path_replace
                } else {
                    &mut view.state.settings.path_replace_with
                };
                if *setting != value {
                    *setting = value;
                    view.save();
                    cx.notify();
                }
            })
            .ok();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let api = self
            .app
            .read_with(cx, |view, _| {
                view.state
                    .active_server
                    .and_then(|id| view.sessions.get(&id).cloned())
            })
            .ok()
            .flatten();
        let Some(api) = api else {
            return;
        };
        self.server = Some(api.server.id);
        let server = api.server.id;
        self.generation += 1;
        let generation = self.generation;
        self.loading = true;
        let task = cx.background_executor().spawn(async move {
            api.random_songs(1, None).map(|songs| {
                songs
                    .into_iter()
                    .next()
                    .map(|song| song.path)
                    .unwrap_or_default()
            })
        });
        cx.spawn(async move |fields, cx| {
            let result = task.await;
            fields
                .update(cx, |fields, cx| {
                    if fields.generation != generation || fields.server != Some(server) {
                        return;
                    }
                    fields.loading = false;
                    match result {
                        Ok(path) => {
                            fields.sample = path.clone();
                            fields
                                .app
                                .update(cx, |view, _| {
                                    view.path_previews.insert(server, path);
                                })
                                .ok();
                        }
                        Err(error) => log::debug!("Path preview could not load: {error}"),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
        cx.notify();
    }
}

impl Render for Fields {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (saved, server, cached) = self
            .app
            .read_with(cx, |view, _| {
                (
                    [
                        view.state.settings.path_replace.clone(),
                        view.state.settings.path_replace_with.clone(),
                    ],
                    view.state.active_server,
                    view.state
                        .active_server
                        .and_then(|id| view.path_previews.get(&id).cloned()),
                )
            })
            .unwrap();
        for (index, saved) in saved.iter().enumerate() {
            if self.saved[index] != *saved {
                self.saved[index] = saved.clone();
                if self.inputs[index].read(cx).value().as_str() != saved {
                    self.inputs[index]
                        .update(cx, |input, cx| input.set_value(saved.clone(), window, cx));
                }
            }
        }
        if self.preview.is_none() && self.server != server {
            self.generation += 1;
            self.server = server;
            self.loading = false;
            self.sample = cached.clone().unwrap_or_default();
            if cached.is_none() {
                self.refresh(cx);
            }
        }
        let preview = self
            .app
            .read_with(cx, |view, _| {
                view.state
                    .settings
                    .resolve_song_path(self.preview.as_deref().unwrap_or(&self.sample))
            })
            .unwrap();
        let preview = preview
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        div()
            .id("path-settings")
            .debug_selector(|| "path-settings".into())
            .v_flex()
            .gap_3()
            .w_full()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                    .child(crate::appearance::text("setting.pathReplace", cx))
                    .when(self.preview.is_none(), |row| {
                        row.child(
                            crate::styles::button("path-preview-refresh", cx)
                                .debug_selector(|| "path-preview-refresh".into())
                                .icon(Icon::default().path("native-icons/refresh.svg"))
                                .tooltip(crate::appearance::text("common.refresh", cx))
                                .loading(self.loading)
                                .disabled(self.server.is_none())
                                .on_click(cx.listener(|fields, _, _, cx| fields.refresh(cx))),
                        )
                    }),
            )
            .child(
                div()
                    .id("path-preview")
                    .debug_selector(|| "path-preview".into())
                    .w_full()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        gpui_component::text::TextView::html(
                            SharedString::from(format!("path-preview-text-{preview}")),
                            format!("<pre>{preview}</pre>"),
                            window,
                            cx,
                        )
                        .selectable(true),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                    .children(self.inputs.iter().map(|input| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(crate::styles::input(input, cx))
                    })),
            )
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{Feishin, Route};
    use feishin_core::{api::Api, model::*, storage::State};
    use gpui::{AppContext, Modifiers, TestAppContext};
    use gpui_component::Root;
    use std::time::Duration;
    use std::{
        cell::RefCell,
        io::{Read, Write},
        net::TcpListener,
        rc::Rc,
    };
    use uuid::Uuid;

    fn state(directory: &std::path::Path) -> State {
        let mut state = State::default();
        state.settings.home.items.clear();
        state.settings.home.feature = false;
        state.settings.lyric_fetch = false;
        state.settings.scrobble = false;
        state.settings.mpv_path = directory.join("missing-player").to_string_lossy().into();
        state.settings.path_replace = "/srv/".into();
        state.settings.path_replace_with = "/local/".into();
        state
    }

    #[gpui::test]
    fn path_editor_matches_original_debounce_preview_refresh_and_immediate_states(
        cx: &mut TestAppContext,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let server = Server {
            id: Uuid::new_v4(),
            name: "Path fixture".into(),
            url: format!("http://{}", listener.local_addr().unwrap()),
            username: "fixture".into(),
            kind: ServerType::Subsonic,
            legacy_auth: false,
            options: Default::default(),
        };
        let worker = std::thread::spawn(move || {
            loop {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = [0; 4096];
                let count = stream.read(&mut bytes).unwrap();
                let request = String::from_utf8_lossy(&bytes[..count]).to_string();
                let preview = request.contains("getRandomSongs");
                let body = if preview {
                    r#"{"subsonic-response":{"status":"ok","randomSongs":{"song":[{"id":"sample","title":"Sample","path":"/srv/next.flac"}]}}}"#
                } else {
                    assert!(request.contains("getPlaylists"), "{request}");
                    r#"{"subsonic-response":{"status":"ok","playlists":{"playlist":[]}}}"#
                };
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                if preview {
                    break request;
                }
            }
        });
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-path-reference-{}", Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let capture = captured.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let app =
                cx.new(|cx| Feishin::new(directory.clone(), Ok(state(&directory)), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.state.active_server = Some(server.id);
                view.path_previews
                    .insert(server.id, "/srv/song.flac".into());
                view.sessions.insert(
                    server.id,
                    Api::new(server.clone(), Session::default()).unwrap(),
                );
                view.settings_search.update(cx, |input, cx| {
                    input.set_value("File path replacement", window, cx)
                });
            });
            capture.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/path-settings-reference.json")).unwrap();
        for case in cases {
            let name = case["name"].as_str().unwrap();
            match name {
                "initial" => {}
                "remove-committed" | "add-committed" => {
                    cx.executor().advance_clock(Duration::from_millis(501))
                }
                "refresh" => {
                    let bounds = cx.debug_bounds("path-preview-refresh").unwrap();
                    cx.simulate_click(bounds.center(), Modifiers::none());
                }
                "external" => {
                    app.update(cx, |view, cx| {
                        view.state.settings.path_replace = "/srv/".into();
                        view.state.settings.path_replace_with = "/restored/".into();
                        view.save();
                        cx.notify();
                    });
                }
                "immediate" => {
                    app.update(cx, |view, cx| {
                        view.route = Route::TagEditor;
                        view.tag_targets = vec![(
                            directory.join("missing.flac"),
                            Item {
                                kind: ItemKind::Song,
                                path: "/srv/missing.flac".into(),
                                ..Default::default()
                            },
                        )];
                        cx.notify();
                    });
                }
                _ => {
                    let (index, value) = match name {
                        "remove-draft" => (0, "/wrong/"),
                        "add-draft" => (1, "/new/"),
                        "immediate-remove" => (0, ""),
                        "immediate-add" => (1, "/fixed/"),
                        _ => unreachable!(),
                    };
                    let input = app.read_with(cx, |view, _| {
                        if index == 0 {
                            view.path_replace.clone()
                        } else {
                            view.path_replace_with.clone()
                        }
                    });
                    cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
                    cx.simulate_keystrokes(if cfg!(target_os = "macos") {
                        "cmd-a"
                    } else {
                        "ctrl-a"
                    });
                    if value.is_empty() {
                        cx.simulate_keystrokes("backspace");
                    } else {
                        cx.simulate_input(value);
                    }
                }
            }
            cx.run_until_parked();
            if name == "refresh" {
                for _ in 0..400 {
                    if app.read_with(cx, |view, _| {
                        view.path_previews
                            .get(&server.id)
                            .is_some_and(|path| path == "/srv/next.flac")
                    }) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                    cx.run_until_parked();
                }
                assert_eq!(
                    app.read_with(cx, |view, _| view.path_previews.get(&server.id).cloned()),
                    Some("/srv/next.flac".into())
                );
            }
            app.read_with(cx, |view, cx| {
                assert_eq!(
                    view.state.settings.path_replace, case["stored"]["pathReplace"],
                    "{name}"
                );
                assert_eq!(
                    view.state.settings.path_replace_with, case["stored"]["pathReplaceWith"],
                    "{name}"
                );
                assert_eq!(
                    view.path_replace.read(cx).value().as_str(),
                    case["inputs"][0].as_str().unwrap(),
                    "{name}"
                );
                assert_eq!(
                    view.path_replace_with.read(cx).value().as_str(),
                    case["inputs"][1].as_str().unwrap(),
                    "{name}"
                );
            });
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert_eq!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == gpui::Role::Button
                            && node.label() == Some("Refresh")),
                    case["refresh"].as_bool().unwrap(),
                    "{name}"
                );
                assert!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.value() == case["preview"].as_str()),
                    "{name}: missing preview {}; values: {:?}",
                    case["preview"],
                    tree.nodes
                        .iter()
                        .filter_map(|(_, node)| node.value())
                        .collect::<Vec<_>>()
                );
            });
        }
        let request = worker.join().unwrap();
        assert!(
            request.contains("getRandomSongs") && request.contains("size=1"),
            "{request}"
        );
        let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(saved.settings.path_replace, "");
        assert_eq!(saved.settings.path_replace_with, "/fixed/");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[gpui::test]
    fn missing_tag_file_can_be_remapped_and_reloaded_without_modifying_audio_or_queue(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-path-reload-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("song.wav");
        let mut wave = b"RIFF".to_vec();
        wave.extend(56u32.to_le_bytes());
        wave.extend(b"WAVEfmt ");
        wave.extend(16u32.to_le_bytes());
        wave.extend(1u16.to_le_bytes());
        wave.extend(1u16.to_le_bytes());
        wave.extend(44100u32.to_le_bytes());
        wave.extend(88200u32.to_le_bytes());
        wave.extend(2u16.to_le_bytes());
        wave.extend(16u16.to_le_bytes());
        wave.extend(b"data");
        wave.extend(20u32.to_le_bytes());
        wave.extend([0; 20]);
        std::fs::write(&path, &wave).unwrap();
        feishin_core::tags::write(
            &path,
            &feishin_core::tags::Patch {
                fields: [("TITLE".into(), vec!["Recovered title".into()])].into(),
                ..Default::default()
            },
        )
        .unwrap();
        let original = std::fs::read(&path).unwrap();
        let captured = Rc::new(RefCell::new(None));
        let capture = captured.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = state(&directory);
            state.settings.path_replace_with = format!("{}/missing/", directory.display());
            let item = Item {
                id: "song".into(),
                kind: ItemKind::Song,
                path: "/srv/song.wav".into(),
                ..Default::default()
            };
            state.queue.add(
                vec![item.clone(), item.clone()],
                feishin_core::queue::Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| view.edit_tag_items(vec![item], window, cx));
            capture.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let wait_for_tags = |cx: &mut gpui::VisualTestContext| {
            for _ in 0..200 {
                cx.run_until_parked();
                if app.read_with(cx, |view, _| !view.tag_busy) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("Tag metadata did not finish loading");
        };
        wait_for_tags(cx);
        assert!(cx.debug_bounds("tag-path-recovery").is_some());
        assert!(cx.debug_bounds("path-preview-refresh").is_none());
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let prefix = format!("{}/", directory.display());
        let input = app.read_with(cx, |view, _| view.path_replace_with.clone());
        cx.update(|window, cx| input.update(cx, |input, cx| input.focus(window, cx)));
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.simulate_input(&prefix);
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.path_replace_with.clone()),
            prefix
        );
        cx.update(|window, _| {
            assert!(
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == path.to_str())
            );
        });
        let reload = cx.debug_bounds("tag-path-reload").unwrap();
        cx.simulate_click(reload.center(), Modifiers::none());
        wait_for_tags(cx);
        app.read_with(cx, |view, _| {
            assert_eq!(view.tag_files.len(), 1, "{}", view.message);
            assert_eq!(view.tag_files[0].path, path);
            assert!(view.tag_fields.iter().any(|field| field.name == "TITLE"));
            assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
        });
        cx.update(|window, _| {
            assert!(
                !window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == gpui::Role::Button
                        && node.label() == Some("Reload"))
            );
        });
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(saved.settings.path_replace_with, prefix);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
