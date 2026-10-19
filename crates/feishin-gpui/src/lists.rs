use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    lists::{self, Align, Config, Size},
    model::{ArtistTab, Item, ItemKind, Sort},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, Sizable, StyledExt, button::ButtonVariants,
    input::InputState,
};

pub struct Editor {
    pub key: String,
    pub config: Config,
    pub base_table: Option<lists::Table>,
    pub page_size: Entity<InputState>,
    pub grid_count: Entity<InputState>,
    pub widths: Vec<Entity<InputState>>,
    pub searches: [Entity<InputState>; 2],
    pub queries: [String; 2],
    search_tasks: [Option<Task<()>>; 2],
    _subscriptions: Vec<Subscription>,
}

#[derive(Clone)]
pub(crate) struct EditorDrag {
    pub editor: EntityId,
    pub id: String,
    pub grid: bool,
}

impl Render for EditorDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .bg(cx.theme().secondary)
            .rounded_md()
            .child(column_label(&self.id, cx))
    }
}

impl Editor {
    pub(crate) fn reorder(&mut self, drag: &EditorDrag, target: &str, bottom: bool) {
        if drag.editor != self.searches[0].entity_id() {
            return;
        }
        let ids = if drag.grid {
            self.config
                .grid
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>()
        } else {
            self.config
                .table
                .columns
                .iter()
                .map(|column| column.id.as_str())
                .collect::<Vec<_>>()
        };
        let (Some(from), Some(to)) = (
            ids.iter().position(|id| *id == drag.id),
            ids.iter().position(|id| *id == target),
        ) else {
            return;
        };
        let destination = if bottom && from > to { to + 1 } else { to };
        if from == destination {
            return;
        }
        if drag.grid {
            let row = self.config.grid.rows.remove(from);
            self.config.grid.rows.insert(destination, row);
        } else {
            let column = self.config.table.columns.remove(from);
            self.config.table.columns.insert(destination, column);
            let width = self.widths.remove(from);
            self.widths.insert(destination, width);
        }
    }
}

fn search_outline(query: &str, label: &str, cx: &App) -> AnyElement {
    let matched = lists::config_search_matches(query, label);
    let color = cx.theme().primary;
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            if matched {
                let mut outline = outline(bounds.dilate(px(4.)), color, BorderStyle::Solid);
                outline.border_widths = px(2.).into();
                outline.corner_radii = px(6.).into();
                window.paint_quad(outline);
            }
        },
    )
    .absolute()
    .size_full()
    .into_any_element()
}

fn size_label(size: Size) -> &'static str {
    match size {
        Size::Compact => "table.config.general.size_compact",
        Size::Default => "table.config.general.size_default",
        Size::Large => "table.config.general.size_large",
    }
}

fn align_label(align: Align) -> &'static str {
    match align {
        Align::Start => "table.config.general.alignLeft",
        Align::Center => "table.config.general.alignCenter",
        Align::End => "table.config.general.alignRight",
    }
}

fn column_label(id: &str, cx: &App) -> String {
    let key = format!(
        "table.config.label.{}",
        match id {
            "name" => "title",
            "artists" => "artist",
            "albumArtists" => "albumArtist",
            "genres" => "genre",
            "userFavorite" => "favorite",
            "userRating" => "rating",
            "lastPlayedAt" => "lastPlayed",
            "createdAt" => "dateAdded",
            "imageUrl" => "image",
            "username" => "owner",
            "bitRate" => "bitrate",
            "container" => "codec",
            _ => id,
        }
    );
    let label = crate::appearance::text(&key, cx);
    if label == key {
        crate::appearance::text(lists::label(id), cx)
    } else {
        label
    }
}

#[derive(Debug, PartialEq)]
enum HeaderLabel {
    Key(String),
    Icon(&'static str),
    Literal(&'static str),
}

fn header_label(id: &str) -> HeaderLabel {
    let key = match id {
        "actions" => return HeaderLabel::Icon("more"),
        "discNumber" => return HeaderLabel::Icon("disc"),
        "duration" => return HeaderLabel::Icon("duration"),
        "playlistReorder" => return HeaderLabel::Icon("drag-vertical"),
        "rowIndex" => return HeaderLabel::Icon("hash"),
        "trackNumber" => return HeaderLabel::Icon("song"),
        "userFavorite" => return HeaderLabel::Icon("favorite"),
        "userRating" => return HeaderLabel::Icon("star"),
        "albumGroup" | "imageUrl" | "__layoutFill" | "skip" => {
            return HeaderLabel::Literal("");
        }
        "id" => return HeaderLabel::Literal("ID"),
        "composer" => return HeaderLabel::Key("table.config.label.composer".into()),
        "name" | "titleArtist" | "titleCombined" => "title",
        "artists" => "artist",
        "albumArtists" => "albumArtist",
        "genres" | "genreBadge" => "genre",
        "bitRate" => "bitrate",
        "container" => "codec",
        "createdAt" => "dateAdded",
        "lastPlayedAt" => "lastPlayed",
        "username" => "owner",
        id => id,
    };
    HeaderLabel::Key(format!("table.column.{key}"))
}

pub(crate) fn table_header(column: &lists::Column, cx: &App) -> Div {
    let content = match header_label(&column.id) {
        HeaderLabel::Icon(icon) => gpui_component::Icon::default()
            .path(format!("native-icons/{icon}.svg"))
            .with_size(cx.theme().font_size * (13. / 16.))
            .into_any_element(),
        HeaderLabel::Key(key) => {
            let label = crate::appearance::text(&key, cx);
            div()
                .truncate()
                .child(if label == key {
                    column_label(&column.id, cx).to_uppercase()
                } else {
                    label.to_uppercase()
                })
                .into_any_element()
        }
        HeaderLabel::Literal(label) => div().child(label).into_any_element(),
    };
    div()
        .h_flex()
        .size_full()
        .min_w_0()
        .overflow_hidden()
        .text_size(cx.theme().font_size * (13. / 16.))
        .font_weight(FontWeight::NORMAL)
        .when(column.align == Align::Center, |cell| cell.justify_center())
        .when(column.align == Align::End, |cell| cell.justify_end())
        .child(content)
}
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TableRow {
    Library,
    Related,
    ArtistSection(String),
    AlbumTrack(String),
    Queue { key: uuid::Uuid, sidebar: bool },
}

pub fn select_row<K: Clone + Eq + std::hash::Hash>(
    selected: &mut std::collections::HashSet<K>,
    order: &mut Vec<K>,
    key: K,
    modifiers: Modifiers,
    range: impl FnOnce(&K, &K) -> Vec<K>,
) {
    let only = selected.len() == 1 && selected.contains(&key);
    order.retain(|key| selected.contains(key));
    if modifiers.control || modifiers.platform {
        if selected.insert(key.clone()) {
            order.push(key);
        } else {
            selected.remove(&key);
            order.retain(|entry| *entry != key);
        }
    } else if modifiers.shift {
        let keys = order
            .last()
            .map_or_else(|| vec![key.clone()], |anchor| range(anchor, &key));
        for key in keys {
            if selected.insert(key.clone()) {
                order.push(key);
            }
        }
    } else {
        selected.clear();
        order.clear();
        if !only {
            selected.insert(key.clone());
            order.push(key);
        }
    }
}

pub fn toggle_all_rows<K: Clone + Eq + std::hash::Hash>(
    selected: &mut std::collections::HashSet<K>,
    order: &mut Vec<K>,
    rows: Vec<K>,
) {
    if selected.len() == rows.len() {
        selected.clear();
        order.clear();
    } else {
        *selected = rows.iter().cloned().collect();
        *order = rows;
    }
}

pub fn selection_range<K: Clone + Eq>(rows: &[K], anchor: &K, key: &K) -> Vec<K> {
    let Some(start) = rows.iter().position(|row| row == anchor) else {
        return vec![];
    };
    let Some(end) = rows.iter().position(|row| row == key) else {
        return vec![];
    };
    rows[start.min(end)..=start.max(end)].to_vec()
}
#[derive(Clone)]
pub struct ColumnDrag {
    pub key: String,
    pub id: String,
    pub resize: bool,
    pub width: f32,
}
impl Render for ColumnDrag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .bg(cx.theme().secondary)
            .rounded_md()
            .child(lists::label(&self.id).to_owned())
    }
}

fn kind_key(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Album => "album",
        ItemKind::Artist => "artist",
        ItemKind::AlbumArtist => "albumArtist",
        ItemKind::Playlist => "playlist",
        ItemKind::Genre => "genre",
        ItemKind::Folder => "folder",
        ItemKind::Radio => "radio",
        _ => "song",
    }
}
fn next_size(size: Size) -> Size {
    match size {
        Size::Compact => Size::Default,
        Size::Default => Size::Large,
        Size::Large => Size::Compact,
    }
}
fn next_align(align: Align) -> Align {
    match align {
        Align::Start => Align::Center,
        Align::Center => Align::End,
        Align::End => Align::Start,
    }
}
pub(crate) fn row_play_context(
    items: &[Item],
    index: usize,
    single: bool,
    bounded: bool,
) -> Vec<Item> {
    let tracks = items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.kind == ItemKind::Song)
        .collect::<Vec<_>>();
    let Some(position) = tracks.iter().position(|(source, _)| *source == index) else {
        return Vec::new();
    };
    let range = if single {
        position..position + 1
    } else if bounded {
        position.saturating_sub(50)..(position + 51).min(tracks.len())
    } else {
        0..tracks.len()
    };
    tracks[range]
        .iter()
        .map(|(_, item)| (*item).clone())
        .collect()
}

impl Feishin {
    pub fn hover_table_row(
        &mut self,
        row: TableRow,
        index: usize,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        let target = (row, index);
        let next = if hovered {
            Some(target)
        } else if self.hovered_table_row.as_ref() == Some(&target) {
            None
        } else {
            return;
        };
        if next != self.hovered_table_row {
            self.hovered_table_row = next;
            cx.notify();
        }
    }

    pub fn select_album_track(
        &mut self,
        album: &str,
        index: usize,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let rows = self.album_track_rows();
        let key = (album.to_owned(), index);
        if !rows.contains(&key) {
            return;
        }
        self.selected.clear();
        self.grid_selection_order.clear();
        if modifiers.shift
            && !modifiers.control
            && !modifiers.platform
            && self
                .album_selection_order
                .last()
                .is_some_and(|anchor| !rows.contains(anchor))
        {
            self.selected_album_tracks.clear();
            self.album_selection_order.clear();
        }
        select_row(
            &mut self.selected_album_tracks,
            &mut self.album_selection_order,
            key,
            modifiers,
            |anchor, key| {
                let range = selection_range(&rows, anchor, key);
                if range.is_empty() {
                    vec![key.clone()]
                } else {
                    range
                }
            },
        );
        cx.notify();
    }

    pub fn album_track_rows(&self) -> Vec<(String, usize)> {
        self.items
            .iter()
            .flat_map(|album| {
                let key = Self::album_key(album);
                let count = self
                    .album_tracks
                    .get(&key)
                    .map_or(0, |tracks| tracks.items.len());
                (0..count).map(move |index| (key.clone(), index))
            })
            .collect()
    }

    fn album_track_click(
        &mut self,
        album: &str,
        index: usize,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if event.click_count() != 2 {
            self.select_album_track(album, index, event.modifiers(), cx);
            return;
        }
        let rows = self.album_track_rows();
        let key = (album.to_owned(), index);
        let Some(position) = rows.iter().position(|row| row == &key) else {
            return;
        };
        let tracks = rows
            .iter()
            .map(|(key, index)| self.album_tracks[key].items[*index].clone())
            .collect::<Vec<_>>();
        let item = tracks[position].clone();
        self.selected.clear();
        self.grid_selection_order.clear();
        self.selected_album_tracks = [key.clone()].into();
        self.album_selection_order = vec![key];
        let tracks = row_play_context(
            &tracks,
            position,
            false,
            !matches!(&self.route, Route::Detail(parent) if parent.kind == ItemKind::Playlist),
        );
        self.add_tracks_from(tracks, feishin_core::queue::Enqueue::Now, Some(&item), cx);
        cx.notify();
    }

    pub fn play_table_context(
        &mut self,
        index: usize,
        mode: feishin_core::queue::Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if item.kind != ItemKind::Song {
            return;
        }
        let bounded = !self.folder_route()
            && !matches!(&self.route, Route::Detail(parent) if matches!(parent.kind, ItemKind::Album | ItemKind::Playlist | ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Folder));
        let single = bounded
            && matches!(
                mode,
                feishin_core::queue::Enqueue::Next | feishin_core::queue::Enqueue::End
            );
        let mut tracks = row_play_context(&self.items, index, single, bounded);
        if shuffle {
            use rand::seq::SliceRandom;
            tracks.shuffle(&mut rand::thread_rng());
        }
        self.add_tracks_from(tracks, mode, Some(&item), cx);
    }

    pub fn table_item_click(&mut self, index: usize, event: &ClickEvent, cx: &mut Context<Self>) {
        if self.folder_route() {
            self.folder_table_click(index, event, cx);
            return;
        }
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if event.click_count() == 2 {
            self.selected.clear();
            self.selected.insert(index);
            self.grid_selection_order = vec![index];
            if item.kind == ItemKind::Song {
                self.play_table_context(index, feishin_core::queue::Enqueue::Now, false, cx);
            } else {
                self.navigate(Route::Detail(Box::new(item)), cx);
            }
        } else {
            self.select_library_index(index, event.modifiers(), cx);
        }
        cx.notify();
    }

    pub fn select_library_index(
        &mut self,
        index: usize,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        select_row(
            &mut self.selected,
            &mut self.grid_selection_order,
            index,
            modifiers,
            |anchor, index| (*anchor.min(index)..=*anchor.max(index)).collect(),
        );
        cx.notify();
    }

    pub fn list_key(&self) -> String {
        if self.is_full_player_view() {
            return "fullScreenSong".into();
        }
        match &self.route {
            Route::Queue => "queueSong",
            Route::Library(ItemKind::Folder) => "song",
            Route::Library(kind) | Route::Favorites(kind) | Route::Search(_, kind) => {
                kind_key(*kind)
            }
            Route::Collection(collection) => kind_key(collection.kind),
            Route::Detail(item) => match item.kind {
                ItemKind::Folder => "song",
                ItemKind::Album => "albumDetail",
                ItemKind::Playlist => {
                    if self.playlist_album_mode() {
                        "playlistAlbum"
                    } else {
                        "playlistSong"
                    }
                }
                ItemKind::Genre => {
                    if self.state.settings.genre_albums {
                        "genreAlbum"
                    } else {
                        "genreSong"
                    }
                }
                ItemKind::Artist | ItemKind::AlbumArtist => {
                    if self.artist_tab == ArtistTab::Albums {
                        "albumArtistAlbum"
                    } else {
                        match self.artist_tab {
                            feishin_core::model::ArtistTab::Favorites
                            | feishin_core::model::ArtistTab::Rated => "albumArtistFavoriteSong",
                            feishin_core::model::ArtistTab::PersonalTop => "albumArtistTopSong",
                            feishin_core::model::ArtistTab::CommunityTop => {
                                "albumArtistCommunitySong"
                            }
                            feishin_core::model::ArtistTab::Similar => "albumArtistSimilar",
                            _ => "albumArtistSong",
                        }
                    }
                }
                _ => "folder",
            },
            Route::Home => "album",
            Route::PlaylistPicker => "playlist",
            _ => "song",
        }
        .into()
    }
    pub fn list_config(&self) -> Config {
        let key = self.list_key();
        let mut config = lists::configured(&self.state.settings.lists, &key);
        if self.folder_route() || key == "fullScreenSong" {
            config.display = "table".into();
            config.pagination = "infinite".into();
        }
        if key == "playlistSong" && self.playlist_tracks_editing() {
            config.display = "table".into();
            config.pagination = "infinite".into();
            config.table.enable_header = true;
            config
                .table
                .columns
                .retain(|column| !matches!(column.id.as_str(), "playlistReorder" | "albumGroup"));
            config.table.columns.insert(
                0,
                lists::Column {
                    id: "playlistReorder".into(),
                    width: 100.,
                    pinned: Some("left".into()),
                    align: Align::Center,
                    ..Default::default()
                },
            );
        }
        config
    }
    pub fn edit_list_config(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        let config = lists::configured(&self.state.settings.lists, &key);
        self.prepare_list_editor(key.clone(), config, window, cx);
        self.navigate(Route::ListSettings(key), cx);
    }
    pub fn reset_list_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.list_editor {
            let key = editor.key.clone();
            self.prepare_list_editor(key.clone(), lists::default_for(&key), window, cx);
            cx.notify();
        }
    }
    pub(crate) fn prepare_list_editor(
        &mut self,
        key: String,
        mut config: Config,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let base_table = (config.display == "detail").then(|| config.table.clone());
        if config.display == "detail"
            && let Some(detail) = &config.detail
        {
            config.table = detail.clone();
        }
        for column in lists::default_columns() {
            if !config.table.columns.iter().any(|old| old.id == column.id) {
                config.table.columns.push(lists::Column {
                    is_enabled: false,
                    ..column
                });
            }
        }
        for (id, _) in lists::COLUMNS {
            if !config.grid.rows.iter().any(|old| old.id == *id) {
                config.grid.rows.push(lists::GridRow {
                    id: (*id).into(),
                    is_enabled: false,
                    ..Default::default()
                });
            }
        }
        let mut make_input = |value: String, label: &str, cx: &mut Context<Self>| {
            let input = cx
                .new(|cx| crate::appearance::input_state(window, cx).aria_label(label.to_owned()));
            input.update(cx, |input, cx| input.set_value(value, window, cx));
            input
        };
        let page_size = make_input(
            config.items_per_page.to_string(),
            "table.config.general.pagination_itemsPerPage",
            cx,
        );
        let grid_count = make_input(
            config.grid.items_per_row.to_string(),
            "table.config.general.itemsPerRow",
            cx,
        );
        let widths: Vec<_> = config
            .table
            .columns
            .iter()
            .map(|column| {
                cx.new(|cx| {
                    crate::appearance::input_state(window, cx)
                        .aria_label(column.id.clone())
                        .label_transform(|id, cx| format!("{} (px)", column_label(id, cx)).into())
                        .default_value(column.width.to_string())
                })
            })
            .collect();
        let searches = ["common.tableColumns", "common.gridRows"].map(|section| {
            cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .placeholder("common.search")
                    .aria_label(section)
                    .label_transform(|key, cx| {
                        if key == "common.search" {
                            crate::appearance::text(key, cx).into()
                        } else {
                            format!(
                                "{}: {}",
                                crate::appearance::text("common.search", cx),
                                crate::appearance::text(key, cx)
                            )
                            .into()
                        }
                    })
            })
        });
        let mut subscriptions: Vec<_> = searches
            .iter()
            .enumerate()
            .map(|(index, input)| {
                cx.subscribe(input, move |this, input, event, cx| {
                    if !matches!(event, gpui_component::input::InputEvent::Change) {
                        return;
                    }
                    let Some(editor) = &mut this.list_editor else {
                        return;
                    };
                    if editor.searches[index] != input {
                        return;
                    }
                    let query = input.read(cx).value().to_string();
                    let timer = cx
                        .background_executor()
                        .timer(std::time::Duration::from_millis(300));
                    editor.search_tasks[index] = Some(cx.spawn(async move |this, cx| {
                        timer.await;
                        this.update(cx, |this, cx| {
                            if let Some(editor) = &mut this.list_editor
                                && editor.searches[index] == input
                            {
                                editor.queries[index] = query;
                                cx.notify();
                            }
                        })
                        .ok();
                    }));
                })
            })
            .collect();
        for input in &widths {
            subscriptions.extend(Self::bind_number_step(input, 0., 2000., 10., window, cx));
        }
        self.list_editor = Some(Editor {
            key: key.clone(),
            config,
            base_table,
            page_size,
            grid_count,
            widths,
            searches,
            queries: Default::default(),
            search_tasks: Default::default(),
            _subscriptions: subscriptions,
        });
    }
    pub fn save_list_config(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.list_editor else {
            return;
        };
        let parsed = (|| -> anyhow::Result<Config> {
            let mut config = editor.config.clone();
            config.items_per_page = editor.page_size.read(cx).value().trim().parse()?;
            config.grid.items_per_row = editor.grid_count.read(cx).value().trim().parse()?;
            for (column, width) in config.table.columns.iter_mut().zip(&editor.widths) {
                let value = width.read(cx).value();
                let value = if value.trim().is_empty() {
                    0.
                } else {
                    value.trim().parse::<f32>()?
                };
                anyhow::ensure!(value.is_finite(), "Invalid column width");
                column.width = value.clamp(0., 2000.);
            }
            if config.display == "detail" {
                config.detail = Some(config.table.clone());
                if let Some(base) = &editor.base_table {
                    config.table = base.clone();
                }
            }
            lists::validate(&std::collections::BTreeMap::from([(
                editor.key.clone(),
                config.clone(),
            )]))?;
            Ok(config)
        })();
        match parsed {
            Ok(config) => {
                self.state.settings.lists.insert(editor.key.clone(), config);
                self.save();
                if self.full_player {
                    cx.notify();
                } else {
                    self.back(cx);
                }
            }
            Err(error) => {
                self.message = format!("Invalid list settings: {error}");
                cx.notify();
            }
        }
    }
    pub fn change_list_page(&mut self, page: usize, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.navigation_restore = None;
        self.library_page = page;
        if self.scrolling_detail_header() {
            self.album_details_scroll.reset(0);
        }
        self.library_scroll
            .scroll_to_item_strict(0, ScrollStrategy::Top);
        self.load_library(false, cx);
    }
    pub fn list_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.list_editor else {
            return div().into_any_element();
        };
        let config = &editor.config;
        div()
            .id("list-settings")
            .debug_selector(|| "list-settings".into())
            .on_drag_move(cx.listener(|_, _: &DragMoveEvent<EditorDrag>, _, cx| cx.notify()))
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(div().text_lg().child(self.tr("common.configure")))
            .child(
                crate::styles::button("save-list-config", cx)
                    .debug_selector(|| "save-list-config".into())
                    .primary()
                    .label(self.tr("common.save"))
                    .on_click(cx.listener(|this, _, _, cx| this.save_list_config(cx))),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .flex_wrap()
                    .child(
                        crate::styles::button("list-display", cx)
                            .disabled(matches!(
                                editor.key.as_str(),
                                "queueSong" | "sideQueue" | "fullScreenSong"
                            ))
                            .label(format!(
                                "{}: {}",
                                self.tr("table.config.general.displayType"),
                                self.tr(match config.display.as_str() {
                                    "poster" => "table.config.view.grid",
                                    "list" => "table.config.view.list",
                                    "detail" => "table.config.view.detail",
                                    _ => "table.config.view.table",
                                })
                            ))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(editor) = &mut this.list_editor {
                                    editor.config.display = match editor.config.display.as_str() {
                                        "table" => "poster",
                                        "poster" => "list",
                                        "list" => "detail",
                                        _ => "table",
                                    }
                                    .into();
                                }
                                cx.notify();
                            })),
                    )
                    .when(editor.key != "fullScreenSong", |container| {
                        container
                            .child(
                                crate::styles::button("list-pagination", cx)
                                    .label(format!(
                                        "{}: {}",
                                        self.tr("table.config.general.pagination"),
                                        self.tr(if config.pagination == "infinite" {
                                            "table.config.general.pagination_infinite"
                                        } else {
                                            "table.config.general.pagination_paginate"
                                        })
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.pagination =
                                                if editor.config.pagination == "infinite" {
                                                    "paginated"
                                                } else {
                                                    "infinite"
                                                }
                                                .into();
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .child(self.tr("table.config.general.pagination_itemsPerPage"))
                                    .child(crate::styles::input(&editor.page_size, cx)),
                            )
                    }),
            )
            .child(div().text_lg().child(self.tr("table.config.view.table")))
            .child(
                crate::styles::button("list-row-size", cx)
                    .label(format!(
                        "{}: {}",
                        self.tr("table.config.general.size"),
                        self.tr(size_label(config.table.size))
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(editor) = &mut this.list_editor {
                            editor.config.table.size = next_size(editor.config.table.size);
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .children(
                        [
                            (
                                "header",
                                config.table.enable_header,
                                "table.config.general.showHeader",
                            ),
                            (
                                "fit",
                                config.table.auto_fit_columns,
                                "table.config.general.autoFitColumns",
                            ),
                            (
                                "alternate",
                                config.table.enable_alternate_row_colors,
                                "table.config.general.alternateRowColors",
                            ),
                            (
                                "horizontal",
                                config.table.enable_horizontal_borders,
                                "table.config.general.horizontalBorders",
                            ),
                            (
                                "vertical",
                                config.table.enable_vertical_borders,
                                "table.config.general.verticalBorders",
                            ),
                            (
                                "hover",
                                config.table.enable_row_hover_highlight,
                                "table.config.general.rowHoverHighlight",
                            ),
                        ]
                        .into_iter()
                        .map(|(id, enabled, label)| {
                            self.setting_checkbox(id, label, enabled, cx)
                                .debug_selector(move || format!("list-setting-{id}"))
                                .on_click(cx.listener(move |this, value: &bool, _, cx| {
                                    if let Some(editor) = &mut this.list_editor {
                                        let table = &mut editor.config.table;
                                        let target = match id {
                                            "header" => &mut table.enable_header,
                                            "fit" => &mut table.auto_fit_columns,
                                            "alternate" => &mut table.enable_alternate_row_colors,
                                            "horizontal" => &mut table.enable_horizontal_borders,
                                            "vertical" => &mut table.enable_vertical_borders,
                                            _ => &mut table.enable_row_hover_highlight,
                                        };
                                        *target = *value;
                                    }
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .justify_between()
                    .child(self.tr("common.tableColumns"))
                    .child(
                        div()
                            .w_64()
                            .debug_selector(|| "list-column-search".into())
                            .child(crate::styles::input(&editor.searches[0], cx)),
                    ),
            )
            .children(
                config
                    .table
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        self.editor_row(&column.id, false, index, cx)
                            .debug_selector(move || format!("list-column-row-{index}"))
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .items_center()
                            .child(search_outline(
                                &editor.queries[0],
                                &column_label(&column.id, cx),
                                cx,
                            ))
                            .child(
                                self.setting_checkbox(
                                    ("list-column-enabled", index),
                                    &column_label(&column.id, cx),
                                    column.is_enabled,
                                    cx,
                                )
                                .debug_selector(move || format!("list-column-enabled-{index}"))
                                .w(px(180.))
                                .on_click(cx.listener(
                                    move |this, value: &bool, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            let column = &mut editor.config.table.columns[index];
                                            column.is_enabled = *value;
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .w(px(140.))
                                    .child(crate::styles::number_input(&editor.widths[index], cx)),
                            )
                            .child(
                                crate::styles::button(("list-column-align", index), cx)
                                    .label(self.tr(align_label(column.align)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            let column = &mut editor.config.table.columns[index];
                                            column.align = next_align(column.align);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                self.setting_checkbox(
                                    ("list-column-auto", index),
                                    "table.config.general.autosize",
                                    column.auto_size,
                                    cx,
                                )
                                .debug_selector(move || format!("list-column-auto-{index}"))
                                .on_click(cx.listener(
                                    move |this, value: &bool, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            let column = &mut editor.config.table.columns[index];
                                            column.auto_size = *value;
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .children(
                                [
                                    ("left", "⇤", "table.config.general.pinToLeft"),
                                    ("right", "⇥", "table.config.general.pinToRight"),
                                ]
                                .into_iter()
                                .map(|(side, glyph, key)| {
                                    crate::styles::button(
                                        (
                                            if side == "left" {
                                                "list-column-pin-left"
                                            } else {
                                                "list-column-pin-right"
                                            },
                                            index,
                                        ),
                                        cx,
                                    )
                                    .debug_selector(move || {
                                        format!("list-column-pin-{side}-{index}")
                                    })
                                    .label(glyph)
                                    .aria_label(self.tr(key))
                                    .tooltip(self.tr(key))
                                    .selected(column.pinned.as_deref() == Some(side))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if let Some(editor) = &mut this.list_editor {
                                                let column =
                                                    &mut editor.config.table.columns[index];
                                                column.pinned = (column.pinned.as_deref()
                                                    != Some(side))
                                                .then(|| side.into());
                                            }
                                            cx.notify();
                                        },
                                    ))
                                }),
                            )
                            .child(
                                crate::styles::button(("list-column-up", index), cx)
                                    .debug_selector(move || format!("list-column-up-{index}"))
                                    .label("↑")
                                    .aria_label(self.tr("table.config.general.moveUp"))
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.table.columns.swap(index, index - 1);
                                            editor.widths.swap(index, index - 1);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("list-column-down", index), cx)
                                    .debug_selector(move || format!("list-column-down-{index}"))
                                    .label("↓")
                                    .aria_label(self.tr("table.config.general.moveDown"))
                                    .disabled(index + 1 == config.table.columns.len())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.table.columns.swap(index, index + 1);
                                            editor.widths.swap(index, index + 1);
                                        }
                                        cx.notify();
                                    })),
                            )
                    }),
            )
            .when(editor.key != "fullScreenSong", |container| {
                container
                    .child(div().text_lg().child(self.tr("table.config.view.grid")))
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_3()
                            .flex_wrap()
                            .child(
                                crate::styles::button("list-card-size", cx)
                                    .label(format!(
                                        "{}: {}",
                                        self.tr("table.config.general.size"),
                                        self.tr(size_label(config.grid.size))
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.grid.size =
                                                next_size(editor.config.grid.size);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button("list-card-gap", cx)
                                    .label(format!(
                                        "{}: {}",
                                        self.tr("table.config.general.gap"),
                                        config.grid.item_gap
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.grid.item_gap =
                                                match editor.config.grid.item_gap.as_str() {
                                                    "xs" => "sm",
                                                    "sm" => "md",
                                                    "md" => "lg",
                                                    "lg" => "xl",
                                                    _ => "xs",
                                                }
                                                .into();
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                self.setting_checkbox(
                                    "list-card-count",
                                    "table.config.general.itemsPerRow",
                                    config.grid.items_per_row_enabled,
                                    cx,
                                )
                                .debug_selector(|| "list-card-count".into())
                                .on_click(cx.listener(
                                    |this, value: &bool, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.grid.items_per_row_enabled = *value;
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                div()
                                    .child(self.tr("table.config.general.itemsPerRow"))
                                    .child(crate::styles::input(&editor.grid_count, cx)),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .justify_between()
                            .child(self.tr("common.gridRows"))
                            .child(
                                div()
                                    .w_64()
                                    .debug_selector(|| "list-card-search".into())
                                    .child(crate::styles::input(&editor.searches[1], cx)),
                            ),
                    )
                    .children(config.grid.rows.iter().enumerate().map(|(index, row)| {
                        self.editor_row(&row.id, true, index, cx)
                            .debug_selector(move || format!("card-row-{index}"))
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(search_outline(
                                &editor.queries[1],
                                &column_label(&row.id, cx),
                                cx,
                            ))
                            .child(
                                self.setting_checkbox(
                                    ("card-row-enabled", index),
                                    &column_label(&row.id, cx),
                                    row.is_enabled,
                                    cx,
                                )
                                .debug_selector(move || format!("card-row-enabled-{index}"))
                                .w(px(200.))
                                .on_click(cx.listener(
                                    move |this, value: &bool, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            let row = &mut editor.config.grid.rows[index];
                                            row.is_enabled = *value;
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                crate::styles::button(("card-row-align", index), cx)
                                    .label(self.tr(align_label(row.align)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            let row = &mut editor.config.grid.rows[index];
                                            row.align = next_align(row.align);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("card-row-up", index), cx)
                                    .debug_selector(move || format!("card-row-up-{index}"))
                                    .label("↑")
                                    .aria_label(self.tr("table.config.general.moveUp"))
                                    .disabled(index == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.grid.rows.swap(index, index - 1);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                crate::styles::button(("card-row-down", index), cx)
                                    .debug_selector(move || format!("card-row-down-{index}"))
                                    .label("↓")
                                    .aria_label(self.tr("table.config.general.moveDown"))
                                    .disabled(index + 1 == config.grid.rows.len())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.list_editor {
                                            editor.config.grid.rows.swap(index, index + 1);
                                        }
                                        cx.notify();
                                    })),
                            )
                    }))
            })
            .child(
                crate::styles::button("reset-list-config", cx)
                    .label(self.tr("common.resetToDefault"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.reset_list_config(window, cx);
                    })),
            )
            .into_any_element()
    }
    fn editor_row(&self, id: &str, grid: bool, index: usize, cx: &Context<Self>) -> Stateful<Div> {
        let editor = self.list_editor.as_ref().unwrap().searches[0].entity_id();
        let id = id.to_owned();
        let drag = EditorDrag {
            editor,
            id: id.clone(),
            grid,
        };
        let target = id.clone();
        let view = cx.weak_entity();
        let prefix = if grid { "card-row" } else { "list-column" };
        div()
            .id((prefix, index))
            .relative()
            .when(
                cx.active_drag::<EditorDrag>().is_some_and(|drag| {
                    drag.editor == editor && drag.grid == grid && drag.id == id
                }),
                |row| row.opacity(0.5),
            )
            .on_drop_with_bounds(move |drag: &EditorDrag, bounds, window, cx| {
                if drag.grid == grid {
                    let bottom = window.mouse_position().y > bounds.center().y;
                    view.update(cx, |view, cx| {
                        if let Some(editor) = &mut view.list_editor {
                            editor.reorder(drag, &target, bottom);
                            cx.notify();
                        }
                        cx.stop_propagation();
                    })
                    .ok();
                }
            })
            .child(
                div()
                    .id(("editor-drag", index))
                    .debug_selector(move || format!("{prefix}-drag-{index}"))
                    .cursor(CursorStyle::OpenHand)
                    .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
                    .child("⠿"),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        if bounds.contains(&window.mouse_position())
                            && cx.active_drag::<EditorDrag>().is_some_and(|drag| {
                                drag.editor == editor && drag.grid == grid && drag.id != id
                            })
                        {
                            let bottom = window.mouse_position().y > bounds.center().y;
                            let y = if bottom {
                                bounds.bottom() - px(2.)
                            } else {
                                bounds.top()
                            };
                            window.paint_quad(fill(
                                Bounds::new(
                                    point(bounds.left(), y),
                                    size(bounds.size.width, px(2.)),
                                ),
                                cx.theme().primary,
                            ));
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
    }
    pub fn configured_columns(&self) -> Vec<lists::Column> {
        let config = self.list_config();
        let table = if config.display == "detail" {
            config.detail.as_ref().unwrap_or(&config.table)
        } else {
            &config.table
        };
        let mut columns = table
            .columns
            .iter()
            .filter(|column| {
                column.is_enabled
                    && (column.id != "albumGroup"
                        || !(self.grouped_song_albums()
                            || (self.main_queue_visible()
                                && crate::queue::album_grouping(&config))))
                    && !(self.playlist_album_mode()
                        && matches!(column.id.as_str(), "userFavorite" | "userRating"))
            })
            .cloned()
            .collect::<Vec<_>>();
        columns.sort_by_key(|column| match column.pinned.as_deref() {
            Some("left") => 0,
            Some("right") => 2,
            _ => 1,
        });
        columns
    }
    pub(crate) fn column_strip(
        &self,
        id: String,
        mut cells: Vec<(lists::Column, AnyElement)>,
        table: &lists::Table,
        detail: bool,
        scroll: &ScrollHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let auto_fit = table.auto_fit_columns;
        let zoom = self.state.settings.zoom;
        let rtl = crate::appearance::rtl(cx);
        let padding = f32::from(cx.theme().font_size) / zoom;
        let scroll = scroll.clone();
        let on_scroll = cx.listener(|_, _: &ScrollWheelEvent, _, cx| cx.notify());
        let key = if id.starts_with("side-queue") {
            "sideQueue".to_owned()
        } else if id.starts_with("artist-") {
            "song".to_owned()
        } else {
            self.list_key()
        };
        let preview = self
            .column_resize_preview
            .clone()
            .filter(|(list, _, _)| *list == key);
        canvas(
            move |bounds, window, cx| {
                let total = f64::from(bounds.size.width) / f64::from(zoom);
                let base = cells
                    .iter()
                    .map(|(column, _)| column.clone())
                    .collect::<Vec<_>>();
                let pinned = base
                    .iter()
                    .filter(|column| column.pinned.is_some())
                    .map(|column| f64::from(column.width))
                    .sum::<f64>();
                let mut columns = if detail {
                    let mut columns = base;
                    let fixed = columns
                        .iter()
                        .filter_map(|column| lists::detail_fixed_width(&column.id))
                        .sum::<f32>();
                    let flexible = columns
                        .iter()
                        .filter(|column| lists::detail_fixed_width(&column.id).is_none())
                        .collect::<Vec<_>>();
                    let reference = flexible.iter().map(|column| column.width).sum::<f32>();
                    let count = flexible.len();
                    let base_total = columns.iter().map(|column| column.width).sum::<f32>();
                    let flex_sum = if base_total <= 0. {
                        100. * count as f32 / columns.len().max(1) as f32
                    } else {
                        100. * reference / base_total
                    };
                    let extra =
                        (total as f32 - fixed - padding * count as f32).max(0.) * flex_sum.min(1.);
                    for column in &mut columns {
                        column.width = lists::detail_fixed_width(&column.id).unwrap_or_else(|| {
                            padding
                                + extra
                                    * if reference > 0. {
                                        column.width / reference
                                    } else if base_total <= 0. {
                                        1. / count.max(1) as f32
                                    } else {
                                        0.
                                    }
                        });
                        column.pinned = None;
                    }
                    columns
                } else {
                    lists::layout_columns(&base, auto_fit, total, (total - pinned).max(0.))
                };
                if !detail
                    && let Some((_, id, width)) = &preview
                    && let Some(column) = columns.iter_mut().find(|column| column.id == *id)
                {
                    column.width = *width;
                }
                let mut left = Vec::new();
                let mut middle = Vec::new();
                let mut right = Vec::new();
                let mut widths = [0.; 3];
                for column in columns {
                    let width = column.width.max(0.) * zoom;
                    let pane = match column.pinned.as_deref() {
                        Some("left") => 0,
                        Some("right") => 2,
                        _ => 1,
                    };
                    widths[pane] += width;
                    let cell = cells
                        .iter()
                        .position(|(base, _)| base.id == column.id)
                        .map(|index| cells.remove(index).1);
                    let element = div()
                        .w(px(width))
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .children(cell);
                    match pane {
                        0 => left.push(element),
                        2 => right.push(element),
                        _ => middle.push(element),
                    };
                }
                let pane = |children, width| {
                    div()
                        .h_flex()
                        .when(rtl, |div| div.flex_row_reverse())
                        .h_full()
                        .w(px(width))
                        .flex_shrink_0()
                        .children(children)
                };
                let mut element = div()
                    .h_flex()
                    .when(rtl, |div| div.flex_row_reverse())
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .when(!left.is_empty(), |div| div.child(pane(left, widths[0])))
                    .child(
                        div()
                            .id(SharedString::from(id))
                            .h_full()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .when(rtl, |div| div.justify_end())
                            .overflow_x_scroll()
                            .track_scroll(&scroll)
                            .on_scroll_wheel(on_scroll)
                            .child(pane(middle, widths[1])),
                    )
                    .when(!right.is_empty(), |div| div.child(pane(right, widths[2])))
                    .into_any_element();
                element.prepaint_as_root(
                    bounds.origin,
                    bounds.size.map(AvailableSpace::Definite),
                    window,
                    cx,
                );
                element
            },
            |_, mut element, window, cx| element.paint(window, cx),
        )
        .h_full()
        .flex_1()
        .min_w_0()
        .into_any_element()
    }
    pub fn queue_rows(&self) -> Vec<feishin_core::queue::Entry> {
        let ordered = self.state.queue.ordered();
        let mut entries = lists::search_items(
            ordered.iter().map(|entry| &entry.item),
            &self.local_filter,
            ItemKind::Song,
        )
        .into_iter()
        .map(|index| ordered[index].clone())
        .collect::<Vec<_>>();
        if let Some((column, descending)) = &self.queue_display_sort {
            entries.sort_by(|a, b| {
                let order = lists::compare(&a.item, &b.item, column);
                if *descending { order.reverse() } else { order }
            });
            if *descending && column == "id" {
                entries.reverse();
            }
        }
        entries
    }
    fn column_key_active(&self, key: &str) -> bool {
        key == self.list_key()
            || key == "sideQueue"
            || (key == "song" && self.artist_overview_active())
    }
    pub fn resize_column(&mut self, drag: &ColumnDrag, x: Pixels, cx: &mut Context<Self>) {
        if !drag.resize || !self.column_key_active(&drag.key) {
            return;
        }
        let origin = self
            .column_resize_origin
            .get_or_insert_with(|| (drag.key.clone(), drag.id.clone(), x, drag.width));
        if origin.0 != drag.key || origin.1 != drag.id {
            *origin = (drag.key.clone(), drag.id.clone(), x, drag.width);
        }
        let delta = f32::from(x - origin.2) / self.state.settings.zoom;
        let delta = if crate::appearance::rtl(cx) {
            -delta
        } else {
            delta
        };
        if delta.is_finite() {
            self.column_resize_preview = Some((
                drag.key.clone(),
                drag.id.clone(),
                lists::resized_column_width(origin.3, delta),
            ));
            cx.notify();
        }
    }
    pub fn drop_column(&mut self, drag: &ColumnDrag, before: Option<&str>, cx: &mut Context<Self>) {
        if !self.column_key_active(&drag.key) {
            return;
        }
        self.column_resize_origin = None;
        let artist_table = drag.key == "song" && self.artist_overview_active();
        let mut config = if drag.key == "sideQueue" || artist_table {
            lists::configured(&self.state.settings.lists, &drag.key)
        } else {
            self.list_config()
        };
        let table = if config.display == "detail" && !artist_table {
            config.detail.get_or_insert_with(|| config.table.clone())
        } else {
            &mut config.table
        };
        if drag.resize {
            if let Some((key, id, width)) = self.column_resize_preview.take()
                && key == drag.key
                && id == drag.id
                && let Some(column) = table.columns.iter_mut().find(|column| column.id == id)
                && !column.auto_size
            {
                column.width = width;
                self.state.settings.lists.insert(key, config);
                self.save();
                cx.notify();
            }
            return;
        }
        let Some(before) = before else {
            return;
        };
        if drag.id == before {
            return;
        }
        if let (Some(from), Some(to)) = (
            table.columns.iter().position(|column| column.id == drag.id),
            table.columns.iter().position(|column| column.id == before),
        ) {
            let column = table.columns.remove(from);
            table.columns.insert(to - usize::from(from < to), column);
            self.state.settings.lists.insert(drag.key.clone(), config);
            self.save();
            cx.notify();
        }
    }
    pub fn column_drag_controls(
        &self,
        column: &lists::Column,
        content: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.column_drag_controls_for(
            &self.list_key(),
            column,
            content,
            self.list_config().display == "detail",
            cx,
        )
    }

    pub fn column_drag_controls_for(
        &self,
        key: &str,
        column: &lists::Column,
        content: AnyElement,
        detail: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if key == "playlistSong" && self.playlist_tracks_editing() {
            return content;
        }
        let zoom = self.state.settings.zoom;
        let measured = std::rc::Rc::new(std::cell::Cell::new(column.width));
        let measure = measured.clone();
        let resize_enabled = !column.auto_size
            && column.id != "__layoutFill"
            && (!detail || lists::detail_fixed_width(&column.id).is_none());
        let prefix = if key == "sideQueue" {
            "side-queue"
        } else {
            "metadata"
        };
        let resize_id = column.id.clone();
        let id = column.id.clone();
        let drag = ColumnDrag {
            key: key.to_owned(),
            id: id.clone(),
            resize: false,
            width: column.width,
        };
        let resize_key = drag.key.clone();
        let resize_column_id = drag.id.clone();
        let resize = ColumnDrag {
            resize: true,
            ..drag.clone()
        };
        div()
            .id(SharedString::from(format!("{prefix}-header-{id}")))
            .role(gpui::Role::ColumnHeader)
            .aria_label(self.tr(lists::label(&column.id)))
            .debug_selector({
                let id = id.clone();
                move || format!("{prefix}-header-{id}")
            })
            .relative()
            .w_full()
            .h_full()
            .on_drag(drag, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
            .on_drop(cx.listener(move |this, drag: &ColumnDrag, _, cx| {
                this.drop_column(drag, Some(&id), cx);
                cx.stop_propagation();
            }))
            .child(content)
            .child(
                canvas(
                    move |bounds, _, _| {
                        if !detail {
                            measure.set(f32::from(bounds.size.width) / zoom);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when(resize_enabled, |div| {
                div.child(
                    gpui::div()
                        .id("column-resize-handle")
                        .debug_selector(move || format!("{prefix}-resize-{resize_id}"))
                        .absolute()
                        .when(crate::appearance::rtl(cx), |div| div.left_0())
                        .when(!crate::appearance::rtl(cx), |div| div.right_0())
                        .top_0()
                        .bottom_0()
                        .w(px(6. * zoom))
                        .cursor(CursorStyle::ResizeLeftRight)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                this.column_resize_origin = Some((
                                    resize_key.clone(),
                                    resize_column_id.clone(),
                                    event.position.x,
                                    measured.get(),
                                ));
                                cx.stop_propagation();
                            }),
                        )
                        .on_drag(resize, |dragged, _, _, cx| {
                            cx.stop_propagation();
                            cx.new(|_| dragged.clone())
                        }),
                )
            })
            .into_any_element()
    }
    pub fn configured_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = self.list_config();
        let cells = self
            .configured_columns()
            .into_iter()
            .map(|column| {
                let cell = div().w_full().h_full().min_w_0().child(
                    crate::styles::button(SharedString::from(format!("header-{}", column.id)), cx)
                        .ghost()
                        .w_full()
                        .h_full()
                        .px_0()
                        .aria_label(column_label(&column.id, cx))
                        .map(|button| match column.align {
                            Align::Start => button.justify_start(),
                            Align::Center => button.justify_center(),
                            Align::End => button.justify_end(),
                        })
                        .child(table_header(&column, cx).w_auto().h_auto())
                        .on_click({
                            let id = column.id.clone();
                            cx.listener(move |this, _, _, cx| {
                                if this.main_queue_visible() {
                                    let descending = this
                                        .queue_display_sort
                                        .as_ref()
                                        .is_some_and(|(old, descending)| old == &id && !descending);
                                    this.queue_display_sort = Some((id.clone(), descending));
                                    cx.notify();
                                    return;
                                }
                                let sort = Sort::from_column(&id);
                                if let Some(sort) = sort {
                                    this.descending = this.sort == sort && !this.descending;
                                    this.sort = sort;
                                    this.reload(false, cx);
                                }
                            })
                        }),
                );
                let cell = self.column_drag_controls(&column, cell.into_any_element(), cx);
                (column, cell)
            })
            .collect();
        let zoom = self.state.settings.zoom;
        let gap = f32::from(cx.theme().font_size) * 0.75;
        let song_action = usize::from(
            self.items
                .first()
                .is_some_and(|item| item.kind == ItemKind::Song),
        );
        let action_count: usize = if self.main_queue_visible() {
            4
        } else if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist) {
            2 + song_action
        } else {
            song_action
        };
        div()
            .id("metadata-header")
            .debug_selector(|| "metadata-header".into())
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .w_full()
            .h(px(40. * zoom))
            .flex_shrink_0()
            .gap_3()
            .px_3()
            .bg(cx.theme().secondary)
            .child(div().w(px(32. * zoom)).flex_shrink_0())
            .child(self.column_strip(
                "header-scroll".into(),
                cells,
                &config.table,
                false,
                &self.list_horizontal_scroll,
                cx,
            ))
            .when(action_count > 0, |div| {
                div.child(
                    gpui::div()
                        .w(px(action_count as f32 * 32. * zoom
                            + action_count.saturating_sub(1) as f32 * gap))
                        .flex_shrink_0(),
                )
            })
            .into_any_element()
    }

    pub fn metadata_columns(
        &self,
        item: &Item,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = self.list_config();
        self.metadata_cells(
            item,
            index,
            &config,
            self.configured_columns(),
            TableRow::Library,
            cx,
        )
    }
    pub(crate) fn play_title_item(
        &mut self,
        item: &Item,
        row: &TableRow,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if let TableRow::Queue { key, .. } = row {
            self.play_queue_entry(*key, cx);
        } else if item.kind == ItemKind::Song {
            self.add_tracks_from(
                vec![item.clone()],
                self.state.settings.play_button_behavior.mode(),
                Some(item),
                cx,
            );
        } else if shuffle {
            self.shuffle_library_item(item.clone(), cx);
        } else {
            self.play_library_item(item.clone(), cx);
        }
    }
    fn title_credits_column(
        &self,
        item: &Item,
        column: &lists::Column,
        config: &Config,
        row: &TableRow,
        grouped: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let artwork = column.id == "titleCombined" && !grouped;
        let album = item.kind == ItemKind::Album;
        let cover = config.table.size.row_height() * self.state.settings.zoom
            - f32::from(cx.theme().font_size) * 0.5;
        let lines = if config.display == "detail" {
            1
        } else {
            match config.table.size {
                Size::Compact => 1,
                Size::Default => 2,
                Size::Large => 3,
            }
        };
        let title = crate::ui::track_title(item, !matches!(row, TableRow::Queue { .. }), cx)
            .when(column.id == "titleCombined", |title| {
                title.inherit_subtitle_line_height()
            });
        div()
            .h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .when(artwork, |div| {
                div.child(crate::row_play::ArtworkPlay {
                    parent_hovered: None,
                    id: format!(
                        "title-credit-artwork-{row:?}-{}-{}",
                        item.server_id, item.id
                    )
                    .into(),
                    selector: format!("title-credit-artwork-{}", item.id).into(),
                    button_selector: format!("title-credit-play-{}", item.id).into(),
                    item: item.clone(),
                    row: row.clone(),
                    compact: config.table.size == Size::Compact,
                    width: Some(px(cover)),
                    height: px(cover),
                    content: self.artwork(item, cover, cx),
                    app: cx.weak_entity(),
                })
            })
            .child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .when(config.table.size != Size::Compact, |div| div.gap_1())
                    .when(column.align == Align::Center, |div| div.text_center())
                    .when(column.align == Align::End, |div| div.text_right())
                    .child(crate::ui::item_title_with_lines(
                        item,
                        title,
                        column.align,
                        Some(lines),
                        cx,
                    ))
                    .child(
                        div()
                            .debug_selector(|| "title-credit-artists".into())
                            .text_size(rems(11. / 16.))
                            .line_height(rems(1.))
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(self.joined_artists(item, album, "title-credit-artists", cx)),
                    ),
            )
            .into_any_element()
    }
    pub fn metadata_cells(
        &self,
        item: &Item,
        index: usize,
        config: &Config,
        columns: Vec<lists::Column>,
        row: TableRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row_hovered = self.hovered_table_row.as_ref() == Some(&(row.clone(), index));
        let selectable = !matches!(row, TableRow::Library) || self.route != Route::PlaylistPicker;
        let grouped = config
            .table
            .columns
            .iter()
            .any(|column| column.id == "albumGroup" && column.is_enabled);
        let row_play_column = crate::row_play::first_number_column(&columns).map(str::to_owned);
        let cells = columns
            .into_iter()
            .map(|column| {
                let value = feishin_core::metadata_format::column_text(
                    item,
                    &column.id,
                    index,
                    config.display == "detail",
                    &self.state.settings.language,
                );
                let mut cell = div()
                    .id(SharedString::from(format!(
                        "metadata-cell-{index}-{}",
                        column.id
                    )))
                    .debug_selector({
                        let id = column.id.clone();
                        let row = row.clone();
                        move || match &row {
                            TableRow::Library => format!("metadata-cell-{index}-{id}"),
                            TableRow::Related => format!("related-cell-{index}-{id}"),
                            TableRow::ArtistSection(section) => {
                                format!("artist-cell-{section}-{index}-{id}")
                            }
                            TableRow::AlbumTrack(album) => {
                                format!("album-track-cell-{album}-{index}-{id}")
                            }
                            TableRow::Queue { key, sidebar } => {
                                format!("queue-cell-{sidebar}-{key}-{id}")
                            }
                        }
                    })
                    .h_full()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .overflow_hidden()
                    .w_full()
                    .min_w_0()
                    .when(config.display != "detail", |div| div.py_1())
                    .when(
                        !matches!(
                            column.id.as_str(),
                            "actions" | "userFavorite" | "userRating" | "__layoutFill"
                        ),
                        |div| div.px_2(),
                    )
                    .when(column.align == Align::Center, |div| div.justify_center())
                    .when(column.align == Align::End, |div| div.justify_end())
                    .when(config.table.enable_vertical_borders, |div| {
                        div.border_r_1().border_color(cx.theme().border)
                    });
                if matches!(column.id.as_str(), "rowIndex" | "trackNumber")
                    && row_play_column.as_deref() == Some(column.id.as_str())
                {
                    let active = crate::row_play::active(item, &row, self.state.queue.current());
                    let caption = if column.id == "rowIndex" {
                        let display_index = if let TableRow::ArtistSection(section) = &row {
                            self.artist_section_indices(section, cx)
                                .iter()
                                .position(|source| *source == index)
                                .unwrap_or(index)
                        } else {
                            index
                        };
                        (display_index + 1).to_string()
                    } else {
                        value.clone()
                    };
                    if crate::row_play::supports(item, &row)
                        && (column.id == "rowIndex" || item.kind == ItemKind::Song)
                    {
                        cell = cell.child(crate::row_play::RowPlay {
                            id: format!(
                                "row-play-target-{row:?}-{index}-{}-{}-{}",
                                column.id, item.server_id, item.id
                            )
                            .into(),
                            item: item.clone(),
                            caption: caption.into(),
                            active,
                            playing: self.playing,
                            enabled: self.library_fetches.is_empty(),
                            app: cx.weak_entity(),
                        });
                    } else if active {
                        cell = cell.child(
                            gpui_component::Icon::default()
                                .path(if self.playing {
                                    "native-icons/play.svg"
                                } else {
                                    "native-icons/pause.svg"
                                })
                                .text_color(cx.theme().primary),
                        );
                    } else {
                        cell = cell.child(caption);
                    }
                } else if column.id == "imageUrl" {
                    let compact = config.table.size == Size::Compact;
                    let height = px(config.table.size.row_height() * self.state.settings.zoom)
                        - cx.theme().font_size * 0.5;
                    let content = div()
                        .h(height)
                        .map(|div| if compact { div.w_full() } else { div.w(height) })
                        .rounded(cx.theme().font_size * 0.3125)
                        .overflow_hidden()
                        .bg(cx.theme().secondary)
                        .child(self.image_content_with_fit(
                            item,
                            feishin_core::images::Placement::Table,
                            compact,
                            cx,
                        ));
                    cell = cell.child(crate::row_play::ArtworkPlay {
                        parent_hovered: None,
                        id: format!(
                            "image-column-{row:?}-{index}-{}-{}",
                            item.server_id, item.id
                        )
                        .into(),
                        selector: format!("image-column-artwork-{}", item.id).into(),
                        button_selector: format!("image-column-play-{}", item.id).into(),
                        item: item.clone(),
                        row: row.clone(),
                        compact,
                        width: None,
                        height,
                        content: content.into_any_element(),
                        app: cx.weak_entity(),
                    });
                } else if column.id == "actions" {
                    cell = cell.child(if let TableRow::ArtistSection(section) = &row {
                        self.artist_section_menu(section, item, index, cx)
                    } else if matches!(row, TableRow::Related) {
                        self.related_item_menu(item, index, cx)
                    } else {
                        self.item_actions(item, index, cx)
                    });
                } else if column.id == "playlistReorder" && matches!(row, TableRow::Library) {
                    cell = cell.child(self.playlist_reorder_controls(item, index, cx));
                } else if column.id == "userFavorite"
                    && matches!(
                        item.kind,
                        ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                    )
                {
                    let item = item.clone();
                    cell = cell.child(
                        crate::styles::button(("column-favorite", index), cx)
                            .debug_selector(move || format!("column-favorite-{index}"))
                            .disabled(self.favorite_requests > 0)
                            .when(!item.favorite && !row_hovered, |button| button.invisible())
                            .ghost()
                            .label(if item.favorite { "♥" } else { "♡" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_favorite(item.clone(), cx);
                                cx.stop_propagation();
                            })),
                    );
                } else if column.id == "userRating"
                    && self.sessions.get(&item.server_id).is_some_and(|api| {
                        api.server.kind != feishin_core::model::ServerType::Jellyfin
                    })
                {
                    cell = cell.child(
                        div()
                            .when(item.rating == 0 && !row_hovered, |div| div.invisible())
                            .child(
                                self.rating(
                                    if let TableRow::ArtistSection(section) = &row {
                                        format!("overview-song-rating-{section}-{index}")
                                    } else {
                                        format!("column-rating-{index}")
                                    },
                                    item,
                                    cx,
                                )
                                .read_only(self.rating_requests > 0),
                            ),
                    );
                } else {
                    cell = cell.child(
                        if matches!(column.id.as_str(), "titleArtist" | "titleCombined") {
                            self.title_credits_column(item, &column, config, &row, grouped, cx)
                        } else if column.id == "name" {
                            crate::ui::item_title_with_lines(
                                item,
                                crate::ui::track_title(
                                    item,
                                    !matches!(row, TableRow::Queue { .. }),
                                    cx,
                                ),
                                column.align,
                                Some(if config.display == "detail" {
                                    1
                                } else {
                                    match config.table.size {
                                        Size::Compact => 1,
                                        Size::Default => 3,
                                        Size::Large => 4,
                                    }
                                }),
                                cx,
                            )
                            .into_any_element()
                        } else if matches!(column.id.as_str(), "genres" | "genreBadge") {
                            self.genre_metadata(
                                item,
                                column.id == "genreBadge",
                                config.display == "detail",
                                "table-genres",
                                cx,
                            )
                        } else if column.id == "composer"
                            && config.display != "detail"
                            && item.participants.contains_key("composer")
                        {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(self.artist_text(
                                    feishin_core::artists::joined(
                                        "",
                                        &item.participants["composer"],
                                        false,
                                    ),
                                    item.server_id,
                                    "table-composers",
                                    cx,
                                ))
                                .into_any_element()
                        } else if matches!(column.id.as_str(), "artists" | "albumArtists") {
                            div()
                                .truncate()
                                .child(self.joined_artists(
                                    item,
                                    column.id == "albumArtists",
                                    "table-artists",
                                    cx,
                                ))
                                .into_any_element()
                        } else {
                            div().truncate().child(value).into_any_element()
                        },
                    );
                    if matches!(
                        column.id.as_str(),
                        "name" | "titleArtist" | "titleCombined" | "album"
                    ) && !(selectable
                        && matches!(item.kind, ItemKind::Song | ItemKind::Folder)
                        && matches!(column.id.as_str(), "name" | "titleArtist" | "titleCombined"))
                    {
                        let item = item.clone();
                        let id = column.id.clone();
                        cell = cell.cursor_pointer().on_click(cx.listener(
                            move |this, _, window, cx| {
                                let target = match id.as_str() {
                                    "album" if !item.album_id.is_empty() => Some(Item {
                                        id: item.album_id.clone(),
                                        name: item.album.clone(),
                                        kind: ItemKind::Album,
                                        ..item.clone()
                                    }),
                                    _ => None,
                                };
                                if this.route == Route::PlaylistPicker {
                                    this.open_add_to_playlist(
                                        this.form_songs.clone(),
                                        vec![item.clone()],
                                        window,
                                        cx,
                                    );
                                } else if let Some(target) = target {
                                    this.navigate(Route::Detail(Box::new(target)), cx);
                                } else if matches!(item.kind, ItemKind::Song | ItemKind::Radio) {
                                    this.play_library_item(item.clone(), cx);
                                } else {
                                    this.navigate(Route::Detail(Box::new(item.clone())), cx);
                                }
                            },
                        ));
                    }
                }
                if selectable
                    && (!matches!(column.id.as_str(), "name" | "titleArtist" | "titleCombined")
                        || matches!(item.kind, ItemKind::Song | ItemKind::Folder))
                    && !matches!(
                        column.id.as_str(),
                        "actions"
                            | "userFavorite"
                            | "userRating"
                            | "artists"
                            | "albumArtists"
                            | "genres"
                            | "genreBadge"
                            | "album"
                    )
                {
                    let row = row.clone();
                    cell =
                        cell.on_click(cx.listener(
                            move |view, event: &ClickEvent, _, cx| match &row {
                                TableRow::Library => view.table_item_click(index, event, cx),
                                TableRow::Related => view.related_row_click(index, event, cx),
                                TableRow::ArtistSection(section) => {
                                    view.artist_section_click(section, index, event, cx)
                                }
                                TableRow::AlbumTrack(album) => {
                                    view.album_track_click(album, index, event, cx)
                                }
                                TableRow::Queue { key, sidebar } => {
                                    view.queue_row_click(*key, *sidebar, event, cx)
                                }
                            },
                        ));
                }
                (column, cell.into_any_element())
            })
            .collect();
        self.column_strip(
            if matches!(row, TableRow::Queue { sidebar: true, .. }) {
                format!("side-queue-metadata-scroll-{index}")
            } else if let TableRow::ArtistSection(section) = &row {
                format!("artist-metadata-scroll-{section}-{index}")
            } else if matches!(row, TableRow::Related) {
                format!("related-metadata-scroll-{index}")
            } else {
                format!("metadata-scroll-{index}")
            },
            cells,
            &config.table,
            config.display == "detail",
            if matches!(row, TableRow::Queue { sidebar: true, .. }) {
                &self.side_queue_horizontal_scroll
            } else if let TableRow::ArtistSection(section) = &row {
                self.artist_sections
                    .get(section)
                    .map_or(&self.list_horizontal_scroll, |state| {
                        &state.horizontal_scroll
                    })
            } else if matches!(row, TableRow::Related) {
                &self.player_related.horizontal_scroll
            } else {
                &self.list_horizontal_scroll
            },
            cx,
        )
    }
}

impl Feishin {
    pub fn show_playing_song(&mut self, cx: &mut Context<Self>) {
        if self.main_queue_visible() {
            self.scroll_queue_to_current();
        }
        if self.side_queue_visible() {
            self.scroll_side_queue_to_current(cx);
        }
        if !self.full_player
            && matches!(
                self.route,
                Route::Library(_)
                    | Route::Collection(_)
                    | Route::Favorites(_)
                    | Route::Search(..)
                    | Route::Detail(_)
                    | Route::Similar(_)
                    | Route::Explore
            )
            && !self.artist_overview_active()
            && self.list_config().display != "grid"
            && let Some(current) = self.current_entry().map(|entry| entry.item.clone())
        {
            let matches = |item: &Item| {
                item.kind == ItemKind::Song
                    && item.id == current.id
                    && item.server_id == current.server_id
            };
            let target = if let Some(index) = self.items.iter().position(matches) {
                let row = if self.scrolling_detail_header() {
                    self.detail_row_position(index)
                } else if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album)
                {
                    self.album_page_rows(1).iter().position(|row| {
                        matches!(row,
                        crate::details::AlbumPageRow::Tracks(indices) if indices.contains(&index))
                    })
                } else if self.grouped_song_albums() {
                    feishin_core::album_metadata::groups(&self.items)
                        .iter()
                        .position(|range| range.contains(&index))
                } else {
                    self.library_scroll
                        .scroll_to_item_strict(index, ScrollStrategy::Center);
                    None
                };
                row.map(|row| (row, TableRow::Library, index))
            } else if self.list_config().display == "detail" {
                self.items.iter().enumerate().find_map(|(index, album)| {
                    let key = Self::album_key(album);
                    let track = self
                        .album_tracks
                        .get(&key)?
                        .items
                        .iter()
                        .position(matches)?;
                    let row = if self.scrolling_detail_header() {
                        self.detail_row_position(index)
                    } else {
                        self.album_group_rows(1).map_or(Some(index), |rows| {
                            rows.iter().position(|row| row.indexes.contains(&index))
                        })
                    }?;
                    Some((row, TableRow::AlbumTrack(key), track))
                })
            } else {
                None
            };
            if let Some((row, source, index)) = target {
                self.track_scroll_target = Some((
                    source,
                    index,
                    self.album_details_scroll.viewport_bounds().size.height,
                ));
                self.album_details_scroll.scroll_to(ListOffset {
                    item_ix: row,
                    offset_in_item: px(0.),
                });
            }
        }
        cx.notify();
    }

    // A grouped row can contain an entire album. Use the actual track bounds
    // after layout so metadata height, font settings and column headers count.
    pub fn track_scroll_marker(&self, cx: &Context<Self>) -> AnyElement {
        let view = cx.entity().downgrade();
        let target = self.track_scroll_target.clone();
        let height = target.as_ref().map_or(px(0.), |(_, _, height)| *height);
        canvas(
            move |bounds, window, cx| {
                window.request_autoscroll(Bounds::new(
                    point(bounds.left(), bounds.center().y - height / 2.),
                    size(bounds.size.width, height),
                ));
                let view = view.clone();
                let target = target.clone();
                window.defer(cx, move |_, cx| {
                    view.update(cx, |view, cx| {
                        if view.track_scroll_target == target {
                            view.track_scroll_target = None;
                            cx.notify();
                        }
                    })
                    .ok();
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::row_play_context;
    use crate::app::{Feishin, Route};
    use feishin_core::{
        model::{Item, ItemKind},
        storage::State,
    };
    use gpui::{
        AppContext, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollStrategy,
        TestAppContext, px, size,
    };
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn table_header_labels_match_original_shared_column_map() {
        use super::{HeaderLabel, header_label};
        use gpui::AssetSource;
        let reference: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(include_str!("../assets/table-header-reference.json")).unwrap();
        for (id, expected) in reference {
            let actual = header_label(&id);
            if let Some(key) = expected["key"].as_str() {
                assert_eq!(actual, HeaderLabel::Key(key.into()), "{id}");
                assert_eq!(expected["uppercase"], true);
            } else if let Some(icon) = expected["icon"].as_str() {
                let icon = match icon {
                    "ellipsisHorizontal" => "more",
                    "dragVertical" => "drag-vertical",
                    "itemSong" => "song",
                    icon => icon,
                };
                let HeaderLabel::Icon(actual) = actual else {
                    panic!("{id}: {actual:?}");
                };
                assert_eq!(actual, icon, "{id}");
                assert!(
                    crate::icons::Assets
                        .load(&format!("native-icons/{icon}.svg"))
                        .unwrap()
                        .is_some()
                );
            } else {
                let HeaderLabel::Literal(actual) = actual else {
                    panic!("{id}: {actual:?}");
                };
                assert_eq!(actual, expected.as_str().unwrap(), "{id}");
            }
        }
    }

    #[test]
    fn row_play_context_matches_original_bounded_track_windows() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-playback-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let mut items = vec![Item {
                kind: ItemKind::Folder,
                ..Default::default()
            }];
            items.extend((0..case["count"].as_u64().unwrap()).map(|index| Item {
                id: index.to_string(),
                kind: ItemKind::Song,
                ..Default::default()
            }));
            let bounded = case["scope"] == "list";
            let single = case["single"].as_bool().unwrap()
                || (bounded && matches!(case["mode"].as_str(), Some("next" | "last")));
            let tracks = row_play_context(
                &items,
                case["index"].as_u64().unwrap() as usize + 1,
                single,
                bounded,
            );
            assert_eq!(
                serde_json::to_value(tracks.iter().map(|item| &item.id).collect::<Vec<_>>())
                    .unwrap(),
                case["ids"],
                "{case}"
            );
        }
        let duplicates = [4, 5].map(|position| Item {
            id: "duplicate".into(),
            kind: ItemKind::Song,
            playlist_position: Some(position),
            ..Default::default()
        });
        assert_eq!(
            row_play_context(&duplicates, 1, true, true)[0].playlist_position,
            Some(5)
        );
    }

    #[gpui::test]
    fn table_text_selects_and_double_click_plays_bounded_context(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-table-controls-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            state.settings.play_button_behavior = feishin_core::queue::PlayBehavior::Next;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1400.), px(1000.)));
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Song);
                view.items = (0..151)
                    .map(|index| Item {
                        id: index.to_string(),
                        name: format!("Track {index}"),
                        kind: ItemKind::Song,
                        ..Default::default()
                    })
                    .collect();
                view.library_scroll.scroll_to_item(55, ScrollStrategy::Top);
                cx.notify();
            })
        });
        cx.run_until_parked();
        let point = cx
            .debug_bounds("metadata-cell-55-titleCombined")
            .unwrap()
            .center();
        cx.simulate_click(point, Modifiers::none());
        assert!(app.read_with(cx, |view, _| view.selected.contains(&55)
            && view.state.queue.entries.is_empty()));
        let control = cx
            .debug_bounds("metadata-cell-57-titleCombined")
            .unwrap()
            .center();
        cx.simulate_click(
            control,
            Modifiers {
                control: true,
                ..Modifiers::none()
            },
        );
        let range = cx
            .debug_bounds("metadata-cell-56-titleCombined")
            .unwrap()
            .center();
        cx.simulate_click(
            range,
            Modifiers {
                shift: true,
                ..Modifiers::none()
            },
        );
        assert_eq!(
            app.read_with(cx, |view, _| view.grid_selection_order.clone()),
            [55, 57, 56]
        );
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.item.id.clone())
                    .collect::<Vec<_>>(),
                (5..=105).map(|index| index.to_string()).collect::<Vec<_>>()
            );
            assert_eq!(view.state.queue.current().unwrap().item.id, "55");
            assert_eq!(view.grid_selection_order, [55]);
            assert!(!view.playing);
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Detail(Box::new(Item {
                    id: "playlist".into(),
                    kind: ItemKind::Playlist,
                    ..Default::default()
                }));
                view.state.settings.playlist_albums = false;
                view.state.queue = Default::default();
                view.album_details_scroll.reset(0);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.album_details_scroll.scroll_to(gpui::ListOffset {
                    item_ix: view.detail_row_position(55).unwrap(),
                    // Keep the target visible below the fixed column header.
                    offset_in_item: -px(40. * view.state.settings.zoom),
                });
                cx.notify();
            })
        });
        cx.run_until_parked();
        let point = cx
            .debug_bounds("metadata-cell-55-titleCombined")
            .unwrap()
            .center();
        let header = cx.debug_bounds("detail-sticky-header").unwrap();
        assert!(
            point.y >= header.bottom(),
            "The double-click must reach the visible row: {point:?}, {header:?}"
        );
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.entries.len(), 151);
            assert_eq!(view.state.queue.current().unwrap().item.id, "55");
        });
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn ordered_selection_matches_original_with_duplicate_queue_tracks() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-selection-reference.json")).unwrap();
        let rows = (1..=7).map(uuid::Uuid::from_u128).collect::<Vec<_>>();
        for case in reference["cases"].as_array().unwrap() {
            let mut order = case["initial"]
                .as_array()
                .unwrap()
                .iter()
                .map(|index| rows[index.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            let mut selected = order.iter().copied().collect();
            super::select_row(
                &mut selected,
                &mut order,
                rows[case["index"].as_u64().unwrap() as usize],
                Modifiers {
                    control: case["ctrl"].as_bool().unwrap(),
                    platform: case["meta"].as_bool().unwrap(),
                    shift: case["shift"].as_bool().unwrap(),
                    ..Modifiers::none()
                },
                |anchor, key| super::selection_range(&rows, anchor, key),
            );
            let expected = case["selected"]
                .as_array()
                .unwrap()
                .iter()
                .map(|index| rows[index.as_u64().unwrap() as usize])
                .collect::<Vec<_>>();
            assert_eq!(order, expected, "{case}");
            assert_eq!(
                selected,
                expected
                    .into_iter()
                    .collect::<std::collections::HashSet<_>>()
            );
        }
    }

    fn click_cell(cx: &mut gpui::VisualTestContext, selector: String, modifiers: Modifiers) {
        let point = cx.debug_bounds(&selector).unwrap().center();
        cx.simulate_click(point, modifiers);
    }

    struct RowProbe {
        app: gpui::Entity<Feishin>,
        side: bool,
        albums: bool,
        _subscription: gpui::Subscription,
    }
    impl gpui::Render for RowProbe {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            use gpui::{ParentElement, Styled};
            let content = self.app.update(cx, |view, cx| {
                if self.albums {
                    view.album_detail_row(0, cx)
                } else if self.side {
                    view.side_queue_view(cx)
                } else {
                    view.queue_view(cx)
                }
            });
            gpui::div().size_full().child(content)
        }
    }

    #[gpui::test]
    fn queue_row_clicks_keep_duplicate_identity_filtered_ranges_and_pages(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-queue-rows-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            for key in ["queueSong", "sideQueue"] {
                let mut config = feishin_core::lists::default_for(key);
                config.table.columns = feishin_core::lists::default_columns();
                for column in &mut config.table.columns {
                    column.is_enabled = column.id == "name";
                }
                state.settings.lists.insert(key.into(), config);
            }
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| RowProbe {
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
                app: app.clone(),
                side: false,
                albums: false,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1200.), px(900.)));
        cx.run_until_parked();
        let keys = cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Queue;
                view.state.queue.add(
                    (0..6)
                        .map(|index| Item {
                            id: "duplicate".into(),
                            name: format!("Track {index}"),
                            artist: if index % 2 == 0 {
                                "Even".into()
                            } else {
                                "Odd".into()
                            },
                            kind: ItemKind::Song,
                            ..Default::default()
                        })
                        .collect(),
                    feishin_core::queue::Enqueue::Now,
                );
                view.state.queue.position = 17.;
                cx.notify();
                view.state.queue.order.clone()
            })
        });
        for side in [false, true] {
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.queue_selection_mut(side).0.clear();
                    view.queue_selection_mut(side).1.clear();
                    view.local_filter.clear();
                    view.side_queue_search
                        .update(cx, |input, cx| input.set_value("", window, cx));
                    view.state.queue.current = Some(keys[0]);
                    view.state.queue.position = 17.;
                    cx.notify();
                });
                probe.update(cx, |probe, cx| {
                    probe.side = side;
                    cx.notify();
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            let cell = |index: usize| format!("queue-cell-{side}-{}-name", keys[index]);
            click_cell(cx, cell(1), Modifiers::none());
            click_cell(
                cx,
                cell(4),
                Modifiers {
                    platform: true,
                    ..Modifiers::none()
                },
            );
            click_cell(
                cx,
                cell(2),
                Modifiers {
                    shift: true,
                    ..Modifiers::none()
                },
            );
            app.read_with(cx, |view, _| {
                assert_eq!(
                    *view.queue_selection(side).1,
                    [keys[1], keys[4], keys[2], keys[3]]
                );
                assert_eq!(view.state.queue.current, Some(keys[0]));
                assert_eq!(view.state.queue.position, 17.);
            });
            let point = cx.debug_bounds(&cell(4)).unwrap().center();
            cx.simulate_event(MouseDownEvent {
                button: MouseButton::Left,
                position: point,
                click_count: 2,
                ..Default::default()
            });
            cx.simulate_event(MouseUpEvent {
                button: MouseButton::Left,
                position: point,
                click_count: 2,
                ..Default::default()
            });
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                assert_eq!(*view.queue_selection(side).1, [keys[4]]);
                assert_eq!(view.state.queue.current, Some(keys[4]));
                assert_eq!(view.state.queue.order, keys);
                assert_eq!(view.state.queue.position, 0.);
                assert!(!view.playing);
            });
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.queue_selection_mut(side).0.clear();
                    view.queue_selection_mut(side).1.clear();
                    if side {
                        view.local_filter = "Odd".into();
                        view.side_queue_search
                            .update(cx, |input, cx| input.set_value("Even", window, cx));
                    } else {
                        view.local_filter = "Even".into();
                    }
                    cx.notify();
                })
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            click_cell(cx, cell(0), Modifiers::none());
            click_cell(
                cx,
                cell(4),
                Modifiers {
                    shift: true,
                    ..Modifiers::none()
                },
            );
            assert_eq!(
                app.read_with(cx, |view, _| view.queue_selection(side).1.clone()),
                [keys[0], keys[2], keys[4]]
            );
        }
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.local_filter.clear();
                view.queue_selected.clear();
                view.queue_selection_order.clear();
                view.queue_display_sort = Some(("name".into(), true));
                let mut config = feishin_core::lists::default_for("queueSong");
                config.pagination = "paginated".into();
                config.items_per_page = 3;
                view.state.settings.lists.insert("queueSong".into(), config);
                view.library_page = 1;
                cx.notify();
            });
            probe.update(cx, |probe, cx| {
                probe.side = false;
                cx.notify();
            });
        });
        cx.run_until_parked();
        for (index, shift) in [(2, false), (0, true)] {
            let point = cx
                .debug_bounds(&format!("queue-cell-false-{}-titleCombined", keys[index]))
                .unwrap()
                .center();
            cx.simulate_click(
                point,
                Modifiers {
                    shift,
                    ..Modifiers::none()
                },
            );
        }
        assert_eq!(
            app.read_with(cx, |view, _| view.queue_selection_order.clone()),
            [keys[2], keys[1], keys[0]]
        );
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.remove_queue_entries(&[keys[1]], cx);
                assert!(!view.queue_selection_order.contains(&keys[1]));
            })
        });
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn expanded_album_tracks_select_and_play_their_own_song_context(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-album-row-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| RowProbe {
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
                app: app.clone(),
                side: false,
                albums: true,
            });
            captured.replace(Some(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1200.), px(1000.)));
        cx.run_until_parked();
        let keys = cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Album);
                view.items = (0..2)
                    .map(|index| Item {
                        id: format!("album-{index}"),
                        name: format!("Album {index}"),
                        kind: ItemKind::Album,
                        ..Default::default()
                    })
                    .collect();
                let mut config = feishin_core::lists::default_for("album");
                config.display = "detail".into();
                view.state.settings.lists.insert("album".into(), config);
                let keys = view
                    .items
                    .iter()
                    .map(Feishin::album_key)
                    .collect::<Vec<_>>();
                for (album, key) in keys.iter().enumerate() {
                    view.album_tracks.insert(
                        key.clone(),
                        crate::details::AlbumTracks {
                            items: (0..3)
                                .map(|index| Item {
                                    id: format!("song-{album}-{index}"),
                                    name: format!("Track {album}-{index}"),
                                    kind: ItemKind::Song,
                                    ..Default::default()
                                })
                                .collect(),
                            ..Default::default()
                        },
                    );
                }
                cx.notify();
                keys
            })
        });
        cx.run_until_parked();
        let cell = |index| format!("album-track-cell-{}-{index}-name", keys[0]);
        click_cell(cx, cell(1), Modifiers::none());
        click_cell(
            cx,
            cell(2),
            Modifiers {
                shift: true,
                ..Modifiers::none()
            },
        );
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.album_selection_order,
                [(keys[0].clone(), 1), (keys[0].clone(), 2)]
            );
            assert!(view.selected.is_empty() && view.state.queue.entries.is_empty());
        });
        let point = cx.debug_bounds(&cell(2)).unwrap().center();
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.entries.len(), 6);
            assert_eq!(view.state.queue.current().unwrap().item.id, "song-0-2");
            assert_eq!(view.route, Route::Library(ItemKind::Album));
        });
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.select_album_track(
                    &keys[1],
                    1,
                    Modifiers {
                        shift: true,
                        ..Modifiers::none()
                    },
                    cx,
                );
                assert_eq!(
                    view.album_selection_order,
                    [
                        (keys[0].clone(), 2),
                        (keys[1].clone(), 0),
                        (keys[1].clone(), 1)
                    ]
                );
            })
        });
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn select_all_toggles_match_original_focused_list_hotkeys() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-hotkeys-reference.json")).unwrap();
        for case in reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|case| case["key"] == "mod+a")
        {
            let mut order = case["initial"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            let mut selected = order.iter().copied().collect();
            if case["focused"].as_bool().unwrap() {
                super::toggle_all_rows(&mut selected, &mut order, vec![0, 1, 2]);
            }
            assert_eq!(
                serde_json::to_value(
                    (0..3)
                        .filter(|index| selected.contains(index))
                        .map(|index| index.to_string())
                        .collect::<Vec<_>>()
                )
                .unwrap(),
                case["selected"],
                "{case}"
            );
        }
    }

    #[gpui::test]
    fn list_keys_follow_focus_order_preferences_and_queue_filters(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-list-keys-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.confirm_queue_changes = false;
            for key in ["queueSong", "sideQueue"] {
                let mut config = feishin_core::lists::default_for(key);
                config.table.columns = feishin_core::lists::default_columns();
                for column in &mut config.table.columns {
                    column.is_enabled = column.id == "name";
                }
                state.settings.lists.insert(key.into(), config);
            }
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1500.), px(1000.)));
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Song);
                view.items = (0..3)
                    .map(|index| Item {
                        id: index.to_string(),
                        name: format!("Track {index}"),
                        kind: ItemKind::Song,
                        ..Default::default()
                    })
                    .collect();
                cx.notify();
            })
        });
        cx.run_until_parked();
        click_cell(
            cx,
            "metadata-cell-2-titleCombined".into(),
            Modifiers::none(),
        );
        click_cell(
            cx,
            "metadata-cell-0-titleCombined".into(),
            Modifiers {
                control: true,
                ..Modifiers::none()
            },
        );
        assert_eq!(
            app.read_with(cx, |view, _| view.grid_selection_order.clone()),
            [2, 0]
        );
        let original: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-hotkeys-reference.json")).unwrap();
        for case in original["cases"].as_array().unwrap().iter().filter(|case| {
            case["focused"] == true
                && case["itemType"] == "song"
                && case["key"] == "listPlayDefault"
                && case["initial"] == serde_json::json!([2, 0])
        }) {
            let mode = case["mode"].as_str().unwrap();
            let old = cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    window.focus(&view.library_focus);
                    view.state.queue = Default::default();
                    view.state.queue.add(
                        ["old", "tail"]
                            .map(|id| Item {
                                id: id.into(),
                                kind: ItemKind::Song,
                                ..Default::default()
                            })
                            .into(),
                        feishin_core::queue::Enqueue::Now,
                    );
                    view.state.queue.position = 23.;
                    view.state.settings.play_button_behavior = match mode {
                        "next" => feishin_core::queue::PlayBehavior::Next,
                        "last" => feishin_core::queue::PlayBehavior::Last,
                        "shuffle" => feishin_core::queue::PlayBehavior::Shuffle,
                        _ => feishin_core::queue::PlayBehavior::Now,
                    };
                    cx.notify();
                    view.state.queue.current
                })
            });
            cx.run_until_parked();
            cx.simulate_keystrokes("enter");
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                let song = case["actions"][0]["play"].as_str().unwrap();
                let expected = match mode {
                    "next" => vec!["old", song, "tail"],
                    "last" => vec!["old", "tail", song],
                    _ => vec![song],
                };
                assert_eq!(
                    view.state
                        .queue
                        .ordered()
                        .iter()
                        .map(|entry| entry.item.id.as_str())
                        .collect::<Vec<_>>(),
                    expected,
                    "{case}"
                );
                if matches!(mode, "next" | "last") {
                    assert_eq!(view.state.queue.current, old);
                    assert_eq!(view.state.queue.position, 23.);
                }
            });
        }
        let primary_a = if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        };
        cx.simulate_keystrokes(primary_a);
        assert_eq!(
            app.read_with(cx, |view, _| view.grid_selection_order.clone()),
            [0, 1, 2]
        );
        cx.simulate_keystrokes(primary_a);
        assert!(app.read_with(cx, |view, _| view.selected.is_empty()));
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.local_search.update(cx, |input, cx| {
                    input.set_value("Before", window, cx);
                    input.focus(window, cx);
                });
            })
        });
        cx.run_until_parked();
        let queue = app.read_with(cx, |view, _| view.state.queue.order.clone());
        cx.simulate_keystrokes(primary_a);
        cx.simulate_input("After");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        app.read_with(cx, |view, cx| {
            assert_eq!(view.local_search.read(cx).value(), "After");
            assert!(view.selected.is_empty());
            assert_eq!(view.state.queue.order, queue);
        });
        for side in [false, true] {
            let keys = cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.route = if side {
                        Route::Library(ItemKind::Song)
                    } else {
                        Route::Queue
                    };
                    view.state.settings.panels.expanded = side;
                    view.state.settings.panels.queue = true;
                    view.state.settings.panels.lyrics = false;
                    view.state.settings.panels.visualizer = false;
                    view.queue_selection_mut(side).0.clear();
                    view.queue_selection_mut(side).1.clear();
                    view.state.queue = Default::default();
                    view.state.queue.add(
                        (0..6)
                            .map(|index| Item {
                                id: "duplicate".into(),
                                name: format!(
                                    "{} {index}",
                                    if index % 2 == 0 { "Even" } else { "Odd" }
                                ),
                                kind: ItemKind::Song,
                                ..Default::default()
                            })
                            .collect(),
                        feishin_core::queue::Enqueue::Now,
                    );
                    view.local_filter = if side { String::new() } else { "Even".into() };
                    view.side_queue_search
                        .update(cx, |input, cx| input.set_value("Even", window, cx));
                    cx.notify();
                    view.state.queue.order.clone()
                })
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.run_until_parked();
            click_cell(
                cx,
                format!("queue-cell-{side}-{}-name", keys[2]),
                Modifiers::none(),
            );
            cx.simulate_keystrokes(primary_a);
            assert_eq!(
                app.read_with(cx, |view, _| view.queue_selection(side).1.clone()),
                [keys[0], keys[2], keys[4]]
            );
            cx.simulate_keystrokes(if side { "backspace" } else { "delete" });
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                assert_eq!(view.state.queue.order, [keys[1], keys[3], keys[5]]);
                assert!(
                    view.queue_selection(side).0.is_empty()
                        && view.queue_selection(side).1.is_empty()
                );
            });
        }
        let _ = std::fs::remove_dir_all(dir);
    }
    #[gpui::test]
    fn queue_views_keep_independent_selection_drag_and_delete_contexts(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-queue-scopes-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.follow_current_song = false;
            for key in ["queueSong", "sideQueue"] {
                let mut config = feishin_core::lists::default_for(key);
                for column in &mut config.table.columns {
                    column.is_enabled = column.id == "name";
                }
                state.settings.lists.insert(key.into(), config);
            }
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| RowProbe {
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
                app: app.clone(),
                side: false,
                albums: false,
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1200.), px(900.)));
        cx.run_until_parked();
        let keys = cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Queue;
                view.state.queue.add(
                    (0..6)
                        .map(|index| Item {
                            id: "duplicate".into(),
                            name: format!(
                                "{} {index}",
                                if index % 2 == 0 { "Even" } else { "Odd" }
                            ),
                            kind: ItemKind::Song,
                            ..Default::default()
                        })
                        .collect(),
                    feishin_core::queue::Enqueue::Now,
                );
                view.state.queue.position = 29.;
                view.side_queue_search
                    .update(cx, |input, cx| input.set_value("Even", window, cx));
                cx.notify();
                view.state.queue.order.clone()
            })
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.run_until_parked();
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-hotkeys-reference.json")).unwrap();
        let check = |step: usize, cx: &mut gpui::VisualTestContext| {
            app.read_with(cx, |view, _| {
                for (side, name) in [(false, "main"), (true, "side")] {
                    let selected = keys
                        .iter()
                        .enumerate()
                        .filter(|(_, key)| view.queue_selection(side).0.contains(key))
                        .map(|(index, _)| index.to_string())
                        .collect::<Vec<_>>();
                    assert_eq!(
                        serde_json::to_value(selected).unwrap(),
                        source["queueStates"][step][name]
                    );
                }
                assert_eq!(view.state.queue.current, Some(keys[0]));
                assert_eq!(view.state.queue.position, 29.);
            })
        };
        click_cell(
            cx,
            format!("queue-cell-false-{}-name", keys[4]),
            Modifiers::none(),
        );
        click_cell(
            cx,
            format!("queue-cell-false-{}-name", keys[1]),
            Modifiers {
                control: true,
                ..Modifiers::none()
            },
        );
        check(0, cx);
        assert_eq!(
            app.read_with(cx, |view, cx| view.first_queue_selection(false, cx)),
            Some(keys[1])
        );
        cx.update(|_, cx| {
            probe.update(cx, |probe, cx| {
                probe.side = true;
                cx.notify();
            })
        });
        cx.run_until_parked();
        click_cell(
            cx,
            format!("queue-cell-true-{}-name", keys[2]),
            Modifiers::none(),
        );
        check(1, cx);
        cx.update(|window, cx| app.update(cx, |view, cx| view.toggle_list_selection(window, cx)));
        check(2, cx);
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.drag_queue_entry(keys[4], true).keys,
                [keys[0], keys[2], keys[4]]
            );
            assert_eq!(
                view.drag_queue_entry(keys[4], false).keys,
                [keys[1], keys[4]]
            );
        });
        cx.update(|_, cx| {
            probe.update(cx, |probe, cx| {
                probe.side = false;
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                window.focus(&view.queue_focus);
                view.toggle_list_selection(window, cx);
                view.toggle_list_selection(window, cx);
            })
        });
        check(3, cx);
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.select_queue_row(keys[1], false, Modifiers::none(), cx);
                // Deleting the side selection also prunes removed rows from the
                // main selection while preserving its unrelated selected track.
                view.queue_selected.insert(keys[4]);
                view.queue_selection_order.push(keys[4]);
                view.remove_queue_selection(true, cx);
                assert_eq!(view.state.queue.order, [keys[1], keys[3], keys[5]]);
                assert_eq!(
                    view.queue_selected,
                    [keys[1]]
                        .into_iter()
                        .collect::<std::collections::HashSet<_>>()
                );
                assert_eq!(view.queue_selection_order, [keys[1]]);
                assert!(
                    view.side_queue_selected.is_empty()
                        && view.side_queue_selection_order.is_empty()
                );
            })
        });
        let _ = std::fs::remove_dir_all(dir);
    }
}
