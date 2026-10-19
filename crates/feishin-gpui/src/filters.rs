use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    filters::{Filters, View},
    model::{Item, ItemKind, ListQuery, ServerType},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, StyledExt,
    button::ButtonVariants,
    input::{InputEvent, InputState},
};

pub struct Editor {
    pub key: String,
    pub page_key: Option<String>,
    pub year_debounce: Option<Task<()>>,
    pub server: uuid::Uuid,
    pub filters: Filters,
    pub years: [Entity<InputState>; 2],
    pub search: Entity<InputState>,
    pub tag: Entity<InputState>,
    pub choice: ItemKind,
    pub local: bool,
    pub target: ItemKind,
    pub tags: bool,
    pub tag_names: Option<Vec<String>>,
    pub tag_values: std::collections::BTreeMap<String, Vec<String>>,
    pub choices: Vec<Item>,
    pub has_more: bool,
    pub loading: bool,
}
fn boolean_label(value: Option<bool>) -> &'static str {
    match value {
        None => "common.none",
        Some(true) => "common.yes",
        Some(false) => "common.no",
    }
}
fn next_boolean(value: Option<bool>) -> Option<bool> {
    match value {
        None => Some(true),
        Some(true) => Some(false),
        Some(false) => None,
    }
}
impl Feishin {
    pub fn view_key(&self) -> String {
        let server = match &self.route {
            Route::Detail(item) | Route::Similar(item) => Some(item.server_id),
            _ => self.state.active_server,
        };
        let scope = match &self.route {
            Route::Collection(collection) => {
                return format!(
                    "{}:collection:{}",
                    server.unwrap_or_default(),
                    collection.id
                );
            }
            Route::Home => "home",
            Route::Favorites(_) => "favorites",
            Route::Search(..) => "search",
            Route::Explore => "explore",
            Route::Similar(_) => "similar",
            Route::Detail(_) => "detail",
            Route::Queue => "queue",
            _ => "library",
        };
        format!("{}:{scope}:{}", server.unwrap_or_default(), self.list_key())
    }
    pub(crate) fn is_browse_route(&self) -> bool {
        matches!(
            self.route,
            Route::Home
                | Route::Library(_)
                | Route::Collection(_)
                | Route::Favorites(_)
                | Route::Detail(_)
                | Route::Similar(_)
                | Route::Search(..)
                | Route::Explore
                | Route::Queue
        )
    }
    pub fn remember_view(&mut self) {
        if self.is_browse_route() {
            let key = self.view_key();
            let view = View {
                filters: self.active_filters.clone(),
                text: self.local_filter.clone(),
                sort: self.sort,
                descending: self.descending,
                artist_role: self.artist_role.clone(),
            };
            if self.state.settings.views.get(&key) != Some(&view) {
                self.state.settings.views.insert(key, view);
                self.save();
            }
        }
    }
    pub fn change_artist_tab(
        &mut self,
        tab: feishin_core::model::ArtistTab,
        cx: &mut Context<Self>,
    ) {
        self.remember_view();
        self.artist_tab = tab;
        self.artist_overview = false;
        self.restore_view();
        self.reload(false, cx);
    }
    pub fn restore_view(&mut self) {
        self.local_search_debounce = None;
        self.queue_search_open = false;
        if !self.is_browse_route() {
            return;
        }
        let view = self
            .state
            .settings
            .views
            .get(&self.view_key())
            .cloned()
            .unwrap_or_else(|| {
                if let Route::Collection(collection) = &self.route { return collection.view.clone(); }
                let sort = match &self.route {
                    Route::Detail(item) if matches!(item.kind,ItemKind::Artist | ItemKind::AlbumArtist) && matches!(self.artist_tab,feishin_core::model::ArtistTab::PersonalTop | feishin_core::model::ArtistTab::CommunityTop | feishin_core::model::ArtistTab::Favorites | feishin_core::model::ArtistTab::Rated) => feishin_core::model::Sort::Id,
                    Route::Detail(item) if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) && self.artist_tab == feishin_core::model::ArtistTab::Albums => feishin_core::model::Sort::ReleaseDate,
                    Route::Detail(item) if item.kind == ItemKind::Playlist => {
                        feishin_core::model::Sort::Id
                    }
                    Route::Detail(item) if item.kind == ItemKind::Album => {
                        feishin_core::model::Sort::Id
                    }
                    _ => feishin_core::model::Sort::Name,
                };
                let filters = if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist) {
                    Filters {artist_all:true,album_artist_all:true,genre_all:true,..Default::default()}
                } else {Filters::default()};
                let descending = matches!(&self.route, Route::Detail(item) if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) && self.artist_tab == feishin_core::model::ArtistTab::Albums);
                View {sort,filters,descending,..Default::default()}
            });
        self.active_filters = view.filters;
        self.local_filter = view.text;
        self.sort = view.sort;
        self.descending = view.descending;
        self.artist_role = view.artist_role;
        self.filter_input_sync = true;
    }
    pub fn edit_filters(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.prepare_filters(true, window, cx);
    }
    pub fn prepare_filters(&mut self, navigate: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.remember_view();
        let api = match &self.route {
            Route::Detail(item) | Route::Similar(item) => {
                self.sessions.get(&item.server_id).cloned()
            }
            _ => self.api(),
        };
        let Some(api) = api else {
            return;
        };
        let key = self.view_key();
        let page_key = self.filter_page_key();
        let filters = self.active_filters.clone();
        let mut input = |value: String, label: &str, cx: &mut Context<Self>| {
            let input = cx
                .new(|cx| crate::appearance::input_state(window, cx).aria_label(label.to_owned()));
            input.update(cx, |input, cx| input.set_value(value, window, cx));
            input
        };
        let years = [
            input(
                filters
                    .min_year
                    .map_or(String::new(), |year| year.to_string()),
                "filter.fromYear",
                cx,
            ),
            input(
                filters
                    .max_year
                    .map_or(String::new(), |year| year.to_string()),
                "filter.toYear",
                cx,
            ),
        ];
        let search = input(String::new(), "common.search", cx);
        let tag = input(String::new(), "common.tags", cx);
        self.filter_subscriptions.clear();
        for input in [&search, &tag] {
            self.filter_subscriptions
                .push(cx.subscribe(input, |this, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.load_filter_choices(cx);
                    }
                }));
        }
        for year in &years {
            self.filter_subscriptions
                .push(cx.subscribe(year, |this, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        let route = this.route.clone();
                        if let Some(editor) = &mut this.filter_editor {
                            let key = editor.key.clone();
                            editor.year_debounce = Some(cx.spawn(async move |this, cx| {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(300))
                                    .await;
                                this.update(cx, |this, cx| {
                                    if this.route == route
                                        && this
                                            .filter_editor
                                            .as_ref()
                                            .is_some_and(|editor| editor.key == key)
                                    {
                                        this.commit_filters(false, cx);
                                    }
                                })
                                .ok();
                            }));
                        }
                    }
                }));
        }
        let genre = matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Genre);
        let local = !genre
            && matches!(
                self.route,
                Route::Detail(_)
                    | Route::Similar(_)
                    | Route::Explore
                    | Route::Search(..)
                    | Route::Queue
            );
        let target = match self.route {
            Route::Library(kind) | Route::Favorites(kind) => kind,
            Route::Collection(ref collection) => collection.kind,
            Route::Detail(_) if genre && self.state.settings.genre_albums => ItemKind::Album,
            _ => ItemKind::Song,
        };
        self.filter_editor = Some(Editor {
            key: key.clone(),
            page_key,
            year_debounce: None,
            server: api.server.id,
            filters,
            years,
            search,
            tag,
            choice: ItemKind::Artist,
            local,
            target,
            tags: false,
            tag_names: None,
            tag_values: Default::default(),
            choices: vec![],
            has_more: false,
            loading: false,
        });
        if navigate {
            self.navigate(Route::ListFilters(key), cx);
        }
        self.load_filter_choices(cx);
    }
    pub fn load_filter_choices(&mut self, cx: &mut Context<Self>) {
        self.load_filter_choices_page(false, cx);
    }
    fn load_filter_choices_page(&mut self, append: bool, cx: &mut Context<Self>) {
        if !matches!(self.route, Route::ListFilters(_)) && !self.filters_pinned() {
            return;
        }
        let Some(editor) = &mut self.filter_editor else {
            return;
        };
        if append && (editor.loading || !editor.has_more) {
            return;
        }
        let Some(api) = self.sessions.get(&editor.server).cloned() else {
            return;
        };
        let query = ListQuery {
            kind: editor.choice,
            search: editor.search.read(cx).value().to_string(),
            offset: if append { editor.choices.len() } else { 0 },
            limit: 100,
            ..Default::default()
        };
        let fetch_names = !editor.local
            && editor.tag_names.is_none()
            && (api.server.kind == ServerType::Jellyfin
                || (api.server.kind == ServerType::Navidrome && api.features().artist_roles));
        let tag = editor
            .tags
            .then(|| editor.tag.read(cx).value().trim().to_owned());
        let target = editor.target;
        let values = tag
            .as_ref()
            .and_then(|name| editor.tag_values.get(name))
            .cloned();
        editor.loading = true;
        self.filter_generation += 1;
        let generation = self.filter_generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            if !this
                .read_with(cx, |this, _| this.filter_generation == generation)
                .unwrap_or(false)
            {
                return;
            }
            let job = cx.background_executor().spawn(async move {
                let names = if fetch_names {
                    Some(api.filter_catalog(target)?)
                } else {
                    None
                };
                let page = if let Some(tag) = tag {
                    if tag.is_empty() {
                        feishin_core::model::Page::default()
                    } else if api.server.kind == ServerType::Jellyfin {
                        let values = values
                            .or_else(|| {
                                names
                                    .as_ref()
                                    .and_then(|catalog| catalog.get(&tag))
                                    .cloned()
                            })
                            .unwrap_or_default();
                        let values: Vec<_> = values
                            .into_iter()
                            .filter(|value| {
                                value.to_lowercase().contains(&query.search.to_lowercase())
                            })
                            .collect();
                        let total = values.len();
                        let items = values
                            .into_iter()
                            .skip(query.offset)
                            .take(query.limit)
                            .map(|value| Item {
                                id: value.clone(),
                                name: value,
                                server_id: api.server.id,
                                kind: ItemKind::Genre,
                                ..Default::default()
                            })
                            .collect();
                        feishin_core::model::Page {
                            items,
                            total: Some(total),
                            has_more: query.offset.saturating_add(query.limit) < total,
                        }
                    } else {
                        api.tag_filter_values(&tag, &query.search, query.offset, query.limit)?
                    }
                } else {
                    api.list(&query)?
                };
                Ok::<_, anyhow::Error>((page, names))
            });
            let result = job.await;
            this.update(cx, |this, cx| {
                if this.filter_generation != generation
                    || !(matches!(this.route, Route::ListFilters(_)) || this.filters_pinned())
                {
                    return;
                }
                if let Some(editor) = &mut this.filter_editor {
                    editor.loading = false;
                    match result {
                        Ok((page, names)) => {
                            editor.has_more = page.has_more && !page.items.is_empty();
                            if append {
                                editor.choices.extend(page.items);
                            } else {
                                editor.choices = page.items;
                            }
                            if let Some(catalog) = names {
                                editor.tag_names = Some(catalog.keys().cloned().collect());
                                editor.tag_values = catalog;
                            }
                        }
                        Err(error) => {
                            if !append {
                                editor.choices.clear();
                                editor.has_more = false;
                            }
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
    pub fn select_filter_value(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.filter_editor else {
            return;
        };
        if editor.tags {
            let name = editor.tag.read(cx).value().trim().to_owned();
            if name.is_empty() {
                return;
            }
            let values = editor
                .filters
                .custom
                .entry(name)
                .or_insert_with(|| serde_json::json!([]));
            if let Some(values) = values.as_array_mut() {
                if values.iter().any(|value| value.as_str() == Some(id)) {
                    values.retain(|value| value.as_str() != Some(id));
                } else {
                    values.push(id.into());
                }
            }
        } else {
            let ids = match editor.choice {
                ItemKind::Genre => &mut editor.filters.genre_ids,
                ItemKind::Folder => &mut editor.filters.library_ids,
                ItemKind::AlbumArtist => &mut editor.filters.album_artist_ids,
                _ => &mut editor.filters.artist_ids,
            };
            if ids.iter().any(|value| value == id) {
                ids.retain(|value| value != id);
            } else {
                let mode = match editor.choice {
                    ItemKind::Genre => "genre",
                    ItemKind::AlbumArtist => "albumArtist",
                    _ => "artist",
                };
                if self
                    .state
                    .settings
                    .single_select_filters
                    .iter()
                    .any(|value| value == mode)
                {
                    ids.clear();
                }
                if editor.choice == ItemKind::Folder
                    && self
                        .sessions
                        .get(&editor.server)
                        .is_some_and(|api| api.server.kind != ServerType::Navidrome)
                {
                    ids.clear();
                }
                ids.push(id.into());
            }
        }
        self.commit_filters(false, cx);
        cx.notify();
    }
    pub fn apply_filters(&mut self, cx: &mut Context<Self>) {
        self.commit_filters(true, cx);
    }
    fn commit_filters(&mut self, close: bool, cx: &mut Context<Self>) {
        let Some(editor) = &self.filter_editor else {
            return;
        };
        let full_page = matches!(&self.route, Route::ListFilters(key) if key == &editor.key);
        if !full_page && (!self.filters_pinned() || editor.key != self.view_key()) {
            return;
        }
        let mut filters = editor.filters.clone();
        let result = (|| -> anyhow::Result<()> {
            let year = |input: &Entity<InputState>| -> anyhow::Result<Option<u32>> {
                let text = input.read(cx).value().trim().to_owned();
                if text.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(text.parse()?))
                }
            };
            filters.min_year = year(&editor.years[0])?;
            filters.max_year = year(&editor.years[1])?;
            filters
                .custom
                .retain(|_, value| value.as_array().is_none_or(|values| !values.is_empty()));
            filters.validate()
        })();
        match result {
            Ok(()) => {
                let mut view = self
                    .state
                    .settings
                    .views
                    .get(&editor.key)
                    .cloned()
                    .unwrap_or_default();
                let changed = view.filters != filters;
                if changed {
                    view.filters = filters.clone();
                    self.state.settings.views.insert(editor.key.clone(), view);
                    if full_page && let Some(visit) = self.history.last_mut() {
                        visit.reset_position();
                    }
                    self.save();
                }
                if self.message.starts_with("Invalid filters:") {
                    self.message.clear();
                }
                if full_page && close {
                    self.back(cx);
                } else if !full_page && changed {
                    self.active_filters = filters;
                    self.reload(false, cx);
                }
                cx.notify();
            }
            Err(error) => {
                self.message = format!("Invalid filters: {error}");
                cx.notify();
            }
        }
    }
    pub fn filters_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.filter_editor else {
            return div().into_any_element();
        };
        let kind = self
            .sessions
            .get(&editor.server)
            .map(|api| api.server.kind)
            .unwrap_or(ServerType::Subsonic);
        let boolean_fields = [
            ("favorite", editor.filters.favorite, "filter.isFavorited"),
            ("rating", editor.filters.has_rating, "filter.isRated"),
            (
                "compilation",
                editor.filters.compilation,
                "filter.isCompilation",
            ),
            (
                "played",
                editor.filters.recently_played,
                "filter.isRecentlyPlayed",
            ),
        ];
        div()
            .id("list-filters")
            .debug_selector(|| "list-filters".into())
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .when(matches!(self.route, Route::ListFilters(_)), |row| {
                        row.child(
                            crate::styles::button("apply-filters", cx)
                                .primary()
                                .label(self.tr("common.close"))
                                .on_click(cx.listener(|this, _, _, cx| this.apply_filters(cx))),
                        )
                    })
                    .child(
                        crate::styles::button("pin-filters", cx)
                            .label(if self.filters_pinned() {
                                "Unpin"
                            } else {
                                "Pin"
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_filter_pin(cx))),
                    )
                    .child(
                        crate::styles::button("clear-filters", cx)
                            .label(self.tr("common.reset"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(editor) = &mut this.filter_editor {
                                    editor.filters = Default::default();
                                    for year in &editor.years {
                                        year.update(cx, |input, cx| {
                                            input.set_value("", window, cx)
                                        });
                                    }
                                }
                                this.commit_filters(false, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .flex_wrap()
                    .children(
                        boolean_fields
                            .into_iter()
                            .filter(|(id, _, _)| {
                                (kind != ServerType::Jellyfin
                                    || matches!(*id, "favorite" | "compilation"))
                                    && (!editor.local || matches!(*id, "favorite" | "rating"))
                            })
                            .map(|(id, value, label)| {
                                crate::styles::button(id, cx)
                                    .label(format!(
                                        "{}: {}",
                                        self.tr(label),
                                        self.tr(boolean_label(value))
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(editor) = &mut this.filter_editor {
                                            let value = match id {
                                                "favorite" => &mut editor.filters.favorite,
                                                "rating" => &mut editor.filters.has_rating,
                                                "compilation" => &mut editor.filters.compilation,
                                                _ => &mut editor.filters.recently_played,
                                            };
                                            *value = next_boolean(*value);
                                        }
                                        this.commit_filters(false, cx);
                                        cx.notify();
                                    }))
                            }),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(
                        div()
                            .child(self.tr("filter.fromYear"))
                            .child(crate::styles::input(&editor.years[0], cx)),
                    )
                    .child(
                        div()
                            .child(self.tr("filter.toYear"))
                            .child(crate::styles::input(&editor.years[1], cx)),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .children(
                        [
                            (ItemKind::Artist, "Artists"),
                            (ItemKind::AlbumArtist, "Album artists"),
                            (ItemKind::Genre, "Genres"),
                            (ItemKind::Folder, "Libraries"),
                        ]
                        .into_iter()
                        .filter(|(choice, _)| !editor.local || *choice != ItemKind::Folder)
                        .filter(|(choice, _)| {
                            *choice != ItemKind::Genre
                                || !matches!(
                                    editor.page_key.as_deref(),
                                    Some("genreAlbum" | "genreSong")
                                )
                        })
                        .map(|(kind, label)| {
                            crate::styles::button(label, cx)
                                .debug_selector(move || format!("filter-choice-{label}"))
                                .label(self.tr(label))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(editor) = &mut this.filter_editor {
                                        editor.choice = kind;
                                        editor.tags = false;
                                    }
                                    this.load_filter_choices(cx);
                                }))
                        }),
                    )
                    .when(
                        matches!(kind, ServerType::Navidrome | ServerType::Jellyfin)
                            && !editor.local,
                        |div| {
                            div.child(
                                crate::styles::button("filter-tags", cx)
                                    .label(self.tr("common.tags"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Some(editor) = &mut this.filter_editor {
                                            editor.tags = true;
                                        }
                                        this.load_filter_choices(cx);
                                    })),
                            )
                        },
                    ),
            )
            .when(!editor.tags && editor.choice != ItemKind::Folder, |div| {
                let mode = match editor.choice {
                    ItemKind::Genre => "genre",
                    ItemKind::AlbumArtist => "albumArtist",
                    _ => "artist",
                };
                let single = self
                    .state
                    .settings
                    .single_select_filters
                    .iter()
                    .any(|value| value == mode);
                div.child(
                    crate::styles::button("filter-select-mode", cx)
                        .label(self.tr(if single {
                            "common.filter_single"
                        } else {
                            "common.filter_multiple"
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this
                                .state
                                .settings
                                .single_select_filters
                                .iter()
                                .any(|value| value == mode)
                            {
                                this.state
                                    .settings
                                    .single_select_filters
                                    .retain(|value| value != mode);
                            } else {
                                this.state.settings.single_select_filters.push(mode.into());
                                if let Some(editor) = &mut this.filter_editor {
                                    match editor.choice {
                                        ItemKind::Genre => &mut editor.filters.genre_ids,
                                        ItemKind::AlbumArtist => {
                                            &mut editor.filters.album_artist_ids
                                        }
                                        _ => &mut editor.filters.artist_ids,
                                    }
                                    .truncate(1);
                                }
                            }
                            this.save();
                            this.commit_filters(false, cx);
                            cx.notify();
                        })),
                )
            })
            .when(
                editor.local && !editor.tags && editor.choice != ItemKind::Folder,
                |div| {
                    let all = match editor.choice {
                        ItemKind::Genre => editor.filters.genre_all,
                        ItemKind::AlbumArtist => editor.filters.album_artist_all,
                        _ => editor.filters.artist_all,
                    };
                    div.child(
                        crate::styles::button("filter-match-all", cx)
                            .label(if all {
                                "Match all selected values"
                            } else {
                                "Match any selected value"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(editor) = &mut this.filter_editor {
                                    let all = match editor.choice {
                                        ItemKind::Genre => &mut editor.filters.genre_all,
                                        ItemKind::AlbumArtist => {
                                            &mut editor.filters.album_artist_all
                                        }
                                        _ => &mut editor.filters.artist_all,
                                    };
                                    *all = !*all;
                                }
                                this.commit_filters(false, cx);
                                cx.notify();
                            })),
                    )
                },
            )
            .when(editor.tags, |div| {
                let query = editor.tag.read(cx).value().to_lowercase();
                div.child(self.tr("common.tags"))
                    .child(crate::styles::input(&editor.tag, cx))
                    .child(
                        gpui::div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .flex_wrap()
                            .gap_2()
                            .children(
                                editor
                                    .tag_names
                                    .iter()
                                    .flatten()
                                    .filter(|name| {
                                        tag_available(name, editor.target, kind)
                                            && name.to_lowercase().contains(&query)
                                    })
                                    .take(100)
                                    .enumerate()
                                    .map(|(index, name)| {
                                        let name = name.clone();
                                        crate::styles::button(("filter-tag-name", index), cx)
                                            .label(name.clone())
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                if let Some(editor) = &mut this.filter_editor {
                                                    editor.tag.update(cx, |input, cx| {
                                                        input.set_value(name.clone(), window, cx)
                                                    });
                                                }
                                                this.load_filter_choices(cx);
                                            }))
                                    }),
                            ),
                    )
            })
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_2()
                    .children({
                        let selected = if editor.tags {
                            editor
                                .filters
                                .custom
                                .get(editor.tag.read(cx).value().trim())
                                .and_then(serde_json::Value::as_array)
                                .map(|values| {
                                    values
                                        .iter()
                                        .filter_map(serde_json::Value::as_str)
                                        .map(str::to_owned)
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        } else {
                            match editor.choice {
                                ItemKind::Genre => &editor.filters.genre_ids,
                                ItemKind::Folder => &editor.filters.library_ids,
                                ItemKind::AlbumArtist => &editor.filters.album_artist_ids,
                                _ => &editor.filters.artist_ids,
                            }
                            .clone()
                        };
                        selected.into_iter().enumerate().map(|(index, id)| {
                            let label = editor
                                .choices
                                .iter()
                                .find(|item| item.id == id)
                                .map_or_else(|| id.clone(), |item| item.name.clone());
                            crate::styles::button(("remove-filter-selection", index), cx)
                                .label(format!("Remove {label}"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_filter_value(&id, cx)
                                }))
                        })
                    }),
            )
            .child(self.tr("common.search"))
            .child(crate::styles::input(&editor.search, cx))
            .when(editor.loading, |div| div.child(self.tr("Loading...")))
            .when(!editor.loading && editor.choices.is_empty(), |div| {
                div.child(self.tr("common.noResultsFromQuery"))
            })
            .children(editor.choices.iter().enumerate().map(|(index, item)| {
                let selected = if editor.tags {
                    editor
                        .filters
                        .custom
                        .get(editor.tag.read(cx).value().trim())
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(&item.id))
                        })
                } else {
                    let ids = match editor.choice {
                        ItemKind::Genre => &editor.filters.genre_ids,
                        ItemKind::Folder => &editor.filters.library_ids,
                        ItemKind::AlbumArtist => &editor.filters.album_artist_ids,
                        _ => &editor.filters.artist_ids,
                    };
                    ids.contains(&item.id)
                };
                let id = item.id.clone();
                crate::styles::button(("filter-value", index), cx)
                    .disabled(editor.loading)
                    .label(format!(
                        "{} {}",
                        if selected { "✓" } else { "□" },
                        item.name
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| this.select_filter_value(&id, cx)))
            }))
            .when(editor.has_more, |div| {
                div.child(
                    crate::styles::button("more-filter-values", cx)
                        .label(crate::appearance::text(
                            if editor.loading {
                                "Loading..."
                            } else {
                                "Load more"
                            },
                            cx,
                        ))
                        .disabled(editor.loading)
                        .on_click(
                            cx.listener(|this, _, _, cx| this.load_filter_choices_page(true, cx)),
                        ),
                )
            })
            .when(
                self.has_standard_list_header()
                    || (matches!(self.route, Route::ListFilters(_))
                        && self.history.last().is_some_and(|visit| {
                            matches!(
                                visit.route,
                                Route::Library(ItemKind::Album | ItemKind::Song)
                            )
                        })),
                |div| {
                    div.child(
                        crate::styles::button("save-collection", cx)
                            .debug_selector(|| "save-collection".into())
                            .label(crate::appearance::text("Save collection", cx))
                            .on_click(cx.listener(|view, _, window, cx| {
                                if matches!(view.route, Route::ListFilters(_)) {
                                    view.apply_filters(cx);
                                }
                                view.edit_collection(None, true, window, cx)
                            })),
                    )
                },
            )
            .when(!editor.filters.custom.is_empty(), |div| {
                div.children(editor.filters.custom.keys().map(|name| {
                    let name = name.clone();
                    crate::styles::button(
                        SharedString::from(format!("remove-custom-filter-{name}")),
                        cx,
                    )
                    .label(format!("Remove {name} filter"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(editor) = &mut this.filter_editor {
                            editor.filters.custom.remove(&name);
                        }
                        this.commit_filters(false, cx);
                        cx.notify();
                    }))
                }))
            })
            .into_any_element()
    }
    pub fn filters_pinned(&self) -> bool {
        let key = if let Route::ListFilters(key) = &self.route {
            key.clone()
        } else {
            self.view_key()
        };
        self.state.settings.pinned_filter_views.contains(&key)
            || self.filter_page_key().is_some_and(|key| {
                self.state
                    .settings
                    .page_sidebars
                    .get(&key)
                    .copied()
                    .unwrap_or(false)
            })
    }
    fn filter_page_key(&self) -> Option<String> {
        match &self.route {
            Route::ListFilters(key) => self
                .filter_editor
                .as_ref()
                .filter(|editor| &editor.key == key)
                .and_then(|editor| editor.page_key.clone()),
            Route::Library(ItemKind::Album | ItemKind::Song) | Route::Collection(_) => {
                Some(self.list_key())
            }
            Route::Detail(item)
                if item.kind == ItemKind::Genre
                    || (matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist)
                        && !self.artist_overview_active()) =>
            {
                Some(self.list_key())
            }
            _ => None,
        }
    }
    pub fn toggle_filter_pin(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.filter_editor else {
            return;
        };
        let key = editor.key.clone();
        let pinned = self.filters_pinned();
        if pinned {
            self.state
                .settings
                .pinned_filter_views
                .retain(|value| value != &key);
        }
        if let Some(page_key) = self.filter_page_key() {
            self.state.settings.page_sidebars.insert(page_key, !pinned);
        } else if !pinned {
            self.state.settings.pinned_filter_views.push(key);
        }
        self.filter_generation += 1;
        if let Some(editor) = &mut self.filter_editor {
            editor.year_debounce = None;
            editor.loading = false;
        }
        self.save();
        if matches!(self.route, Route::ListFilters(_)) {
            self.back(cx);
        } else {
            if !pinned {
                self.load_filter_choices(cx);
            }
            cx.notify();
        }
    }
}

fn tag_available(name: &str, target: ItemKind, server: ServerType) -> bool {
    if server != ServerType::Navidrome {
        return true;
    }
    let name = name.to_lowercase();
    if name == "genre" {
        return false;
    }
    if target == ItemKind::Song {
        !matches!(name.as_str(), "disctotal" | "tracktotal")
    } else {
        !matches!(
            name.as_str(),
            "asin"
                | "barcode"
                | "copyright"
                | "encodedby"
                | "isrc"
                | "key"
                | "language"
                | "musicbrainz_workid"
                | "script"
                | "website"
                | "work"
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{Feishin, Route};
    use feishin_core::model::{ItemKind, ServerType};
    #[gpui::test]
    fn filter_choices_page_search_and_keep_selection(cx: &mut gpui::TestAppContext) {
        use feishin_core::{
            api::Api,
            model::{Server, Session},
            storage::State,
        };
        use gpui::AppContext;
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
            time::{Duration, Instant},
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Filter paging".into(),
            url: format!("http://{}", listener.local_addr().unwrap()),
            username: "fixture".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let (stop, done) = std::sync::mpsc::channel();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured_requests = requests.clone();
        let worker = std::thread::spawn(move || {
            while matches!(done.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)) {
                let Ok((mut socket, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                let path = request.split_whitespace().nth(1).unwrap();
                let url = url::Url::parse(&format!("http://localhost{path}")).unwrap();
                let params = url
                    .query_pairs()
                    .collect::<std::collections::HashMap<_, _>>();
                let start = params
                    .get("_start")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let search = params
                    .get("name")
                    .or_else(|| params.get("tag_value"))
                    .map_or("", |s| s.as_ref());
                let (values, total) = if url.path() == "/api/artist" || url.path() == "/api/tag" {
                    captured_requests.lock().unwrap().push((
                        url.path().to_owned(),
                        start,
                        search.to_owned(),
                    ));
                    let values = (0..101)
                        .map(|index| {
                            serde_json::json!({
                                "id": format!("value-{index}"), "name": format!("Value {index:03}"),
                                "tagValue": format!("Value {index:03}")
                            })
                        })
                        .filter(|value| value["name"].as_str().unwrap().contains(search))
                        .collect::<Vec<_>>();
                    let total = values.len();
                    (
                        values.into_iter().skip(start).take(100).collect::<Vec<_>>(),
                        total,
                    )
                } else {
                    (vec![], 0)
                };
                let body = serde_json::to_string(&values).unwrap();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Total-Count: {total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        struct Fixture(
            Option<std::sync::mpsc::Sender<()>>,
            Option<std::thread::JoinHandle<()>>,
        );
        impl Drop for Fixture {
            fn drop(&mut self) {
                self.0.take();
                self.1.take().unwrap().join().unwrap();
            }
        }
        let _fixture = Fixture(Some(stop), Some(worker));
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-filter-pages-{}", uuid::Uuid::new_v4()));
        let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.playlist_list = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Album);
                view.state.active_server = Some(server.id);
                view.sessions
                    .insert(server.id, Api::new(server, Session::default()).unwrap());
                view.prepare_filters(false, window, cx);
            });
            captured.replace(Some(app.clone()));
            gpui_component::Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(gpui::px(1320.), gpui::px(840.)));
        let wait = |cx: &mut gpui::VisualTestContext| {
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(301));
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                cx.run_until_parked();
                if app.read_with(cx, |view, _| !view.filter_editor.as_ref().unwrap().loading) {
                    break;
                }
                assert!(Instant::now() < deadline, "Filter choices did not load");
                std::thread::sleep(Duration::from_millis(5));
            }
        };
        for tags in [false, true] {
            if tags {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        let editor = view.filter_editor.as_mut().unwrap();
                        editor.tags = true;
                        editor
                            .tag
                            .update(cx, |input, cx| input.set_value("mood", window, cx));
                        editor
                            .search
                            .update(cx, |input, cx| input.set_value("", window, cx));
                        view.load_filter_choices(cx);
                    })
                });
            }
            wait(cx);
            app.read_with(cx, |view, _| {
                let editor = view.filter_editor.as_ref().unwrap();
                assert_eq!(editor.choices.len(), 100);
                assert!(editor.has_more);
            });
            cx.update(|window, cx| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let target = tree
                    .nodes
                    .iter()
                    .find(|(_, node)| node.label() == Some("Load more"))
                    .unwrap()
                    .0;
                window.dispatch_accessibility_action_for_test(
                    gpui::accesskit::ActionRequest {
                        action: gpui::accesskit::Action::Click,
                        target_node: target,
                        target_tree: gpui::accesskit::TreeId::ROOT,
                        data: None,
                    },
                    cx,
                );
            });
            wait(cx);
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    let editor = view.filter_editor.as_ref().unwrap();
                    assert_eq!(editor.choices.len(), 101);
                    assert_eq!(editor.choices[100].id, "value-100");
                    assert!(!editor.has_more);
                    view.select_filter_value("value-100", cx);
                    view.filter_editor
                        .as_ref()
                        .unwrap()
                        .search
                        .update(cx, |input, cx| input.set_value("099", window, cx));
                })
            });
            wait(cx);
            app.read_with(cx, |view, _| {
                let editor = view.filter_editor.as_ref().unwrap();
                assert_eq!(editor.choices.len(), 1);
                assert_eq!(editor.choices[0].id, "value-99");
                assert!(!editor.has_more);
                assert!(view.active_filters.artist_ids.contains(&"value-100".into()));
                if tags {
                    assert_eq!(
                        view.active_filters.custom["mood"],
                        serde_json::json!(["value-100"])
                    );
                }
            });
        }
        // Jellyfin supplies a cached catalog instead of the Navidrome paging API.
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                let id = view.state.active_server.unwrap();
                let mut server = view.sessions[&id].server.clone();
                server.kind = ServerType::Jellyfin;
                view.sessions
                    .insert(id, Api::new(server, Session::default()).unwrap());
                let editor = view.filter_editor.as_mut().unwrap();
                editor.tag_names = Some(vec!["Tags".into()]);
                editor.tag_values.insert(
                    "Tags".into(),
                    (0..101).map(|index| format!("Tag {index:03}")).collect(),
                );
                editor
                    .tag
                    .update(cx, |input, cx| input.set_value("Tags", window, cx));
                editor
                    .search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                view.load_filter_choices(cx);
            })
        });
        wait(cx);
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                assert_eq!(view.filter_editor.as_ref().unwrap().choices.len(), 100);
                assert!(view.filter_editor.as_ref().unwrap().has_more);
                view.load_filter_choices_page(true, cx);
            })
        });
        wait(cx);
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                let editor = view.filter_editor.as_ref().unwrap();
                assert_eq!(editor.choices.len(), 101);
                assert_eq!(editor.choices[100].id, "Tag 100");
                assert!(!editor.has_more);
                editor
                    .search
                    .update(cx, |input, cx| input.set_value("100", window, cx));
            })
        });
        wait(cx);
        app.read_with(cx, |view, _| {
            let editor = view.filter_editor.as_ref().unwrap();
            assert_eq!(editor.choices.len(), 1);
            assert_eq!(editor.choices[0].id, "Tag 100");
            assert!(!editor.has_more);
        });
        let requests = requests.lock().unwrap();
        for path in ["/api/artist", "/api/tag"] {
            assert_eq!(
                requests
                    .iter()
                    .filter(|(p, _, _)| p == path)
                    .map(|(_, start, query)| (*start, query.as_str()))
                    .collect::<Vec<_>>(),
                [(0, ""), (100, ""), (0, "099")]
            );
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn pinned_filter_search_refreshes_choices_and_stops_after_unpin(cx: &mut gpui::TestAppContext) {
        use feishin_core::{
            api::Api,
            model::{Server, Session},
            storage::State,
        };
        use gpui::AppContext;
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-pinned-filter-{}", uuid::Uuid::new_v4()));
        let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            gpui_component::Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                let server = Server {
                    id: uuid::Uuid::new_v4(),
                    name: "Fixture".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "test".into(),
                    kind: ServerType::Navidrome,
                    legacy_auth: false,
                    options: Default::default(),
                };
                view.state.active_server = Some(server.id);
                view.sessions
                    .insert(server.id, Api::new(server, Session::default()).unwrap());
                view.route = Route::Library(ItemKind::Album);
                assert!(view.filters_pinned(), "Albums start with filters open");
                view.prepare_filters(false, window, cx);
                assert!(
                    view.filter_editor.as_ref().unwrap().loading,
                    "Opening pinned filters must schedule choices"
                );
                // Cancel before the debounce expires; protocol mapping has independent fixtures.
                view.filter_generation += 1;
                view.filter_editor.as_mut().unwrap().loading = false;
                view.filter_editor
                    .as_ref()
                    .unwrap()
                    .search
                    .update(cx, |input, cx| input.set_value("Artist", window, cx));
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                assert!(
                    view.filter_editor.as_ref().unwrap().loading,
                    "Pinned search changes must schedule refreshed choices"
                );
                view.filter_generation += 1;
                view.filter_editor.as_mut().unwrap().loading = false;
                view.toggle_filter_pin(cx);
                view.filter_editor
                    .as_ref()
                    .unwrap()
                    .search
                    .update(cx, |input, cx| input.set_value("Hidden", window, cx));
            })
        });
        let generation = app.read_with(cx, |view, _| view.filter_generation);
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.filter_generation, generation);
            assert!(!view.filter_editor.as_ref().unwrap().loading);
            assert!(!view.state.settings.page_sidebars["album"]);
        });
        // The panel uses the same live editor as the full filter page. Choice
        // fetching is covered above; no network is needed for these interactions.
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.sessions.clear();
                view.toggle_filter_pin(cx);
            });
        });
        cx.simulate_resize(gpui::size(gpui::px(1320.), gpui::px(840.)));
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(600));
        cx.run_until_parked();
        let panel = cx.debug_bounds("pinned-filter-panel").unwrap();
        assert_eq!(panel.size.width, gpui::px(300.));
        assert!(panel.origin.x < cx.debug_bounds("library-list-rows").unwrap().origin.x);
        app.read_with(cx, |view, cx| {
            assert_eq!(
                view.library_grid_layout(cx).columns,
                3,
                "The original 1320px window retains three album columns beside its filter panel"
            );
        });
        fn click(cx: &mut gpui::VisualTestContext, label: &str) {
            let position = cx.update(|window, _| {
                let bounds = window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == gpui::Role::Button && node.label() == Some(label)
                    })
                    .and_then(|(_, node)| node.bounds())
                    .unwrap_or_else(|| panic!("Missing button: {label}"));
                let scale = f64::from(window.scale_factor());
                gpui::point(
                    gpui::px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                    gpui::px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
                )
            });
            cx.simulate_click(position, gpui::Modifiers::none());
        }
        click(cx, "Is favorited: None");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.active_filters.favorite, Some(true));
            assert_eq!(
                view.state.settings.views[&view.view_key()].filters.favorite,
                Some(true)
            );
        });
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.filter_editor.as_ref().unwrap().years[0]
                    .update(cx, |input, cx| input.set_value("2001", window, cx));
            });
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(299));
        cx.run_until_parked();
        app.read_with(cx, |view, _| assert_eq!(view.active_filters.min_year, None));
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2));
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.active_filters.min_year, Some(2001))
        });
        click(cx, "Reset");
        cx.run_until_parked();
        app.read_with(cx, |view, _| assert!(view.active_filters.is_empty()));
        cx.simulate_resize(gpui::size(gpui::px(1320.), gpui::px(1500.)));
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                let editor = view.filter_editor.as_mut().unwrap();
                editor.tags = true;
                editor.filters.favorite = Some(true);
                editor
                    .tag
                    .update(cx, |input, cx| input.set_value("mood", window, cx));
                view.select_filter_value("calm", cx);
                assert_eq!(
                    view.active_filters.custom["mood"],
                    serde_json::json!(["calm"])
                );
            });
        });
        cx.run_until_parked();
        click(cx, "Remove mood filter");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(
                view.filter_editor
                    .as_ref()
                    .unwrap()
                    .filters
                    .custom
                    .is_empty()
            );
            assert!(
                view.active_filters.custom.is_empty(),
                "Removing a tag applies immediately"
            );
            assert_eq!(view.active_filters.favorite, Some(true));
            let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
            assert_eq!(
                saved.settings.views[&view.view_key()].filters,
                view.active_filters
            );
        });
        cx.simulate_resize(gpui::size(gpui::px(1100.), gpui::px(840.)));
        for (font_size, visible) in [(16., true), (20., false)] {
            cx.update(|window, cx| {
                gpui_component::Theme::global_mut(cx).font_size = gpui::px(font_size);
                window.set_rem_size(gpui::px(font_size));
                window.refresh();
            });
            cx.run_until_parked();
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert_eq!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.label() == Some("Unpin")),
                    visible,
                    "The original 36em container breakpoint follows its font size"
                );
            });
        }
        cx.simulate_resize(gpui::size(gpui::px(650.), gpui::px(840.)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            let tree = window.accessibility_tree_for_test().unwrap();
            assert!(
                !tree
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some("Unpin"))
            );
            app.update(cx, |view, cx| {
                assert!(
                    view.filters_pinned(),
                    "Narrow windows retain the saved preference"
                );
                view.navigate(Route::Library(ItemKind::Song), cx);
                assert!(view.filters_pinned(), "Tracks start with filters open");
                view.navigate(Route::Favorites(ItemKind::Song), cx);
                assert!(
                    !view.filters_pinned(),
                    "Favorites has no default filter sidebar"
                );
                view.navigate(Route::Library(ItemKind::Artist), cx);
                assert!(!view.filters_pinned());
                view.route = Route::Detail(Box::new(feishin_core::model::Item {
                    kind: ItemKind::AlbumArtist,
                    ..Default::default()
                }));
                view.state
                    .settings
                    .page_sidebars
                    .insert("albumArtistAlbum".into(), true);
                view.artist_overview = true;
                assert!(
                    !view.filters_pinned(),
                    "An artist overview is separate from its album list"
                );
                view.artist_overview = false;
                assert!(view.filters_pinned());
            });
        });
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert!(saved.settings.page_sidebars["album"]);
        std::fs::remove_dir_all(dir).ok();
    }
}
