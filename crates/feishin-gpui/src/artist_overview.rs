use crate::scroll::VerticalScroll;
use crate::{
    app::{Feishin, Route},
    queue::Dragged,
};
use feishin_core::{
    artist_sections,
    model::{ArtistTab, Item, ItemKind, ServerType, Sort},
    queue::{Enqueue, PlayBehavior},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    button::ButtonVariants,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
    text::{TextView, TextViewStyle},
};

pub(crate) fn biography_view(
    info: &feishin_core::model::ArtistInfo,
    fallback: &str,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let fallback_info;
    let info = if info.biography.is_empty() && !fallback.is_empty() {
        fallback_info = feishin_core::model::ArtistInfo {
            biography: feishin_core::lyric_sources::text_from_html(fallback),
            biography_html: feishin_core::lyric_sources::formatted_html(fallback),
            ..Default::default()
        };
        &fallback_info
    } else {
        info
    };
    let content = if info.biography_html.is_empty() {
        div()
            .children(
                info.biography
                    .lines()
                    .map(|line| div().child(line.to_owned())),
            )
            .into_any_element()
    } else {
        div()
            .debug_selector(|| "artist-biography-richtext".into())
            .w_full()
            .min_w_0()
            .child(
                TextView::html(
                    "artist-biography-html",
                    info.biography_html.clone(),
                    window,
                    cx,
                )
                .style(TextViewStyle {
                    safe_urls_only: true,
                    ..Default::default()
                })
                .selectable(true),
            )
            .into_any_element()
    };
    crate::library_header::spoiler(
        "artist-biography",
        &info.biography,
        56.,
        false,
        content,
        window,
        cx,
    )
    .aria_label(crate::appearance::text("Biography", cx))
    .text_size(cx.theme().font_size * 0.875)
    .line_height(cx.theme().font_size * 1.125)
    .into_any_element()
}

#[derive(Default)]
pub struct Section {
    pub items: Vec<Item>,
    pub loading: bool,
    pub error: String,
    pub generation: u64,
    pub page: usize,
    pub selected: std::collections::HashSet<String>,
    pub selection_order: Vec<String>,
    pub scroll: UniformListScrollHandle,
    pub horizontal_scroll: ScrollHandle,
    pub random_order: std::cell::RefCell<(Vec<usize>, Vec<usize>)>,
    pub expanded_groups: std::collections::HashSet<String>,
    pub search: String,
    pub search_task: Option<Task<()>>,
    pub grid: std::cell::RefCell<crate::grid_carousel::State>,
}

fn album_columns(width: f32) -> usize {
    [520., 720., 960., 1152., 1280., 1440.]
        .into_iter()
        .filter(|breakpoint| width >= *breakpoint)
        .count()
        + 2
}

fn album_grid(width: Pixels, zoom: f32, cx: &App) -> (Div, Pixels) {
    let gap = cx.theme().font_size * 0.75;
    let columns = album_columns(f32::from(width) / zoom);
    // Chromium rounds flex bases down to 1/64 CSS px. This also prevents f32
    // addition from wrapping the final card into a spurious extra row.
    let card_width = px(
        (f32::from(width - gap * (columns - 1) as f32) / columns as f32 / zoom * 64.).floor() / 64.
            * zoom,
    )
    .max(px(1.));
    (
        div()
            .h_flex()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .gap(gap)
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse()),
        card_width,
    )
}

fn overview_frame(cx: &App) -> Div {
    let font = cx.theme().font_size;
    div()
        .debug_selector(|| "artist-overview-body".into())
        .v_flex()
        .w_full()
        .min_w_0()
        .flex_shrink_0()
        .gap(font * 2.)
        .px(font * 2.)
        .pt(font)
        .pb(font * 5.)
        .when(crate::appearance::rtl(cx), |body| body.items_end())
}

impl Feishin {
    pub(crate) fn related_artist_indices(&self) -> Vec<usize> {
        let current = match &self.route {
            Route::Detail(item) => item.id.as_str(),
            _ => "",
        };
        self.artist_sections
            .get("similarArtists")
            .into_iter()
            .flat_map(|section| section.items.iter().enumerate())
            .filter(|(_, item)| item.id != current)
            .map(|(index, _)| index)
            .collect()
    }
    fn related_artists_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        self.grid_carousel_view(crate::grid_carousel::Source::RelatedArtists, window, cx)
    }

    fn artist_albums_view(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let font = cx.theme().font_size;
        let (sort, descending) = self.artist_album_sort();
        let controls = div()
            .h_flex()
            .w_full()
            .gap(font * 0.5)
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(crate::styles::input(&self.artist_searches[2], cx)),
            )
            .child(
                crate::styles::button("artist-album-sort", cx)
                    .debug_selector(|| "artist-album-sort".into())
                    .label(
                        self.tr(feishin_core::lists::ALBUM_SORTS
                            .iter()
                            .find(|(candidate, _)| *candidate == sort)
                            .map_or(sort.label(), |(_, label)| label)),
                    )
                    .tooltip(self.tr("Sort by"))
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        move |mut menu, _, cx| {
                            for (value, label) in feishin_core::lists::ALBUM_SORTS {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(label, cx))
                                        .checked(value == sort)
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |view, cx| {
                                                view.set_artist_album_sort(value, descending, cx)
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(
                crate::styles::button("artist-album-sort-direction", cx)
                    .debug_selector(|| "artist-album-sort-direction".into())
                    .label(if descending { "↓" } else { "↑" })
                    .tooltip(self.tr(if descending {
                        "Descending"
                    } else {
                        "Ascending"
                    }))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.set_artist_album_sort(sort, !descending, cx)
                    })),
            )
            .child(
                crate::styles::button("artist-album-grouping", cx)
                    .debug_selector(|| "artist-album-grouping".into())
                    .icon(gpui_component::IconName::Settings)
                    .tooltip(self.tr("Settings"))
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        let combined = self.state.settings.artist_grouping == "all";
                        move |mut menu, _, cx| {
                            for (value, label) in [
                                ("all", "page.albumArtistDetail.groupingTypeAll"),
                                ("primary", "page.albumArtistDetail.groupingTypePrimary"),
                            ] {
                                let view = view.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(label, cx))
                                        .checked(combined == (value == "all"))
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |view, cx| {
                                                view.state.settings.artist_grouping = value.into();
                                                view.save();
                                                cx.notify();
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            );
        let mut result = div()
            .id("artist-overview-recentAlbums")
            .debug_selector(|| "artist-overview-recentAlbums".into())
            .v_flex()
            .w_full()
            .min_w_0()
            .gap(font * 0.75)
            .child(controls);
        if let Some(data) = self.artist_sections.get("recentAlbums") {
            if data.loading {
                result = result.child(self.tr("Loading..."));
            }
            if !data.error.is_empty() {
                result = result.child(data.error.clone()).child(
                    crate::styles::button("artist-albums-retry", cx)
                        .label(self.tr("Retry"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.load_artist_section("recentAlbums", cx)
                        })),
                );
            }
        }
        let groups = self.artist_album_groups();
        if let Some(data) = self.artist_sections.get_mut("recentAlbums") {
            data.expanded_groups
                .retain(|key| groups.iter().any(|(group, _)| group == key));
        }
        let width = (self.detail_header_width - font * 4.).max(px(0.));
        let mut sections = div().v_flex().w_full().min_w_0().gap(font * 2.5);
        for (group, indices) in groups {
            let label =
                feishin_core::releases::section_label(&group, &self.state.settings.language);
            let mut heading = div()
                .h_flex()
                .w_full()
                .items_center()
                .gap(font * 0.75)
                .mb(font * 0.75)
                .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                .child(
                    div()
                        .text_size(font * 1.5)
                        .line_height(font * 2.)
                        .font_weight(FontWeight::BOLD)
                        .child(label),
                )
                .child(
                    div()
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .px(font * 0.5)
                        .text_size(font * 0.75)
                        .child(indices.len().to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(2. * self.state.settings.zoom))
                        .bg(cx.theme().border),
                );
            for (label, mode) in [
                ("now", Enqueue::Now),
                ("next", Enqueue::Next),
                ("last", Enqueue::End),
            ] {
                let click = group.clone();
                let held = group.clone();
                let selector = format!("artist-album-group-{group}-{label}");
                heading = heading.child(
                    crate::styles::play_control(
                        SharedString::from(selector.clone()),
                        mode,
                        self.state.settings.zoom,
                        cx,
                    )
                    .debug_selector(move || selector.clone())
                    .disabled(!self.library_fetches.is_empty())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.play_artist_album_group(&click, mode, false, cx)
                    }))
                    .on_long_press(cx.listener(move |view, _, _, cx| {
                        view.play_artist_album_group(&held, mode, true, cx)
                    })),
                );
            }
            let data = &self.artist_sections["recentAlbums"];
            let expanded = data.expanded_groups.contains(&group);
            let visible = indices
                .iter()
                .take(if expanded { usize::MAX } else { 100 })
                .map(|index| (*index, data.items[*index].clone()))
                .collect::<Vec<_>>();
            let (mut cards, card_width) = album_grid(width, self.state.settings.zoom, cx);
            for (index, album) in visible {
                self.fetch_item_cover_at(&album, feishin_core::images::Placement::ItemCard, cx);
                cards = cards.child(
                    div()
                        .id(("artist-album-card", index))
                        .debug_selector(move || format!("artist-album-card-{index}"))
                        .w(card_width)
                        .min_w_0()
                        .flex_shrink_0()
                        .child(self.item_card(
                            &album,
                            index,
                            crate::ui::CardSource::Carousel,
                            Some(card_width),
                            cx,
                        )),
                );
            }
            let selector = format!("artist-album-group-{group}");
            let mut section = div()
                .id(SharedString::from(selector.clone()))
                .debug_selector(move || selector.clone())
                .v_flex()
                .w_full()
                .gap(font * 0.75)
                .child(heading)
                .child(cards);
            if !expanded && indices.len() > 100 {
                let selector = format!("artist-album-more-{group}");
                section = section.child(
                    div().h_flex().justify_center().child(
                        crate::styles::button(SharedString::from(selector.clone()), cx)
                            .debug_selector(move || selector.clone())
                            .ghost()
                            .label(self.tr("action.viewMore"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if let Some(data) = view.artist_sections.get_mut("recentAlbums") {
                                    data.expanded_groups.insert(group.clone());
                                }
                                cx.notify();
                            })),
                    ),
                );
            }
            sections = sections.child(section);
        }
        result.child(sections).into_any_element()
    }

    pub fn artist_overview_active(&self) -> bool {
        self.artist_overview
            && matches!(&self.route, Route::Detail(item) if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist))
    }
    pub fn load_artist_overview(&mut self, cx: &mut Context<Self>) {
        self.items.clear();
        self.has_more = false;
        self.loading = false;
        self.artist_sections.clear();
        self.load_artist_info(cx);
        let fields = self
            .state
            .settings
            .artist_items
            .iter()
            .filter(|field| !field.disabled)
            .map(|field| field.id.clone())
            .collect::<Vec<_>>();
        for id in fields {
            if id == "biography" {
                continue;
            }
            if id == "similarArtists" {
                self.artist_sections.insert(
                    id,
                    Section {
                        items: self
                            .artist_info
                            .as_ref()
                            .map(|info| info.similar.clone())
                            .unwrap_or_default(),
                        loading: self.artist_info.is_none() && self.artist_info_target.is_some(),
                        ..Default::default()
                    },
                );
            } else {
                self.load_artist_section(&id, cx);
            }
        }
        cx.notify();
    }
    pub fn load_artist_section(&mut self, id: &str, cx: &mut Context<Self>) {
        if id == "similarArtists" {
            self.artist_info_target = None;
            self.artist_info = None;
            if let Some(section) = self.artist_sections.get_mut(id) {
                section.loading = true;
                section.error.clear();
            }
            self.load_artist_info(cx);
            return;
        }
        let Route::Detail(artist) = &self.route else {
            return;
        };
        let artist = artist.as_ref().clone();
        let Some(api) = self.sessions.get(&artist.server_id).cloned() else {
            self.artist_sections.insert(
                id.into(),
                Section {
                    error: "Sign in to this artist's server".into(),
                    ..Default::default()
                },
            );
            return;
        };
        let tab = match id {
            "recentAlbums" | "compilations" => ArtistTab::Albums,
            "topSongs"
                if self.state.settings.artist_top_personal
                    || api.server.kind == ServerType::Jellyfin =>
            {
                ArtistTab::PersonalTop
            }
            "topSongs" => ArtistTab::CommunityTop,
            "favoriteSongs"
                if (self.state.settings.artist_favorites_rating
                    || !self.state.settings.show_favorites)
                    && self.state.settings.show_ratings
                    && api.server.kind != ServerType::Jellyfin =>
            {
                ArtistTab::Rated
            }
            "favoriteSongs" if self.state.settings.show_favorites => ArtistTab::Favorites,
            _ => {
                return;
            }
        };
        self.artist_section_generation += 1;
        let generation = self.artist_section_generation;
        let previous = self.artist_sections.remove(id).unwrap_or_default();
        self.artist_sections.insert(
            id.into(),
            Section {
                loading: true,
                error: String::new(),
                generation,
                search: self.artist_searches[match id {
                    "topSongs" => 0,
                    "favoriteSongs" => 1,
                    _ => 2,
                }]
                .read(cx)
                .value()
                .to_string(),
                ..previous
            },
        );
        let id = id.to_owned();
        let kind = id.clone();
        let task = cx.background_executor().spawn(smol::unblock(move || {
            let mut items = api.all_artist_items(&artist, tab)?;
            if kind == "compilations" {
                items.retain(|item| item.is_compilation);
            }
            Ok::<_, anyhow::Error>(items)
        }));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let Some(section) = this
                    .artist_sections
                    .get_mut(&id)
                    .filter(|section| section.generation == generation)
                else {
                    return;
                };
                section.loading = false;
                match result {
                    Ok(items) => {
                        section
                            .selected
                            .retain(|id| items.iter().any(|item| &item.id == id));
                        section
                            .selection_order
                            .retain(|id| section.selected.contains(id));
                        section.items = items;
                    }
                    Err(error) => {
                        log::debug!("Artist overview section unavailable: {error}");
                        section.error = error.to_string();
                    }
                }
                this.fetch_covers(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn artist_favorite_sort(&self) -> (Sort, bool) {
        let Route::Detail(artist) = &self.route else {
            return (Sort::Id, false);
        };
        let key = format!("{}:detail:albumArtistFavoriteSong", artist.server_id);
        self.state
            .settings
            .views
            .get(&key)
            .map_or((Sort::Id, false), |view| (view.sort, view.descending))
    }
    pub fn artist_album_sort(&self) -> (Sort, bool) {
        if self.artist_tab == ArtistTab::Albums {
            return (self.sort, self.descending);
        }
        let Route::Detail(artist) = &self.route else {
            return (Sort::ReleaseDate, true);
        };
        self.state
            .settings
            .views
            .get(&format!("{}:detail:albumArtistAlbum", artist.server_id))
            .map_or((Sort::ReleaseDate, true), |view| {
                (view.sort, view.descending)
            })
    }
    pub fn set_artist_album_sort(&mut self, sort: Sort, descending: bool, cx: &mut Context<Self>) {
        let Route::Detail(artist) = &self.route else {
            return;
        };
        let view = self
            .state
            .settings
            .views
            .entry(format!("{}:detail:albumArtistAlbum", artist.server_id))
            .or_default();
        view.sort = sort;
        view.descending = descending;
        if self.artist_tab == ArtistTab::Albums {
            self.sort = sort;
            self.descending = descending;
        }
        if let Some(section) = self.artist_sections.get_mut("recentAlbums") {
            section.random_order.get_mut().1.clear();
        }
        self.save();
        cx.notify();
    }
    pub fn artist_album_groups(&self) -> Vec<(String, Vec<usize>)> {
        let Route::Detail(artist) = &self.route else {
            return vec![];
        };
        let Some(section) = self.artist_sections.get("recentAlbums") else {
            return vec![];
        };
        let mut indices = feishin_core::lists::search_albums(&section.items, &section.search);
        let (sort, descending) = self.artist_album_sort();
        if sort == Sort::Random {
            let mut cached = section.random_order.borrow_mut();
            if cached.0 != indices || cached.1.is_empty() {
                use rand::seq::SliceRandom;
                let mut shuffled = indices.clone();
                shuffled.shuffle(&mut rand::thread_rng());
                *cached = (indices, shuffled);
            }
            indices = cached.1.clone();
        } else if sort == Sort::Id && descending {
            indices.reverse();
        } else {
            indices.sort_by(|a, b| {
                feishin_core::lists::compare_albums(
                    &section.items[*a],
                    &section.items[*b],
                    sort,
                    descending,
                )
            });
        }
        let items = indices
            .iter()
            .map(|index| section.items[*index].clone())
            .collect::<Vec<_>>();
        feishin_core::releases::groups(
            &items,
            &artist.id,
            self.state.settings.artist_grouping == "all",
            &self.state.settings.artist_release_types,
        )
        .into_iter()
        .map(|(label, rows)| {
            (
                label,
                rows.into_iter().map(|index| indices[index]).collect(),
            )
        })
        .collect()
    }
    pub fn play_artist_album_group(
        &mut self,
        group: &str,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Some((_, indices)) = self
            .artist_album_groups()
            .into_iter()
            .find(|(key, _)| key == group)
        else {
            return;
        };
        let albums = indices
            .into_iter()
            .map(|index| self.artist_sections["recentAlbums"].items[index].clone())
            .collect();
        self.enqueue_items(albums, mode, shuffle, cx);
    }
    pub fn set_artist_favorite_sort(
        &mut self,
        sort: Sort,
        descending: bool,
        cx: &mut Context<Self>,
    ) {
        let Route::Detail(artist) = &self.route else {
            return;
        };
        let key = format!("{}:detail:albumArtistFavoriteSong", artist.server_id);
        let view = self.state.settings.views.entry(key).or_default();
        view.sort = sort;
        view.descending = descending;
        if let Some(section) = self.artist_sections.get_mut("favoriteSongs") {
            section.random_order.get_mut().1.clear();
        }
        self.save();
        cx.notify();
    }
    pub fn artist_section_indices(&self, id: &str, _cx: &App) -> Vec<usize> {
        if id == "similarArtists" {
            return self.related_artist_indices();
        }
        if id == "recentAlbums" {
            return self
                .artist_album_groups()
                .into_iter()
                .flat_map(|(_, indices)| indices)
                .collect();
        }
        let Some(section) = self.artist_sections.get(id) else {
            return vec![];
        };
        let mut indices =
            feishin_core::lists::search_items(&section.items, &section.search, ItemKind::Song);
        if id == "favoriteSongs" {
            let (sort, descending) = self.artist_favorite_sort();
            if sort == Sort::Random {
                let mut cached = section.random_order.borrow_mut();
                if cached.0 != indices || cached.1.is_empty() {
                    use rand::seq::SliceRandom;
                    let mut shuffled = indices.clone();
                    shuffled.shuffle(&mut rand::thread_rng());
                    *cached = (indices, shuffled);
                }
                indices = cached.1.clone();
            } else if sort != Sort::Id {
                indices.sort_by(|a, b| {
                    let order = feishin_core::lists::compare(
                        &section.items[*a],
                        &section.items[*b],
                        sort.column(),
                    );
                    if descending { order.reverse() } else { order }
                });
            } else if descending {
                indices.reverse();
            }
        }
        indices
    }

    pub fn play_artist_section(
        &mut self,
        id: &str,
        index: usize,
        mode: Option<Enqueue>,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = self.artist_sections.get(id) else {
            return;
        };
        let Some(start) = section.items.get(index).cloned() else {
            return;
        };
        let behavior = self.state.settings.play_button_behavior;
        let shuffle = mode.is_none() && behavior == PlayBehavior::Shuffle;
        let mode = mode.unwrap_or(behavior.mode());
        if start.kind != ItemKind::Song {
            self.enqueue(start, mode, cx);
            return;
        }
        let mut songs = self
            .artist_section_indices(id, cx)
            .into_iter()
            .map(|index| section.items[index].clone())
            .collect::<Vec<_>>();
        if matches!(mode, Enqueue::Now) && shuffle {
            use rand::seq::SliceRandom;
            songs.shuffle(&mut rand::thread_rng());
        }
        self.add_tracks_from(songs, mode, Some(&start), cx);
        cx.notify();
    }
    pub fn artist_section_action_items(&self, id: &str, index: usize, cx: &App) -> Vec<Item> {
        let Some(section) = self.artist_sections.get(id) else {
            return vec![];
        };
        let Some(item) = section.items.get(index) else {
            return vec![];
        };
        if !section.selected.contains(&item.id) {
            return vec![item.clone()];
        }
        self.artist_section_indices(id, cx)
            .into_iter()
            .filter(|index| section.selected.contains(&section.items[*index].id))
            .map(|index| section.items[index].clone())
            .collect()
    }
    pub fn first_artist_selection(&self, id: &str, cx: &App) -> Option<usize> {
        let section = self.artist_sections.get(id)?;
        self.artist_section_indices(id, cx)
            .into_iter()
            .find(|index| section.selected.contains(&section.items[*index].id))
    }
    pub fn select_artist_section(&mut self, id: &str, cx: &mut Context<Self>) {
        let rows = self.artist_section_indices(id, cx);
        if let Some(section) = self.artist_sections.get_mut(id) {
            let rows = rows
                .into_iter()
                .map(|index| section.items[index].id.clone())
                .collect();
            crate::lists::toggle_all_rows(
                &mut section.selected,
                &mut section.selection_order,
                rows,
            );
        }
        cx.notify();
    }
    pub fn artist_section_click(
        &mut self,
        id: &str,
        index: usize,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if event.click_count() == 2 {
            self.play_artist_section(id, index, Some(Enqueue::Now), cx);
            return;
        }
        let rows = self.artist_section_indices(id, cx);
        if let Some(section) = self.artist_sections.get_mut(id)
            && let Some(item) = section.items.get(index)
        {
            let key = item.id.clone();
            let rows: Vec<_> = rows
                .into_iter()
                .map(|index| section.items[index].id.clone())
                .collect();
            crate::lists::select_row(
                &mut section.selected,
                &mut section.selection_order,
                key,
                event.modifiers(),
                |a, b| crate::lists::selection_range(&rows, a, b),
            );
        }
        cx.notify();
    }
    pub fn artist_section_menu(
        &self,
        id: &str,
        item: &Item,
        index: usize,
        cx: &Context<Self>,
    ) -> AnyElement {
        crate::styles::button(("artist-song-menu", index), cx)
            .ghost()
            .label("⋯")
            .tooltip(self.tr("More"))
            .dropdown_menu(self.item_menu_builder(
                item,
                self.artist_section_action_items(id, index, cx),
                false,
                cx,
            ))
            .into_any_element()
    }
    pub fn enqueue_artist_section(
        &mut self,
        id: &str,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(section) = self.artist_sections.get(id) else {
            return;
        };
        let mut items = section.items.clone();
        if shuffle {
            use rand::seq::SliceRandom;
            items.shuffle(&mut rand::thread_rng());
        }
        self.add_tracks_from(items, mode, None, cx);
    }
    fn artist_song_table(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let Some(section) = self.artist_sections.get(id) else {
            return div().into_any_element();
        };
        let indices = self.artist_section_indices(id, cx);
        if indices.is_empty() {
            return div()
                .p_4()
                .child(self.tr("common.noResultsFromQuery"))
                .into_any_element();
        }
        let mut config = self
            .state
            .settings
            .lists
            .get("song")
            .cloned()
            .unwrap_or_else(|| feishin_core::lists::default_for("song"));
        config.display = "table".into();
        let mut columns: Vec<_> = config
            .table
            .columns
            .iter()
            .filter(|column| column.is_enabled && column.id != "albumGroup")
            .cloned()
            .collect();
        columns.sort_by_key(|column| match column.pinned.as_deref() {
            Some("left") => 0,
            Some("right") => 2,
            _ => 1,
        });
        let row_config = config.clone();
        let row_columns = columns.clone();
        let section_id = id.to_owned();
        let rows = uniform_list(
            SharedString::from(format!("artist-overview-songs-{id}")),
            indices.len(),
            cx.processor(move |this, range: std::ops::Range<usize>, _, cx| {
                range
                    .map(|display_index| {
                        let index = indices[display_index];
                        let Some(section) = this.artist_sections.get(&section_id) else {
                            return div().into_any_element();
                        };
                        let Some(item) = section.items.get(index).cloned() else {
                            return div().into_any_element();
                        };
                        let selected = section.selected.contains(&item.id);
                        this.fetch_item_cover_at(&item, feishin_core::images::Placement::Table, cx);
                        let row = crate::lists::TableRow::ArtistSection(section_id.clone());
                        let hovered = row.clone();
                        let selector = section_id.clone();
                        div()
                            .id(("artist-song-row", index))
                            .debug_selector(move || {
                                format!("artist-section-song-{selector}-{index}")
                            })
                            .role(Role::Row)
                            .aria_label(item.name.clone())
                            .aria_selected(selected)
                            .aria_row_index(display_index)
                            .h(px(
                                row_config.table.size.row_height() * this.state.settings.zoom
                            ))
                            .w_full()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .when(
                                selected
                                    || row_config.table.enable_alternate_row_colors
                                        && display_index % 2 == 1,
                                |div| div.bg(cx.theme().secondary),
                            )
                            .when(row_config.table.enable_horizontal_borders, |div| {
                                div.border_b_1().border_color(cx.theme().border)
                            })
                            .when(row_config.table.enable_row_hover_highlight, |div| {
                                div.hover(|style| style.bg(cx.theme().secondary))
                            })
                            .on_hover(cx.listener(move |this, hovered_state, _, cx| {
                                this.hover_table_row(hovered.clone(), index, *hovered_state, cx)
                            }))
                            .on_drag(
                                Dragged {
                                    items: this.artist_section_action_items(&section_id, index, cx),
                                    keys: vec![],
                                    sidebar_playlist: false,
                                    playlist_source: None,
                                },
                                crate::context_preview::drag_constructor(cx),
                            )
                            .child(this.metadata_cells(
                                &item,
                                index,
                                &row_config,
                                row_columns.clone(),
                                row,
                                cx,
                            ))
                            .context_menu(this.item_menu_builder(
                                &item,
                                this.artist_section_action_items(&section_id, index, cx),
                                false,
                                cx,
                            ))
                            .into_any_element()
                    })
                    .collect()
            }),
        )
        .track_scroll(section.scroll.clone())
        .size_full();
        let focus_index = usize::from(id == "favoriteSongs");
        // The original artist container uses 64px for default rows; table rows themselves are 66px.
        let row_height = match config.table.size {
            feishin_core::lists::Size::Default => 64.,
            size => size.row_height(),
        };
        let count = self.artist_section_indices(id, cx).len();
        let selector = id.to_owned();
        div()
            .id(SharedString::from(format!("artist-table-{id}")))
            .debug_selector(move || format!("artist-table-{selector}"))
            .role(Role::Table)
            .aria_label(self.tr(artist_sections::label(id)))
            .key_context("ArtistSongList")
            .track_focus(&self.artist_song_focus[focus_index])
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, _| {
                    window.focus(&this.artist_song_focus[focus_index])
                }),
            )
            .v_flex()
            .w_full()
            .when(config.table.enable_header, |div| {
                let cells = columns
                    .into_iter()
                    .map(|column| {
                        let label = crate::lists::table_header(&column, cx).into_any_element();
                        let label =
                            self.column_drag_controls_for("song", &column, label, false, cx);
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
                            format!("artist-header-scroll-{id}"),
                            cells,
                            &config.table,
                            false,
                            &section.horizontal_scroll,
                            cx,
                        )),
                )
            })
            .child(
                div()
                    .relative()
                    .h(px(row_height
                        * count.min(5) as f32
                        * self.state.settings.zoom))
                    .w_full()
                    .child(rows)
                    .child(crate::scroll::Scrollbar::vertical(
                        SharedString::from(format!("artist-scrollbar-{id}")),
                        &section.scroll,
                    ))
                    .child(
                        crate::scroll::Scrollbar::horizontal(
                            SharedString::from(format!("artist-horizontal-scrollbar-{id}")),
                            &section.horizontal_scroll,
                        )
                        .in_viewport(&section.scroll),
                    ),
            )
            .into_any_element()
    }
    pub fn artist_overview_view(
        &mut self,
        header: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let font = cx.theme().font_size;
        let content = div()
            .id("artist-overview-content")
            .debug_selector(|| "artist-overview-content".into())
            .v_flex()
            .size_full()
            .native_vertical_scroll()
            .scroll_handle(&self.page_scroll)
            .child(header);
        let mut body = overview_frame(cx);
        if let Route::Detail(artist) = &self.route {
            let radio = artist.as_ref().clone();
            body = body.child(
                div()
                    .h_flex()
                    .w_full()
                    .flex_wrap()
                    .gap(font)
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                    .children(
                        [
                            (
                                "discography",
                                "page.albumArtistDetail.viewDiscography",
                                ArtistTab::Albums,
                            ),
                            (
                                "songs",
                                "page.albumArtistDetail.viewAllTracks",
                                ArtistTab::Songs,
                            ),
                        ]
                        .into_iter()
                        .map(|(id, label, tab)| {
                            crate::styles::button(
                                SharedString::from(format!("artist-overview-action-{id}")),
                                cx,
                            )
                            .debug_selector(move || format!("artist-overview-action-{id}"))
                            .ghost()
                            .h(font * 1.5)
                            .p_0()
                            .text_size(font * 0.875)
                            .label(self.tr(label).to_uppercase())
                            .on_click(
                                cx.listener(move |view, _, _, cx| view.change_artist_tab(tab, cx)),
                            )
                        }),
                    )
                    .child(
                        crate::styles::button("artist-overview-radio", cx)
                            .debug_selector(|| "artist-overview-radio".into())
                            .ghost()
                            .h(font * 1.5)
                            .p_0()
                            .text_size(font * 0.875)
                            .label(self.tr("player.artistRadio").to_uppercase())
                            .icon(
                                gpui_component::Icon::default()
                                    .path("native-icons/radio.svg")
                                    .size(font),
                            )
                            .disabled(!self.library_fetches.is_empty())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.enqueue_artist_radio(radio.clone(), Enqueue::Now, false, cx)
                            })),
                    ),
            );
            let genres = feishin_core::genres::links(artist);
            if !genres.is_empty() {
                let title = feishin_core::i18n::translate_with(
                    &self.state.settings.language,
                    "entity.genre",
                    &serde_json::json!({"count":genres.len()}),
                )
                .unwrap_or_else(|| self.tr("entity.genre"));
                body = body.child(
                    div()
                        .debug_selector(|| "artist-overview-genres".into())
                        .v_flex()
                        .w_full()
                        .gap(font * 0.25)
                        .child(
                            div()
                                .text_size(font * 0.8125)
                                .line_height(font)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title.to_uppercase()),
                        )
                        .child(self.detail_genre_links(artist, cx)),
                );
            }
            if !self.state.settings.external_links.links(artist).is_empty() {
                body = body.child(
                    div()
                        .debug_selector(|| "artist-overview-external".into())
                        .v_flex()
                        .w_full()
                        .gap(font * 0.25)
                        .child(
                            div()
                                .text_size(font * 0.8125)
                                .line_height(font)
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.tr("common.externalLinks").to_uppercase()),
                        )
                        .child(self.external_links(artist, cx)),
                );
            }
        }
        let fields = self.state.settings.artist_items.clone();
        let jellyfin = matches!(&self.route, Route::Detail(item) if self.sessions.get(&item.server_id).is_some_and(|api| api.server.kind == ServerType::Jellyfin));
        let ratings = self.state.settings.show_ratings && !jellyfin;
        let rated = (self.state.settings.artist_favorites_rating
            || !self.state.settings.show_favorites)
            && ratings;
        let personal = self.state.settings.artist_top_personal || jellyfin;
        for field in fields.into_iter().filter(|field| !field.disabled) {
            let id = field.id;
            if id == "recentAlbums" {
                body = body.child(self.artist_albums_view(cx));
                continue;
            }
            if id == "similarArtists" {
                if let Some(related) = self.related_artists_view(window, cx) {
                    body = body.child(related);
                }
                continue;
            }
            let selector = id.clone();
            let mut section = div()
                .id(SharedString::from(format!("artist-overview-{id}")))
                .debug_selector(move || format!("artist-overview-{selector}"))
                .v_flex()
                .gap_2()
                .w_full();
            let mut header = div().h_flex().gap_2().items_center().child(
                div()
                    .flex_1()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.tr(artist_sections::label(&id))),
            );
            if id == "biography" {
                let artist = match &self.route {
                    Route::Detail(artist) => artist,
                    _ => unreachable!(),
                };
                let title = feishin_core::i18n::translate_with(
                    &self.state.settings.language,
                    "page.albumArtistDetail.about",
                    &serde_json::json!({"artist": artist.name}),
                )
                .unwrap_or_else(|| self.tr("Biography"));
                section = section.max_w(px(1280. * self.state.settings.zoom)).gap_0();
                header = div()
                    .text_size(font * 1.5)
                    .line_height(font * 2.)
                    .font_weight(FontWeight::BOLD)
                    .child(title);
                if let Some(info) = &self.artist_info {
                    let fallback = match &self.route {
                        Route::Detail(artist) => artist.comment.as_str(),
                        _ => "",
                    };
                    if info.biography.is_empty() && fallback.is_empty() {
                        continue;
                    }
                    section = section
                        .child(header)
                        .child(biography_view(info, fallback, window, cx));
                } else if !self.artist_info_error.is_empty() {
                    section = section
                        .child(header)
                        .child(self.artist_info_error.clone())
                        .child(
                            crate::styles::button("artist-biography-retry", cx)
                                .label(self.tr("Retry"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.artist_info_target = None;
                                    this.load_artist_info(cx);
                                })),
                        );
                    if let Route::Detail(artist) = &self.route
                        && !artist.comment.is_empty()
                    {
                        section = section.child(biography_view(
                            &Default::default(),
                            &artist.comment,
                            window,
                            cx,
                        ));
                    }
                } else {
                    section = section
                        .child(header)
                        .child(self.tr("Loading artist information..."));
                }
                body = body.child(section);
                continue;
            }
            if id == "topSongs" {
                header = header.child(
                    crate::styles::button("artist-top-mode", cx)
                        .label(self.tr(if personal {
                            "Most played"
                        } else {
                            "Community top songs"
                        }))
                        .disabled(jellyfin)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.settings.artist_top_personal =
                                !this.state.settings.artist_top_personal;
                            this.save();
                            this.load_artist_section("topSongs", cx);
                        })),
                );
            }
            if id == "favoriteSongs" {
                if !self.state.settings.show_favorites && !ratings {
                    continue;
                }
                header = header.child(
                    crate::styles::button("artist-favorite-mode", cx)
                        .label(self.tr(if rated {
                            "Rated songs"
                        } else {
                            "Favorite songs"
                        }))
                        .disabled(!self.state.settings.show_favorites || !ratings)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.settings.artist_favorites_rating =
                                !this.state.settings.artist_favorites_rating;
                            this.save();
                            this.load_artist_section("favoriteSongs", cx);
                        })),
                );
            }
            if id == "favoriteSongs" {
                let (sort, descending) = self.artist_favorite_sort();
                header = header
                    .child(
                        crate::styles::button("artist-favorite-sort", cx)
                            .debug_selector(|| "artist-favorite-sort".into())
                            .label(
                                self.tr(feishin_core::lists::SONG_SORTS
                                    .iter()
                                    .find(|(candidate, _)| *candidate == sort)
                                    .map_or(sort.label(), |(_, label)| label)),
                            )
                            .tooltip(self.tr("Sort by"))
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                move |mut menu, _, cx| {
                                    for (sort, label) in feishin_core::lists::SONG_SORTS {
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(crate::appearance::text(label, cx))
                                                .on_click(move |_, _, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.set_artist_favorite_sort(
                                                            sort, descending, cx,
                                                        )
                                                    })
                                                    .ok();
                                                }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        crate::styles::button("artist-favorite-sort-direction", cx)
                            .debug_selector(|| "artist-favorite-sort-direction".into())
                            .label(self.tr(if descending {
                                "Descending"
                            } else {
                                "Ascending"
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_artist_favorite_sort(sort, !descending, cx)
                            })),
                    );
            }
            let tab = match id.as_str() {
                "recentAlbums" | "compilations" => ArtistTab::Albums,
                "similarArtists" => ArtistTab::Similar,
                "topSongs" if personal => ArtistTab::PersonalTop,
                "topSongs" => ArtistTab::CommunityTop,
                "favoriteSongs" if rated => ArtistTab::Rated,
                _ => ArtistTab::Favorites,
            };
            header = header.child(
                crate::styles::button(
                    SharedString::from(format!("artist-overview-see-all-{id}")),
                    cx,
                )
                .label(self.tr("See all"))
                .on_click(cx.listener(move |this, _, _, cx| this.change_artist_tab(tab, cx))),
            );
            if id == "topSongs" || id == "favoriteSongs" {
                for (label, mode) in [
                    ("now", Enqueue::Now),
                    ("next", Enqueue::Next),
                    ("last", Enqueue::End),
                ] {
                    let source = id.clone();
                    let held = source.clone();
                    let disabled = self
                        .artist_sections
                        .get(&id)
                        .is_none_or(|section| section.items.is_empty());
                    header = header.child(
                        crate::styles::play_control(
                            SharedString::from(format!("artist-section-play-{id}-{label}")),
                            mode,
                            self.state.settings.zoom,
                            cx,
                        )
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.enqueue_artist_section(&source, mode, false, cx)
                        }))
                        .on_long_press(cx.listener(
                            move |this, _, _, cx| {
                                this.enqueue_artist_section(&held, mode, true, cx)
                            },
                        )),
                    );
                }
            }
            section = section.child(header);
            if id != "similarArtists" {
                section = section.child(crate::styles::input(
                    &self.artist_searches[match id.as_str() {
                        "topSongs" => 0,
                        "favoriteSongs" => 1,
                        _ => 2,
                    }],
                    cx,
                ));
            }
            if let Some(data) = self.artist_sections.get(&id) {
                if data.loading {
                    section = section.child(self.tr("Loading..."));
                }
                if !data.error.is_empty() {
                    let retry = id.clone();
                    section = section.child(data.error.clone()).child(
                        crate::styles::button(
                            SharedString::from(format!("artist-overview-retry-{id}")),
                            cx,
                        )
                        .label(self.tr("Retry"))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.load_artist_section(&retry, cx)),
                        ),
                    );
                }
                let indices = self.artist_section_indices(&id, cx);
                if id == "topSongs" || id == "favoriteSongs" {
                    section = section.child(self.artist_song_table(&id, cx));
                } else {
                    let page = data.page;
                    let mut cards = div().h_flex().flex_wrap().gap_3();
                    let items = indices
                        .iter()
                        .skip(page * 12)
                        .take(12)
                        .map(|index| (*index, data.items[*index].clone()))
                        .collect::<Vec<_>>();
                    let group_labels = if id == "recentAlbums" {
                        if let Route::Detail(artist) = &self.route {
                            feishin_core::releases::groups(
                                &data.items,
                                &artist.id,
                                self.state.settings.artist_grouping == "all",
                                &self.state.settings.artist_release_types,
                            )
                            .into_iter()
                            .flat_map(|(label, indices)| {
                                indices.into_iter().map(move |index| (index, label.clone()))
                            })
                            .collect::<std::collections::HashMap<_, _>>()
                        } else {
                            Default::default()
                        }
                    } else {
                        Default::default()
                    };
                    let mut last_group = None;
                    for (index, item) in items {
                        if let Some(label) = group_labels.get(&index)
                            && last_group.as_ref() != Some(label)
                        {
                            cards = cards.child(
                                div()
                                    .w_full()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.tr(label)),
                            );
                            last_group = Some(label.clone());
                        }
                        self.fetch_item_cover_at(
                            &item,
                            feishin_core::images::Placement::ItemCard,
                            cx,
                        );
                        let open = item.clone();
                        cards = cards.child(
                            div()
                                .w(px(180.))
                                .v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .id(("artist-overview-item", index))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.navigate(Route::Detail(Box::new(open.clone())), cx)
                                        }))
                                        .child(self.artwork_at(
                                            &item,
                                            120.,
                                            feishin_core::images::Placement::ItemCard,
                                            cx,
                                        ))
                                        .child(div().truncate().child(item.name.clone())),
                                )
                                .child(self.item_actions(&item, index, cx)),
                        );
                    }
                    section = section.child(cards);
                    if indices.len() > 12 {
                        let before = id.clone();
                        let after = id.clone();
                        section = section.child(
                            div()
                                .h_flex()
                                .gap_2()
                                .child(
                                    crate::styles::button(
                                        SharedString::from(format!("artist-overview-prev-{id}")),
                                        cx,
                                    )
                                    .label(self.tr("Previous page"))
                                    .disabled(page == 0)
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if let Some(section) =
                                                this.artist_sections.get_mut(&before)
                                            {
                                                section.page = section.page.saturating_sub(1);
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    crate::styles::button(
                                        SharedString::from(format!("artist-overview-next-{id}")),
                                        cx,
                                    )
                                    .label(self.tr("Next page"))
                                    .disabled((page + 1) * 12 >= indices.len())
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            if let Some(section) =
                                                this.artist_sections.get_mut(&after)
                                            {
                                                section.page += 1;
                                            }
                                            cx.notify();
                                        },
                                    )),
                                ),
                        );
                    }
                }
            }
            body = body.child(section);
        }
        content.child(body).into_any_element()
    }
}

#[cfg(test)]
mod layout_tests {
    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px};
    #[gpui::test]
    fn album_grid_matches_original_responsive_columns_and_rtl(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        struct Probe(serde_json::Value, f32);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui_component::Theme::global_mut(cx).font_size =
                    px(self.0["font"].as_f64().unwrap() as f32 * self.1);
                cx.set_global(crate::appearance::Language(
                    if self.0["rtl"] == true { "ar" } else { "en" }.into(),
                ));
                let width = px(self.0["width"].as_f64().unwrap() as f32 * self.1);
                let (grid, card_width) = super::album_grid(width, self.1, cx);
                div()
                    .w(width)
                    .child(
                        grid.debug_selector(|| "grid".into())
                            .children((0..17).map(|index| {
                                div()
                                    .debug_selector(move || format!("card-{index}"))
                                    .w(card_width)
                                    .h(px(89. * self.1))
                                    .flex_shrink_0()
                            })),
                    )
            }
        }
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/artist-album-layout-reference.json"))
                .unwrap();
        let (probe, cx) = cx.add_window_view(|_, _| Probe(cases[0].clone(), 1.));
        cx.simulate_resize(gpui::size(px(3000.), px(2000.)));
        for zoom in [1., 1.5] {
            for case in &cases {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        *view = Probe(case.clone(), zoom);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                assert_eq!(
                    super::album_columns(case["width"].as_f64().unwrap() as f32),
                    case["columns"].as_u64().unwrap() as usize
                );
                let origin = cx.debug_bounds("grid").unwrap().origin;
                for index in 0..17 {
                    let actual = cx.debug_bounds(&format!("card-{index}")).unwrap();
                    for (axis, (actual, expected)) in [
                        actual.origin.x - origin.x,
                        actual.origin.y - origin.y,
                        actual.size.width,
                        actual.size.height,
                    ]
                    .into_iter()
                    .zip(case["cards"][index].as_array().unwrap())
                    .enumerate()
                    {
                        let offset = if axis < 2 {
                            case["grid"][axis].as_f64().unwrap()
                        } else {
                            0.
                        };
                        let expected = (expected.as_f64().unwrap() - offset) as f32 * zoom;
                        assert!(
                            (f32::from(actual) - expected).abs() < 1.01,
                            "{case} zoom {zoom} card {index}: {actual:?} vs {expected}"
                        );
                    }
                }
            }
        }
    }
    #[gpui::test]
    fn overview_content_matches_original_grid_layout(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        struct Probe(serde_json::Value, f32);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui_component::Theme::global_mut(cx).font_size =
                    px(self.0["font"].as_f64().unwrap() as f32 * self.1);
                cx.set_global(crate::appearance::Language(
                    if self.0["rtl"] == true { "ar" } else { "en" }.into(),
                ));
                div()
                    .w(px(self.0["width"].as_f64().unwrap() as f32 * self.1))
                    .child(
                        super::overview_frame(cx).children(
                            ["actions", "genres", "external", "biography"]
                                .into_iter()
                                .map(|id| {
                                    div()
                                        .debug_selector(move || id.into())
                                        .w_full()
                                        .flex_shrink_0()
                                        .h(px(if id == "actions" { 24. } else { 80. } * self.1))
                                        .when(id == "biography", |section| {
                                            section.max_w(px(1280. * self.1))
                                        })
                                }),
                        ),
                    )
            }
        }
        let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/artist-overview-layout-reference.json"
        ))
        .unwrap();
        let (probe, cx) = cx.add_window_view(|_, _| Probe(cases[0].clone(), 1.));
        cx.simulate_resize(gpui::size(px(2500.), px(1600.)));
        for zoom in [1., 1.5] {
            for case in &cases {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        *view = Probe(case.clone(), zoom);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let body = cx.debug_bounds("artist-overview-body").unwrap();
                for id in ["body", "actions", "genres", "external", "biography"] {
                    let actual = if id == "body" {
                        body
                    } else {
                        cx.debug_bounds(id).unwrap()
                    };
                    let expected = &case[id];
                    for (key, actual, offset) in [
                        (
                            "x",
                            actual.origin.x - body.origin.x,
                            case["body"]["x"].as_f64().unwrap(),
                        ),
                        ("y", actual.origin.y - body.origin.y, 0.),
                        ("width", actual.size.width, 0.),
                        ("height", actual.size.height, 0.),
                    ] {
                        let expected = (expected[key].as_f64().unwrap() - offset) as f32 * zoom;
                        assert!(
                            (f32::from(actual) - expected).abs() <= 1.01,
                            "{case} zoom {zoom} {id} {key}: {actual:?} vs {expected}"
                        );
                    }
                }
            }
        }
    }
}
