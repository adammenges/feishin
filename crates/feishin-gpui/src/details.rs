use crate::app::{Feishin, Route};
use feishin_core::{
    filters::Filters,
    home::Kind,
    lists,
    model::{Item, ItemKind, ListQuery, ServerType, Sort},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants},
    checkbox::Checkbox,
};

pub struct RelatedAlbums {
    pub title: String,
    pub query: ListQuery,
    pub state: crate::home::Section,
}

pub enum AlbumPageRow {
    Disc(u32, Vec<usize>),
    Tracks(Vec<usize>),
    Metadata,
    Controls,
    GridEnd,
    Copyright,
    RelatedStart,
    Bottom,
    Related(usize),
}

#[derive(Clone, Copy)]
pub(crate) struct ContentLayout {
    pub large: bool,
    pub wide: bool,
    pub rtl: bool,
    pub font: Pixels,
    pub zoom: f32,
}
impl ContentLayout {
    pub fn new(width: Pixels, font: Pixels, zoom: f32) -> Self {
        Self {
            large: width >= font * 75.,
            wide: width >= font * 48.,
            rtl: false,
            font,
            zoom,
        }
    }
    pub fn rtl(mut self, rtl: bool) -> Self {
        self.rtl = rtl;
        self
    }
    pub fn padding(self) -> Pixels {
        self.font * 2.
    }
    pub fn metadata_width(self) -> Pixels {
        px(300. * self.zoom)
    }
    pub fn songs_right_padding(self) -> Pixels {
        self.padding()
            + if self.large {
                self.metadata_width() + self.font * 1.5
            } else {
                px(0.)
            }
    }
    pub fn songs_frame(self) -> Div {
        div()
            .w_full()
            .min_w_0()
            .pl(if self.rtl {
                self.songs_right_padding()
            } else {
                self.padding()
            })
            .pr(if self.rtl {
                self.padding()
            } else {
                self.songs_right_padding()
            })
    }
    pub fn metadata_frame(self) -> Div {
        div()
            .v_flex()
            .min_h_0()
            .flex_wrap()
            .items_center()
            .text_center()
            .gap(self.font * 1.5)
            .when(self.wide, |column| {
                column
                    .flex_row()
                    .when(self.rtl, |column| column.flex_row_reverse())
                    .justify_start()
                    .text_left()
            })
    }
}

/// Keep metadata beside a virtual list, constrained to the same grid section.
/// Both children are measured before placing the sticky column, so scrolling
/// does not depend on the previous frame's row positions.
pub(crate) fn sticky_metadata(
    mut page: AnyElement,
    metadata: AnyElement,
    scroll: ListState,
    section: std::ops::Range<usize>,
    layout: ContentLayout,
    on_extra: impl Fn(Pixels, &mut App) + 'static,
) -> AnyElement {
    canvas(
        move |bounds, window, cx| {
            let mut metadata = div()
                .v_flex()
                .w(layout.metadata_width())
                .max_h(
                    (window.viewport_size().height - px(90. * layout.zoom) - layout.font * 5.)
                        .max(px(0.)),
                )
                .child(metadata)
                .into_any_element();
            let metadata_size = metadata.layout_as_root(
                size(
                    AvailableSpace::Definite(layout.metadata_width()),
                    AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            page.layout_as_root(
                size(
                    AvailableSpace::Definite(bounds.size.width),
                    AvailableSpace::Definite(bounds.size.height),
                ),
                window,
                cx,
            );
            page.prepaint_at(bounds.origin, window, cx);
            if let Some(height) = scroll.measured_height_for_range(section.clone()) {
                on_extra((metadata_size.height - height).max(px(0.)), cx);
            }
            let viewport = scroll.viewport_bounds();
            let top_row = scroll.logical_scroll_top().item_ix;
            let start = scroll.bounds_for_item(section.start).map(|row| row.top());
            let end = scroll.bounds_for_item(section.end).map(|row| row.bottom());
            let top = start
                .unwrap_or(viewport.top())
                .max(viewport.top() + layout.font * 5.);
            let top = end.map_or(top, |end| top.min(end - metadata_size.height));
            let visible = (top_row <= section.end)
                && (start.is_some() || top_row >= section.start)
                && top < viewport.bottom()
                && top + metadata_size.height > viewport.top();
            if visible {
                window.with_content_mask(Some(ContentMask { bounds: viewport }), |window| {
                    metadata.prepaint_at(
                        point(
                            if layout.rtl {
                                viewport.left() + layout.padding()
                            } else {
                                viewport.right() - layout.padding() - layout.metadata_width()
                            },
                            top,
                        ),
                        window,
                        cx,
                    );
                });
            }
            (page, visible.then_some(metadata), viewport)
        },
        |_, (mut page, metadata, viewport), window, cx| {
            page.paint(window, cx);
            if let Some(mut metadata) = metadata {
                window.with_content_mask(Some(ContentMask { bounds: viewport }), |window| {
                    metadata.paint(window, cx)
                });
            }
        },
    )
    .flex_1()
    .min_h_0()
    .w_full()
    .into_any_element()
}

pub(crate) fn sticky_disc_index(
    counts: &[usize],
    row_height: Pixels,
    header_height: Pixels,
    header: bool,
    table_top: Pixels,
    sticky_top: Pixels,
) -> Option<usize> {
    let mut row_index = 1;
    let mut selected = None;
    for (index, count) in counts.iter().enumerate() {
        let row_top = table_top
            + header_height
            + if header {
                header_height + row_height * (row_index - 1) as f32
            } else {
                row_height * row_index as f32
            };
        if row_top <= sticky_top + row_height {
            selected = Some(index);
        } else if selected.is_some() {
            break;
        }
        row_index += count + 1;
    }
    selected
}

#[derive(Default)]
pub struct AlbumTracks {
    pub items: Vec<Item>,
    pub loading: bool,
    pub error: Option<String>,
}
impl Feishin {
    pub(crate) fn missing_album_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use gpui_component::{Icon, Sizable};
        let Route::Detail(item) = &self.route else {
            return div().into_any_element();
        };
        let item = item.as_ref().clone();
        let font = cx.theme().font_size;
        let header = crate::library_header::Layout::new(self.library_width, false, cx);
        let favorite = item.clone();
        let content = div()
            .v_flex()
            .w_full()
            .child(
                header
                    .frame()
                    .child(self.detail_background_view(cx))
                    .child(self.detail_artwork(&item, header.artwork_size(), cx))
                    .child(
                        header
                            .metadata()
                            .child(
                                header
                                    .item_type(
                                        crate::library_header::type_label(
                                            ItemKind::Song,
                                            &self.state.settings.language,
                                        )
                                        .to_uppercase(),
                                    )
                                    .id("missing-album-type")
                                    .role(Role::Link)
                                    .key_context("FormControl")
                                    .focusable()
                                    .tab_stop(true)
                                    .cursor_pointer()
                                    .on_key_down(cx.listener(
                                        |view, event: &KeyDownEvent, _, cx| {
                                            if event.keystroke.key == "enter" {
                                                cx.stop_propagation();
                                                view.navigate(Route::Library(ItemKind::Song), cx);
                                            }
                                        },
                                    ))
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.navigate(Route::Library(ItemKind::Song), cx)
                                    })),
                            )
                            .child(header.title(item.name.clone(), window))
                            .child(
                                header
                                    .metadata_row()
                                    .gap(font * 0.5)
                                    .text_size(font)
                                    .child(
                                        item.column_data
                                            .get("releaseYear")
                                            .cloned()
                                            .unwrap_or_default(),
                                    )
                                    .child("•")
                                    .child(if item.duration > 0. {
                                        feishin_core::metadata_format::duration_text(
                                            item.duration,
                                            &self.state.settings.language,
                                            false,
                                        )
                                    } else {
                                        String::new()
                                    }),
                            )
                            .child(
                                header
                                    .metadata_row()
                                    .text_size(font)
                                    .max_h(font * 4.)
                                    .overflow_hidden()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.joined_artists(
                                        &item,
                                        true,
                                        "missing-album-artists",
                                        cx,
                                    )),
                            ),
                    ),
            )
            .child(
                div()
                    .debug_selector(|| "missing-album-content".into())
                    .v_flex()
                    .gap(font * 2.)
                    .px(font * 2.)
                    .pt(font)
                    .pb(font * 5.)
                    .overflow_hidden()
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .gap(font)
                            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                            .child(
                                crate::styles::button("missing-album-play", cx)
                                    .debug_selector(|| "missing-album-play".into())
                                    .primary()
                                    .size(font * 3.)
                                    .rounded_full()
                                    .icon(
                                        Icon::default()
                                            .path("native-icons/play.svg")
                                            .with_size(font * 1.125),
                                    )
                                    .aria_label(self.tr("player.play"))
                                    .disabled(self.loading || !self.library_fetches.is_empty())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.play_detail(
                                            view.state.settings.play_button_behavior.mode(),
                                            view.state.settings.play_button_behavior
                                                == feishin_core::queue::PlayBehavior::Shuffle,
                                            cx,
                                        )
                                    })),
                            )
                            .when(self.state.settings.show_favorites, |row| {
                                row.child(
                                    crate::styles::button("missing-album-favorite", cx)
                                        .debug_selector(|| "missing-album-favorite".into())
                                        .ghost()
                                        .icon(
                                            Icon::default()
                                                .path(if item.favorite {
                                                    "native-icons/favorite-filled.svg"
                                                } else {
                                                    "native-icons/favorite.svg"
                                                })
                                                .text_color(if item.favorite {
                                                    cx.theme().primary
                                                } else {
                                                    cx.theme().foreground
                                                }),
                                        )
                                        .aria_label(self.tr(if item.favorite {
                                            "Unfavorite"
                                        } else {
                                            "Favorite"
                                        }))
                                        .disabled(self.loading || self.favorite_requests > 0)
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.toggle_favorite(favorite.clone(), cx)
                                        })),
                                )
                            })
                            .child(self.library_item_menu(&item, usize::MAX, cx)),
                    )
                    .when(!feishin_core::genres::links(&item).is_empty(), |body| {
                        body.child(self.detail_genre_links(&item, cx))
                    })
                    .when(!item.comment.is_empty(), |body| {
                        body.child(crate::library_header::description(
                            &item.comment,
                            false,
                            window,
                            cx,
                        ))
                    })
                    .child(
                        div()
                            .h_flex()
                            .items_center()
                            .justify_center()
                            .gap(px(5. * self.state.settings.zoom))
                            .debug_selector(|| "missing-album-notice".into())
                            .child(
                                Icon::default()
                                    .path("native-icons/error.svg")
                                    .with_size(px(30. * self.state.settings.zoom))
                                    .text_color(cx.theme().danger),
                            )
                            .child(
                                div()
                                    .text_size(font * 1.5)
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.tr(if self.loading {
                                        "Loading..."
                                    } else {
                                        "error.badAlbum"
                                    })),
                            ),
                    )
                    .when(!self.message.is_empty() && !self.loading, |body| {
                        body.child(
                            crate::styles::button("missing-album-retry", cx)
                                .debug_selector(|| "missing-album-retry".into())
                                .label(self.tr("common.retry"))
                                .on_click(cx.listener(|view, _, _, cx| view.reload(false, cx))),
                        )
                    }),
            );
        div()
            .relative()
            .size_full()
            .min_h_0()
            .child(
                div()
                    .id("missing-album-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.page_scroll)
                    .child(content),
            )
            .child(crate::scroll::Scrollbar::vertical(
                "missing-album-scrollbar",
                &self.page_scroll,
            ))
            .into_any_element()
    }

    pub fn load_detail_metadata(&mut self, cx: &mut Context<Self>) {
        let Route::Detail(item) = &self.route else {
            return;
        };
        if !matches!(
            item.kind,
            ItemKind::Song
                | ItemKind::Album
                | ItemKind::Artist
                | ItemKind::AlbumArtist
                | ItemKind::Playlist
        ) {
            return;
        }
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            return;
        };
        let original = item.as_ref().clone();
        if original.kind == ItemKind::Song {
            self.loading = true;
        }
        let request = original.clone();
        let generation = self.album_tracks_generation;
        let task = cx
            .background_executor()
            .spawn(async move { api.item(&request.id, request.kind) });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                view.receive_detail_metadata(generation, &original, result, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn receive_detail_metadata(
        &mut self,
        generation: u64,
        original: &Item,
        result: anyhow::Result<Item>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.album_tracks_generation {
            return;
        }
        let Route::Detail(current) = &mut self.route else {
            return;
        };
        let identity = |item: &Item| (item.server_id, item.id.clone(), item.kind);
        if identity(current) != identity(original) {
            return;
        }
        if original.kind == ItemKind::Song {
            self.loading = false;
        }
        match result {
            Ok(mut item) if identity(&item) == identity(original) => {
                // Keep edits made while the metadata request was in flight.
                if current.favorite != original.favorite {
                    item.favorite = current.favorite;
                }
                if current.rating != original.rating {
                    item.set_rating(current.rating);
                }
                if current.cover_revision != original.cover_revision {
                    item.cover_id.clone_from(&current.cover_id);
                    item.cover_revision.clone_from(&current.cover_revision);
                }
                if item.kind == ItemKind::Album {
                    item.playlist_songs.clear();
                }
                **current = item.clone();
                self.fetch_item_cover(&item, cx);
                self.load_album_related(&item, cx);
                cx.notify();
            }
            Ok(_) => log::warn!("Detail metadata returned a different item"),
            Err(error) => {
                if original.kind == ItemKind::Song {
                    self.message = error.to_string();
                }
                log::debug!("Detail metadata unavailable: {error}");
                cx.notify();
            }
        }
    }

    pub fn refresh_library(&mut self, cx: &mut Context<Self>) {
        self.refresh_folder_browser();
        self.query_cache_generation = self.query_cache_generation.wrapping_add(1);
        self.artist_info_target = None;
        self.artist_info = None;
        self.artist_info_error.clear();
        self.reload(false, cx);
    }

    fn load_album_related(&mut self, album: &Item, cx: &mut Context<Self>) {
        if album.kind != ItemKind::Album || !self.album_related.is_empty() {
            return;
        }
        if let Some(artist) = album.album_artist_ids.first().filter(|id| !id.is_empty()) {
            self.album_related.push(RelatedAlbums {
                title: self.tr("page.albumDetail.moreFromArtist"),
                query: ListQuery {
                    kind: ItemKind::Album,
                    sort: Sort::Year,
                    descending: true,
                    limit: 20,
                    filters: Filters {
                        artist_ids: vec![artist.clone()],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                state: Default::default(),
            });
        }
        for (genre, name) in feishin_core::genres::links(album).into_iter().take(2) {
            let title = feishin_core::i18n::translate_with(
                &self.state.settings.language,
                "page.albumDetail.moreFromGeneric",
                &serde_json::json!({"item":name}),
            )
            .unwrap_or_else(|| format!("More from {name}"));
            self.album_related.push(RelatedAlbums {
                title,
                query: ListQuery {
                    kind: ItemKind::Album,
                    sort: Sort::Random,
                    limit: 20,
                    filters: Filters {
                        genre_ids: vec![genre.clone()],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                state: Default::default(),
            });
        }
        for index in 0..self.album_related.len() {
            self.load_album_related_page(index, false, cx);
        }
    }

    pub fn load_album_related_page(&mut self, index: usize, more: bool, cx: &mut Context<Self>) {
        let Route::Detail(album) = &self.route else {
            return;
        };
        if album.kind != ItemKind::Album {
            return;
        }
        let owner = (album.server_id, album.id.clone());
        let Some(api) = self.sessions.get(&owner.0).cloned() else {
            return;
        };
        let Some(section) = self.album_related.get_mut(index) else {
            return;
        };
        if section.state.loading || (more && !section.state.has_more) {
            return;
        }
        self.album_related_generation = self.album_related_generation.wrapping_add(1);
        let generation = self.album_related_generation;
        let metadata_generation = self.album_tracks_generation;
        section.state.generation = generation;
        section.state.loading = true;
        section.state.loading_more = more;
        section.state.error.clear();
        let mut query = section.query.clone();
        let pages = if more {
            1
        } else {
            section.state.offset.div_ceil(20).max(1)
        };
        query.offset = if more { section.state.offset } else { 0 };
        let task = cx.background_executor().spawn(smol::unblock(move || {
            crate::home::load_pages(&api, query, pages)
        }));
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                if metadata_generation != view.album_tracks_generation
                    || !matches!(&view.route, Route::Detail(album) if (album.server_id, album.id.clone()) == owner)
                { return; }
                let Some(section) = view.album_related.get_mut(index)
                    .filter(|section| section.state.generation == generation) else { return };
                section.state.loading = false;
                section.state.loading_more = false;
                match result {
                    Ok(pages) => section.state.receive(pages, more, Kind::Random, Some(&owner.1)),
                    Err(error) => {
                        log::debug!("Related album request failed: {error}");
                        section.state.error = error.to_string();
                    }
                }
                view.fetch_covers(cx);
                cx.notify();
            }).ok();
        }).detach();
        cx.notify();
    }

    pub fn album_page_rows(&self, grid_columns: usize) -> Vec<AlbumPageRow> {
        let mut discs: Vec<(u32, Vec<usize>)> = Vec::new();
        for (index, song) in self.items.iter().enumerate() {
            if let Some((disc, indexes)) = discs.last_mut()
                && *disc == song.disc
            {
                indexes.push(index);
            } else {
                discs.push((song.disc, vec![index]));
            }
        }
        let grouped = discs.len() > 1
            && self.local_filter.trim().is_empty()
            && matches!(self.sort, Sort::Track | Sort::Id);
        let mut rows = vec![AlbumPageRow::Metadata, AlbumPageRow::Controls];
        if grouped {
            for (disc, indexes) in discs {
                rows.push(AlbumPageRow::Disc(disc, indexes.clone()));
                rows.extend(
                    indexes
                        .chunks(grid_columns.max(1))
                        .map(|indexes| AlbumPageRow::Tracks(indexes.to_vec())),
                );
            }
        } else {
            rows.extend(
                (0..self.items.len())
                    .collect::<Vec<_>>()
                    .chunks(grid_columns.max(1))
                    .map(|indexes| AlbumPageRow::Tracks(indexes.to_vec())),
            );
        }
        rows.extend([
            AlbumPageRow::GridEnd,
            AlbumPageRow::Copyright,
            AlbumPageRow::RelatedStart,
        ]);
        rows.extend((0..self.album_related.len()).map(AlbumPageRow::Related));
        rows.push(AlbumPageRow::Bottom);
        rows
    }

    pub fn album_page_row(
        &mut self,
        row: &AlbumPageRow,
        grid: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = self.album_content_layout(cx);
        match row {
            AlbumPageRow::Tracks(indexes) if grid => layout
                .songs_frame()
                .child(self.library_grid_row(indexes, cx))
                .into_any_element(),
            AlbumPageRow::Tracks(indexes) => layout
                .songs_frame()
                .h_flex()
                .gap_3()
                .children(indexes.iter().map(|index| {
                    if grid {
                        self.card(&self.items[*index], *index, cx)
                    } else {
                        self.row(&self.items[*index], *index, cx)
                    }
                }))
                .into_any_element(),
            AlbumPageRow::Disc(disc, indexes) => layout
                .songs_frame()
                .child(self.album_disc_row(
                    *disc,
                    indexes,
                    px(40. * self.state.settings.zoom),
                    false,
                    cx,
                ))
                .into_any_element(),
            AlbumPageRow::RelatedStart => div().h(layout.font * 3.).into_any_element(),
            AlbumPageRow::Related(index) => {
                let Some(carousel) = self.grid_carousel_view(
                    crate::grid_carousel::Source::Album(*index),
                    window,
                    cx,
                ) else {
                    return div().h_0().into_any_element();
                };
                div()
                    .px(layout.padding())
                    .when(
                        self.album_related[..*index]
                            .iter()
                            .any(|section| !section.state.hidden()),
                        |row| row.pt(layout.font),
                    )
                    .child(carousel)
                    .into_any_element()
            }
            AlbumPageRow::Metadata => div()
                .w_full()
                .px(layout.padding())
                .pt(layout.font)
                .when(!layout.large, |row| {
                    row.pb(layout.font).child(self.album_metadata_view(cx))
                })
                .into_any_element(),
            AlbumPageRow::Controls => layout
                .songs_frame()
                .child(self.library_list_controls(cx))
                .into_any_element(),
            AlbumPageRow::GridEnd => div()
                .debug_selector(|| "album-grid-end".into())
                .h(if layout.large {
                    self.album_metadata_extra
                } else {
                    px(0.)
                })
                .into_any_element(),
            AlbumPageRow::Copyright => {
                let Route::Detail(album) = &self.route else {
                    return div().into_any_element();
                };
                let year = album
                    .column_data
                    .get("releaseYear")
                    .filter(|value| !value.is_empty())
                    .cloned()
                    .or_else(|| (album.year > 0).then(|| album.year.to_string()));
                div()
                    .px(layout.padding())
                    .pt(layout.font)
                    .v_flex()
                    .gap(layout.font * 0.25)
                    .text_size(layout.font * 0.8125)
                    .text_color(cx.theme().muted_foreground)
                    .children(album.record_labels.iter().map(|label| {
                        format!(
                            "℗{} {label}",
                            year.as_ref()
                                .map_or(String::new(), |year| format!(" {year}"))
                        )
                    }))
                    .into_any_element()
            }
            AlbumPageRow::Bottom => div().h(layout.font * 5.).into_any_element(),
        }
    }

    pub(crate) fn album_disc_row(
        &self,
        disc: u32,
        indexes: &[usize],
        height: Pixels,
        sticky: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checked =
            !indexes.is_empty() && indexes.iter().all(|index| self.selected.contains(index));
        let indexes = indexes.to_vec();
        let subtitle = indexes
            .first()
            .and_then(|index| self.items[*index].column_data.get("discSubtitle").cloned())
            .or_else(|| {
                if let Route::Detail(album) = &self.route {
                    album
                        .column_data
                        .get("discs")
                        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                        .and_then(|discs| discs[disc.to_string()].as_str().map(str::to_owned))
                } else {
                    None
                }
            });
        let label = format!(
            "{} {disc}{}",
            self.tr("common.disc"),
            subtitle.map_or(String::new(), |text| format!(" - {text}"))
        );
        div()
            .w_full()
            .h(height)
            .h_flex()
            .px(cx.theme().font_size * 0.75)
            .bg(cx.theme().background)
            .occlude()
            .flex_shrink_0()
            .items_center()
            .debug_selector(move || {
                format!("album-{}disc-{disc}", if sticky { "sticky-" } else { "" })
            })
            .child(
                Checkbox::new(("album-disc-selection", disc))
                    .debug_selector(move || {
                        format!(
                            "album-{}disc-select-{disc}",
                            if sticky { "sticky-" } else { "" }
                        )
                    })
                    .label(label)
                    .checked(checked)
                    .on_click(cx.listener(move |view, value: &bool, _, cx| {
                        for index in &indexes {
                            if *value {
                                view.selected.insert(*index);
                            } else {
                                view.selected.remove(index);
                            }
                        }
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn album_content_layout(&self, cx: &App) -> ContentLayout {
        ContentLayout::new(
            self.detail_header_width,
            cx.theme().font_size,
            self.state.settings.zoom,
        )
        .rtl(crate::appearance::rtl(cx))
    }

    pub(crate) fn album_sticky_metadata(
        &self,
        page: AnyElement,
        start: usize,
        end: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = self.album_content_layout(cx);
        let metadata = self.album_metadata_view(cx);
        let view = cx.entity();
        sticky_metadata(
            page,
            metadata,
            self.album_details_scroll.clone(),
            start..end,
            layout,
            move |extra, cx| {
                view.update(cx, |view, cx| {
                    if (view.album_metadata_extra - extra).abs() > px(0.01) {
                        view.album_metadata_extra = extra;
                        view.album_details_scroll.set_item_height(end, extra);
                        cx.notify();
                    }
                });
            },
        )
    }

    fn open_album_tag_filter(
        &mut self,
        server: uuid::Uuid,
        field: &str,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        if self.state.active_server != Some(server) {
            self.choose_server(server, cx);
        }
        let entry = self
            .state
            .settings
            .views
            .entry(format!("{server}:library:album"))
            .or_default();
        entry.filters = Filters {
            custom: std::collections::BTreeMap::from([(field.into(), serde_json::json!([value]))]),
            ..Default::default()
        };
        entry.text.clear();
        self.navigate(Route::Library(ItemKind::Album), cx);
    }

    pub fn detail_genre_links(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        div()
            .h_flex()
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .flex_wrap()
            .gap(cx.theme().font_size * 0.5)
            .children(
                feishin_core::genres::links(item)
                    .into_iter()
                    .enumerate()
                    .map(|(index, (id, name))| {
                        let genre = Item {
                            id,
                            server_id: item.server_id,
                            kind: ItemKind::Genre,
                            name,
                            ..Default::default()
                        };
                        let button = if item.kind == ItemKind::Album {
                            self.album_metadata_link(("detail-metadata-genre", index), cx)
                        } else {
                            crate::styles::button(("detail-metadata-genre", index), cx)
                                .outline()
                                .h(cx.theme().font_size * 1.5)
                                .px(cx.theme().font_size * 0.625)
                                .text_size(cx.theme().font_size * 0.875)
                                .rounded(ButtonRounded::Size(cx.theme().font_size * 0.5))
                        };
                        button
                            .debug_selector(move || format!("detail-metadata-genre-{index}"))
                            .label(genre.name.clone())
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.navigate(Route::Detail(Box::new(genre.clone())), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    fn album_metadata_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Route::Detail(album) = &self.route else {
            return div().into_any_element();
        };
        let server = album.server_id;
        let backend = self.sessions.get(&server).map(|api| api.server.kind);
        let tags =
            feishin_core::metadata_format::album_tag_labels(album, &self.state.settings.language);
        let layout = self.album_content_layout(cx);
        layout
            .metadata_frame()
            .id("album-metadata")
            .debug_selector(|| "album-metadata".into())
            .w_full()
            .min_w_0()
            .when(layout.large, |column| column.w(layout.metadata_width()))
            .when(!feishin_core::genres::links(album).is_empty(), |column| {
                column.child(
                    self.album_metadata_group(
                        feishin_core::i18n::translate_with(
                            &self.state.settings.language,
                            "entity.genre",
                            &serde_json::json!({"count": feishin_core::genres::links(album).len()}),
                        )
                        .unwrap_or_else(|| self.tr("entity.genre")),
                        self.detail_genre_links(album, cx),
                        cx,
                    ),
                )
            })
            .when(!tags.is_empty(), |row| {
                row.child(self.album_metadata_pills("common.tags", &tags, cx))
            })
            .when(!album.record_labels.is_empty(), |row| {
                row.child(
                    self.album_metadata_group(
                        self.tr("common.recordLabel"),
                        self.album_pill_row(cx)
                            .children(album.record_labels.iter().enumerate().map(
                                |(index, label)| {
                                    if backend == Some(ServerType::Subsonic) {
                                        self.album_metadata_pill(label, cx).into_any_element()
                                    } else {
                                        let value = label.clone();
                                        let field = if backend == Some(ServerType::Jellyfin) {
                                            "Studios"
                                        } else {
                                            "recordlabel"
                                        };
                                        self.album_metadata_link(("album-record-label", index), cx)
                                            .label(label.clone())
                                            .debug_selector(move || {
                                                format!("album-record-label-{index}")
                                            })
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.open_album_tag_filter(
                                                    server, field, &value, cx,
                                                )
                                            }))
                                            .into_any_element()
                                    }
                                },
                            ))
                            .into_any_element(),
                        cx,
                    ),
                )
            })
            .when_some(
                album.tags.get("mood").filter(|values| !values.is_empty()),
                |row, moods| row.child(self.album_metadata_pills("common.mood", moods, cx)),
            )
            .when_some(
                album
                    .tags
                    .get("grouping")
                    .filter(|values| !values.is_empty()),
                |row, groups| {
                    row.child(
                        self.album_metadata_group(
                            self.tr("common.grouping"),
                            self.album_pill_row(cx)
                                .children(groups.iter().enumerate().map(|(index, group)| {
                                    if backend == Some(ServerType::Navidrome) {
                                        let value = group.clone();
                                        self.album_metadata_link(("album-grouping", index), cx)
                                            .label(group.clone())
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.open_album_tag_filter(
                                                    server, "grouping", &value, cx,
                                                )
                                            }))
                                            .into_any_element()
                                    } else {
                                        self.album_metadata_pill(group, cx).into_any_element()
                                    }
                                }))
                                .into_any_element(),
                            cx,
                        ),
                    )
                },
            )
            .when(
                !self.state.settings.external_links.links(album).is_empty(),
                |column| {
                    column.child(self.album_metadata_group(
                        self.tr("common.externalLinks"),
                        self.external_links(album, cx),
                        cx,
                    ))
                },
            )
            .into_any_element()
    }

    fn album_metadata_group(&self, title: String, content: AnyElement, cx: &App) -> Div {
        let font = cx.theme().font_size;
        div()
            .v_flex()
            .gap(font * 0.25)
            .min_w_0()
            .max_w_full()
            .items_center()
            .when(self.album_content_layout(cx).wide, |group| {
                group.items_start()
            })
            .child(
                div()
                    .text_size(font * 0.8125)
                    .line_height(font)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_uppercase()),
            )
            .child(content)
    }

    fn album_pill_row(&self, cx: &App) -> Div {
        div()
            .h_flex()
            .flex_wrap()
            .gap(cx.theme().font_size * 0.5)
            .justify_center()
            .when(self.album_content_layout(cx).wide, |row| {
                row.justify_start()
            })
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
    }

    fn album_metadata_pill(&self, value: &str, cx: &App) -> Div {
        let font = cx.theme().font_size;
        div()
            .border_1()
            .border_color(cx.theme().border)
            .rounded(font * 5. / 16.)
            .bg(cx.theme().background.opacity(0.5))
            .px(font * 0.5)
            .text_size(font * 0.875)
            .line_height(font * 1.375)
            .child(value.to_owned())
    }

    fn album_metadata_link(&self, id: impl Into<ElementId>, cx: &App) -> Button {
        let font = cx.theme().font_size;
        Button::new(id)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(cx.theme().background.opacity(0.5))
                    .hover(cx.theme().secondary.opacity(0.5))
                    .border(cx.theme().border)
                    .foreground(cx.theme().foreground)
                    .shadow(false),
            )
            .rounded(ButtonRounded::Size(font * 5. / 16.))
            .cursor_pointer()
            .h(font * 1.5)
            .px(font * 0.5)
            .text_size(font * 0.875)
            .line_height(font * 1.375)
            .font_weight(FontWeight::NORMAL)
    }

    fn album_metadata_pills(&self, title: &str, values: &[String], cx: &App) -> Div {
        self.album_metadata_group(
            self.tr(title),
            self.album_pill_row(cx)
                .children(
                    values
                        .iter()
                        .map(|value| self.album_metadata_pill(value, cx)),
                )
                .into_any_element(),
            cx,
        )
    }

    pub fn external_links(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("external-provider-links")
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .flex_wrap()
            .gap(cx.theme().font_size * 0.25)
            .children(
                self.state
                    .settings
                    .external_links
                    .links(item)
                    .into_iter()
                    .enumerate()
                    .map(|(i, (name, url))| {
                        let button = crate::styles::button(("external-provider-link", i), cx)
                            .debug_selector(move || format!("external-provider-link-{i}"))
                            .ghost();
                        let button = {
                            let (icon, key) = match name {
                                "Last.fm" => ("lastfm.png", "lastfm"),
                                "ListenBrainz" => ("listenbrainz.svg", "listenbrainz"),
                                "MusicBrainz" => ("musicbrainz.svg", "musicbrainz"),
                                "Qobuz" => ("qobuz.png", "qobuz"),
                                "Spotify" => ("spotify.svg", "spotify"),
                                _ => unreachable!("Unknown external provider"),
                            };
                            let label = self.tr(&format!("action.openIn.{key}"));
                            button
                                .aria_label(label.clone())
                                .tooltip(label)
                                .size(cx.theme().font_size * 2.25)
                                .p_0()
                                .rounded(ButtonRounded::Size(cx.theme().font_size * 5. / 16.))
                                .child(
                                    img(ImageSource::Resource(Resource::Embedded(
                                        format!("provider-logos/{icon}").into(),
                                    )))
                                    .size(cx.theme().font_size * 1.25)
                                    .object_fit(ObjectFit::Fill),
                                )
                        };
                        button.on_click(move |_, _, cx| cx.open_url(&url))
                    }),
            )
            .into_any_element()
    }

    pub fn grouped_song_albums(&self) -> bool {
        let config = self.list_config();
        let table = if config.display == "detail" {
            config.detail.as_ref().unwrap_or(&config.table)
        } else {
            &config.table
        };
        !matches!(self.route, Route::Queue)
            && config.display != "poster"
            && !self.items.is_empty()
            && self.items.iter().all(|item| item.kind == ItemKind::Song)
            && (config.display == "detail"
                || table
                    .columns
                    .iter()
                    .any(|column| column.is_enabled && column.id == "albumGroup"))
    }

    fn group_album(&mut self, song: &Item, cx: &mut Context<Self>) -> Item {
        let mut album = Item {
            id: song.album_id.clone(),
            name: song.album.clone(),
            kind: ItemKind::Album,
            favorite: false,
            rating: 0,
            ..song.clone()
        };
        let key = Self::album_key(&album);
        if let Some(cached) = self.album_metadata_cache.get(&key) {
            return cached.clone().unwrap_or(album);
        }
        self.album_metadata_cache.insert(key.clone(), None);
        if !album.id.is_empty()
            && !album.id.starts_with("dummy/")
            && let Some(api) = self.sessions.get(&album.server_id).cloned()
        {
            let id = album.id.clone();
            let generation = self.album_tracks_generation;
            let task = cx
                .background_executor()
                .spawn(async move { api.item(&id, ItemKind::Album) });
            cx.spawn(async move |this, cx| {
                let result = task.await;
                this.update(cx, |this, cx| {
                    if generation != this.album_tracks_generation {
                        return;
                    }
                    match result {
                        Ok(album) => {
                            this.fetch_item_cover(&album, cx);
                            this.album_metadata_cache.insert(key, Some(album));
                        }
                        Err(error) => log::debug!("Album group metadata fetch failed: {error}"),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        album.album = song.album.clone();
        album
    }

    fn album_metadata_header(
        &mut self,
        album: &Item,
        play: &Item,
        songs: &[Item],
        index: usize,
        queue_key: Option<(uuid::Uuid, bool)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let resolved = play.kind == ItemKind::Album
            || self
                .album_metadata_cache
                .get(&Self::album_key(album))
                .is_some_and(|album| album.is_some());
        self.fetch_item_cover(album, cx);
        let config = &self.state.settings.album_group;
        let size = config.artwork_size();
        let rows = config.rows(album, songs, &self.state.settings.language);
        let open = album.clone();
        let play = play.clone();
        let favorite = album.clone();
        let mut info = div()
            .v_flex()
            .min_w_0()
            .gap_1()
            .child(
                crate::styles::button(("album-group-title", index), cx)
                    .ghost()
                    .label(album.name.clone())
                    .disabled(album.id.is_empty())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.navigate(Route::Detail(Box::new(open.clone())), cx)
                    })),
            )
            .children(rows.into_iter().map(|(id, value)| {
                let artists = id == "albumArtists";
                let genres = id == "genres";
                div()
                    .debug_selector(move || format!("album-group-{index}-{id}"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if artists {
                        self.joined_artists(&play, true, "album-group-artists", cx)
                    } else if genres {
                        self.linked_metadata(
                            feishin_core::artists::listed(
                                if songs.is_empty() {
                                    feishin_core::genres::links(album)
                                } else {
                                    feishin_core::genres::grouped(songs)
                                },
                                ", ",
                            ),
                            album.server_id,
                            ItemKind::Genre,
                            "album-group-genres",
                            cx,
                        )
                    } else {
                        value.into_any_element()
                    })
            }));
        if config.show_favorite_rating && resolved && album.playlist_songs.is_empty() {
            info = info.child(
                div()
                    .h_flex()
                    .gap_1()
                    .when(self.state.settings.show_favorites, |div| {
                        div.child(
                            crate::styles::button(("album-group-favorite", index), cx)
                                .ghost()
                                .disabled(self.favorite_requests > 0)
                                .label(if album.favorite { "♥" } else { "♡" })
                                .tooltip(self.tr("Favorite"))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.toggle_favorite(favorite.clone(), cx)
                                })),
                        )
                    })
                    .when(
                        self.state.settings.show_ratings
                            && self
                                .sessions
                                .get(&album.server_id)
                                .is_some_and(|api| api.server.kind != ServerType::Jellyfin),
                        |div| {
                            div.child(
                                self.rating(format!("album-group-rating-{index}"), album, cx)
                                    .read_only(self.rating_requests > 0),
                            )
                        },
                    ),
            );
        }
        let image = div()
            .id(("album-group-play", index))
            .debug_selector(move || {
                if queue_key.is_some_and(|(_, side)| side) {
                    format!("side-queue-album-play-{index}")
                } else {
                    format!("album-group-play-{index}")
                }
            })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some((key, _)) = queue_key {
                    this.play_queue_entry(key, cx);
                } else if play.kind == ItemKind::Song {
                    let behavior = this.state.settings.play_button_behavior;
                    this.play_table_context(
                        index,
                        behavior.mode(),
                        behavior == feishin_core::queue::PlayBehavior::Shuffle,
                        cx,
                    );
                } else {
                    this.play_library_item(play.clone(), cx);
                }
            }))
            .child(self.artwork(album, size, cx));
        div()
            .id(("album-group-header", index))
            .debug_selector(move || format!("album-group-header-{index}"))
            .flex()
            .gap_2()
            .items_start()
            .when(config.vertical_layout, |div| div.flex_col())
            .when(!config.vertical_layout, |div| div.flex_row())
            .child(image)
            .child(info)
            .into_any_element()
    }

    pub fn queue_album_header(
        &mut self,
        songs: &[Item],
        key: uuid::Uuid,
        index: usize,
        side: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(first) = songs.first() else {
            return div().into_any_element();
        };
        let album = self.group_album(first, cx);
        let header = self.album_metadata_header(&album, first, songs, index, Some((key, side)), cx);
        div()
            .debug_selector(move || {
                format!("{}-queue-album-{index}", if side { "side" } else { "main" })
            })
            .w_full()
            .py_3()
            .px_2()
            .child(header)
            .into_any_element()
    }

    pub fn grouped_song_row(
        &mut self,
        range: std::ops::Range<usize>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let songs = self.items[range.clone()].to_vec();
        let first = &songs[0];
        if first.album.is_empty() {
            return self.row(first, range.start, cx);
        }
        let album = self.group_album(first, cx);
        let header = self.album_metadata_header(&album, first, &songs, range.start, None, cx);
        let tracks = div().v_flex().flex_1().min_w_0().children(
            songs
                .iter()
                .enumerate()
                .map(|(i, song)| self.row(song, range.start + i, cx)),
        );
        div()
            .id(("song-album-group", range.start))
            .debug_selector(move || format!("song-album-group-{}", range.start))
            .h_flex()
            .gap_3()
            .py_3()
            .items_start()
            .w_full()
            .child(header)
            .child(tracks)
            .into_any_element()
    }

    pub fn album_key(item: &Item) -> String {
        if item.playlist_songs.is_empty() {
            format!("{}:{}", item.server_id, item.id)
        } else {
            format!(
                "{}:{}:playlist:{}",
                item.server_id,
                item.id,
                item.playlist_position.unwrap_or(0)
            )
        }
    }
    fn load_album_tracks(&mut self, album: &Item, cx: &mut Context<Self>) {
        let key = Self::album_key(album);
        if !album.playlist_songs.is_empty() {
            self.album_tracks.insert(
                key,
                AlbumTracks {
                    items: feishin_core::playlist_albums::ordered_tracks(album),
                    ..Default::default()
                },
            );
            return;
        }
        if self.album_tracks.contains_key(&key) {
            return;
        }
        let Some(api) = self.sessions.get(&album.server_id).cloned() else {
            self.album_tracks.insert(
                key,
                AlbumTracks {
                    error: Some("Sign in to this album's server".into()),
                    ..Default::default()
                },
            );
            return;
        };
        self.album_tracks.insert(
            key.clone(),
            AlbumTracks {
                loading: true,
                ..Default::default()
            },
        );
        let album = album.clone();
        let generation = self.album_tracks_generation;
        let task = cx
            .background_executor()
            .spawn(async move { api.all_tracks(&album) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.album_tracks_generation != generation {
                    return;
                }
                let tracks = match result {
                    Ok(mut items) => {
                        lists::sort_items(&mut items, Sort::Track, false);
                        AlbumTracks {
                            items,
                            ..Default::default()
                        }
                    }
                    Err(error) => {
                        log::warn!("Album detail tracks failed: {error}");
                        AlbumTracks {
                            error: Some(error.to_string()),
                            ..Default::default()
                        }
                    }
                };
                this.album_tracks.insert(key.clone(), tracks);
                let count = this.album_details_scroll.item_count();
                this.album_details_scroll.splice(0..count, count);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn album_detail_row(&mut self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(album) = self.items.get(index).cloned() else {
            return div().into_any_element();
        };
        self.load_album_tracks(&album, cx);
        let key = Self::album_key(&album);
        let songs = self
            .album_tracks
            .get(&key)
            .map(|tracks| tracks.items.clone())
            .unwrap_or_default();
        let header = self.album_metadata_header(&album, &album, &songs, index, None, cx);
        let tracks = self.album_tracks.get(&key);
        let mut config = self.list_config();
        config.table = config
            .detail
            .clone()
            .unwrap_or_else(|| lists::default_for("albumDetail").table);
        let columns = self.configured_columns();
        let selected = self.selected.contains(&index);
        let mut row = div()
            .id(SharedString::from(format!("album-detail-{key}")))
            .v_flex()
            .gap_2()
            .py_3()
            .w_full()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .items_center()
                    .child(
                        crate::styles::button("select-album", cx)
                            .ghost()
                            .label(if selected { "✓" } else { "□" })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.selected_album_tracks.clear();
                                this.album_selection_order.clear();
                                if !this.selected.insert(index) {
                                    this.selected.remove(&index);
                                }
                                cx.notify();
                            })),
                    )
                    .child(header),
            );
        if let Some(tracks) = tracks {
            if tracks.loading {
                row = row.child(crate::appearance::text("Loading album tracks...", cx));
            }
            if let Some(error) = &tracks.error {
                let key = key.clone();
                row = row.child(error.clone()).child(
                    crate::styles::button("retry-album-tracks", cx)
                        .label(crate::appearance::text("Retry", cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.album_tracks.remove(&key);
                            cx.notify();
                        })),
                );
            }
            if !tracks.items.is_empty() && config.table.enable_header {
                let cells = columns
                    .iter()
                    .map(|column| {
                        let cell = crate::lists::table_header(column, cx).into_any_element();
                        let cell = self.column_drag_controls(column, cell, cx);
                        (column.clone(), cell)
                    })
                    .collect();
                row = row.child(
                    div()
                        .h_flex()
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .gap_2()
                        .h(px(
                            config.table.size.detail_row_height() * self.state.settings.zoom
                        ))
                        .when(!crate::appearance::rtl(cx), |div| {
                            div.pl(px(64. * self.state.settings.zoom
                                + 2. * f32::from(cx.theme().font_size) * 0.5))
                        })
                        .when(crate::appearance::rtl(cx), |div| {
                            div.pr(px(64. * self.state.settings.zoom
                                + 2. * f32::from(cx.theme().font_size) * 0.5))
                        })
                        .child(self.column_strip(
                            format!("album-header-{key}"),
                            cells,
                            &config.table,
                            true,
                            &self.list_horizontal_scroll,
                            cx,
                        )),
                );
            }
            row = row.children(tracks.items.iter().enumerate().map(|(track_index, track)| {
                let song = track.clone();
                let held_song = song.clone();
                let hover_row = crate::lists::TableRow::AlbumTrack(key.clone());
                div()
                    .id(("album-track", track_index))
                    .debug_selector({
                        let album = album.id.clone();
                        move || format!("album-detail-track-{album}-{track_index}")
                    })
                    .when(
                        matches!(&self.track_scroll_target, Some((crate::lists::TableRow::AlbumTrack(target), index, _)) if *target == key && *index == track_index),
                        |div| div.relative().child(self.track_scroll_marker(cx)),
                    )
                    .on_drag(
                        self.drag_album_track(&key, track_index),
                        crate::context_preview::drag_constructor(cx),
                    )
                    .on_hover(cx.listener(move |this, hovered, _, cx| this.hover_table_row(hover_row.clone(), track_index, *hovered, cx)))
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .items_center()
                    .gap_2()
                    .h(px(config.table.size.detail_row_height() * self.state.settings.zoom))
                    .w_full()
                    .when(config.table.enable_horizontal_borders, |div| {
                        div.border_b_1().border_color(cx.theme().border)
                    })
                    .bg(crate::styles::row_background(config.table.enable_alternate_row_colors && track_index % 2 == 1, self.selected_album_tracks.contains(&(key.clone(), track_index)), false, cx))
                    .when(config.table.enable_row_hover_highlight, |div| {
                        div.hover(|style| style.bg(crate::styles::row_background(config.table.enable_alternate_row_colors && track_index % 2 == 1, self.selected_album_tracks.contains(&(key.clone(), track_index)), true, cx)))
                    })
                    .child(
                        crate::styles::button("select-album-track", cx)
                            .ghost()
                            .label(
                                if self
                                    .selected_album_tracks
                                    .contains(&(key.clone(), track_index))
                                {
                                    "✓"
                                } else {
                                    "□"
                                },
                            )
                            .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                            .on_click({
                                let key = key.clone();
                                cx.listener(move |this, _, _, cx| {
                                    this.select_album_track(
                                        &key,
                                        track_index,
                                        Modifiers {
                                            control: true,
                                            ..Modifiers::none()
                                        },
                                        cx,
                                    );
                                })
                            }),
                    )
                    .child(
                        crate::styles::button("play-album-track", cx)
                            .disabled(!self.library_fetches.is_empty())
                            .ghost()
                            .label(crate::appearance::text("▶", cx))
                            .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                            .tooltip(format!(
                                "{}\n{}",
                                self.tr("Play"),
                                self.tr("Hold to shuffle")
                            ))
                            .on_long_press(cx.listener(move |view, _, _, cx| {
                                view.shuffle_library_item(held_song.clone(), cx)
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play_library_item(song.clone(), cx)
                            })),
                    )
                    .child(self.metadata_cells(
                        track,
                        track_index,
                        &config,
                        columns.clone(),
                        crate::lists::TableRow::AlbumTrack(key.clone()),
                        cx,
                    ))
            }));
        }
        row.into_any_element()
    }
}

#[cfg(test)]
mod content_tests {
    use super::sticky_disc_index;
    use gpui::px;
    #[test]
    fn sticky_discs_match_original_hook_states() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/album-disc-sticky-reference.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            for zoom in [1., 1.25] {
                let counts: Vec<usize> = serde_json::from_value(case["counts"].clone()).unwrap();
                let actual = sticky_disc_index(
                    &counts,
                    px(case["rowHeight"].as_f64().unwrap() as f32 * zoom),
                    px(40. * zoom),
                    case["header"].as_bool().unwrap(),
                    px(case["top"].as_f64().unwrap() as f32 * zoom),
                    px(case["stickyTop"].as_f64().unwrap() as f32 * zoom),
                );
                assert_eq!(
                    actual,
                    case["stickyGroupIndex"]
                        .as_u64()
                        .map(|index| index as usize),
                    "{case}"
                );
            }
        }
    }
}
