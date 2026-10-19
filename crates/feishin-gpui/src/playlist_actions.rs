use crate::app::{Feishin, Route};
use feishin_core::model::{Item, ItemKind};
use gpui::{Context, PromptLevel, Window, prelude::*};
use std::collections::HashSet;

pub struct TrackEdit {
    pub id: uuid::Uuid,
    pub playlist: Item,
    pub original: Option<Vec<Item>>,
    pub filter: String,
    pub saving: bool,
}

impl Feishin {
    pub(crate) fn playlist_tracks_editing(&self) -> bool {
        self.playlist_tracks_edit.as_ref().is_some_and(|edit|
            matches!(&self.route, Route::Detail(item) if item.id == edit.playlist.id && item.server_id == edit.playlist.server_id && item.kind == ItemKind::Playlist))
    }

    pub(crate) fn discard_playlist_tracks_edit(&mut self) {
        if let Some(edit) = self.playlist_tracks_edit.take() {
            self.local_filter = edit.filter;
            if let Some(original) = edit.original {
                self.items = original;
            }
            self.selected.clear();
            self.grid_selection_order.clear();
            self.loading = false;
        }
    }

    pub(crate) fn start_playlist_tracks_edit(&mut self, cx: &mut Context<Self>) {
        let Route::Detail(playlist) = self.route.clone() else {
            return;
        };
        if !self.can_edit_playlist(&playlist)
            || playlist.rules.is_some()
            || self.playlist_tracks_edit.is_some()
        {
            return;
        }
        let Some(api) = self.sessions.get(&playlist.server_id).cloned() else {
            return;
        };
        let id = uuid::Uuid::new_v4();
        self.playlist_tracks_edit = Some(TrackEdit {
            id,
            playlist: *playlist.clone(),
            original: None,
            filter: std::mem::take(&mut self.local_filter),
            saving: false,
        });
        self.request_generation = self.request_generation.wrapping_add(1);
        self.search_debounce = None;
        self.loading = true;
        self.selected.clear();
        self.grid_selection_order.clear();
        self.items.clear();
        let task = cx
            .background_executor()
            .spawn(async move { api.all_tracks(&playlist) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |view, cx| {
                let Some(edit) = view
                    .playlist_tracks_edit
                    .as_mut()
                    .filter(|edit| edit.id == id)
                else {
                    return;
                };
                view.loading = false;
                match result {
                    Ok(songs) => {
                        edit.original = Some(songs.clone());
                        view.total = Some(songs.len());
                        view.has_more = false;
                        view.items = songs;
                        view.library_page = 0;
                        view.library_scroll
                            .scroll_to_item_strict(0, gpui::ScrollStrategy::Top);
                    }
                    Err(error) => {
                        log::warn!("Could not load playlist for editing: {error}");
                        view.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn edit_playlist_draft_selection(
        &mut self,
        action: feishin_core::api::PlaylistSelectionAction,
        cx: &mut Context<Self>,
    ) {
        if self.playlist_tracks_edit.as_ref().is_none_or(|edit| {
            edit.saving || edit.original.is_none() || !self.can_edit_playlist(&edit.playlist)
        }) {
            return;
        }
        let selected = self
            .selected
            .iter()
            .copied()
            .filter(|index| *index < self.items.len())
            .collect::<HashSet<_>>();
        if selected.is_empty() {
            return;
        }
        let before = match action {
            feishin_core::api::PlaylistSelectionAction::MoveBefore(before) => {
                Some(before.unwrap_or(self.items.len()).min(self.items.len()))
            }
            _ => None,
        };
        let insertion =
            before.map(|before| before - selected.iter().filter(|index| **index < before).count());
        let (mut kept, mut moved) = (Vec::new(), Vec::new());
        for (index, item) in self.items.drain(..).enumerate() {
            if selected.contains(&index) {
                moved.push(item)
            } else {
                kept.push(item)
            }
        }
        self.selected.clear();
        if let Some(insertion) = insertion {
            self.selected.extend(insertion..insertion + moved.len());
            kept.splice(insertion..insertion, moved);
        }
        for (index, item) in kept.iter_mut().enumerate() {
            item.playlist_position = Some(index);
        }
        self.items = kept;
        self.total = Some(self.items.len());
        self.grid_selection_order = self.selected.iter().copied().collect();
        self.grid_selection_order.sort_unstable();
        cx.notify();
    }

    pub(crate) fn save_playlist_tracks_edit(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.playlist_tracks_edit.as_ref() else {
            return;
        };
        if edit.saving || !self.can_edit_playlist(&edit.playlist) {
            return;
        }
        let Some(original) = edit.original.clone() else {
            return;
        };
        let Some(api) = self.sessions.get(&edit.playlist.server_id).cloned() else {
            return;
        };
        let (id, playlist, songs) = (edit.id, edit.playlist.clone(), self.items.clone());
        self.playlist_tracks_edit.as_mut().unwrap().saving = true;
        let task = cx.background_executor().spawn(async move {
            let fresh = api.all_tracks(&playlist)?;
            anyhow::ensure!(
                fresh.len() == original.len()
                    && fresh
                        .iter()
                        .zip(&original)
                        .all(|(a, b)| a.id == b.id && a.playlist_entry_id == b.playlist_entry_id),
                "Playlist changed on the server; cancel and reload before saving"
            );
            api.replace_playlist_tracks_checked(&playlist, &songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |view, cx| {
                let Some(edit) = view
                    .playlist_tracks_edit
                    .as_mut()
                    .filter(|edit| edit.id == id)
                else {
                    return;
                };
                edit.saving = false;
                match result {
                    Ok(()) => {
                        view.discard_playlist_tracks_edit();
                        view.refresh_playlists(cx);
                        view.reload(false, cx);
                        view.message = view.tr("Playlist updated");
                    }
                    Err(error) => {
                        log::warn!("Could not save playlist draft: {error}");
                        view.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn playlist_edit_toolbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        use gpui_component::{Disableable, StyledExt};
        let edit = self.playlist_tracks_edit.as_ref().unwrap();
        let disabled = edit.original.is_none() || edit.saving;
        gpui::div()
            .h_flex()
            .flex_wrap()
            .gap_2()
            .child(
                crate::styles::button("save-playlist-tracks", cx)
                    .debug_selector(|| "save-playlist-tracks".into())
                    .label(self.tr("common.saveAndReplace"))
                    .disabled(disabled)
                    .on_click(cx.listener(|view, _, _, cx| view.save_playlist_tracks_edit(cx))),
            )
            .child(
                crate::styles::button("save-playlist-tracks-as", cx)
                    .debug_selector(|| "save-playlist-tracks-as".into())
                    .label(self.tr("Save as playlist"))
                    .disabled(disabled)
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.edit_playlist(None, view.items.clone(), window, cx)
                    })),
            )
            .child(
                crate::styles::button("remove-playlist-draft-selected", cx)
                    .debug_selector(|| "remove-playlist-draft-selected".into())
                    .label(self.tr("Remove selected"))
                    .disabled(disabled || self.selected.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.edit_playlist_draft_selection(
                            feishin_core::api::PlaylistSelectionAction::Remove,
                            cx,
                        )
                    })),
            )
            .child(
                crate::styles::button("cancel-playlist-tracks", cx)
                    .debug_selector(|| "cancel-playlist-tracks".into())
                    .label(self.tr("common.cancel").to_uppercase())
                    .disabled(edit.saving)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.discard_playlist_tracks_edit();
                        view.reload(false, cx);
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn playlist_reorder_target(
        &self,
        index: usize,
        down: bool,
        edge: bool,
    ) -> Option<usize> {
        let selected = if self.selected.contains(&index) {
            self.selected
                .iter()
                .copied()
                .filter(|index| *index < self.items.len())
                .collect::<Vec<_>>()
        } else if index < self.items.len() {
            vec![index]
        } else {
            return None;
        };
        let first = *selected.iter().min()?;
        let last = *selected.iter().max()?;
        if down && last + 1 < self.items.len() {
            Some(if edge { self.items.len() } else { last + 2 })
        } else if !down && first > 0 {
            Some(if edge { 0 } else { first - 1 })
        } else {
            None
        }
    }

    pub(crate) fn reorder_playlist_row(
        &mut self,
        song: &Item,
        index: usize,
        down: bool,
        edge: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(&self.route, Route::Detail(playlist) if playlist.kind == ItemKind::Playlist && self.can_edit_playlist(playlist))
            || self.items.get(index).is_none_or(|item| {
                item.id != song.id || item.playlist_entry_id != song.playlist_entry_id
            })
        {
            return;
        }
        if !self.selected.contains(&index) {
            self.selected = [index].into_iter().collect();
            cx.notify();
        }
        let Some(before) = self.playlist_reorder_target(index, down, edge) else {
            return;
        };
        let position = self.playlist_boundary_position(before);
        self.playlist_selection_action(
            feishin_core::api::PlaylistSelectionAction::MoveBefore(Some(position)),
            cx,
        );
    }

    pub(crate) fn playlist_reorder_controls(
        &self,
        item: &Item,
        index: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        use gpui_component::{Disableable, IconName, Sizable, StyledExt, button::ButtonVariants};
        let editable = matches!(&self.route, Route::Detail(playlist) if playlist.kind == ItemKind::Playlist && self.can_edit_playlist(playlist));
        let zoom = self.state.settings.zoom;
        let dragged = self.drag_library_item(index);
        gpui::div()
            .h_flex()
            .justify_center()
            .w_full()
            .gap_1()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .children([false, true].map(|down| {
                let song = item.clone();
                let held = song.clone();
                crate::styles::button(
                    (
                        if down {
                            "playlist-reorder-down"
                        } else {
                            "playlist-reorder-up"
                        },
                        index,
                    ),
                    cx,
                )
                .debug_selector(move || {
                    format!(
                        "playlist-reorder-{}-{index}",
                        if down { "down" } else { "up" }
                    )
                })
                .small()
                .ghost()
                .size(gpui::px(20. * zoom))
                .px_0()
                .flex_shrink_0()
                .icon(if down {
                    IconName::ArrowDown
                } else {
                    IconName::ArrowUp
                })
                .tooltip(format!(
                    "{}\n{}",
                    self.tr(if down {
                        "action.moveDown"
                    } else {
                        "action.moveUp"
                    }),
                    self.tr(if down {
                        "action.holdToMoveToBottom"
                    } else {
                        "action.holdToMoveToTop"
                    })
                ))
                .disabled(!editable)
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.reorder_playlist_row(&song, index, down, false, cx);
                    cx.stop_propagation();
                }))
                .on_long_press(cx.listener(move |view, _, _, cx| {
                    view.reorder_playlist_row(&held, index, down, true, cx);
                    cx.stop_propagation();
                }))
            }))
            .child(
                gpui::div()
                    .id(("playlist-reorder-drag", index))
                    .debug_selector(move || format!("playlist-reorder-drag-{index}"))
                    .size(gpui::px(20. * zoom))
                    .h_flex()
                    .justify_center()
                    .child("⠿")
                    .when(editable, |div| {
                        div.cursor_grab()
                            .on_drag(dragged, crate::context_preview::drag_constructor(cx))
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn can_edit_playlist(&self, item: &Item) -> bool {
        self.sessions
            .get(&item.server_id)
            .is_some_and(|api| item.can_edit_playlist(&api.session()))
    }

    pub(crate) fn delete_playlists(
        &mut self,
        mut items: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut seen = HashSet::new();
        items.retain(|item| seen.insert((item.server_id, item.id.clone())));
        if items.is_empty() || items.iter().any(|item| !self.can_edit_playlist(item)) {
            return;
        }
        let confirmation = window.prompt(
            PromptLevel::Warning,
            &self.tr("form.deletePlaylist.title"),
            Some(&self.tr("common.areYouSure")),
            &[self.tr("Cancel").as_str(), self.tr("Delete").as_str()],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if confirmation.await != Ok(1) {
                return;
            }
            this.update(cx, |this, cx| {
                // Recheck after confirmation in case the account changed meanwhile.
                if items.iter().any(|item| !this.can_edit_playlist(item)) {
                    return;
                }
                let sessions = this.sessions.clone();
                let route = this.route.clone();
                let task = cx.background_executor().spawn(async move {
                    let mut deleted = HashSet::new();
                    let mut errors = vec![];
                    for item in items {
                        match sessions[&item.server_id].delete_playlist(&item.id) {
                            Ok(()) => {
                                deleted.insert((item.server_id, item.id));
                            }
                            Err(error) => {
                                log::warn!("Could not delete playlist: {error}");
                                errors.push(error.to_string());
                            }
                        }
                    }
                    (deleted, errors)
                });
                cx.spawn(async move |this, cx| {
                    let (deleted, errors) = task.await;
                    this.update(cx, |this, cx| {
                        this.playlist_options
                            .retain(|item| !deleted.contains(&(item.server_id, item.id.clone())));
                        if this.route == route && !deleted.is_empty() {
                            this.navigate(Route::Library(ItemKind::Playlist), cx);
                        }
                        this.refresh_playlists(cx);
                        this.message = if errors.is_empty() {
                            this.tr("action.deletePlaylist")
                        } else {
                            format!(
                                "Deleted {} playlists; {} failed.\n{}",
                                deleted.len(),
                                errors.len(),
                                errors.join("\n")
                            )
                        };
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feishin_core::{
        api::Api,
        model::{Server, ServerType, Session},
        storage::State,
    };
    use gpui::{
        Entity, Modifiers, Render, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase,
        VisualTestContext, div, point,
    };
    use gpui_component::{Root, menu::DropdownMenu};
    use std::{
        cell::RefCell,
        io::{Read, Write},
        net::TcpListener,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, Instant},
    };

    #[gpui::test]
    fn playlist_drafts_match_original_moves_and_save_cancel_failure_boundaries(
        cx: &mut TestAppContext,
    ) {
        use gpui::{MouseButton, px};
        use gpui_component::StyledExt;
        use std::sync::atomic::AtomicU8;
        struct Probe {
            view: Entity<Feishin>,
            _subscription: gpui::Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| {
                    div()
                        .v_flex()
                        .w(px(1000.))
                        .when(view.playlist_tracks_editing(), |div| {
                            div.child(view.playlist_edit_toolbar(cx))
                        })
                        .children(
                            view.items
                                .iter()
                                .enumerate()
                                .map(|(index, item)| view.row(item, index, cx)),
                        )
                })
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let mode = Arc::new(AtomicU8::new(0));
        let response_mode = mode.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(90);
            while !stopped.load(Ordering::Relaxed) && Instant::now() < deadline {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut data = [0; 16384];
                let n = stream.read(&mut data).unwrap();
                let line = String::from_utf8_lossy(&data[..n])
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned();
                received.lock().unwrap().push(line.clone());
                let mut status = "200 OK";
                let body = if line.contains("/api/playlist/list/tracks?")
                    && response_mode.load(Ordering::Relaxed) == 4
                {
                    status = "500 Internal Server Error";
                    serde_json::json!({"error":"fixture load failure"})
                } else if line.contains("/api/playlist/list/tracks?") {
                    serde_json::Value::Array((0..6).map(|index| serde_json::json!({"id": if index == 5 && response_mode.load(Ordering::Relaxed) == 1 { "changed".to_owned() } else {index.to_string()}, "mediaFileId": if index==2||index==4 {"duplicate".to_owned()} else {format!("song-{index}")}, "title":format!("Track {index}")})).collect())
                } else if line.contains("/rest/createPlaylist.view?") {
                    if response_mode.load(Ordering::Relaxed) == 2 {
                        status = "500 Internal Server Error";
                        serde_json::json!({"error":"fixture save failure"})
                    } else {
                        serde_json::json!({"subsonic-response":{"status":"ok","playlist":{"id":"list","name":"Playlist"}}})
                    }
                } else if line.contains("/api/playlist/list ") {
                    serde_json::json!({"id":"list","name":"Playlist","ownerId":"me"})
                } else {
                    serde_json::json!([])
                };
                let body = body.to_string();
                write!(stream,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nX-Total-Count: 6\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        let api = Api::new(
            Server {
                id: uuid::Uuid::new_v4(),
                name: "Fixture".into(),
                url: format!("http://{address}"),
                username: "me".into(),
                kind: ServerType::Navidrome,
                legacy_auth: false,
                options: Default::default(),
            },
            Session {
                user_id: "me".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let playlist = Item {
            id: "list".into(),
            name: "Playlist".into(),
            kind: ItemKind::Playlist,
            server_id: api.server.id,
            ..Default::default()
        };
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-playlist-draft-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.mpv_path = dir.join("missing-mpv").to_string_lossy().into();
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            view.update(cx, |view, _| {
                view.sessions.insert(api.server.id, api.clone());
                view.route = Route::Detail(Box::new(playlist.clone()));
                view.local_filter = "before editing".into();
            });
            captured.replace(Some(view.clone()));
            let probe = cx.new(|cx| Probe {
                view: view.clone(),
                _subscription: cx.observe(&view, |_, _, cx| cx.notify()),
            });
            Root::new(probe, window, cx)
        });
        let view = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(px(1200.), px(900.)));
        cx.update(|_, cx| view.update(cx, |view, cx| view.start_playlist_tracks_edit(cx)));
        cx.run_until_parked();
        let original = view.read_with(cx, |view, _| {
            view.playlist_tracks_edit
                .as_ref()
                .unwrap()
                .original
                .clone()
                .unwrap()
        });
        assert_eq!(original.len(), 6);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.state.settings.playlist_albums = true;
                assert_eq!(view.list_key(), "playlistSong");
                assert_eq!(view.list_config().table.columns[0].id, "playlistReorder");
                assert!(view.state.settings.playlist_albums);
                view.full_player = true;
                view.state.settings.player_display.active_tab = "queue".into();
                assert_eq!(view.list_key(), "fullScreenSong");
                assert!(
                    !view
                        .list_config()
                        .table
                        .columns
                        .iter()
                        .any(|column| column.id == "playlistReorder" && column.is_enabled)
                );
                view.full_player = false;
                view.state.settings.playlist_albums = false;
                let mut stale = original[0].clone();
                stale.playlist_entry_id = "stale".into();
                view.reorder_playlist_row(&stale, 0, true, false, cx);
                assert_eq!(view.items[0].playlist_entry_id, "0");
            })
        });
        let queue = view.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/playlist-reorder-reference.json"))
                .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            cx.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.items = original.clone();
                    view.selected = serde_json::from_value(case["selected"].clone()).unwrap();
                    let index = case["index"].as_u64().unwrap() as usize;
                    let down = case["down"].as_bool().unwrap();
                    let edge = case["edge"].as_bool().unwrap();
                    assert_eq!(
                        view.playlist_reorder_target(index, down, edge),
                        case["before"].as_u64().map(|n| n as usize),
                        "{case}"
                    );
                    view.reorder_playlist_row(&original[index], index, down, edge, cx);
                    let order = view
                        .items
                        .iter()
                        .map(|song| song.playlist_entry_id.parse::<usize>().unwrap())
                        .collect::<Vec<_>>();
                    assert_eq!(
                        serde_json::to_value(order).unwrap(),
                        case["order"],
                        "{case}"
                    );
                    let mut selected = view
                        .selected
                        .iter()
                        .map(|index| {
                            view.items[*index]
                                .playlist_entry_id
                                .parse::<usize>()
                                .unwrap()
                        })
                        .collect::<Vec<_>>();
                    selected.sort_unstable();
                    assert_eq!(
                        serde_json::to_value(selected).unwrap(),
                        case["selection"],
                        "{case}"
                    );
                })
            });
        }
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .all(|request| request.starts_with("GET ") && !request.contains("createPlaylist")),
            "Draft moves must not write to the server"
        );
        let reset = |cx: &mut VisualTestContext| {
            cx.update(|_, cx| {
                view.update(cx, |view, cx| {
                    view.items = original.clone();
                    view.selected.clear();
                    cx.notify();
                })
            });
            cx.run_until_parked();
        };
        let order = |cx: &mut VisualTestContext| {
            view.read_with(cx, |view, _| {
                view.items
                    .iter()
                    .map(|song| song.playlist_entry_id.clone())
                    .collect::<Vec<_>>()
            })
        };
        let click = |selector: &str, cx: &mut VisualTestContext| {
            let point = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(point, Modifiers::none());
            cx.run_until_parked();
        };
        reset(cx);
        click("playlist-reorder-up-2", cx);
        assert_eq!(order(cx), ["0", "2", "1", "3", "4", "5"]);
        let button = cx.debug_bounds("playlist-reorder-down-1").unwrap().center();
        cx.simulate_mouse_down(button, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.run_until_parked();
        cx.simulate_mouse_up(button, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(order(cx), ["0", "1", "3", "4", "5", "2"]);
        let drag = |from: usize, to: usize, after: bool, cx: &mut VisualTestContext| {
            let start = cx
                .debug_bounds(&format!("playlist-reorder-drag-{from}"))
                .unwrap()
                .center();
            let row = cx.debug_bounds(&format!("library-row-{to}")).unwrap();
            let target = gpui::point(
                row.center().x,
                if after {
                    row.bottom() - px(2.)
                } else {
                    row.origin.y + px(2.)
                },
            );
            cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                start + gpui::point(px(10.), px(0.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.run_until_parked();
            cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
        };
        drag(5, 1, false, cx);
        assert_eq!(order(cx), ["0", "2", "1", "3", "4", "5"]);
        drag(1, 5, true, cx);
        assert_eq!(order(cx), ["0", "1", "3", "4", "5", "2"]);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                let drag = view.drag_library_item(5);
                view.drop_playlist_tracks(&drag, Some(1), cx);
            })
        });
        cx.run_until_parked();
        assert_eq!(order(cx), ["0", "2", "1", "3", "4", "5"]);
        let writes = || {
            requests
                .lock()
                .unwrap()
                .iter()
                .filter(|line| line.contains("createPlaylist"))
                .count()
        };
        assert_eq!(writes(), 0);
        mode.store(1, Ordering::Relaxed);
        click("save-playlist-tracks", cx);
        view.read_with(cx, |view, _| {
            assert!(view.message.contains("changed on the server"));
            assert!(view.playlist_tracks_edit.is_some());
        });
        assert_eq!(writes(), 0);
        mode.store(2, Ordering::Relaxed);
        click("save-playlist-tracks", cx);
        view.read_with(cx, |view, _| {
            assert!(!view.playlist_tracks_edit.as_ref().unwrap().saving)
        });
        assert_eq!(writes(), 1);
        assert_eq!(order(cx), ["0", "2", "1", "3", "4", "5"]);
        mode.store(0, Ordering::Relaxed);
        click("save-playlist-tracks", cx);
        view.read_with(cx, |view, _| {
            assert!(view.playlist_tracks_edit.is_none());
            assert_eq!(view.local_filter, "before editing");
            assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
        });
        assert_eq!(writes(), 2);
        let saved = requests
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|line| line.contains("createPlaylist"))
            .cloned()
            .unwrap();
        let ids = saved
            .split('&')
            .filter_map(|part| part.strip_prefix("songId="))
            .map(|value| value.split(' ').next().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            ids,
            [
                "song-0",
                "duplicate",
                "song-1",
                "song-3",
                "duplicate",
                "song-5"
            ]
        );
        cx.update(|_, cx| view.update(cx, |view, cx| view.start_playlist_tracks_edit(cx)));
        cx.run_until_parked();
        reset(cx);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.selected = [2, 4].into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        click("remove-playlist-draft-selected", cx);
        assert_eq!(order(cx), ["0", "1", "3", "5"]);
        click("cancel-playlist-tracks", cx);
        assert_eq!(writes(), 2);
        view.read_with(cx, |view, _| assert!(view.playlist_tracks_edit.is_none()));
        cx.update(|_, cx| view.update(cx, |view, cx| view.start_playlist_tracks_edit(cx)));
        cx.run_until_parked();
        reset(cx);
        click("save-playlist-tracks-as", cx);
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::PlaylistEditor);
            assert_eq!(view.form_songs.len(), 6);
            assert!(view.form_target.is_none());
            assert!(view.playlist_tracks_edit.is_none());
        });
        assert_eq!(writes(), 2);
        mode.store(4, Ordering::Relaxed);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.route = Route::Detail(Box::new(playlist.clone()));
                view.start_playlist_tracks_edit(cx);
            })
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(!view.loading);
            assert!(
                view.playlist_tracks_edit
                    .as_ref()
                    .unwrap()
                    .original
                    .is_none()
            );
        });
        click("cancel-playlist-tracks", cx);
        view.read_with(cx, |view, _| assert!(view.playlist_tracks_edit.is_none()));
        mode.store(0, Ordering::Relaxed);
        cx.update(|_, cx| {
            view.update(cx, |view, cx| {
                view.start_playlist_tracks_edit(cx);
                view.navigate(Route::Home, cx);
            })
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Home);
            assert!(view.playlist_tracks_edit.is_none());
        });
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn playlist_context_actions_enforce_ownership_confirm_bulk_delete_and_refresh_partial_failures(
        cx: &mut TestAppContext,
    ) {
        struct Probe {
            view: Entity<Feishin>,
            _subscription: gpui::Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| {
                    let item = view.items.first().cloned().unwrap_or_default();
                    div()
                        .child(
                            crate::styles::button("playlist-action-probe", cx)
                                .debug_selector(|| "playlist-action-probe".into())
                                .label("More")
                                .dropdown_menu(view.library_menu_builder(&item, 0, cx)),
                        )
                        .into_any_element()
                })
            }
        }
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let received = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !stopped.load(Ordering::Relaxed) {
                assert!(Instant::now() < deadline, "Playlist fixture timed out");
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut data = [0; 16384];
                let n = stream.read(&mut data).unwrap();
                let request = String::from_utf8_lossy(&data[..n]);
                let line = request.lines().next().unwrap().to_owned();
                received.lock().unwrap().push(line.clone());
                let (status, body) = if line.contains("id=failed") {
                    (
                        "200 OK",
                        r#"{"subsonic-response":{"status":"failed","error":{"code":50,"message":"Fixture deletion failed"}}}"#,
                    )
                } else if line.starts_with("DELETE ") {
                    ("204 No Content", "")
                } else if line.contains("/rest/deletePlaylist.view?") {
                    ("200 OK", r#"{"subsonic-response":{"status":"ok"}}"#)
                } else {
                    panic!("Unexpected playlist request: {line}");
                };
                write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-playlist-actions-{}", uuid::Uuid::new_v4()));
        let mut apis = vec![];
        let mut items = vec![];
        for kind in ServerType::ALL {
            let server = uuid::Uuid::new_v4();
            apis.push(
                Api::new(
                    Server {
                        id: server,
                        name: "Fixture".into(),
                        url: format!("http://{address}"),
                        username: "me".into(),
                        kind,
                        legacy_auth: false,
                        options: Default::default(),
                    },
                    Session {
                        user_id: "me".into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            );
            items.push(Item {
                id: format!("playlist-{}", kind.label()),
                server_id: server,
                kind: ItemKind::Playlist,
                name: format!("{} playlist", kind.label()),
                owner_id: "me".into(),
                ..Default::default()
            });
        }
        // Keep the single-item menu independent of Navidrome's sharing entry.
        let own = items[1].clone();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| {
                for api in apis {
                    view.sessions.insert(api.server.id, api);
                }
                view.items = vec![own.clone()];
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|cx| Probe {
                view: app.clone(),
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
            });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            for item in &items {
                let mut radio = item.clone();
                radio.kind = ItemKind::Radio;
                let api = &view.sessions[&radio.server_id];
                assert_eq!(
                    view.can_edit_radio(&radio),
                    api.server.kind == ServerType::Jellyfin
                );
            }
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.items = vec![own.clone()];
                cx.notify();
            })
        });
        cx.run_until_parked();
        let click = |selector, cx: &mut VisualTestContext| {
            if selector != "playlist-action-probe" {
                let position = cx.debug_bounds("popup-menu-item-0").unwrap().center();
                cx.simulate_event(ScrollWheelEvent {
                    position,
                    delta: ScrollDelta::Lines(point(0., -100.)),
                    touch_phase: TouchPhase::Moved,
                    modifiers: Modifiers::none(),
                });
                cx.run_until_parked();
            }
            let point = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_click(point, Modifiers::none());
            cx.run_until_parked();
        };
        click("playlist-action-probe", cx);
        click("context-edit-playlist", cx);
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::PlaylistEditor
        );
        assert_eq!(
            app.read_with(cx, |view, _| view.form_target.clone()),
            Some(own.clone())
        );
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.items = vec![own.clone()];
                view.items[0].public = true;
                view.items[0].owner_id = "another-user".into();
                view.edit_playlist(Some(view.items[0].clone()), vec![], window, cx);
                cx.notify();
            })
        });
        click("playlist-action-probe", cx);
        click("context-delete-playlist", cx);
        assert!(!cx.has_pending_prompt());
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::Settings
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let mut failed = own.clone();
        failed.id = "failed".into();
        items.push(failed);
        items.push(own.clone());
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.items = items.clone();
                view.selected.extend(0..items.len());
                view.playlist_options = items.clone();
                view.items[2].public = true;
                view.items[2].owner_id = "another-user".into();
                cx.notify();
            })
        });
        click("playlist-action-probe", cx);
        click("context-delete-playlist", cx);
        assert!(
            !cx.has_pending_prompt(),
            "A shared public selection disables the bulk action"
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.items = items.clone();
                cx.notify();
            })
        });
        click("playlist-action-probe", cx);
        click("context-delete-playlist", cx);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert!(requests.lock().unwrap().is_empty());
        click("playlist-action-probe", cx);
        click("context-delete-playlist", cx);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Delete");
        cx.run_until_parked();
        let lines = requests.lock().unwrap().clone();
        assert_eq!(
            lines.len(),
            4,
            "Every unique playlist must be attempted: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("DELETE /api/playlist/playlist-Navidrome "))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("DELETE /Items/playlist-Jellyfin "))
        );
        assert!(
            lines
                .iter()
                .any(|line| line.contains("/rest/deletePlaylist.view?")
                    && line.contains("id=playlist-Subsonic"))
        );
        assert!(lines.iter().any(|line| line.contains("id=failed")));
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Library(ItemKind::Playlist));
            assert_eq!(view.playlist_options.len(), 1);
            assert_eq!(view.playlist_options[0].id, "failed");
            assert!(view.message.contains("3 playlists; 1 failed"));
            assert!(view.message.contains("Fixture deletion failed"));
        });
        stop.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).ok();
    }
}
