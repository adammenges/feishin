use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::{Item, ItemKind, ServerType},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, Sizable, StyledExt,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
};

impl Feishin {
    pub fn has_library_selection(&self) -> bool {
        !self.selected.is_empty() || !self.selected_album_tracks.is_empty()
    }
    pub fn selected_library_items(&self) -> Vec<Item> {
        let mut items = self
            .items
            .iter()
            .enumerate()
            .filter(|(index, _)| self.selected.contains(index))
            .flat_map(|(_, item)| item.action_items())
            .collect::<Vec<_>>();
        for album in &self.items {
            let key = Self::album_key(album);
            if let Some(tracks) = self.album_tracks.get(&key) {
                items.extend(
                    tracks
                        .items
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| {
                            self.selected_album_tracks.contains(&(key.clone(), *index))
                        })
                        .map(|(_, item)| item.clone()),
                );
            }
        }
        items
    }
    pub fn drag_album_track(&self, key: &str, index: usize) -> crate::queue::Dragged {
        let items = if self.selected_album_tracks.contains(&(key.into(), index)) {
            self.selected_library_items()
        } else {
            self.album_tracks
                .get(key)
                .and_then(|tracks| tracks.items.get(index))
                .cloned()
                .into_iter()
                .collect()
        };
        crate::queue::Dragged {
            sidebar_playlist: false,
            playlist_source: None,
            keys: vec![],
            items,
        }
    }
    #[cfg(test)]
    pub fn update_library_item(&mut self, item: &Item) {
        self.update_library_fields(item, true, true);
    }
    fn update_library_fields(&mut self, item: &Item, favorite: bool, rating: bool) {
        let update = |value: &mut Item| {
            if value.id == item.id && value.server_id == item.server_id && value.kind == item.kind {
                if favorite {
                    value.favorite = item.favorite;
                }
                if rating {
                    value.rating = item.rating;
                    if let Some(rating) = item.column_data.get("userRating") {
                        value
                            .column_data
                            .insert("userRating".into(), rating.clone());
                    } else {
                        value.column_data.remove("userRating");
                    }
                }
            }
        };
        for value in self
            .artist_sections
            .values_mut()
            .flat_map(|section| &mut section.items)
        {
            update(value);
            for song in &mut value.playlist_songs {
                update(song);
            }
        }
        for value in self.album_metadata_cache.values_mut().flatten() {
            update(value);
        }
        for value in self
            .items
            .iter_mut()
            .chain(self.player_related.items.iter_mut())
        {
            update(value);
            for song in &mut value.playlist_songs {
                update(song);
            }
        }
        for section in self.home_sections.values_mut() {
            for value in &mut section.items {
                update(value);
            }
        }
        for section in &mut self.album_related {
            for value in &mut section.state.items {
                update(value);
            }
        }
        for value in &mut self.info_items {
            update(value);
        }
        if let Some(value) = &mut self.previous_played {
            update(value);
        }
        if let Some((_, value)) = &mut self.last_played {
            update(value);
        }
        if let Some(info) = &mut self.artist_info {
            for value in &mut info.similar {
                update(value);
            }
        }
        for value in &mut self.state.queue.entries {
            update(&mut value.item);
        }
        for value in self
            .album_tracks
            .values_mut()
            .flat_map(|tracks| &mut tracks.items)
        {
            update(value);
        }
        if let Route::Detail(value) | Route::Similar(value) = &mut self.route {
            update(value);
        }
    }
    pub fn apply_library_values(
        &mut self,
        changed: &[Item],
        favorite: bool,
        rating: bool,
        cx: &mut Context<Self>,
    ) {
        for item in changed {
            self.update_library_fields(item, favorite, rating);
        }
        let Route::Detail(artist) = &self.route else {
            return;
        };
        if !matches!(artist.kind, ItemKind::Artist | ItemKind::AlbumArtist)
            || !changed
                .iter()
                .any(|item| item.kind == ItemKind::Song && item.server_id == artist.server_id)
        {
            return;
        }
        if self.artist_overview_active() {
            if let Some(section) = self.artist_sections.get_mut("favoriteSongs") {
                let items = std::mem::take(&mut section.items);
                let page = section.page;
                let selected = std::mem::take(&mut section.selected);
                let order = std::mem::take(&mut section.selection_order);
                let scroll = section.scroll.clone();
                let horizontal_scroll = section.horizontal_scroll.clone();
                self.load_artist_section("favoriteSongs", cx);
                if let Some(section) = self.artist_sections.get_mut("favoriteSongs") {
                    section.items = items;
                    section.page = page;
                    section.selected = selected;
                    section.selection_order = order;
                    section.scroll = scroll;
                    section.horizontal_scroll = horizontal_scroll;
                }
            }
        } else if matches!(
            self.artist_tab,
            feishin_core::model::ArtistTab::Favorites | feishin_core::model::ArtistTab::Rated
        ) {
            self.restore_visit(self.current_visit(), cx);
        }
    }
    pub fn set_library_values(
        &mut self,
        items: Vec<Item>,
        favorite: Option<bool>,
        rating: Option<u8>,
        cx: &mut Context<Self>,
    ) {
        self.favorite_requests += usize::from(favorite.is_some());
        self.rating_requests += usize::from(rating.is_some());
        cx.notify();
        let sessions = self.sessions.clone();
        let task = cx.background_executor().spawn(smol::unblock(move || {
            let mut changed = vec![];
            let mut seen = std::collections::HashSet::new();
            for mut item in items {
                if !seen.insert((item.server_id, item.id.clone(), item.kind)) {
                    continue;
                }
                let result = (|| -> anyhow::Result<()> {
                    let api = sessions
                        .get(&item.server_id)
                        .ok_or_else(|| anyhow::anyhow!("Sign in to this item's server"))?;
                    if let Some(favorite) = favorite {
                        api.favorite(&item, favorite)?;
                        item.favorite = favorite;
                    }
                    if let Some(rating) = rating {
                        api.rate(&item, rating)?;
                        item.set_rating(rating);
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    return (changed, Some(error.to_string()));
                }
                changed.push(item);
            }
            (changed, None)
        }));
        cx.spawn(async move |this, cx| {
            let (changed, error) = task.await;
            this.update(cx, |this, cx| {
                this.favorite_requests -= usize::from(favorite.is_some());
                this.rating_requests -= usize::from(rating.is_some());
                this.apply_library_values(&changed, favorite.is_some(), rating.is_some(), cx);
                this.message = error.unwrap_or_else(|| format!("Updated {} items", changed.len()));
                this.save();
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn open_info(&mut self, items: Vec<Item>, cx: &mut Context<Self>) {
        self.info_items = items;
        self.navigate(Route::Info, cx);
        self.reload_info(cx);
    }
    pub fn reload_info(&mut self, cx: &mut Context<Self>) {
        self.info_generation += 1;
        let generation = self.info_generation;
        let items = self.info_items.clone();
        let sessions = self.sessions.clone();
        self.info_loading = true;
        let task = cx.background_executor().spawn(async move {
            let mut loaded = vec![];
            let mut errors = vec![];
            for mut item in items {
                if matches!(
                    item.kind,
                    ItemKind::Song
                        | ItemKind::Album
                        | ItemKind::Artist
                        | ItemKind::AlbumArtist
                        | ItemKind::Playlist
                ) && let Some(api) = sessions.get(&item.server_id)
                {
                    match api.item(&item.id, item.kind) {
                        Ok(mut full) => {
                            full.playlist_entry_id = item.playlist_entry_id.clone();
                            full.playlist_position = item.playlist_position;
                            item = full;
                        }
                        Err(error) => {
                            log::debug!("Full item information unavailable: {error}");
                            errors.push(error.to_string());
                        }
                    }
                }
                loaded.push(item);
            }
            (loaded, errors)
        });
        cx.spawn(async move |this, cx| {
            let (items, errors) = task.await;
            this.update(cx, |this, cx| {
                if generation != this.info_generation {
                    return;
                }
                this.info_items = items;
                this.info_loading = false;
                if this.route == Route::Info && !errors.is_empty() {
                    this.message = errors.join("\n");
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn reveal_library_item(&self, item: &Item, cx: &App) {
        if item.path.is_empty() {
            return;
        }
        let settings = &self.state.settings;
        let path = settings.resolve_song_path(&item.path);
        cx.reveal_path(std::path::Path::new(&path));
    }
    pub fn library_item_menu(
        &self,
        item: &Item,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::styles::button(("item-more", index), cx)
            .ghost()
            .label(crate::appearance::text("More", cx))
            .dropdown_menu(self.library_menu_builder(item, index, cx))
            .into_any_element()
    }
    pub fn library_menu_builder(
        &self,
        item: &Item,
        index: usize,
        cx: &Context<Self>,
    ) -> impl Fn(
        gpui_component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui_component::menu::PopupMenu>,
    ) -> gpui_component::menu::PopupMenu
    + Clone
    + 'static {
        let items = if self.selected.contains(&index) {
            self.selected_library_items()
        } else {
            item.action_items()
        };
        self.item_menu_builder(item, items, false, cx)
    }
    pub fn item_menu_builder(
        &self,
        item: &Item,
        items: Vec<Item>,
        skip_first_song: bool,
        cx: &Context<Self>,
    ) -> impl Fn(
        gpui_component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui_component::menu::PopupMenu>,
    ) -> gpui_component::menu::PopupMenu
    + Clone
    + 'static {
        let rating_supported = self.state.settings.show_ratings
            && items.iter().all(|item| {
                self.sessions
                    .get(&item.server_id)
                    .is_some_and(|api| api.server.kind != ServerType::Jellyfin)
            });
        let share_supported = !items.is_empty()
            && items.iter().all(|item| {
                item.server_id == items[0].server_id
                    && self
                        .sessions
                        .get(&item.server_id)
                        .is_some_and(|api| api.server.kind == ServerType::Navidrome)
            });
        let favorite_supported = self.state.settings.show_favorites
            && items.iter().all(|item| {
                matches!(
                    item.kind,
                    ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                )
            });
        let editable_tags = items
            .iter()
            .all(|item| matches!(item.kind, ItemKind::Song | ItemKind::Album));
        let playlists =
            !items.is_empty() && items.iter().all(|item| item.kind == ItemKind::Playlist);
        let editable_playlists = playlists && items.iter().all(|item| self.can_edit_playlist(item));
        let item = item.clone();
        let view = cx.weak_entity();
        let play_behavior = self.state.settings.play_button_behavior;
        move |mut menu: gpui_component::menu::PopupMenu,
              _: &mut Window,
              cx: &mut Context<gpui_component::menu::PopupMenu>| {
            menu = menu.kind(gpui_component::menu::PopupMenuKind::Context);
            if items.is_empty() {
                return menu;
            }
            if let Some(parent) = view.upgrade() {
                let first = items[0].clone();
                parent.update(cx, |this, cx| this.fetch_item_cover(&first, cx));
                let preview = cx.new(|cx| crate::context_preview::Preview {
                    view: parent.downgrade(),
                    item: first,
                    count: items.len(),
                    drag: false,
                    _subscription: Some(cx.observe(&parent, |_, _, cx| cx.notify())),
                });
                menu = menu.bottom_content(preview);
            }
            for label in [
                "Information",
                "Go to album",
                "Go to artist",
                "Go to album artist",
                "Show in file explorer",
                "Set rating",
                "Add to playlist",
                "Download",
                "Share",
                "Edit audio tags",
                "Favorite",
                "Unfavorite",
                "Play now",
                "Play next",
                "Add last",
                "Shuffle play",
                "Shuffle next",
                "Shuffle last",
            ] {
                if (label == "Download" && items.len() > 1)
                    || (label == "Share" && matches!(item.kind, ItemKind::Genre | ItemKind::Radio))
                {
                    continue;
                }
                if (label.starts_with("Go to") && items.len() != 1)
                    || (label == "Share" && !share_supported)
                    || (matches!(label, "Favorite" | "Unfavorite") && !favorite_supported)
                    || (matches!(label, "Edit audio tags") && !editable_tags)
                {
                    continue;
                }
                if label == "Go to album" && item.album_id.is_empty()
                    || label == "Go to artist" && item.artist_id.is_empty()
                    || label == "Go to album artist" && item.album_artist_id.is_empty()
                    || label == "Show in file explorer" && item.path.is_empty()
                {
                    continue;
                }
                let item = item.clone();
                let items = items.clone();
                let view = view.clone();
                if label == "Download"
                    && !matches!(
                        item.kind,
                        ItemKind::Song
                            | ItemKind::Album
                            | ItemKind::Artist
                            | ItemKind::AlbumArtist
                            | ItemKind::Folder
                    )
                    || label == "Add to playlist" && item.kind == ItemKind::Radio
                {
                    continue;
                }
                if label == "Set rating" {
                    if !rating_supported {
                        continue;
                    }
                    if matches!(
                        item.kind,
                        ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                    ) {
                        for rating in 0..=5 {
                            let items = items.clone();
                            let view = view.clone();
                            menu = menu.item(
                                PopupMenuItem::new(crate::appearance::text(
                                    &format!("Rating: {rating}"),
                                    cx,
                                ))
                                .on_click(move |_, _, cx| {
                                    view.update(cx, |this, cx| {
                                        this.set_library_values(
                                            items.clone(),
                                            None,
                                            Some(rating),
                                            cx,
                                        )
                                    })
                                    .ok();
                                }),
                            );
                        }
                    }
                    continue;
                }
                menu = menu.item(
                    PopupMenuItem::new(crate::appearance::text(label, cx)).on_click(
                        move |_, window, cx| {
                            view.update(cx, |this, cx| match label {
                                "Information" => this.open_info(items.clone(), cx),
                                "Share" => this.open_share(items.clone(), window, cx),
                                "Edit audio tags" => this.edit_tag_items(items.clone(), window, cx),
                                "Favorite" | "Unfavorite" => this.set_library_values(
                                    items.clone(),
                                    Some(label == "Favorite"),
                                    None,
                                    cx,
                                ),
                                "Play now" | "Play next" | "Add last" | "Shuffle play"
                                | "Shuffle next" | "Shuffle last" => {
                                    let mode = match label {
                                        "Play next" | "Shuffle next" => Enqueue::Next,
                                        "Add last" | "Shuffle last" => Enqueue::End,
                                        _ => Enqueue::Now,
                                    };
                                    this.enqueue_items(
                                        items.clone(),
                                        mode,
                                        label.starts_with("Shuffle"),
                                        cx,
                                    );
                                }
                                "Show in file explorer" => {
                                    for item in &items {
                                        this.reveal_library_item(item, cx);
                                    }
                                }
                                "Download" => this.download(item.clone(), cx),
                                "Add to playlist" => {
                                    this.open_add_to_playlist(items.clone(), vec![], window, cx);
                                }
                                _ => {
                                    let (id, name, kind) = match label {
                                        "Go to album" => {
                                            (&item.album_id, &item.album, ItemKind::Album)
                                        }
                                        "Go to artist" => {
                                            (&item.artist_id, &item.artist, ItemKind::Artist)
                                        }
                                        _ => (
                                            &item.album_artist_id,
                                            &item.album_artist,
                                            ItemKind::AlbumArtist,
                                        ),
                                    };
                                    this.navigate(
                                        Route::Detail(Box::new(Item {
                                            id: id.clone(),
                                            name: name.clone(),
                                            kind,
                                            server_id: item.server_id,
                                            ..Default::default()
                                        })),
                                        cx,
                                    );
                                }
                            })
                            .ok();
                        },
                    ),
                );
            }
            if playlists {
                menu = menu.separator();
                if items.len() == 1 {
                    let item = items[0].clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::element(|_, cx| {
                            div()
                                .debug_selector(|| "context-edit-playlist".into())
                                .child(crate::appearance::text("action.editPlaylist", cx))
                        })
                        .disabled(!editable_playlists)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| {
                                this.edit_playlist(Some(item.clone()), vec![], window, cx)
                            })
                            .ok();
                        }),
                    );
                }
                let items = items.clone();
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::element(|_, cx| {
                        div()
                            .debug_selector(|| "context-delete-playlist".into())
                            .child(crate::appearance::text("action.deletePlaylist", cx))
                    })
                    .disabled(!editable_playlists)
                    .on_click(move |_, window, cx| {
                        view.update(cx, |this, cx| {
                            this.delete_playlists(items.clone(), window, cx)
                        })
                        .ok();
                    }),
                );
            }
            if items.len() == 1 && matches!(item.kind, ItemKind::Song | ItemKind::Album) {
                let kind = if item.kind == ItemKind::Song {
                    "Track radio"
                } else {
                    "Album radio"
                };
                for (suffix, mode, shuffle) in [
                    (
                        "",
                        play_behavior.mode(),
                        play_behavior == feishin_core::queue::PlayBehavior::Shuffle,
                    ),
                    (" - Play now", Enqueue::Now, false),
                    (" - Play next", Enqueue::Next, false),
                    (" - Add last", Enqueue::End, false),
                ] {
                    let item = item.clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(crate::appearance::text(&format!("{kind}{suffix}"), cx))
                            .on_click(move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.enqueue_related_radio(
                                        item.clone(),
                                        mode,
                                        skip_first_song,
                                        shuffle,
                                        cx,
                                    )
                                })
                                .ok();
                            }),
                    );
                }
            }
            if items.len() == 1 && matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
                for (label, mode, shuffle) in [
                    (
                        "Artist radio",
                        play_behavior.mode(),
                        play_behavior == feishin_core::queue::PlayBehavior::Shuffle,
                    ),
                    (
                        "Artist radio - Play now",
                        feishin_core::queue::Enqueue::Now,
                        false,
                    ),
                    (
                        "Artist radio - Play next",
                        feishin_core::queue::Enqueue::Next,
                        false,
                    ),
                    (
                        "Artist radio - Add last",
                        feishin_core::queue::Enqueue::End,
                        false,
                    ),
                ] {
                    let item = item.clone();
                    let view = view.clone();
                    menu = menu.item(
                        PopupMenuItem::new(crate::appearance::text(label, cx)).on_click(
                            move |_, _, cx| {
                                view.update(cx, |this, cx| {
                                    this.enqueue_artist_radio(item.clone(), mode, shuffle, cx)
                                })
                                .ok();
                            },
                        ),
                    );
                }
            }
            menu
        }
    }
    pub fn queue_menu_builder(
        &self,
        entry: &feishin_core::queue::Entry,
        sidebar: bool,
        cx: &Context<Self>,
    ) -> impl Fn(
        gpui_component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui_component::menu::PopupMenu>,
    ) -> gpui_component::menu::PopupMenu
    + Clone
    + 'static {
        let keys = self
            .state
            .queue
            .order
            .iter()
            .copied()
            .filter(|key| {
                if self.queue_selection(sidebar).0.contains(&entry.key) {
                    self.queue_selection(sidebar).0.contains(key)
                } else {
                    *key == entry.key
                }
            })
            .collect::<Vec<_>>();
        let items = self
            .state
            .queue
            .ordered()
            .into_iter()
            .filter(|entry| keys.contains(&entry.key))
            .map(|entry| entry.item.clone())
            .collect();
        let base = self.item_menu_builder(&entry.item, items, true, cx);
        let view = cx.weak_entity();
        move |menu, window, cx| {
            let mut menu = base(menu, window, cx);
            for label in [
                "Remove from queue",
                "Move to top",
                "Move next",
                "Move to end",
                "Shuffle selected",
                "Shuffle all",
            ] {
                let keys = keys.clone();
                let view = view.clone();
                menu = menu.item(
                    PopupMenuItem::new(crate::appearance::text(label, cx)).on_click(
                        move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                match label {
                                    "Remove from queue" => {
                                        this.remove_queue_entries(&keys, cx);
                                        return;
                                    }
                                    "Shuffle selected" => this.state.queue.shuffle_selected(&keys),
                                    "Shuffle all" => this.state.queue.shuffle_all(),
                                    _ => {
                                        let start = if label == "Move next" {
                                            this.state
                                                .queue
                                                .up_next
                                                .first()
                                                .and_then(|key| {
                                                    this.state
                                                        .queue
                                                        .order
                                                        .iter()
                                                        .position(|id| id == key)
                                                })
                                                .unwrap_or_else(|| {
                                                    this.state
                                                        .queue
                                                        .current
                                                        .and_then(|key| {
                                                            this.state
                                                                .queue
                                                                .order
                                                                .iter()
                                                                .position(|entry| *entry == key)
                                                        })
                                                        .map_or(0, |index| index + 1)
                                                })
                                        } else {
                                            0
                                        };
                                        let before = if label == "Move to end" {
                                            None
                                        } else {
                                            this.state
                                                .queue
                                                .order
                                                .iter()
                                                .skip(start)
                                                .find(|key| !keys.contains(key))
                                                .copied()
                                        };
                                        this.state.queue.move_before(&keys, before);
                                    }
                                }
                                this.arm_next();
                                this.save();
                                cx.notify();
                            })
                            .ok();
                        },
                    ),
                );
            }
            menu
        }
    }
    pub fn library_selection_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let items = self.selected_library_items();
        let Some(first) = items.first() else {
            return div().into_any_element();
        };
        let language = &self.state.settings.language;
        let count = feishin_core::i18n::translate_with(
            language,
            "common.countSelected",
            &serde_json::json!({"count": self.selected.len() + self.selected_album_tracks.len()}),
        )
        .unwrap_or_else(|| format!("{} selected", items.len()));
        let help = [
            ("CTRL + A", "action.selectAll"),
            ("CTRL + Click", "action.addOrRemoveFromSelection"),
            ("SHIFT + Click", "action.selectRangeOfItems"),
        ]
        .map(|(keys, key)| {
            (
                keys,
                feishin_core::i18n::translate(language, key).unwrap_or_else(|| key.into()),
            )
        });
        crate::styles::apply(
            div()
                .id("library-selection-dialog")
                .debug_selector(|| "library-selection-dialog".into())
                .h_flex()
                .items_center()
                .justify_between()
                .gap_6()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .items_center()
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .child(
                            div()
                                .id("selection-help")
                                .debug_selector(|| "selection-help".into())
                                .child(
                                    gpui_component::Icon::default()
                                        .path("native-icons/keyboard.svg"),
                                )
                                .tooltip(move |window, cx| {
                                    let help = help.clone();
                                    gpui_component::tooltip::Tooltip::element(move |_, _| {
                                        div().v_flex().gap_2().children(help.iter().map(
                                            |(keys, label)| {
                                                div()
                                                    .h_flex()
                                                    .gap_3()
                                                    .child(*keys)
                                                    .child(label.clone())
                                            },
                                        ))
                                    })
                                    .build(window, cx)
                                }),
                        )
                        .child(div().font_weight(FontWeight::MEDIUM).text_sm().child(count)),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_1()
                        .child(
                            crate::styles::button("selected-clear", cx)
                                .debug_selector(|| "selected-clear".into())
                                .ghost()
                                .xsmall()
                                .icon(
                                    gpui_component::Icon::default().path("native-icons/close.svg"),
                                )
                                .tooltip(crate::appearance::text("Clear selection", cx))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.selected.clear();
                                    this.selected_album_tracks.clear();
                                    cx.notify();
                                })),
                        )
                        .child(
                            crate::styles::button("selected-more", cx)
                                .debug_selector(|| "selected-more".into())
                                .ghost()
                                .xsmall()
                                .icon(gpui_component::Icon::default().path("native-icons/more.svg"))
                                .tooltip(crate::appearance::text("More", cx))
                                .dropdown_menu(self.item_menu_builder(
                                    first,
                                    items.clone(),
                                    false,
                                    cx,
                                )),
                        ),
                ),
            "div",
            "library-selection-dialog",
            &["fs-selection-dialog-module-selection-indicator"],
            None,
            cx,
        )
        .into_any_element()
    }
    fn information_value(
        &self,
        item: &Item,
        id: &str,
        value: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match id {
            "genres" | "artists" | "albumArtists" | "album" => {
                let (links, kind, selector) = match id {
                    "genres" => (
                        feishin_core::genres::links(item),
                        ItemKind::Genre,
                        "information-genres",
                    ),
                    "album" => (
                        vec![(item.album_id.clone(), item.album.clone())],
                        ItemKind::Album,
                        "information-album",
                    ),
                    _ => (
                        feishin_core::artists::links(item, id == "albumArtists"),
                        ItemKind::AlbumArtist,
                        if id == "artists" {
                            "information-artists"
                        } else {
                            "information-album-artists"
                        },
                    ),
                };
                self.linked_metadata(
                    feishin_core::artists::listed(links, " • "),
                    item.server_id,
                    kind,
                    selector,
                    cx,
                )
            }
            "path" => {
                let path = self.state.settings.resolve_song_path(&item.path);
                let copy_path = path.clone();
                let open_path = path.clone();
                div()
                    .h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        crate::styles::button("information-copy-path", cx)
                            .small()
                            .ghost()
                            .debug_selector(|| "information-copy-path".into())
                            .label(self.tr("page.itemDetail.copyPath"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy_path.clone()))
                            })),
                    )
                    .child(
                        crate::styles::button("information-open-path", cx)
                            .small()
                            .ghost()
                            .label(self.tr("page.itemDetail.openFile"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.open_with_system(std::path::Path::new(&open_path))
                            })),
                    )
                    .child(path)
                    .into_any_element()
            }
            "mbid" => {
                let url = format!(
                    "https://musicbrainz.org/{}/{}",
                    if item.kind == ItemKind::Album {
                        "release"
                    } else {
                        "artist"
                    },
                    item.musicbrainz_id
                );
                crate::styles::button("information-mbid", cx)
                    .ghost()
                    .label(value)
                    .on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
                    .into_any_element()
            }
            "biography" => crate::lyrics_markup::Text(feishin_core::lyric_sources::formatted_html(
                &item.comment,
            ))
            .into_any_element(),
            "comment" => {
                crate::lyrics_markup::Text(feishin_core::information::comment_html(&item.comment))
                    .into_any_element()
            }
            "isCompilation" | "userFavorite" | "smart" => div()
                .id("information-boolean")
                .role(Role::Image)
                .aria_label(value.clone())
                .child(if value == "true" { "✓" } else { "✕" })
                .into_any_element(),
            "userRating" => div()
                .id("information-rating")
                .role(Role::Image)
                .aria_label(format!("{} / 5", item.rating))
                .child(format!(
                    "{}{}",
                    "★".repeat(item.rating.min(5) as usize),
                    "☆".repeat(5 - item.rating.min(5) as usize)
                ))
                .into_any_element(),
            _ => value.into_any_element(),
        }
    }
    pub fn info_view(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("item-information")
            .debug_selector(|| "item-information".into())
            .native_vertical_scroll()
            .v_flex()
            .gap_4()
            .h_full()
            .child(
                crate::styles::button("refresh-info", cx)
                    .label(if self.info_loading {
                        "Loading information..."
                    } else {
                        "Refresh information"
                    })
                    .disabled(self.info_loading)
                    .on_click(cx.listener(|this, _, _, cx| this.reload_info(cx))),
            )
            .children(self.info_items.iter().enumerate().map(|(index, item)| {
                div()
                    .id(("information-item", index))
                    .v_flex()
                    .gap_2()
                    .child(div().text_xl().child(item.name.clone()))
                    .children(
                        feishin_core::information::fields(item.kind)
                            .iter()
                            .filter_map(|field| {
                                let id = field.0;
                                let value = feishin_core::information::text(
                                    item,
                                    id,
                                    &self.state.settings.language,
                                );
                                if value.is_empty() {
                                    return None;
                                }
                                Some(
                                    div()
                                        .id(SharedString::from(format!("information-field-{id}")))
                                        .h_flex()
                                        .when(crate::appearance::rtl(cx), |div| {
                                            div.flex_row_reverse()
                                        })
                                        .gap_3()
                                        .debug_selector(move || {
                                            format!("information-field-{index}-{id}")
                                        })
                                        .child(div().w(px(180.)).child(
                                            feishin_core::information::label(
                                                field,
                                                &self.state.settings.language,
                                            ),
                                        ))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .child(self.information_value(item, id, value, cx)),
                                        ),
                                )
                            }),
                    )
                    .when(!item.participants.is_empty(), |div| {
                        div.child(
                            gpui::div()
                                .h_flex()
                                .gap_3()
                                .child(
                                    gpui::div()
                                        .w(px(180.))
                                        .child(self.tr("common.additionalParticipants")),
                                )
                                .child(item.participants.len().to_string()),
                        )
                        .children(item.participants.iter().map(|(role, artists)| {
                            let mut label = role.chars();
                            let label = label.next().map_or_else(String::new, |first| {
                                first.to_uppercase().collect::<String>() + label.as_str()
                            });
                            gpui::div()
                                .id(SharedString::from(format!("participant-{role}")))
                                .h_flex()
                                .gap_3()
                                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                .child(gpui::div().w(px(180.)).child(label))
                                .child(
                                    gpui::div().flex_1().min_w_0().child(
                                        self.named_artists(
                                            artists
                                                .iter()
                                                .map(|(id, name)| {
                                                    (
                                                        id.clone(),
                                                        if name.is_empty() {
                                                            "-".into()
                                                        } else {
                                                            name.clone()
                                                        },
                                                    )
                                                })
                                                .collect(),
                                            item.server_id,
                                            " • ",
                                            "information-participants",
                                            cx,
                                        ),
                                    ),
                                )
                        }))
                    })
                    .when(
                        matches!(item.kind, ItemKind::Song | ItemKind::Album)
                            && !item.tags.is_empty(),
                        |div| {
                            div.child(
                                gpui::div()
                                    .h_flex()
                                    .gap_3()
                                    .child(gpui::div().w(px(180.)).child(self.tr("common.tags")))
                                    .child(item.tags.len().to_string()),
                            )
                            .children(item.tags.iter().map(
                                |(tag, values)| {
                                    let mut chars = tag.chars();
                                    let label = chars.next().map_or_else(String::new, |first| {
                                        format!("{}{}", first.to_uppercase(), chars.as_str())
                                    });
                                    gpui::div()
                                        .h_flex()
                                        .gap_3()
                                        .when(crate::appearance::rtl(cx), |row| {
                                            row.flex_row_reverse()
                                        })
                                        .child(gpui::div().w(px(180.)).child(label))
                                        .child(if values.is_empty() {
                                            gpui::div()
                                                .id(SharedString::from(format!(
                                                    "information-tag-{tag}"
                                                )))
                                                .role(Role::Image)
                                                .aria_label("true")
                                                .child("✓")
                                                .into_any_element()
                                        } else {
                                            values.join(" • ").into_any_element()
                                        })
                                },
                            ))
                        },
                    )
                    .child(
                        crate::styles::button(("copy-item-info", index), cx)
                            .label(crate::appearance::text("Copy information", cx))
                            .on_click({
                                let item = item.clone();
                                cx.listener(move |_, _, _, cx| {
                                    if let Ok(text) = serde_json::to_string_pretty(&item) {
                                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                                    }
                                })
                            }),
                    )
            }))
            .into_any_element()
    }
}
