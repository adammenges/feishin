use crate::{
    app::{Feishin, Route},
    scroll::VerticalScroll,
};
use feishin_core::{
    auto_dj::Mode,
    model::{Item, ItemKind, ListQuery, ServerType},
    queue::Enqueue,
    shuffle::{Config, Played},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Selectable, StyledExt,
    checkbox::Checkbox,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    menu::{DropdownMenu, PopupMenuItem},
};

pub struct Form {
    pub inputs: [Entity<InputState>; 5],
    genres: Vec<Item>,
    request: uuid::Uuid,
    server: Option<uuid::Uuid>,
    albums: std::rc::Rc<std::cell::Cell<bool>>,
    _subscriptions: Vec<Subscription>,
}

impl Feishin {
    pub fn open_random_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.init_random_form(window, cx);
        self.navigate(Route::Random, cx);
    }

    pub fn close_random_form(&mut self, cx: &mut Context<Self>) {
        self.cancel_library_fetches(cx);
        self.back(cx);
    }

    fn init_random_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let config = &self.state.settings.shuffle;
        let values = [
            config.limit.to_string(),
            config.min_year.to_string(),
            config.max_year.to_string(),
            config.genre.clone(),
            config.music_folder.clone(),
        ];
        let labels = [
            "How many songs?",
            "From year",
            "To year",
            "Genre",
            "Music library ID (optional)",
        ];
        let albums = std::rc::Rc::new(std::cell::Cell::new(config.kind == Mode::Albums));
        let inputs: [Entity<InputState>; 5] = std::array::from_fn(|index| {
            let albums = albums.clone();
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .aria_label(labels[index])
                    .default_value(values[index].clone())
                    .label_transform(move |label, cx| {
                        let albums = index == 0 && albums.get();
                        crate::appearance::text(
                            if albums {
                                "How many albums?"
                            } else {
                                label.as_ref()
                            },
                            cx,
                        )
                        .into()
                    })
            })
        });
        let mut subscriptions = Vec::new();
        for input in &inputs {
            subscriptions.push(cx.subscribe(input, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Ok(config) = view.read_random_form(cx) {
                        view.state.settings.shuffle = config;
                        view.save();
                    }
                    cx.notify();
                }
            }));
        }
        for (index, input) in inputs.iter().enumerate().take(3) {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |view, _, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let Some(form) = &view.random_form else {
                        return;
                    };
                    let input = form.inputs[index].clone();
                    let value = input
                        .read(cx)
                        .value()
                        .parse::<i64>()
                        .unwrap_or(if index == 0 { 100 } else { 2000 });
                    let value = (value
                        + if *action == StepAction::Increment {
                            1
                        } else {
                            -1
                        })
                    .clamp(
                        if index == 0 { 1 } else { 1850 },
                        if index == 0 { 500 } else { 2050 },
                    );
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                },
            ));
        }
        let api = self.api();
        let request = uuid::Uuid::new_v4();
        self.random_form = Some(Form {
            inputs,
            genres: Vec::new(),
            request,
            server: api.as_ref().map(|api| api.server.id),
            albums,
            _subscriptions: subscriptions,
        });
        let Some(api) = api else { return };
        let task = cx.background_executor().spawn(async move {
            api.all_items(ListQuery {
                kind: ItemKind::Genre,
                ..Default::default()
            })
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                let Some(form) = view
                    .random_form
                    .as_mut()
                    .filter(|form| form.request == request)
                else {
                    return;
                };
                match result {
                    Ok(genres) => form.genres = genres,
                    Err(error) => {
                        log::debug!("Play random genre choices failed: {error}");
                        view.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn read_random_form(&self, cx: &App) -> anyhow::Result<Config> {
        let form = self
            .random_form
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Open Play random first"))?;
        let mut config = self.state.settings.shuffle.clone();
        let number = |index: usize| -> anyhow::Result<u32> {
            let value = form.inputs[index].read(cx).value();
            if value.trim().is_empty() {
                return Ok(if index == 0 { 500 } else { 0 });
            }
            value
                .trim()
                .parse()
                .map_err(|_| anyhow::anyhow!("Enter whole numbers for count and years"))
        };
        config.limit = number(0)? as usize;
        config.min_year = number(1)?;
        config.max_year = number(2)?;
        config.genre = form.inputs[3].read(cx).value().trim().into();
        config.music_folder = form.inputs[4].read(cx).value().trim().into();
        config.validate()?;
        Ok(config)
    }

    fn enqueue_random(&mut self, mode: Enqueue, shuffle: bool, cx: &mut Context<Self>) {
        if !self.library_fetches.is_empty() {
            return;
        }
        let config = match self.read_random_form(cx) {
            Ok(config) => config,
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
                return;
            }
        };
        if self.api().is_none() {
            self.message = "Sign in to a music server".into();
            cx.notify();
            return;
        }
        self.state.settings.shuffle = config.clone();
        self.save();
        self.enqueue_random_config(config, mode, shuffle, cx);
    }

    pub(crate) fn enqueue_random_config(
        &mut self,
        config: Config,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.library_fetches.is_empty() {
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        let close_form = self.route == Route::Random;
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(request, cancelled.clone());
        log::info!("Preparing {} random {:?} picks", config.limit, config.kind);
        let api = api.with_cancellation(cancelled.clone());
        let task = cx
            .background_executor()
            .spawn(async move { api.random_library_tracks(&config, shuffle) });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                view.library_fetches.remove(&request);
                if !cancelled.load(std::sync::atomic::Ordering::Relaxed)
                    && (!matches!(mode, Enqueue::Now) || request == view.play_request_generation)
                {
                    match result {
                        Ok(tracks) => {
                            view.add_tracks(tracks, mode, cx);
                            if close_form && view.route == Route::Random {
                                view.back(cx);
                            }
                        }
                        Err(error) => {
                            log::debug!("Play random fetch failed: {error}");
                            view.message = error.to_string();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn random_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let api = self.api();
        let server = api.as_ref().map(|api| api.server.id);
        let jellyfin = api.is_some_and(|api| api.server.kind == ServerType::Jellyfin);
        if self
            .random_form
            .as_ref()
            .is_none_or(|form| form.server != server)
        {
            self.init_random_form(window, cx);
        }
        let form = self.random_form.as_ref().unwrap();
        let config = &self.state.settings.shuffle;
        let busy = !self.library_fetches.is_empty();
        let albums = config.kind == Mode::Albums;
        form.albums.set(albums);
        let genres = form.genres.clone();
        let genre_label = genres
            .iter()
            .find(|genre| {
                if jellyfin {
                    genre.id == config.genre
                } else {
                    genre.name == config.genre
                }
            })
            .map(|genre| genre.name.clone())
            .unwrap_or_else(|| {
                if config.genre.is_empty() {
                    self.tr("All genres")
                } else {
                    config.genre.clone()
                }
            });
        let view = cx.weak_entity();
        let mut content = div()
            .native_vertical_scroll()
            .v_flex()
            .gap_4()
            .p_4()
            .max_w(px(540.))
            .child(div().text_xl().child(self.tr("Play random")))
            .child(
                div().h_flex().gap_2().children(
                    [(Mode::Songs, "Songs"), (Mode::Albums, "Albums")]
                        .into_iter()
                        .map(|(kind, label)| {
                            crate::styles::button(
                                if kind == Mode::Songs {
                                    "random-songs"
                                } else {
                                    "random-albums"
                                },
                                cx,
                            )
                            .debug_selector(move || {
                                if kind == Mode::Songs {
                                    "random-songs".into()
                                } else {
                                    "random-albums".into()
                                }
                            })
                            .label(self.tr(label))
                            .selected(config.kind == kind)
                            .disabled(busy)
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.state.settings.shuffle.kind = kind;
                                    view.save();
                                    cx.notify();
                                },
                            ))
                        }),
                ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(self.tr(if albums {
                        "How many albums?"
                    } else {
                        "How many songs?"
                    }))
                    .child(crate::styles::number_input(&form.inputs[0], cx).disabled(busy)),
            );
        for (index, enabled, label) in [
            (1, config.enable_min_year, "From year"),
            (2, config.enable_max_year, "To year"),
        ] {
            content = content.child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(
                        Checkbox::new(if index == 1 {
                            "random-min-year"
                        } else {
                            "random-max-year"
                        })
                        .debug_selector(move || {
                            if index == 1 {
                                "random-min-year".into()
                            } else {
                                "random-max-year".into()
                            }
                        })
                        .label(self.tr(label))
                        .checked(enabled)
                        .disabled(busy || (index == 2 && albums))
                        .on_click(cx.listener(
                            move |view, checked: &bool, _, cx| {
                                if index == 1 {
                                    view.state.settings.shuffle.enable_min_year = *checked;
                                } else {
                                    view.state.settings.shuffle.enable_max_year = *checked;
                                }
                                view.save();
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        crate::styles::number_input(&form.inputs[index], cx)
                            .disabled(busy || (index == 2 && albums)),
                    ),
            );
        }
        content = content
            .child(
                div().v_flex().gap_2().child(self.tr("Genre")).child(
                    crate::styles::button("random-genres", cx)
                        .debug_selector(|| "random-genres".into())
                        .label(genre_label)
                        .disabled(busy)
                        .dropdown_menu(move |menu, _, cx| {
                            let mut menu = menu.item(
                                PopupMenuItem::new(crate::appearance::text("All genres", cx))
                                    .on_click({
                                        let view = view.clone();
                                        move |_, window, cx| {
                                            view.update(cx, |view, cx| {
                                                if let Some(form) = &view.random_form {
                                                    form.inputs[3].update(cx, |input, cx| {
                                                        input.set_value("", window, cx)
                                                    });
                                                }
                                            })
                                            .ok();
                                        }
                                    }),
                            );
                            for genre in &genres {
                                let value = if jellyfin {
                                    genre.id.clone()
                                } else {
                                    genre.name.clone()
                                };
                                let view = view.clone();
                                menu = menu.item(PopupMenuItem::new(genre.name.clone()).on_click(
                                    move |_, window, cx| {
                                        view.update(cx, |view, cx| {
                                            if let Some(form) = &view.random_form {
                                                form.inputs[3].update(cx, |input, cx| {
                                                    input.set_value(value.clone(), window, cx)
                                                });
                                            }
                                        })
                                        .ok();
                                    },
                                ));
                            }
                            menu
                        }),
                ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .child(self.tr("Music library ID (optional)"))
                    .child(crate::styles::input(&form.inputs[4], cx).disabled(busy)),
            );
        if jellyfin && !albums {
            content = content.child(
                div().h_flex().flex_wrap().gap_2().children(
                    [
                        (Played::All, "All tracks"),
                        (Played::Never, "Only unplayed tracks"),
                        (Played::Played, "Only played tracks"),
                    ]
                    .into_iter()
                    .map(|(played, label)| {
                        crate::styles::button(
                            SharedString::from(format!("random-played-{played:?}")),
                            cx,
                        )
                        .debug_selector(move || format!("random-played-{played:?}"))
                        .label(self.tr(label))
                        .selected(config.played == played)
                        .disabled(busy)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.state.settings.shuffle.played = played;
                            view.save();
                            cx.notify();
                        }))
                    }),
                ),
            )
        }
        content
            .child(
                crate::styles::button("random-cancel", cx)
                    .debug_selector(|| "random-cancel".into())
                    .label(self.tr("Cancel"))
                    .on_click(cx.listener(|view, _, _, cx| view.close_random_form(cx))),
            )
            .child(
                div()
                    .h_flex()
                    .gap_3()
                    .items_center()
                    .justify_center()
                    .children(
                        [
                            (Enqueue::Next, "player.addNext", "random-next"),
                            (Enqueue::Now, "player.play", "random-now"),
                            (Enqueue::End, "player.addLast", "random-last"),
                        ]
                        .into_iter()
                        .map(|(mode, label, id)| {
                            crate::styles::play_control(id, mode, self.state.settings.zoom, cx)
                                .disabled(busy || server.is_none())
                                .tooltip(format!(
                                    "{}\n{}",
                                    self.tr(label),
                                    self.tr("player.holdToShuffle")
                                ))
                                .on_long_press(cx.listener(move |view, _, _, cx| {
                                    view.enqueue_random(mode, true, cx)
                                }))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.enqueue_random(mode, false, cx)
                                }))
                        }),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Enqueue, Feishin, Item, Mode, Played, Route, ServerType};
    use feishin_core::{
        api::Api,
        model::{Server, Session},
        storage::State,
    };
    use gpui::{AppContext, Modifiers, MouseButton, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        io::{Read, Write},
        net::TcpListener,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    #[gpui::test]
    fn random_form_mouse_filters_persistence_queue_modes_and_cancellation(cx: &mut TestAppContext) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let worker = {
            let requests = requests.clone();
            let stopping = stopping.clone();
            std::thread::spawn(move || {
                'connections: while !stopping.load(Ordering::Relaxed) {
                    let (mut stream, _) = match listener.accept() {
                        Ok(connection) => connection,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2));
                            continue;
                        }
                        Err(error) => panic!("{error}"),
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut bytes = Vec::new();
                    while !bytes.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                        let mut buffer = [0; 4096];
                        let count = match stream.read(&mut buffer) {
                            Ok(0) | Err(_) => continue 'connections,
                            Ok(count) => count,
                        };
                        if count == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                    let request = String::from_utf8(bytes).unwrap();
                    let payload = if request.contains("MusicGenres?") {
                        serde_json::json!({"Items":[{"Id":"genre-id","Name":"Jazz"}],"TotalRecordCount":1})
                    } else if request.contains("IncludeItemTypes=Playlist") {
                        serde_json::json!({"Items":[],"TotalRecordCount":0})
                    } else if request.contains("IncludeItemTypes=MusicAlbum") {
                        serde_json::json!({"Items":[{"Id":"album-a","Name":"First album","Type":"MusicAlbum"},{"Id":"album-b","Name":"Second album","Type":"MusicAlbum"}],"TotalRecordCount":2})
                    } else {
                        serde_json::json!({"Items":[{"Id":"same","Name":"First duplicate","Type":"Audio"},{"Id":"same","Name":"Second duplicate","Type":"Audio"}],"TotalRecordCount":2})
                    }.to_string();
                    requests.lock().unwrap().push(request);
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(),
                        payload
                    );
                }
            })
        };
        cx.update(|cx| {
            gpui_component::init(cx);
            cx.bind_keys([gpui::KeyBinding::new(
                "escape",
                crate::app::LeaveFullPlayer,
                Some("Feishin && !RecordingShortcut"),
            )]);
        });
        let dir =
            std::env::temp_dir().join(format!("feishin-random-form-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            state.settings.scrobble = false;
            state.settings.lyric_fetch = false;
            state.settings.mpv_path = dir.join("missing-mpv").to_string_lossy().into();
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.simulate_resize(size(px(1400.), px(1000.)));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            url: format!("http://{address}"),
            username: "fixture".into(),
            kind: ServerType::Jellyfin,
            legacy_auth: false,
            options: Default::default(),
        };
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.servers = vec![server.clone()];
                view.state.active_server = Some(server.id);
                view.sessions.insert(
                    server.id,
                    Api::new(
                        server.clone(),
                        Session {
                            token: "fixture-token".into(),
                            user_id: "fixture-user".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                view.route = Route::Queue;
                view.state.queue.add(
                    ["old", "tail"]
                        .into_iter()
                        .map(|id| Item {
                            id: id.into(),
                            name: id.into(),
                            server_id: server.id,
                            ..Default::default()
                        })
                        .collect(),
                    Enqueue::Now,
                );
                view.state.queue.position = 23.;
                cx.notify();
            })
        });
        cx.run_until_parked();
        let click = |cx: &mut VisualTestContext, id: &str| {
            let bounds = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id}"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
        };
        click(cx, "random-library");
        assert!(app.read_with(cx, |view, _| view.route == Route::Random));
        let next = cx.debug_bounds("random-next").unwrap();
        let now = cx.debug_bounds("random-now").unwrap();
        let last = cx.debug_bounds("random-last").unwrap();
        for (bounds, width) in [(next, 32.), (now, 48.), (last, 32.)] {
            assert_eq!(f32::from(bounds.size.width), width);
            assert_eq!(f32::from(bounds.size.height), width);
            assert_eq!(bounds.center().y, now.center().y);
        }
        assert_eq!(f32::from(now.left() - next.right()), 12.);
        assert_eq!(f32::from(last.left() - now.right()), 12.);
        let accessible =
            cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        for label in ["Next", "Play", "Last"] {
            assert!(
                accessible
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == gpui::accesskit::Role::Button
                        && node.label().is_some_and(|value| value.starts_with(label))),
                "{label}"
            );
        }
        click(cx, "random-cancel");
        assert!(app.read_with(cx, |view, _| view.route == Route::Queue));
        click(cx, "random-library");
        cx.update(|window, cx| {
            app.update(cx, |view, _| view.focus.focus(window));
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| view.route == Route::Queue));
        click(cx, "random-library");
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.random_form.as_ref().unwrap().inputs[1]
                    .update(cx, |input, cx| input.focus(window, cx));
            })
        });
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a backspace"
        } else {
            "ctrl-a backspace"
        });
        cx.run_until_parked();
        app.read_with(cx, |view, cx| {
            assert_eq!(
                view.random_form.as_ref().unwrap().inputs[1]
                    .read(cx)
                    .value()
                    .as_ref(),
                ""
            );
            assert_eq!(view.state.settings.shuffle.min_year, 0);
            assert!(!view.state.settings.shuffle.enable_min_year);
        });
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.random_form.as_ref().unwrap().inputs[0]
                    .update(cx, |input, cx| input.set_value("2", window, cx));
                view.random_form.as_ref().unwrap().inputs[1]
                    .update(cx, |input, cx| input.set_value("2000", window, cx));
            })
        });
        click(cx, "random-min-year");
        click(cx, "random-max-year");
        click(cx, "random-played-Never");
        click(cx, "random-genres");
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.settings.shuffle.genre, "genre-id");
            assert_eq!(view.state.settings.shuffle.limit, 2);
        });
        let current = app.read_with(cx, |view, _| view.state.queue.current);
        click(cx, "random-next");
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Queue);
            assert_eq!(view.state.queue.current, current);
            assert_eq!(view.state.queue.position, 23.);
            assert_eq!(
                view.state
                    .queue
                    .ordered()
                    .iter()
                    .map(|entry| entry.item.id.as_str())
                    .collect::<Vec<_>>(),
                ["old", "same", "same", "tail"]
            );
        });
        let records = requests.lock().unwrap().clone();
        assert!(
            records
                .iter()
                .any(|request| request.contains("SortBy=Random")
                    && request.contains("GenreIds=genre-id")
                    && request.contains("IsPlayed=false")
                    && request.contains("Years=2000%2C2001")
                    && request.contains("Limit=2"))
        );
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(saved.settings.shuffle.genre, "genre-id");
        assert_eq!(saved.settings.shuffle.played, Played::Never);

        click(cx, "random-library");
        click(cx, "random-albums");
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.settings.shuffle.kind, Mode::Albums);
            assert_eq!(view.state.settings.shuffle.max(), None);
        });
        let position = cx.debug_bounds("random-last").unwrap().center();
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.entries.len(), 8);
            assert_eq!(view.state.queue.current, current);
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<std::collections::HashSet<_>>()
                    .len(),
                8
            );
        });
        let records = requests.lock().unwrap().clone();
        assert!(
            records
                .iter()
                .any(|request| request.contains("ParentId=album-a"))
        );
        assert!(
            records
                .iter()
                .any(|request| request.contains("ParentId=album-b"))
        );
        assert!(
            records
                .iter()
                .filter(|request| request.contains("IncludeItemTypes=MusicAlbum"))
                .all(|request| !request.contains("Years=") && !request.contains("IsPlayed="))
        );

        click(cx, "random-library");
        click(cx, "random-songs");
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.random_form.as_ref().unwrap().inputs[0]
                    .update(cx, |input, cx| input.set_value("501", window, cx))
            })
        });
        click(cx, "random-now");
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            8
        );
        assert!(app.read_with(cx, |view, _| view.message.contains("1 to 500")));
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.random_form.as_ref().unwrap().inputs[0]
                    .update(cx, |input, cx| input.set_value("2", window, cx));
                view.enqueue_random(Enqueue::Now, false, cx);
                view.close_random_form(cx);
            })
        });
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            8
        );
        assert!(app.read_with(cx, |view, _| view.route == Route::Queue));
        click(cx, "random-library");
        click(cx, "random-now");
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.entries.len(), 2);
            assert_ne!(view.state.queue.current, current);
        });

        // Home genre playback uses its own fixed request and preserves this form's options.
        let saved_options = feishin_core::shuffle::Config {
            kind: Mode::Albums,
            limit: 7,
            genre: "saved-genre".into(),
            music_folder: "saved-folder".into(),
            played: Played::Never,
            enable_min_year: true,
            enable_max_year: true,
            ..Default::default()
        };
        let wait_for_genres = |cx: &mut VisualTestContext| {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                cx.run_until_parked();
                if app.read_with(cx, |view, _| {
                    view.home_sections
                        .get(&feishin_core::home::Kind::Genres)
                        .is_some_and(|section| !section.loading && !section.items.is_empty())
                }) {
                    cx.refresh().unwrap();
                    cx.run_until_parked();
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "Home genre request did not complete"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        };
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.shuffle = saved_options.clone();
                view.state.settings.play_button_behavior = feishin_core::queue::PlayBehavior::Last;
                view.state.settings.home.items = vec![feishin_core::home::Section {
                    id: feishin_core::home::Kind::Genres,
                    disabled: false,
                }];
                view.navigate(Route::Home, cx);
            });
        });
        cx.run_until_parked();
        wait_for_genres(cx);
        let tile = cx.debug_bounds("home-genre-0").unwrap();
        cx.simulate_mouse_move(tile.center(), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let before = requests.lock().unwrap().len();
        let position = cx.debug_bounds("home-genre-play-0").unwrap().center();
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.run_until_parked();
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            requests.lock().unwrap().len(),
            before,
            "A long hold suppresses genre Play"
        );
        assert!(app.read_with(cx, |view, _| view.route == Route::Home));
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.library_fetches
                    .insert(u64::MAX, Arc::new(AtomicBool::new(false)));
                cx.notify();
            });
        });
        cx.run_until_parked();
        click(cx, "home-genre-play-0");
        assert_eq!(
            requests.lock().unwrap().len(),
            before,
            "Pending work disables genre Play"
        );
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.library_fetches.clear();
                cx.notify();
            });
        });
        cx.run_until_parked();
        let previous = app.read_with(cx, |view, _| view.state.queue.current);
        click(cx, "home-genre-play-0");
        let records = requests.lock().unwrap().clone();
        let random = records[before..]
            .iter()
            .find(|request| request.contains("SortBy=Random"))
            .unwrap();
        for expected in ["GenreIds=genre-id", "Limit=100", "IncludeItemTypes=Audio"] {
            assert!(random.contains(expected), "{random}");
        }
        for absent in ["IsPlayed=", "Years=", "saved-folder", "saved-genre"] {
            assert!(!random.contains(absent), "{random}");
        }
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert_eq!(view.state.queue.entries.len(), 2);
            assert_ne!(
                view.state.queue.current, previous,
                "Genre Play replaces the queue despite the saved Last behavior"
            );
            assert_eq!(view.state.settings.shuffle, saved_options);
        });
        assert_eq!(
            feishin_core::storage::load(&dir.join("state.json"))
                .unwrap()
                .settings
                .shuffle,
            saved_options
        );
        let previous = app.read_with(cx, |view, _| view.state.queue.current);
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.enqueue_random_config(
                    feishin_core::shuffle::Config {
                        genre: "genre-id".into(),
                        ..Default::default()
                    },
                    Enqueue::Now,
                    false,
                    cx,
                );
                view.cancel_library_fetches(cx);
            });
        });
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.current),
            previous
        );
        click(cx, "home-genres-more");
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::Library(feishin_core::model::ItemKind::Genre)
        );
        cx.update(|_, cx| app.update(cx, |view, cx| view.navigate(Route::Home, cx)));
        wait_for_genres(cx);
        let tile = cx.debug_bounds("home-genre-0").unwrap();
        cx.simulate_click(
            gpui::point(tile.left() + px(24.), tile.center().y),
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| matches!(&view.route, Route::Detail(item) if item.id == "genre-id" && item.kind == feishin_core::model::ItemKind::Genre)));
        stopping.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).ok();
    }
}
