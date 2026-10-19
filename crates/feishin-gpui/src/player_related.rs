use crate::{app::Feishin, lists::TableRow, queue::Dragged};
use feishin_core::{
    model::{Item, ItemKind},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IconName, StyledExt,
    button::ButtonVariants,
    menu::{ContextMenuExt, DropdownMenu},
};
use std::collections::HashSet;

#[derive(Default)]
pub struct Related {
    source: Option<(uuid::Uuid, String, String, String)>,
    generation: u64,
    visible: bool,
    loaded_at: Option<std::time::Instant>,
    pub items: Vec<Item>,
    pub selected: HashSet<usize>,
    pub selection_order: Vec<usize>,
    pub scroll: UniformListScrollHandle,
    pub horizontal_scroll: ScrollHandle,
    loading: bool,
    error: Option<String>,
}

impl Feishin {
    pub fn sync_player_related(&mut self, cx: &mut Context<Self>) {
        if !self.full_player || self.state.settings.player_display.active_tab != "related" {
            self.player_related.visible = false;
            return;
        }
        let opening = !self.player_related.visible;
        self.player_related.visible = true;
        let candidate = self
            .current_entry()
            .filter(|entry| entry.item.kind == ItemKind::Song)
            .and_then(|entry| {
                self.sessions
                    .get(&entry.item.server_id)
                    .cloned()
                    .map(|api| (entry.item.clone(), api))
            });
        let Some((item, api)) = candidate else {
            if self.player_related.source.take().is_some() {
                self.player_related.generation = self.player_related.generation.wrapping_add(1);
                self.player_related.items.clear();
                self.player_related.selected.clear();
                self.player_related.selection_order.clear();
                self.player_related.error = None;
                self.player_related.loading = false;
            }
            return;
        };
        let source = (
            item.server_id,
            item.id.clone(),
            api.server.url.clone(),
            api.server.username.clone(),
        );
        let stale = self.player_related.loaded_at.is_none_or(|loaded| {
            cx.background_executor()
                .now()
                .saturating_duration_since(loaded)
                >= std::time::Duration::from_secs(10)
        });
        if self.player_related.source.as_ref() == Some(&source)
            && (self.player_related.loading || !opening || !stale)
        {
            return;
        }
        let state = &mut self.player_related;
        state.source = Some(source);
        state.generation = state.generation.wrapping_add(1);
        state.items.clear();
        state.selected.clear();
        state.selection_order.clear();
        state.loading = true;
        state.error = None;
        state.scroll.scroll_to_item_strict(0, ScrollStrategy::Top);
        let generation = state.generation;
        let task = cx
            .background_executor()
            .spawn(async move { smol::unblock(move || api.similar_songs(&item)).await });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.player_related.generation != generation {
                    return;
                }
                this.player_related.loading = false;
                match result {
                    Ok(items) => {
                        this.player_related.loaded_at = Some(cx.background_executor().now());
                        for item in &items {
                            this.fetch_item_cover_at(
                                item,
                                feishin_core::images::Placement::Table,
                                cx,
                            );
                        }
                        this.player_related.items = items;
                    }
                    Err(error) => {
                        this.player_related.loaded_at = None;
                        log::warn!("Could not load related player tracks: {error}");
                        this.player_related.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn related_action_items(&self, index: usize) -> Vec<Item> {
        let state = &self.player_related;
        if state.selected.contains(&index) {
            state
                .items
                .iter()
                .enumerate()
                .filter(|(index, _)| state.selected.contains(index))
                .map(|(_, item)| item.clone())
                .collect()
        } else {
            state.items.get(index).cloned().into_iter().collect()
        }
    }

    pub fn play_related(
        &mut self,
        index: usize,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.player_related.items.get(index).cloned() else {
            return;
        };
        let mut tracks = crate::lists::row_play_context(
            &self.player_related.items,
            index,
            matches!(mode, Enqueue::Next | Enqueue::End),
            true,
        );
        if shuffle {
            use rand::seq::SliceRandom;
            tracks.shuffle(&mut rand::thread_rng());
        }
        self.add_tracks_from(tracks, mode, Some(&item), cx);
    }

    pub fn related_row_click(&mut self, index: usize, event: &ClickEvent, cx: &mut Context<Self>) {
        if event.click_count() == 2 {
            self.play_related(index, Enqueue::Now, false, cx);
        } else {
            let state = &mut self.player_related;
            crate::lists::select_row(
                &mut state.selected,
                &mut state.selection_order,
                index,
                event.modifiers(),
                |anchor, index| (*anchor.min(index)..=*anchor.max(index)).collect(),
            );
            cx.notify();
        }
    }

    pub fn related_item_menu(&self, item: &Item, index: usize, cx: &Context<Self>) -> AnyElement {
        crate::styles::button(("related-menu", index), cx)
            .ghost()
            .icon(IconName::Ellipsis)
            .tooltip(self.tr("More"))
            .dropdown_menu(self.item_menu_builder(
                item,
                self.related_action_items(index),
                false,
                cx,
            ))
            .into_any_element()
    }

    pub fn player_related_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.player_related.loading {
            return div()
                .p_4()
                .child(self.tr("player.playbackFetchInProgress"))
                .into_any_element();
        }
        if let Some(error) = &self.player_related.error {
            return div()
                .v_flex()
                .p_4()
                .gap_3()
                .child(error.clone())
                .child(
                    crate::styles::button("player-related-retry", cx)
                        .label(self.tr("common.retry"))
                        .debug_selector(|| "player-related-retry".into())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.player_related.source = None;
                            this.sync_player_related(cx);
                            cx.notify();
                        })),
                )
                .into_any_element();
        }
        let mut config = self.list_config();
        config.display = "table".into();
        let mut columns = config
            .table
            .columns
            .iter()
            .filter(|column| column.is_enabled)
            .cloned()
            .collect::<Vec<_>>();
        columns.sort_by_key(|column| match column.pinned.as_deref() {
            Some("left") => 0,
            Some("right") => 2,
            _ => 1,
        });
        let row_config = config.clone();
        let row_columns = columns.clone();
        let rows = uniform_list(
            "player-related-list",
            self.player_related.items.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|index| {
                        let item = &this.player_related.items[index];
                        let selected = this.player_related.selected.contains(&index);
                        div()
                            .id(("related-row", index))
                            .on_hover(cx.listener(move |this, hovered, _, cx| {
                                this.hover_table_row(TableRow::Related, index, *hovered, cx)
                            }))
                            .debug_selector(move || format!("player-related-row-{index}"))
                            .role(Role::Row)
                            .aria_label(item.name.clone())
                            .aria_selected(selected)
                            .aria_row_index(index)
                            .h(px(
                                row_config.table.size.row_height() * this.state.settings.zoom
                            ))
                            .w_full()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .when(
                                selected
                                    || row_config.table.enable_alternate_row_colors
                                        && index % 2 == 1,
                                |div| div.bg(cx.theme().secondary),
                            )
                            .when(row_config.table.enable_horizontal_borders, |div| {
                                div.border_b_1().border_color(cx.theme().border)
                            })
                            .when(row_config.table.enable_row_hover_highlight, |div| {
                                div.hover(|style| style.bg(cx.theme().secondary))
                            })
                            .on_drag(
                                Dragged {
                                    sidebar_playlist: false,
                                    playlist_source: None,
                                    keys: vec![],
                                    items: this.related_action_items(index),
                                },
                                crate::context_preview::drag_constructor(cx),
                            )
                            .child(this.metadata_cells(
                                item,
                                index,
                                &row_config,
                                row_columns.clone(),
                                TableRow::Related,
                                cx,
                            ))
                            .context_menu(this.item_menu_builder(
                                item,
                                this.related_action_items(index),
                                false,
                                cx,
                            ))
                            .into_any_element()
                    })
                    .collect()
            }),
        )
        .track_scroll(self.player_related.scroll.clone())
        .flex_1()
        .min_h_0();
        div()
            .id("player-related")
            .role(Role::Table)
            .aria_label(self.tr("page.fullscreenPlayer.related"))
            .debug_selector(|| "player-related".into())
            .key_context("RelatedList")
            .track_focus(&self.related_focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, _| window.focus(&this.related_focus)),
            )
            .v_flex()
            .size_full()
            .when(config.table.enable_header, |div| {
                let cells = columns
                    .into_iter()
                    .map(|column| {
                        let label = crate::lists::table_header(&column, cx).into_any_element();
                        let label = self.column_drag_controls_for(
                            "fullScreenSong",
                            &column,
                            label,
                            false,
                            cx,
                        );
                        (column, label)
                    })
                    .collect();
                div.child(
                    gpui::div()
                        .h_flex()
                        .h(px(40. * self.state.settings.zoom))
                        .w_full()
                        .flex_shrink_0()
                        .child(self.column_strip(
                            "related-header-scroll".into(),
                            cells,
                            &config.table,
                            false,
                            &self.player_related.horizontal_scroll,
                            cx,
                        )),
                )
            })
            .when(self.player_related.items.is_empty(), |div| {
                div.child(self.tr("common.noResultsFromQuery"))
            })
            .child(
                div()
                    .relative()
                    .v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(rows)
                    .child(crate::scroll::Scrollbar::vertical(
                        "related-scrollbar",
                        &self.player_related.scroll,
                    ))
                    .child(
                        crate::scroll::Scrollbar::horizontal(
                            "related-horizontal-scrollbar",
                            &self.player_related.horizontal_scroll,
                        )
                        .in_viewport(&self.player_related.scroll),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Enqueue, Feishin, HashSet, Item, ItemKind};
    use crate::app::Route;
    use feishin_core::{
        api::Api,
        lists,
        model::{Server, ServerType, Session},
        storage::State,
    };
    use gpui::{AppContext, Modifiers, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        rc::Rc,
        sync::mpsc,
        time::{Duration, Instant},
    };

    #[gpui::test]
    fn related_player_requests_selection_retry_and_song_changes_preserve_browsing_and_drafts(
        cx: &mut TestAppContext,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let (send, receive) = mpsc::channel();
        let (request_start, request_ready) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let mut related = 0;
            let mut awaiting_request = true;
            while related < 5 {
                // Start the deadline when the UI expects a request, after fixture setup.
                if awaiting_request && request_ready.recv().is_err() {
                    return;
                }
                awaiting_request = false;
                let deadline = Instant::now() + Duration::from_secs(15);
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "Related request never arrived");
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                loop {
                    let mut part = [0; 4096];
                    let count = stream.read(&mut part).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&part[..count]);
                    if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                        break;
                    }
                }
                let request = String::from_utf8(bytes).unwrap();
                if request.contains("/rest/getCoverArt.view?") {
                    write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .unwrap();
                    continue;
                }
                assert!(
                    request.contains("/rest/getSimilarSongs.view?"),
                    "Unexpected request: {}",
                    request.lines().next().unwrap()
                );
                assert!(request.contains("count=50") && !request.contains("musicFolderId="));
                send.send((stream, request)).unwrap();
                related += 1;
                awaiting_request = true;
            }
        });
        let api = Api::new(
            Server {
                id: uuid::Uuid::new_v4(),
                name: "Related fixture".into(),
                url: format!("http://{address}"),
                username: "user".into(),
                kind: ServerType::Subsonic,
                legacy_auth: false,
                options: Default::default(),
            },
            Session::default(),
        )
        .unwrap();
        let seed = Item {
            id: "seed".into(),
            server_id: api.server.id,
            name: "Current song".into(),
            ..Default::default()
        };
        let playlist = Item {
            id: "playlist".into(),
            kind: ItemKind::Playlist,
            server_id: api.server.id,
            name: "Unsaved playlist".into(),
            ..Default::default()
        };
        let draft = vec![Item {
            id: "draft".into(),
            name: "Unrelated draft".into(),
            ..seed.clone()
        }];
        let edit_id = uuid::Uuid::new_v4();
        let dir =
            std::env::temp_dir().join(format!("feishin-player-related-{}", uuid::Uuid::new_v4()));
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.lyric_fetch = false;
            state.settings.panels.expanded = false;
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.confirm_large_fetch = false;
            state.settings.mpv_path = dir.join("missing-mpv").to_string_lossy().into_owned();
            state
                .settings
                .shortcuts
                .get_mut(&feishin_core::shortcuts::Action::ListPlayLast)
                .unwrap()
                .key = "alt-l".into();
            state.queue.add(vec![seed], Enqueue::Now);
            let mut config = lists::default_for("fullScreenSong");
            config.table.enable_header = true;
            for column in &mut config.table.columns {
                column.is_enabled = column.id == "name";
            }
            state.settings.lists.insert("fullScreenSong".into(), config);
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                app.sessions.insert(api.server.id, api);
                app.route = Route::Detail(Box::new(playlist.clone()));
                app.items = draft.clone();
                app.total = Some(1);
                app.selected.insert(0);
                app.grid_selection_order = vec![0];
                app.playlist_tracks_edit = Some(crate::playlist_actions::TrackEdit {
                    id: edit_id,
                    playlist: playlist.clone(),
                    original: Some(vec![]),
                    filter: "draft filter".into(),
                    saving: false,
                });
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1400.), px(900.)));
        cx.run_until_parked();
        let history = app.read_with(cx, |app, _| {
            (app.history.clone(), app.forward_history.clone())
        });
        let click = |cx: &mut VisualTestContext, id: &str| {
            let position = cx.debug_bounds(id).unwrap().center();
            cx.simulate_click(position, Modifiers::none());
            cx.run_until_parked();
            if id == "player-lyrics" || id.starts_with("full-player-tab-") {
                cx.executor().advance_clock(Duration::from_millis(800));
                cx.update(|window, _| window.refresh());
                cx.run_until_parked();
            }
        };
        let request = |cx: &mut VisualTestContext| {
            request_start.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                cx.run_until_parked();
                if let Ok(request) = receive.try_recv() {
                    break request;
                }
                assert!(Instant::now() < deadline, "Expected related request");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        let reply = |mut stream: TcpStream, status: &str, ids: &[&str]| {
            let songs = ids
                .iter()
                .map(|id| serde_json::json!({"id":id,"title":id}))
                .collect::<Vec<_>>();
            let body = serde_json::json!({"subsonic-response":{"status":"ok","similarSongs":{"song":songs}}}).to_string();
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        };
        let settle = |cx: &mut VisualTestContext| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.read_with(cx, |app, _| app.player_related.loading) {
                cx.run_until_parked();
                assert!(Instant::now() < deadline, "Related result did not settle");
                std::thread::sleep(Duration::from_millis(5));
            }
            cx.run_until_parked();
        };
        click(cx, "player-lyrics");
        click(cx, "full-player-tab-related");
        let (old, _) = request(cx);
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.queue.entries[0].item.id = "replacement".into();
                cx.notify();
            })
        });
        let (new, line) = request(cx);
        assert!(line.contains("id=replacement"));
        reply(new, "200 OK", &["b", "a"]);
        settle(cx);
        reply(old, "200 OK", &["stale"]);
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(5));
            cx.run_until_parked();
        }
        app.read_with(cx, |app, _| {
            assert_eq!(
                app.player_related
                    .items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["b", "a"]
            );
            assert_eq!(app.list_key(), "fullScreenSong");
        });
        click(cx, "related-cell-0-name");
        assert!(cx.update(|window, cx| app.read(cx).related_focus.contains_focused(window, cx)));
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(app.player_related.selected.len(), 2);
            assert_eq!(app.related_action_items(0).len(), 2);
            assert_eq!(app.selected, HashSet::from([0]));
        });
        cx.simulate_keystrokes("alt-l");
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(
                app.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.item.id.as_str())
                    .collect::<Vec<_>>(),
                ["replacement", "b"]
            );
            assert!(!app.playing);
        });
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                let mut changed = app.player_related.items[0].clone();
                changed.favorite = true;
                app.update_library_item(&changed);
                assert!(app.player_related.items[0].favorite);
                cx.notify();
            })
        });
        click(cx, "full-player-tab-lyrics");
        click(cx, "full-player-tab-related");
        assert!(
            receive.try_recv().is_err(),
            "Reopening the same related results should reuse them"
        );
        app.read_with(cx, |app, _| {
            assert_eq!(app.route, Route::Detail(Box::new(playlist.clone())));
            assert_eq!((app.history.clone(), app.forward_history.clone()), history);
            assert_eq!(
                app.items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["draft"]
            );
            let edit = app.playlist_tracks_edit.as_ref().unwrap();
            assert_eq!(edit.id, edit_id);
            assert!(edit.original.as_ref().unwrap().is_empty());
            assert_eq!(edit.filter, "draft filter");
            assert!(!app.playing);
            assert_eq!(app.state.queue.entries.len(), 2);
        });
        click(cx, "full-player-tab-lyrics");
        cx.executor().advance_clock(Duration::from_secs(11));
        click(cx, "full-player-tab-related");
        let (expired, _) = request(cx);
        reply(expired, "200 OK", &["refreshed"]);
        settle(cx);
        assert_eq!(
            app.read_with(cx, |app, _| app.player_related.items[0].id.clone()),
            "refreshed"
        );
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.queue.entries[0].item.id = "error".into();
                cx.notify();
            })
        });
        let (error, _) = request(cx);
        reply(error, "500 Internal Server Error", &[]);
        settle(cx);
        assert!(app.read_with(cx, |app, _| app.player_related.error.is_some()));
        click(cx, "player-related-retry");
        let (retry, _) = request(cx);
        reply(retry, "200 OK", &[]);
        settle(cx);
        app.read_with(cx, |app, _| {
            assert!(app.player_related.error.is_none() && app.player_related.items.is_empty())
        });
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.toggle_full_player(cx);
                assert_eq!(app.route, Route::Detail(Box::new(playlist)));
                assert_eq!(app.playlist_tracks_edit.as_ref().unwrap().id, edit_id);
                app.toggle_full_player(cx);
                app.navigate(Route::Home, cx);
                assert!(!app.full_player);
                assert_eq!(app.history.len(), history.0.len() + 1);
            })
        });
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
