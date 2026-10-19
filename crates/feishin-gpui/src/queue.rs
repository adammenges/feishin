use crate::app::Feishin;
use feishin_core::{
    model::{Item, ItemKind},
    queue::Enqueue,
};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, StyledExt};
use uuid::Uuid;

pub fn album_grouping(config: &feishin_core::lists::Config) -> bool {
    let table = if config.display == "detail" {
        config.detail.as_ref().unwrap_or(&config.table)
    } else {
        &config.table
    };
    config.display == "detail"
        || table
            .columns
            .iter()
            .any(|column| column.is_enabled && column.id == "albumGroup")
}

#[derive(Debug, PartialEq)]
pub enum GroupedQueueRow {
    Section { up_next: bool, count: usize },
    Album(std::ops::Range<usize>),
    Track(usize),
}

// A header and individual virtual tracks keep large albums from loading every cover.
pub fn grouped_queue_rows<'a>(songs: impl Iterator<Item = &'a Item>) -> Vec<GroupedQueueRow> {
    let songs = songs.collect::<Vec<_>>();
    let mut rows = Vec::with_capacity(songs.len());
    let mut start = 0;
    for index in 1..=songs.len() {
        if index == songs.len()
            || songs[index].album.is_empty()
            || songs[index - 1].album.is_empty()
            || songs[index].album != songs[index - 1].album
            || songs[index].server_id != songs[index - 1].server_id
        {
            if !songs[start].album.is_empty() {
                rows.push(GroupedQueueRow::Album(start..index));
            }
            rows.extend((start..index).map(GroupedQueueRow::Track));
            start = index;
        }
    }
    rows
}

// Section boundaries also split album groups, so every header points to real track indices.
pub fn display_rows(
    entries: &[&feishin_core::queue::Entry],
    priority: &[Uuid],
    albums: bool,
    sections: bool,
) -> Vec<GroupedQueueRow> {
    let priority = priority
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let mut result = vec![];
    let mut start = 0;
    while start < entries.len() {
        let up_next = priority.contains(&entries[start].key);
        let end = if sections {
            (start + 1..entries.len())
                .find(|index| priority.contains(&entries[*index].key) != up_next)
                .unwrap_or(entries.len())
        } else {
            entries.len()
        };
        if sections {
            result.push(GroupedQueueRow::Section {
                up_next,
                count: end - start,
            });
        }
        if albums {
            result.extend(
                grouped_queue_rows(entries[start..end].iter().map(|entry| &entry.item))
                    .into_iter()
                    .map(|row| match row {
                        GroupedQueueRow::Album(range) => {
                            GroupedQueueRow::Album(range.start + start..range.end + start)
                        }
                        GroupedQueueRow::Track(index) => GroupedQueueRow::Track(index + start),
                        section => section,
                    }),
            );
        } else {
            result.extend((start..end).map(GroupedQueueRow::Track));
        }
        start = end;
    }
    result
}

/// Match the original closest-edge tie break: the midpoint belongs to the top half.
pub fn row_drop_listener(
    cx: &Context<Feishin>,
    action: impl Fn(&mut Feishin, &Dragged, bool, &mut Window, &mut Context<Feishin>) + 'static,
) -> impl Fn(&Dragged, Bounds<Pixels>, &mut Window, &mut App) + 'static {
    let view = cx.weak_entity();
    move |dragged, bounds, window, cx| {
        let after = window.mouse_position().y > bounds.center().y;
        view.update(cx, |view, cx| action(view, dragged, after, window, cx))
            .ok();
    }
}

#[derive(Clone)]
pub struct PlaylistSource {
    pub playlist: Item,
    pub entries: Vec<(usize, String, String)>,
}
#[derive(Clone)]
pub struct Dragged {
    pub sidebar_playlist: bool,
    pub playlist_source: Option<PlaylistSource>,
    pub keys: Vec<Uuid>,
    pub items: Vec<Item>,
}
impl Feishin {
    pub fn queue_has_sections(&self, sidebar: bool) -> bool {
        self.state.settings.up_next_queue
            && !self.state.queue.up_next.is_empty()
            && if sidebar {
                self.side_queue_filter.trim().is_empty()
            } else {
                self.local_filter.trim().is_empty() && self.queue_display_sort.is_none()
            }
    }

    pub fn queue_display_rows(
        &self,
        entries: &[&feishin_core::queue::Entry],
        config: &feishin_core::lists::Config,
        sidebar: bool,
    ) -> Vec<GroupedQueueRow> {
        display_rows(
            entries,
            &self.state.queue.up_next,
            album_grouping(config),
            self.queue_has_sections(sidebar),
        )
    }

    pub fn queue_section_header(
        &self,
        up_next: bool,
        count: usize,
        row: usize,
        sidebar: bool,
        cx: &App,
    ) -> AnyElement {
        let label = self.tr(if up_next {
            "player.queueSection_upNext"
        } else {
            "player.queueSection_queue"
        });
        div()
            .id(SharedString::from(format!("queue-section-{sidebar}-{row}")))
            .role(Role::Heading)
            .aria_label(label)
            .debug_selector(move || {
                if up_next {
                    "up-next-section".into()
                } else {
                    "queue-section".into()
                }
            })
            .h_flex()
            .h(px(32.))
            .px_3()
            .gap_2()
            .bg(cx.theme().secondary)
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().foreground)
            .whitespace_nowrap()
            .map(|mut div| {
                div.text_style()
                    .get_or_insert_with(Default::default)
                    .letter_spacing_fraction = Some(0.06);
                div
            })
            .child(self.tr(if up_next {
                "player.queueSection_upNext"
            } else {
                "player.queueSection_queue"
            }))
            .child(count.to_string())
            .into_any_element()
    }

    pub fn queue_selection(&self, sidebar: bool) -> (&std::collections::HashSet<Uuid>, &Vec<Uuid>) {
        if sidebar {
            (&self.side_queue_selected, &self.side_queue_selection_order)
        } else {
            (&self.queue_selected, &self.queue_selection_order)
        }
    }
    pub fn queue_selection_mut(
        &mut self,
        sidebar: bool,
    ) -> (&mut std::collections::HashSet<Uuid>, &mut Vec<Uuid>) {
        if sidebar {
            (
                &mut self.side_queue_selected,
                &mut self.side_queue_selection_order,
            )
        } else {
            (&mut self.queue_selected, &mut self.queue_selection_order)
        }
    }
    pub fn first_queue_selection(&self, sidebar: bool, cx: &App) -> Option<Uuid> {
        let (selected, order) = self.queue_selection(sidebar);
        self.visible_queue_keys(sidebar, cx)
            .into_iter()
            .find(|key| selected.contains(key))
            .or_else(|| {
                order
                    .iter()
                    .find(|key| selected.contains(key) && self.state.queue.order.contains(key))
                    .copied()
            })
    }

    pub fn side_queue_rows(&self) -> Vec<(usize, &feishin_core::queue::Entry)> {
        let ordered = self.state.queue.ordered();
        feishin_core::lists::search_items(
            ordered.iter().map(|entry| &entry.item),
            &self.side_queue_filter,
            ItemKind::Song,
        )
        .into_iter()
        .map(|index| (index, ordered[index]))
        .collect()
    }

    pub fn visible_queue_keys(&self, sidebar: bool, _cx: &App) -> Vec<Uuid> {
        if sidebar {
            self.side_queue_rows()
                .into_iter()
                .map(|(_, entry)| entry.key)
                .collect()
        } else {
            let config = self.list_config();
            let entries = self.queue_rows();
            let (start, count) = if !self.is_full_player_view() && config.pagination == "paginated"
            {
                (
                    self.library_page * config.items_per_page,
                    config.items_per_page,
                )
            } else {
                (0, entries.len())
            };
            entries
                .iter()
                .skip(start)
                .take(count)
                .map(|entry| entry.key)
                .collect()
        }
    }

    pub fn select_queue_row(
        &mut self,
        key: Uuid,
        sidebar: bool,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let rows = self.visible_queue_keys(sidebar, cx);
        if !rows.contains(&key) {
            return;
        }
        let live = self.state.queue.order.clone();
        let (selected, order) = self.queue_selection_mut(sidebar);
        selected.retain(|key| live.contains(key));
        crate::lists::select_row(selected, order, key, modifiers, |anchor, key| {
            crate::lists::selection_range(&rows, anchor, key)
        });
        cx.notify();
    }

    pub fn queue_row_click(
        &mut self,
        key: Uuid,
        sidebar: bool,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if !self.state.queue.order.contains(&key) {
            return;
        }
        if event.click_count() == 2 {
            let (selected, order) = self.queue_selection_mut(sidebar);
            *selected = [key].into();
            *order = vec![key];
            self.play_queue_entry(key, cx);
        } else {
            self.select_queue_row(key, sidebar, event.modifiers(), cx);
        }
        cx.notify();
    }

    pub fn poll_queue_autosave(&mut self, cx: &mut Context<Self>) {
        if self.state.settings.queue_autosave
            && !self.queue_save_busy
            && self
                .api()
                .is_some_and(|api| api.server.kind != feishin_core::model::ServerType::Jellyfin)
            && self.queue_autosave.observe(
                self.state.queue.current,
                self.state.settings.queue_autosave_count,
            )
        {
            self.save_server_queue(cx);
        }
    }

    pub fn save_server_queue(&mut self, cx: &mut Context<Self>) {
        if self.queue_save_busy || self.queue_restore_busy {
            return;
        }
        let Some(api) = self.api() else { return };
        let songs = self
            .state
            .queue
            .ordered()
            .into_iter()
            .map(|entry| entry.item.clone())
            .collect::<Vec<_>>();
        let current = self
            .state
            .queue
            .order
            .iter()
            .position(|key| Some(*key) == self.state.queue.current);
        let position = self.state.queue.position;
        self.queue_save_busy = true;
        let task = cx
            .background_executor()
            .spawn(async move { api.save_queue(&songs, current, position) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.queue_save_busy = false;
                match result {
                    Ok(()) => this.message = "Queue saved to server".into(),
                    Err(error) => {
                        log::warn!("Server queue save failed: {error}");
                        this.message = format!("Could not save queue to server: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn supports_server_queue(&self) -> bool {
        self.api().is_some_and(|api| {
            let features = api.features();
            features.native_queue || features.index_queue
        })
    }

    pub fn load_server_queue(&mut self, cx: &mut Context<Self>) {
        if self.queue_save_busy || self.queue_restore_busy {
            return;
        }
        let Some(api) = self.api() else { return };
        self.queue_restore_busy = true;
        self.play_request_generation += 1;
        let generation = self.play_request_generation;
        let server = api.server.id;
        let task = cx
            .background_executor()
            .spawn(async move { api.load_queue() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.queue_restore_busy = false;
                if generation == this.play_request_generation
                    && this.state.active_server == Some(server)
                {
                    match result {
                        Ok((songs, current, position)) => {
                            this.restore_server_queue(songs, current, position, cx);
                        }
                        Err(error) => {
                            log::warn!("Server queue restore failed: {error}");
                            this.message = error.to_string();
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

    pub fn shuffle_queue(&mut self, cx: &mut Context<Self>) {
        self.state.queue.shuffle_all();
        self.arm_next();
        self.save();
        cx.notify();
    }

    pub fn create_playlist_from_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let songs = self
            .state
            .queue
            .ordered()
            .into_iter()
            .map(|entry| entry.item.clone())
            .collect();
        self.edit_playlist(None, songs, window, cx);
    }

    pub fn play_queue_entry(&mut self, key: Uuid, cx: &mut Context<Self>) {
        if !self
            .state
            .queue
            .entries
            .iter()
            .any(|entry| entry.key == key)
        {
            return;
        }
        self.play_request_generation += 1;
        self.state.queue.current = Some(key);
        self.state.queue.position = 0.;
        self.play_current(cx);
    }
    pub fn drag_library_item(&self, index: usize) -> Dragged {
        let items = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                if self.selected.contains(&index) {
                    self.selected.contains(i)
                } else {
                    *i == index
                }
            })
            .flat_map(|(_, item)| item.action_items())
            .collect();
        let playlist_source = if let crate::app::Route::Detail(playlist) = &self.route {
            (playlist.kind == ItemKind::Playlist).then(|| PlaylistSource {
                playlist: playlist.as_ref().clone(),
                entries: self
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        if self.selected.contains(&index) {
                            self.selected.contains(i)
                        } else {
                            *i == index
                        }
                    })
                    .map(|(i, item)| (i, item.id.clone(), item.playlist_entry_id.clone()))
                    .collect(),
            })
        } else {
            None
        };
        Dragged {
            playlist_source,
            sidebar_playlist: false,
            keys: vec![],
            items,
        }
    }
    pub fn drop_playlist_tracks(
        &mut self,
        dragged: &Dragged,
        before: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let crate::app::Route::Detail(target) = self.route.clone() else {
            return;
        };
        if target.kind != ItemKind::Playlist {
            return;
        }
        if let Some(source) = &dragged.playlist_source
            && source.playlist.id == target.id
            && source.playlist.server_id == target.server_id
        {
            if source.entries.iter().any(|(index, id, entry)| {
                self.items
                    .get(*index)
                    .is_none_or(|item| item.id != *id || item.playlist_entry_id != *entry)
            }) {
                self.message =
                    "Playlist view changed during dragging; reload before moving tracks".into();
                cx.notify();
                return;
            }
            self.selected = source.entries.iter().map(|(index, _, _)| *index).collect();
            self.playlist_selection_action(
                feishin_core::api::PlaylistSelectionAction::MoveBefore(
                    before.map(|index| self.playlist_boundary_position(index)),
                ),
                cx,
            );
        } else if !self.playlist_tracks_editing()
            && let Some(api) = self.sessions.get(&target.server_id).cloned()
        {
            let items = dragged.items.clone();
            self.library_mutation(
                move || {
                    let mut songs = vec![];
                    for item in items {
                        songs.extend(api.all_tracks(&item)?);
                    }
                    api.add_to_playlist_checked(&target.id, &songs)
                },
                "Added to playlist",
                None,
                cx,
            );
        }
    }
    pub fn drag_queue_entry(&self, key: Uuid, sidebar: bool) -> Dragged {
        let selected = self.queue_selection(sidebar).0;
        let entries = self
            .state
            .queue
            .ordered()
            .into_iter()
            .filter(|entry| {
                if selected.contains(&key) {
                    selected.contains(&entry.key)
                } else {
                    entry.key == key
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        Dragged {
            sidebar_playlist: false,
            playlist_source: None,
            keys: entries.iter().map(|entry| entry.key).collect(),
            items: entries.into_iter().map(|entry| entry.item).collect(),
        }
    }
    pub fn drop_queue(&mut self, dragged: &Dragged, before: Option<Uuid>, cx: &mut Context<Self>) {
        self.drop_queue_at(dragged, before, false, cx);
    }
    pub fn drop_queue_at(
        &mut self,
        dragged: &Dragged,
        target: Option<Uuid>,
        after: bool,
        cx: &mut Context<Self>,
    ) {
        if !dragged.keys.is_empty() {
            self.state.queue.move_at(&dragged.keys, target, after);
            self.arm_next();
            self.save();
            cx.notify();
            return;
        }
        let pin_above_up_next = self.state.queue.pins_above_up_next(target, after);
        let items = dragged.items.clone();
        let context = dragged
            .playlist_source
            .as_ref()
            .map(|source| source.playlist.id.clone())
            .or_else(|| {
                (items.len() == 1 && items[0].kind == ItemKind::Playlist)
                    .then(|| items[0].id.clone())
            });
        self.play_request_generation += 1;
        let request = self.play_request_generation;
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.library_fetches.insert(request, cancelled.clone());
        let sessions = self
            .sessions
            .iter()
            .map(|(id, api)| (*id, api.clone().with_cancellation(cancelled.clone())))
            .collect::<std::collections::HashMap<_, _>>();
        let task = cx.background_executor().spawn(async move {
            let mut songs = vec![];
            for item in items {
                if item.kind == ItemKind::Radio {
                    anyhow::bail!("Radio stations play live and cannot be queued");
                }
                if item.kind == ItemKind::Song {
                    songs.push(item);
                } else if let Some(api) = sessions.get(&item.server_id) {
                    songs.extend(api.all_tracks(&item)?);
                } else {
                    anyhow::bail!("Sign in to the server for these items");
                }
            }
            Ok::<_, anyhow::Error>(songs)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.library_fetches.remove(&request);
                cx.notify();
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                match result {
                    Ok(mut songs) => {
                        this.filter_playback_tracks(&mut songs);
                        if let Some(context) = context.filter(|context| !context.is_empty()) {
                            for song in &mut songs {
                                song.context_playlist_id = Some(context.clone());
                            }
                        }
                        let start = this.state.queue.entries.len();
                        this.state.queue.add(songs, Enqueue::End);
                        let added = this.state.queue.entries[start..]
                            .iter()
                            .map(|entry| entry.key)
                            .collect::<Vec<_>>();
                        this.state
                            .queue
                            .move_at_boundary(&added, target, after, pin_above_up_next);
                        this.arm_next();
                        this.save();
                    }
                    Err(error) => {
                        log::warn!("Could not add dropped items to the queue: {error}");
                        this.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn move_queue_selection(&mut self, before: Option<Uuid>, cx: &mut Context<Self>) {
        let keys = self.queue_selected.iter().copied().collect::<Vec<_>>();
        let before = before
            .and_then(|key| {
                self.state
                    .queue
                    .order
                    .iter()
                    .position(|entry| *entry == key)
            })
            .and_then(|index| {
                self.state.queue.order[index..]
                    .iter()
                    .find(|key| !self.queue_selected.contains(key))
            })
            .copied();
        self.state.queue.move_before(&keys, before);
        self.arm_next();
        self.save();
        cx.notify();
    }
    pub fn remove_queue_selection(&mut self, sidebar: bool, cx: &mut Context<Self>) {
        let selected = self
            .queue_selection(sidebar)
            .0
            .iter()
            .copied()
            .collect::<Vec<_>>();
        self.remove_queue_entries(&selected, cx);
    }
    pub fn remove_queue_entries(&mut self, selected: &[Uuid], cx: &mut Context<Self>) {
        let current = self.radio_item.is_none()
            && self
                .state
                .queue
                .current
                .is_some_and(|key| selected.contains(&key));
        if current {
            if self
                .state
                .queue
                .entries
                .iter()
                .all(|entry| selected.contains(&entry.key))
            {
                self.report("stop", cx);
            }
            self.play_request_generation += 1;
        }
        // Remove other entries first so the next current track cannot be one being removed.
        self.queue_selected.retain(|key| !selected.contains(key));
        self.queue_selection_order
            .retain(|key| !selected.contains(key));
        self.side_queue_selected
            .retain(|key| !selected.contains(key));
        self.side_queue_selection_order
            .retain(|key| !selected.contains(key));
        for key in selected
            .iter()
            .filter(|key| Some(**key) != self.state.queue.current)
            .copied()
            .collect::<Vec<_>>()
        {
            self.state.queue.remove(key);
        }
        if let Some(key) = self
            .state
            .queue
            .current
            .filter(|key| selected.contains(key))
        {
            self.state.queue.remove(key);
        }
        if current {
            self.play_current(cx);
        } else {
            self.arm_next();
        }
        self.save();
        cx.notify();
    }
}

#[cfg(test)]
mod upstream_tests {
    use super::{GroupedQueueRow, display_rows};
    use crate::app::Feishin;
    use feishin_core::{
        model::Item,
        queue::{Enqueue, Queue},
        storage::State,
    };
    use gpui::{AppContext, TestAppContext};
    use gpui_component::Root;
    use uuid::Uuid;

    #[test]
    fn sections_split_album_groups_without_changing_track_indices() {
        use GroupedQueueRow::{Album, Section, Track};
        let song = Item {
            album: "Album".into(),
            ..Default::default()
        };
        let mut queue = Queue::default();
        queue.add(vec![song.clone(), song.clone()], Enqueue::Now);
        queue.add(vec![song], Enqueue::Next);
        let entries = queue.ordered();
        assert_eq!(
            display_rows(&entries, &queue.up_next, true, true),
            [
                Section {
                    up_next: false,
                    count: 1
                },
                Album(0..1),
                Track(0),
                Section {
                    up_next: true,
                    count: 1
                },
                Album(1..2),
                Track(1),
                Section {
                    up_next: false,
                    count: 1
                },
                Album(2..3),
                Track(2),
            ]
        );
        assert_eq!(
            display_rows(&entries, &queue.up_next, false, false),
            [Track(0), Track(1), Track(2)]
        );
        assert_eq!(
            display_rows(&entries[1..], &queue.up_next, false, true),
            [
                Section {
                    up_next: true,
                    count: 1
                },
                Track(0),
                Section {
                    up_next: false,
                    count: 1
                },
                Track(1),
            ]
        );
    }

    #[gpui::test]
    fn up_next_native_sections_follow_search_playback_and_saved_setting(cx: &mut TestAppContext) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-up-next-{}", Uuid::new_v4()));
        let mut captured = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = captured.unwrap();
        let section_visible = |cx: &mut gpui::VisualTestContext| {
            cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.role() == gpui::Role::Heading
                            && !node.is_hidden()
                            && node.label() == Some("Up next")
                    })
            })
        };

        view.update(cx, |view, cx| {
            view.route = crate::app::Route::Queue;
            view.state.queue.add(
                vec![
                    Item {
                        name: "Current".into(),
                        ..Default::default()
                    },
                    Item {
                        name: "Later".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            view.state.queue.add(
                vec![Item {
                    name: "Pending".into(),
                    ..Default::default()
                }],
                Enqueue::Next,
            );
            view.save();
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        assert!(section_visible(cx));
        view.update(cx, |view, cx| {
            view.scroll_queue_to_current();
            view.submit_local_filter("Pending".into(), cx);
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        assert!(!section_visible(cx));
        view.update(cx, |view, cx| {
            view.local_filter.clear();
            view.state.settings.up_next_queue = false;
            view.save();
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().unwrap();
        assert!(!section_visible(cx));
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert!(!saved.settings.up_next_queue);
        assert!(saved.queue.up_next.is_empty());
        assert_eq!(saved.queue.entries.len(), 3);
        std::fs::remove_dir_all(dir).ok();
    }
}
