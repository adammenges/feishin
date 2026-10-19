use crate::app::*;
use crate::scroll::VerticalScroll;
use feishin_core::{model::*, queue::Enqueue, storage};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, Selectable, Sizable, StyledExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
};

enum DetailPageRow {
    Header,
    Album(crate::details::AlbumPageRow),
    GroupedSongs(std::ops::Range<usize>),
    AlbumDetail(crate::album_groups::AlbumRow),
    Albums(crate::album_groups::AlbumRow),
    Items(Vec<usize>),
    Empty,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardSource {
    Library,
    Carousel,
}

pub(crate) fn content_mark(item: &Item) -> Option<(&'static str, &'static str)> {
    if !matches!(item.kind, ItemKind::Song | ItemKind::Album) {
        return None;
    }
    match item.explicit_status() {
        Some("EXPLICIT") => Some(("🅴", "Explicit")),
        Some("CLEAN") => Some(("🅲", "Clean")),
        _ => None,
    }
}

pub(crate) fn content_mark_stylesheet() -> String {
    let mut source = include_str!("../assets/styles/explicit-indicator.css").to_owned();
    for name in [
        "root",
        "size-xs",
        "size-sm",
        "size-md",
        "size-lg",
        "size-xl",
        "size-2xl",
        "size-3xl",
        "size-4xl",
        "muted",
        "with-space",
    ] {
        source = source.replace(
            &format!(".{name}"),
            &format!(".fs-explicit-indicator-module-{name}"),
        );
    }
    source
}

pub(crate) fn content_mark_style(cx: &App) -> StyleRefinement {
    cx.try_global::<crate::styles::NativeStyles>()
        .map(|styles| {
            styles.style(
                feishin_core::styles::Target::new(
                    "span",
                    "content-mark",
                    &[
                        "fs-explicit-indicator-module-root",
                        "fs-explicit-indicator-module-size-lg",
                        "fs-explicit-indicator-module-with-space",
                    ],
                ),
                None,
                f32::from(cx.theme().font_size),
            )
        })
        .unwrap_or_default()
}

#[derive(IntoElement)]
pub(crate) struct TitleText {
    text: String,
    subtitle_start: usize,
    subtitle_line_height: Option<Pixels>,
    mark: Option<(&'static str, &'static str)>,
}

impl From<String> for TitleText {
    fn from(text: String) -> Self {
        let subtitle_start = text.len();
        Self {
            text,
            subtitle_start,
            subtitle_line_height: None,
            mark: None,
        }
    }
}

impl TitleText {
    pub(crate) fn inherit_subtitle_line_height(mut self) -> Self {
        self.subtitle_line_height = None;
        self
    }

    pub(crate) fn styled(self, window: &Window, cx: &App) -> StyledText {
        let style = window.text_style();
        let mut text = String::new();
        let mut runs = Vec::new();
        if let Some((symbol, _)) = self.mark {
            let refinement = content_mark_style(cx);
            let mut mark_style = style.clone();
            mark_style.font_family = "Symbola".into();
            mark_style.font_size = rems(1.).into();
            if let Some(refinement) = &refinement.text {
                mark_style.refine(refinement);
            }
            let mut run = mark_style.to_run(symbol.len());
            run.font_size = Some(mark_style.font_size.to_pixels(window.rem_size()));
            run.letter_spacing += refinement
                .padding
                .right
                .map_or(window.rem_size() * 0.25, |padding| {
                    padding.to_pixels(mark_style.font_size, window.rem_size())
                });
            text.push_str(symbol);
            runs.push(run);
        }
        if self.subtitle_start > 0 {
            runs.push(style.to_run(self.subtitle_start));
        }
        if self.subtitle_start < self.text.len() {
            let mut run = style.to_run(self.text.len() - self.subtitle_start);
            run.font_size = Some(window.rem_size() * (13. / 16.));
            run.line_height = self.subtitle_line_height;
            run.color = cx.theme().muted_foreground;
            runs.push(run);
        }
        text.push_str(&self.text);
        StyledText::new(text)
            .with_runs(runs)
            .with_accessible_text(self.text)
    }
}

impl RenderOnce for TitleText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.styled(window, cx)
    }
}

pub(crate) fn item_title(
    item: &Item,
    text: impl Into<TitleText>,
    align: feishin_core::lists::Align,
    cx: &App,
) -> Div {
    item_title_with_lines(item, text, align, None, cx)
}

pub(crate) fn item_title_with_lines(
    item: &Item,
    text: impl Into<TitleText>,
    align: feishin_core::lists::Align,
    lines: Option<usize>,
    _cx: &App,
) -> Div {
    let mut text = text.into();
    text.mark = content_mark(item);
    div()
        .relative()
        .h_flex()
        .w_full()
        .min_w_0()
        .children(text.mark.map(|(_, label)| {
            div()
                .id("content-mark")
                .absolute()
                .w(rems(1.25))
                .h(rems(1.))
                .role(Role::Label)
                .aria_label(label)
        }))
        .child(
            div()
                .debug_selector(|| "item-title-text".into())
                .w_full()
                .min_w_0()
                .map(|div| match lines {
                    Some(lines) => div
                        .max_h_full()
                        .text_size(rems(14. / 16.))
                        .line_height(rems(18. / 16.))
                        .whitespace_normal()
                        .line_clamp(lines),
                    None => div.truncate(),
                })
                .when(align == feishin_core::lists::Align::Center, |div| {
                    div.text_center()
                })
                .when(align == feishin_core::lists::Align::End, |div| {
                    div.text_right()
                })
                .child(text),
        )
        .when(lines.is_some(), |div| div.max_h_full())
}

pub(crate) fn track_title(item: &Item, subtitle: bool, cx: &App) -> TitleText {
    let mut title = TitleText::from(item.name.clone());
    title.subtitle_line_height = Some(cx.theme().font_size);
    if let Some(subtitle) = item
        .column_data
        .get("trackSubtitle")
        .filter(|text| subtitle && item.kind == ItemKind::Song && !text.is_empty())
    {
        title.text.push_str(&format!(" ({subtitle})"));
    }
    title
}

impl Feishin {
    pub(crate) fn artwork(&self, item: &Item, size: f32, cx: &App) -> AnyElement {
        self.artwork_at(item, size, feishin_core::images::Placement::Table, cx)
    }

    pub(crate) fn item_actions(
        &self,
        item: &Item,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.item_actions_for(item, index, None, cx)
    }
    fn item_actions_for(
        &self,
        item: &Item,
        index: usize,
        source: Option<CardSource>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let carousel = source == Some(CardSource::Carousel);
        let play = item.clone();
        let held_play = item.clone();
        let held_next = item.clone();
        let held_add = item.clone();
        let next = item.clone();
        let add = item.clone();
        let favorite = item.clone();
        let download = item.clone();
        let similar = item.clone();
        let editable_radio = item.clone();
        let deletable = item.clone();
        let shareable = item.clone();
        let expandable = item.clone();
        gpui::div()
            .h_flex()
            .when(source.is_some(), |row| row.flex_wrap())
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .gap_1()
            .child(
                crate::styles::button(("play", index), cx)
                    .disabled(!self.library_fetches.is_empty())
                    .debug_selector(move || format!("library-play-{index}"))
                    .small()
                    .ghost()
                    .label(self.tr("▶"))
                    .tooltip(format!(
                        "{}\n{}",
                        self.tr("Play"),
                        self.tr("Hold to shuffle")
                    ))
                    .on_long_press(cx.listener(move |view, _, _, cx| {
                        if let Some(source) = source {
                            view.play_card_item(source, &held_play, index, Enqueue::Now, true, cx);
                        } else {
                            view.shuffle_library_item(held_play.clone(), cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(source) = source {
                            this.play_card_item(source, &play, index, Enqueue::Now, false, cx);
                        } else {
                            this.play_library_item(play.clone(), cx);
                        }
                    })),
            )
            .child(
                crate::styles::button(("next", index), cx)
                    .disabled(item.kind == ItemKind::Radio || !self.library_fetches.is_empty())
                    .small()
                    .ghost()
                    .label(self.tr("↳"))
                    .tooltip(format!(
                        "{}\n{}",
                        self.tr("Play next"),
                        self.tr("Hold to shuffle")
                    ))
                    .on_long_press(cx.listener(move |view, _, _, cx| {
                        if let Some(source) = source {
                            view.play_card_item(source, &held_next, index, Enqueue::Next, true, cx);
                        } else {
                            view.enqueue_items(vec![held_next.clone()], Enqueue::Next, true, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(source) = source {
                            this.play_card_item(source, &next, index, Enqueue::Next, false, cx);
                        } else {
                            this.enqueue(next.clone(), Enqueue::Next, cx);
                        }
                    })),
            )
            .child(
                crate::styles::button(("add", index), cx)
                    .disabled(item.kind == ItemKind::Radio || !self.library_fetches.is_empty())
                    .small()
                    .ghost()
                    .icon(IconName::Plus)
                    .tooltip(format!(
                        "{}\n{}",
                        self.tr("Add to queue"),
                        self.tr("Hold to shuffle")
                    ))
                    .on_long_press(cx.listener(move |view, _, _, cx| {
                        if let Some(source) = source {
                            view.play_card_item(source, &held_add, index, Enqueue::End, true, cx);
                        } else {
                            view.enqueue_items(vec![held_add.clone()], Enqueue::End, true, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(source) = source {
                            this.play_card_item(source, &add, index, Enqueue::End, false, cx);
                        } else {
                            this.enqueue(add.clone(), Enqueue::End, cx);
                        }
                    })),
            )
            .when(
                self.state.settings.show_favorites
                    && item.playlist_songs.is_empty()
                    && matches!(
                        item.kind,
                        ItemKind::Song | ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                    ),
                |div| {
                    div.child(
                        crate::styles::button(("favorite", index), cx)
                            .small()
                            .ghost()
                            .icon(if item.favorite {
                                IconName::Heart
                            } else {
                                IconName::HeartOff
                            })
                            .selected(item.favorite)
                            .tooltip(crate::appearance::text("Favorite", cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_favorite(favorite.clone(), cx)
                            })),
                    )
                },
            )
            .when(item.kind == ItemKind::Song, |div| {
                div.child(
                    crate::styles::button(("similar-songs", index), cx)
                        .small()
                        .ghost()
                        .label(crate::appearance::text("≈", cx))
                        .tooltip(crate::appearance::text("Similar songs", cx))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_similar(similar.clone(), cx)
                        })),
                )
                .child(
                    crate::styles::button(("download", index), cx)
                        .small()
                        .ghost()
                        .icon(IconName::ArrowDown)
                        .tooltip(crate::appearance::text("Download original file", cx))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.download(download.clone(), cx)),
                        ),
                )
                .when(
                    self.state.settings.show_ratings
                        && item.playlist_songs.is_empty()
                        && self
                            .sessions
                            .get(&item.server_id)
                            .is_some_and(|api| api.server.kind != ServerType::Jellyfin),
                    |div| div.child(self.rating(format!("rating-{index}"), item, cx)),
                )
            })
            .when(
                self.sessions
                    .get(&item.server_id)
                    .is_some_and(|api| api.server.kind == ServerType::Navidrome)
                    && !matches!(item.kind, ItemKind::Genre | ItemKind::Radio),
                |div| {
                    div.child(
                        crate::styles::button(("share-item", index), cx)
                            .small()
                            .ghost()
                            .icon(IconName::ExternalLink)
                            .tooltip(crate::appearance::text("Share", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_share(shareable.action_items(), window, cx)
                            })),
                    )
                },
            )
            .when(
                item.kind == ItemKind::Radio && self.can_edit_radio(item),
                |div| {
                    let upload = item.clone();
                    let remove = item.clone();
                    div.child(
                        crate::styles::button(("edit-radio", index), cx)
                            .small()
                            .ghost()
                            .icon(IconName::Settings)
                            .tooltip(crate::appearance::text("Edit station", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_radio(Some(editable_radio.clone()), window, cx)
                            })),
                    )
                    .when(
                        self.sessions.get(&item.server_id).is_some_and(|api| {
                            matches!(
                                api.server.kind,
                                ServerType::Navidrome | ServerType::Jellyfin
                            )
                        }),
                        |div| {
                            div.child(
                                crate::styles::button(("radio-image", index), cx)
                                    .small()
                                    .ghost()
                                    .label(crate::appearance::text("Artwork", cx))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.choose_library_image(upload.clone(), window, cx)
                                    })),
                            )
                            .child(
                                crate::styles::button(("remove-radio-image", index), cx)
                                    .small()
                                    .ghost()
                                    .label(crate::appearance::text("Remove artwork", cx))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.change_library_image(remove.clone(), None, cx)
                                    })),
                            )
                        },
                    )
                },
            )
            .when(item.kind == ItemKind::Album, |div| {
                div.child(
                    crate::styles::button(("expand-album", index), cx)
                        .debug_selector(move || format!("expand-album-{index}"))
                        .small()
                        .ghost()
                        .icon(IconName::ChevronDown)
                        .selected(self.expanded_album.as_ref().is_some_and(|album| {
                            Self::album_key(&album.item) == Self::album_key(item)
                        }))
                        .tooltip(self.tr("Expand"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.toggle_expanded_album(expandable.clone(), cx)
                        })),
                )
            })
            .child(if carousel {
                crate::styles::button(("carousel-more", index), cx)
                    .ghost()
                    .icon(IconName::Ellipsis)
                    .tooltip(self.tr("More"))
                    .dropdown_menu(self.item_menu_builder(item, item.action_items(), false, cx))
                    .into_any_element()
            } else {
                self.library_item_menu(item, index, cx)
            })
            .when(
                item.kind == ItemKind::Playlist || self.can_edit_radio(item),
                |div| {
                    div.child(
                        crate::styles::button(("delete-item", index), cx)
                            .small()
                            .ghost()
                            .icon(IconName::Delete)
                            .tooltip(crate::appearance::text("Delete", cx))
                            .disabled(
                                item.kind == ItemKind::Playlist && !self.can_edit_playlist(item),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.delete_library_item(deletable.clone(), window, cx)
                            })),
                    )
                },
            )
            .into_any_element()
    }

    pub(crate) fn row(&self, item: &Item, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let open = item.clone();
        let playlist = item.clone();
        let item_for_remove = item.clone();
        let item_for_move = item.clone();
        let item_for_top = item.clone();
        let target = if let Route::Detail(item) = &self.route {
            Some(item.clone())
        } else {
            None
        };
        let dragged = self.drag_library_item(index);
        let config = self.list_config();
        let configured = config.display != "list";
        gpui::div()
            .id(("item", index))
            .on_hover(cx.listener(move |this, hovered, _, cx| this.hover_table_row(crate::lists::TableRow::Library, index, *hovered, cx)))
            .relative()
            .role(gpui::Role::Row)
            .aria_label(item.name.clone())
            .aria_selected(self.selected.contains(&index))
            .aria_row_index(index)
            .debug_selector(move || format!("library-row-{index}"))
            .when(
                matches!(&self.track_scroll_target, Some((crate::lists::TableRow::Library, target, _)) if *target == index),
                |div| div.relative().child(self.track_scroll_marker(cx)),
            )

            .on_drop_with_bounds(
                crate::queue::row_drop_listener(cx, move |this, dragged, after, _, cx| {
                    if matches!(&this.route, Route::Detail(item) if item.kind == ItemKind::Playlist)
                    {
                        this.drop_playlist_tracks(dragged, Some(index + usize::from(after)), cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .on_drag(dragged, crate::context_preview::drag_constructor(cx))
            .h(px(config.table.size.row_height() * self.state.settings.zoom))
            .w_full()
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .gap_3()
            .px_3()
            .when(config.table.enable_horizontal_borders, |div| {
                div.border_b_1().border_color(cx.theme().border)
            })
            .bg(crate::styles::row_background(config.table.enable_alternate_row_colors && index % 2 == 1, self.selected.contains(&index), false, cx))
            .when(config.table.enable_row_hover_highlight, |div| {
                div.hover(|style| style.bg(crate::styles::row_background(config.table.enable_alternate_row_colors && index % 2 == 1, self.selected.contains(&index), true, cx)))
            })
            .child(
                crate::styles::button(("select", index), cx)
                    .selected(self.selected.contains(&index))
                    .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                    .small()
                    .ghost()
                    .label(if self.selected.contains(&index) {
                        "✓"
                    } else {
                        "□"
                    })
                    .tooltip(crate::appearance::text("Select item", cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_library_index(
                            index,
                            Modifiers {
                                control: true,
                                ..Modifiers::none()
                            },
                            cx,
                        );
                    })),
            )
            .when(configured, |div| {
                div.child(self.metadata_columns(item, index, cx))
            })
            .when(!configured, |div| {
                div.child(self.artwork(item, 42.0, cx))
                    .child(
                        gpui::div()
                            .id(("open", index))
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .cursor_pointer()
                            .child(
                                gpui::div()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(item_title(
                                        item,
                                        item.name.clone(),
                                        feishin_core::lists::Align::Start,
                                        cx,
                                    )),
                            )
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if item.artist.is_empty() {
                                        self.tr(&Route::Library(item.kind).title())
                                    } else {
                                        format!("{}  ·  {}", item.artist, item.album)
                                    }),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.route == Route::PlaylistPicker {
                                    this.open_add_to_playlist(this.form_songs.clone(), vec![open.clone()], window, cx);
                                } else if matches!(open.kind, ItemKind::Song | ItemKind::Radio) {
                                    this.play_library_item(open.clone(), cx);
                                } else {
                                    this.navigate(Route::Detail(Box::new(open.clone())), cx);
                                }
                            })),
                    )
                    .when(item.duration > 0.0, |div| {
                        div.child(
                            gpui::div()
                                .w(px(55.0))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(duration_label(item.duration)),
                        )
                    })
                    .when(self.route != Route::PlaylistPicker, |div| {
                        div.child(self.item_actions(item, index, cx))
                    })
            })
            .when(item.kind == ItemKind::Song, |div| {
                div.child(
                    crate::styles::button(("playlist", index), cx)
                        .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                        .small()
                        .ghost()
                        .icon(IconName::BookOpen)
                        .tooltip(crate::appearance::text("Add to playlist", cx))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_add_to_playlist(vec![playlist.clone()], vec![], window, cx);
                        })),
                )
            })
            .when(
                target
                    .as_ref()
                    .is_some_and(|item| item.kind == ItemKind::Playlist),
                |div| {
                    div.child(
                        crate::styles::button(("remove-track", index), cx)
                            .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                            .small()
                            .ghost()
                            .icon(IconName::Minus)
                            .tooltip(crate::appearance::text("Remove from playlist", cx))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.remove_playlist_track(
                                    item_for_remove.clone(),
                                    index,
                                    cx,
                                );
                            })),
                    )
                    .child(
                        crate::styles::button(("move-playlist-track", index), cx)
                            .w(px(32. * self.state.settings.zoom)).px_0().flex_shrink_0()
                            .small()
                            .ghost()
                            .icon(IconName::ArrowUp)
                            .tooltip(crate::appearance::text("Move up", cx))
                            .disabled(self.playlist_reorder_target(index, false, false).is_none())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.reorder_playlist_row(&item_for_move, index, false, false, cx)
                            }))
                            .on_long_press(cx.listener(move |this, _, _, cx| {
                                this.reorder_playlist_row(&item_for_top, index, false, true, cx)
                            })),
                    )
                },
            )
            .context_menu(self.library_menu_builder(item, index, cx))
            .into_any_element()
    }

    pub(crate) fn play_card_item(
        &mut self,
        source: CardSource,
        item: &Item,
        index: usize,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if source == CardSource::Library && item.kind == ItemKind::Song {
            self.play_table_context(index, mode, shuffle, cx);
        } else {
            self.enqueue_items(vec![item.clone()], mode, shuffle, cx);
        }
    }

    pub(crate) fn click_card_item(
        &mut self,
        source: CardSource,
        item: &Item,
        index: usize,
        event: &ClickEvent,
        cx: &mut Context<Self>,
    ) {
        if source == CardSource::Library
            && (self.state.settings.enable_grid_multiselect || item.kind == ItemKind::Song)
        {
            self.table_item_click(index, event, cx);
        } else if source == CardSource::Carousel
            && (item.kind == ItemKind::Song || event.click_count() == 2)
        {
            // Standalone cards have no list selection or double-click control.
        } else if item.kind == ItemKind::Radio {
            self.play_library_item(item.clone(), cx);
        } else {
            self.navigate(Route::Detail(Box::new(item.clone())), cx);
        }
    }

    pub(crate) fn card(&self, item: &Item, index: usize, cx: &mut Context<Self>) -> AnyElement {
        self.item_card(item, index, CardSource::Library, None, cx)
    }
    pub(crate) fn item_card(
        &self,
        item: &Item,
        index: usize,
        source: CardSource,
        width: Option<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let carousel = matches!(source, CardSource::Carousel);
        let config = if carousel {
            let key = if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
                "albumArtist"
            } else if item.kind == ItemKind::Playlist {
                "playlist"
            } else if item.kind == ItemKind::Song {
                "song"
            } else {
                "album"
            };
            self.state
                .settings
                .lists
                .get(key)
                .cloned()
                .unwrap_or_else(|| feishin_core::lists::default_for(key))
        } else {
            self.list_config()
        };
        crate::item_card::Card {
            app: cx.weak_entity(),
            item: item.clone(),
            index,
            source,
            width,
            config,
        }
        .into_any_element()
    }

    pub(crate) fn render_card(
        &self,
        props: &crate::item_card::Card,
        state: std::rc::Rc<std::cell::RefCell<crate::item_card::State>>,
        frame: crate::item_card::Frame,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item = &props.item;
        let index = props.index;
        let source = props.source;
        let config = &props.config;
        let carousel = source == CardSource::Carousel;
        let compact = !carousel && config.grid.size == feishin_core::lists::Size::Compact;
        let open = item.clone();
        let font = cx.theme().font_size;
        let radius = font * 5. / 16.;
        let hovered = state.clone();
        let measure = state.clone();
        let app = cx.weak_entity();
        let art_width = frame.width;
        let selected = !carousel && self.selected.contains(&index);
        let round = carousel && matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist);
        let variant = if compact { "compact" } else { "poster" };
        let mut card_target = crate::item_card::css_target("div", &["container", variant]);
        if selected {
            card_target
                .classes
                .push("fs-item-card-module-selected".into());
        }
        let mut image_target = crate::item_card::css_target("div", &["image-container"]);
        if round {
            image_target
                .classes
                .push("fs-item-card-module-is-round".into());
        }
        if frame.hovered {
            image_target.states.push("hover".into());
        }
        let mut metadata_target = crate::item_card::css_target("div", &["detail-container"]);
        if compact {
            metadata_target
                .classes
                .push("fs-item-card-module-compact".into());
        }
        let mut metadata_parents = vec![card_target.clone()];
        if compact {
            metadata_parents.push(image_target.clone());
        }
        let mut row_parents = metadata_parents.clone();
        row_parents.push(metadata_target.clone());
        let action_items = if selected {
            self.selected_library_items()
        } else {
            item.action_items()
        };
        let dragged = if carousel {
            crate::queue::Dragged {
                sidebar_playlist: false,
                playlist_source: None,
                keys: vec![],
                items: vec![item.clone()],
            }
        } else {
            self.drag_library_item(index)
        };
        let menu = self.item_menu_builder(item, action_items, false, cx);
        let drag_view = cx.weak_entity();
        let drag_preview = crate::context_preview::drag_constructor(cx);
        let select_on_drag = !carousel && self.state.settings.enable_grid_multiselect && !selected;
        let metadata_height = font
            * (0.5
                + config
                    .grid
                    .rows
                    .iter()
                    .filter(|row| row.is_enabled)
                    .enumerate()
                    .map(|(i, _)| if i == 0 { 18. / 16. } else { 1. })
                    .sum::<f32>());
        let metadata = gpui::div()
            .id(("card-metadata", index))
            .debug_selector(move || format!("card-metadata-{index}"))
            .v_flex()
            .w_full()
            .min_w_0()
            .overflow_hidden()
            .when(!compact, |div| div.pt(font * 0.5).gap(font * 0.25))
            .when(compact, |div| {
                div.absolute()
                    .bottom(-metadata_height * frame.dim)
                    .left_0()
                    .p(font * 0.25)
                    .bg(cx.theme().background.opacity(0.5))
                    .backdrop_blur(px(2. * self.state.settings.zoom))
                    .opacity(1. - frame.dim)
            })
            .children(
                config
                    .grid
                    .rows
                    .iter()
                    .filter(|row| row.is_enabled)
                    .enumerate()
                    .map(|(row_index, row)| {
                        let text = feishin_core::metadata_format::card_text(
                            item,
                            &row.id,
                            index,
                            &self.state.settings.language,
                        );
                        let title = row_index == 0;
                        let open = item.clone();
                        let mut row_classes = vec![
                            "row",
                            variant,
                            match row.align {
                                feishin_core::lists::Align::Center => "align-center",
                                feishin_core::lists::Align::End => "align-end",
                                _ => "align-start",
                            },
                        ];
                        row_classes.push(if title { "bold" } else { "muted" });
                        if matches!(row.id.as_str(), "artists" | "albumArtists") {
                            row_classes.push("artists-row");
                        }
                        gpui::div()
                            .id(("card-metadata-row", row_index))
                            .debug_selector(move || format!("card-metadata-{index}-{row_index}"))
                            .w_full()
                            .min_w_0()
                            .truncate()
                            .text_size(font * if title { 14. / 16. } else { 13. / 16. })
                            .line_height(font * if title { 18. / 16. } else { 1. })
                            .when(title, |div| div.font_weight(FontWeight::MEDIUM))
                            .when(
                                row.id == "name"
                                    && matches!(
                                        item.kind,
                                        ItemKind::Album
                                            | ItemKind::AlbumArtist
                                            | ItemKind::Genre
                                            | ItemKind::Playlist
                                    ),
                                |div| {
                                    let keyboard_open = open.clone();
                                    div.role(Role::Link)
                                        .aria_label(item.name.clone())
                                        .key_context("FormControl")
                                        .focusable()
                                        .tab_stop(true)
                                        .cursor_pointer()
                                        .hover(|style| style.underline())
                                        .focus(|style| style.underline())
                                        .on_key_down(cx.listener(
                                            move |view, event: &KeyDownEvent, _, cx| {
                                                if event.keystroke.key == "enter" {
                                                    view.navigate(
                                                        Route::Detail(Box::new(
                                                            keyboard_open.clone(),
                                                        )),
                                                        cx,
                                                    );
                                                    cx.stop_propagation();
                                                }
                                            },
                                        ))
                                        .on_click(cx.listener(move |view, _, _, cx| {
                                            view.navigate(
                                                Route::Detail(Box::new(open.clone())),
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        }))
                                },
                            )
                            .when(!title && !compact, |div| {
                                div.text_color(cx.theme().muted_foreground)
                            })
                            .when(row.align == feishin_core::lists::Align::Center, |div| {
                                div.text_center()
                            })
                            .when(row.align == feishin_core::lists::Align::End, |div| {
                                div.text_right()
                            })
                            .map(|div| {
                                crate::styles::apply_in(
                                    div,
                                    crate::item_card::css_target("p", &row_classes),
                                    row_parents.clone(),
                                    cx,
                                )
                            })
                            .child(
                                if matches!(
                                    row.id.as_str(),
                                    "name" | "titleArtist" | "titleCombined"
                                ) {
                                    item_title(item, text, row.align, cx).into_any_element()
                                } else if row.id == "artists" {
                                    self.named_artists(
                                        feishin_core::artists::links(item, false),
                                        item.server_id,
                                        " • ",
                                        "card-artists",
                                        cx,
                                    )
                                } else if row.id == "albumArtists" {
                                    self.joined_artists(item, true, "card-album-artists", cx)
                                } else {
                                    text.into_any_element()
                                },
                            )
                    }),
            )
            .map(|div| crate::styles::apply_in(div, metadata_target, metadata_parents, cx));
        let mut art = gpui::div()
            .id(SharedString::from(format!(
                "card-open-{}-{:?}-{}-{index}",
                item.server_id, item.kind, item.id
            )))
            .debug_selector(move || format!("card-open-{index}"))
            .relative()
            .w_full()
            .map(|mut div| {
                div.style().aspect_ratio = Some(1.);
                div
            })
            .rounded(radius)
            .overflow_hidden()
            .bg(cx.theme().secondary)
            .cursor_pointer()
            .child(self.image_content_in(
                item,
                feishin_core::images::Placement::ItemCard,
                if round { art_width / 2. } else { radius },
                Some((
                    {
                        let mut target = crate::item_card::css_target("img", &["image"]);
                        target.classes.push("fs-image-module-image".into());
                        if round {
                            target.classes.push("fs-item-card-module-is-round".into());
                        }
                        target
                    },
                    vec![card_target.clone(), image_target.clone()],
                )),
                cx,
            ))
            .child(
                gpui::div()
                    .debug_selector(move || format!("card-shade-{index}"))
                    .absolute()
                    .inset_0()
                    .rounded(radius)
                    .when(round, |d| d.rounded_full())
                    .bg(black()
                        .opacity(frame.dim * if cx.theme().mode.is_dark() { 0.7 } else { 0.5 }))
                    .map(|div| {
                        crate::styles::apply_in(
                            div,
                            feishin_core::styles::Target::pseudo("before"),
                            [card_target.clone(), image_target.clone()],
                            cx,
                        )
                    }),
            )
            .when(self.state.settings.show_favorites && item.favorite, |art| {
                art.child(
                    crate::item_card::favorite_badge(
                        self.state.settings.zoom,
                        frame.dim,
                        size(art_width, frame.height),
                        vec![card_target.clone(), image_target.clone()],
                        cx,
                    )
                    .debug_selector(move || format!("card-favorite-badge-{index}")),
                )
            })
            .when(self.state.settings.show_ratings && item.rating > 0, |art| {
                art.child(
                    gpui::div()
                        .debug_selector(move || format!("card-rating-badge-{index}"))
                        .absolute()
                        .top(font * 0.5)
                        .right(font * 0.5)
                        .px(font * 0.5)
                        .py(font * 0.25)
                        .rounded(radius)
                        .text_size(font * 14. / 16.)
                        .line_height(font * 18. / 16.)
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(cx.theme().primary)
                        .shadow(vec![BoxShadow {
                            color: black().opacity(0.5),
                            offset: point(px(0.), px(2. * self.state.settings.zoom)),
                            blur_radius: px(8. * self.state.settings.zoom),
                            spread_radius: px(0.),
                        }])
                        .text_shadows(vec![TextShadow {
                            color: Some(black().opacity(0.8)),
                            offset: point(px(0.), px(self.state.settings.zoom)),
                            blur_radius: px(2. * self.state.settings.zoom),
                        }])
                        .opacity(1. - frame.dim)
                        .map(|div| {
                            div.refine_style(&crate::item_card::badge_style(
                                "rating-badge",
                                vec![card_target.clone(), image_target.clone()],
                                frame.dim,
                                cx,
                            ))
                        })
                        .child(item.rating.to_string()),
                )
            })
            .on_hover(cx.listener(move |_, visible, _, cx| {
                hovered
                    .borrow_mut()
                    .hover(*visible, cx.background_executor().now());
                cx.notify();
            }))
            .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
                view.click_card_item(source, &open, index, event, cx)
            }))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let measure = measure.clone();
                        let app = app.clone();
                        window.defer(cx, move |_, cx| {
                            if measure.borrow().size != bounds.size {
                                measure.borrow_mut().size = bounds.size;
                                app.update(cx, |_, cx| cx.notify()).ok();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        art = crate::styles::apply_in(art, image_target.clone(), [card_target.clone()], cx);
        let mut metadata = Some(metadata);
        if compact {
            art = art.child(metadata.take().unwrap());
        }
        if let Some((opacity, position, _)) = frame.controls {
            art = art.child(
                gpui::div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w_full()
                    .h(frame.height)
                    .opacity(opacity)
                    .when(compact, |div| div.paint_scale(position))
                    .when(!compact, |div| {
                        div.top(px(position * self.state.settings.zoom))
                    })
                    .child(self.card_controls(
                        item,
                        index,
                        source,
                        size(art_width, frame.height),
                        vec![card_target.clone(), image_target.clone()],
                        cx,
                    )),
            );
        }
        gpui::div()
            .id(("card", index))
            .when(!carousel || item.kind != ItemKind::Playlist, |card| {
                card.on_drag(dragged, move |dragged, position, window, cx| {
                    if select_on_drag {
                        drag_view
                            .update(cx, |view, cx| {
                                view.selected = [index].into();
                                view.grid_selection_order = vec![index];
                                cx.notify();
                            })
                            .ok();
                    }
                    drag_preview(dragged, position, window, cx)
                })
            })
            .debug_selector(move || format!("item-card-{index}"))
            .relative()
            .v_flex()
            .flex_1()
            .min_w_0()
            .rounded(radius)
            .when(!carousel, |card| card.h_full())
            .child(
                div()
                    .v_flex()
                    .size_full()
                    .overflow_hidden()
                    .rounded(radius)
                    .child(art)
                    .children(metadata),
            )
            .when(selected, |div| {
                let offset = match config.grid.item_gap.as_str() {
                    "xs" => px(2. * self.state.settings.zoom),
                    "sm" => font * 0.25,
                    _ => font * 0.5,
                };
                let border = px(2. * self.state.settings.zoom);
                div.child(
                    gpui::div()
                        .absolute()
                        .top(-offset - border)
                        .left(-offset - border)
                        .bottom(-offset - border)
                        .right(-offset - border)
                        .rounded(radius + offset + border)
                        .border(border)
                        .border_color(cx.theme().primary),
                )
            })
            .context_menu(menu)
            .map(|div| crate::styles::apply_in(div, card_target, [], cx))
            .into_any_element()
    }

    fn library_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        if let Route::Detail(item) = &self.route
            && item.kind == ItemKind::Playlist
        {
            return self.playlist_page_header(item, window, cx);
        }
        let mut container = gpui::div()
            .id("library-controls")
            .v_flex()
            .w_full()
            .flex_shrink_0()
            .gap_3()
            .when(self.has_detail_page_header(), |container| container.gap_0());
        if let Route::Detail(item) = &self.route {
            let item = item.as_ref();
            let item = item.clone();
            if item.kind == ItemKind::Genre {
                container = container.child(
                    crate::styles::button("genre-target", cx)
                        .label(if self.state.settings.genre_albums {
                            "Show songs"
                        } else {
                            "Show albums"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.remember_view();
                            this.state.settings.genre_albums = !this.state.settings.genre_albums;
                            this.restore_view();
                            this.save();
                            this.reload(false, cx);
                        })),
                );
            }
            if self.has_local_filter()
                && item.kind != ItemKind::Album
                && !self.artist_overview_active()
            {
                container = container.child(self.detail_search_control(cx));
            }
            {
                let measured_width = self.detail_header_width;
                let header_width = if measured_width > px(0.) {
                    measured_width
                } else {
                    window.viewport_size().width - px(self.sidebar_pixel_width())
                };
                let header = crate::library_header::Layout::new(
                    header_width,
                    item.kind == ItemKind::Playlist,
                    cx,
                );
                let large_artist = matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist)
                    && header_width >= cx.theme().font_size * 75.;
                let measured = cx.weak_entity();
                let item_kind = item.kind;
                let library_kind = if item_kind == ItemKind::Artist {
                    ItemKind::AlbumArtist
                } else {
                    item_kind
                };
                let release_type = (item_kind == ItemKind::Album)
                    .then(|| item.column_data.get("releaseType"))
                    .flatten()
                    .filter(|value| !value.is_empty())
                    .and_then(|value| {
                        feishin_core::releases::metadata_labels(
                            std::slice::from_ref(value),
                            &self.state.settings.language,
                        )
                        .into_iter()
                        .next()
                    });
                let title =
                    if item_kind == ItemKind::Playlist && self.state.settings.playlist_folders {
                        feishin_core::playlist_folders::leaf(
                            &item.name,
                            &self.state.settings.playlist_separator,
                        )
                    } else {
                        &item.name
                    };
                container = container.child(
                    header
                        .frame()
                        .when(large_artist, |frame| {
                            // The original artist header is a direct flex child with a
                            // 340px minimum; large artwork extends into its bottom padding.
                            frame.h(px(340. * self.state.settings.zoom)).items_start()
                        })
                        .child(
                            canvas(
                                move |bounds, window, cx| {
                                    if bounds.size.width != measured_width {
                                        let measured = measured.clone();
                                        window.defer(cx, move |_, cx| {
                                            measured
                                                .update(cx, |view, cx| {
                                                    if view.detail_header_width != bounds.size.width
                                                    {
                                                        view.detail_header_width =
                                                            bounds.size.width;
                                                        cx.notify();
                                                    }
                                                })
                                                .ok();
                                        });
                                    }
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .size_full(),
                        )
                        .child(self.detail_background_view(cx))
                        .child(self.detail_header_artwork(&item, header.artwork_size(), cx))
                        .child(
                            header
                                .metadata()
                                .when(large_artist, |metadata| {
                                    metadata.min_h(header.artwork_size())
                                })
                                .child(
                                    header
                                        .metadata_row()
                                        .gap(cx.theme().font_size * 0.5)
                                        .child(
                                            header
                                                .item_type(
                                                    release_type
                                                        .clone()
                                                        .unwrap_or_else(|| {
                                                            if item.rules.is_some() {
                                                                self.tr("entity.smartPlaylist")
                                                            } else {
                                                                crate::library_header::type_label(
                                                                    item_kind,
                                                                    &self.state.settings.language,
                                                                )
                                                            }
                                                        })
                                                        .to_uppercase(),
                                                )
                                                .id("library-detail-type")
                                                .role(Role::Link)
                                                .key_context("FormControl")
                                                .focusable()
                                                .tab_stop(true)
                                                .cursor_pointer()
                                                .on_key_down(cx.listener(
                                                    move |view, event: &KeyDownEvent, _, cx| {
                                                        if event.keystroke.key == "enter" {
                                                            cx.stop_propagation();
                                                            view.navigate(
                                                                Route::Library(library_kind),
                                                                cx,
                                                            );
                                                        }
                                                    },
                                                ))
                                                .on_click(cx.listener(move |view, _, _, cx| {
                                                    view.navigate(Route::Library(library_kind), cx);
                                                })),
                                        )
                                        .when_some(
                                            release_type
                                                .as_ref()
                                                .and_then(|_| item.column_data.get("version"))
                                                .filter(|value| !value.is_empty()),
                                            |row, version| {
                                                row.child(
                                                    gpui::div()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child("•"),
                                                )
                                                .child(version.clone())
                                            },
                                        ),
                                )
                                .child(header.title(title.to_owned(), window))
                                .child(
                                    gpui::div()
                                        .v_flex()
                                        .w_full()
                                        .gap(cx.theme().font_size * 0.75)
                                        .when(
                                            matches!(
                                                item_kind,
                                                ItemKind::Album
                                                    | ItemKind::Artist
                                                    | ItemKind::AlbumArtist
                                            ),
                                            |content| {
                                                content.child(
                                                    self.detail_header_metadata(&item, &header, cx),
                                                )
                                            },
                                        )
                                        .when(
                                            !item.comment.is_empty()
                                                && !matches!(
                                                    item_kind,
                                                    ItemKind::Artist
                                                        | ItemKind::AlbumArtist
                                                        | ItemKind::Album
                                                ),
                                            |metadata| {
                                                metadata.child(if item_kind == ItemKind::Playlist {
                                                    crate::library_header::description(
                                                        &item.comment,
                                                        true,
                                                        window,
                                                        cx,
                                                    )
                                                } else {
                                                    gpui::div()
                                                        .text_sm()
                                                        .child(item.comment.clone())
                                                        .into_any_element()
                                                })
                                            },
                                        )
                                        .child(self.detail_header_menu(&item, header_width, cx)),
                                ),
                        ),
                );
            }
            if item.kind == ItemKind::Album && !item.comment.is_empty() {
                container = container.child(
                    div()
                        .px(cx.theme().font_size * 2.)
                        .pt(cx.theme().font_size)
                        .child(crate::library_header::description(
                            &item.comment,
                            false,
                            window,
                            cx,
                        )),
                );
            }
            if item.kind != ItemKind::Album && !self.artist_overview_active() {
                container = container.child(self.external_links(&item, cx));
            }
            if matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist)
                && !self.artist_overview_active()
            {
                container = container.child(self.detail_genre_links(&item, cx));
                container = container.child(
                    gpui::div()
                        .h_flex()
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .flex_wrap()
                        .gap_2()
                        .child(
                            crate::styles::button("artist-overview", cx)
                                .label(self.tr("Overview"))
                                .selected(self.artist_overview)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.artist_overview = true;
                                    this.reload(false, cx);
                                })),
                        )
                        .children(
                            ArtistTab::ALL
                                .into_iter()
                                .enumerate()
                                .filter(|(_, tab)| {
                                    (*tab != ArtistTab::Rated
                                        || self.state.settings.show_ratings
                                            && self.sessions.get(&item.server_id).is_some_and(
                                                |api| api.server.kind != ServerType::Jellyfin,
                                            ))
                                        && (*tab != ArtistTab::Favorites
                                            || self.state.settings.show_favorites)
                                        && (*tab != ArtistTab::CommunityTop
                                            || self.api().is_some_and(|api| {
                                                api.server.kind != ServerType::Jellyfin
                                            }))
                                })
                                .map(|(i, tab)| {
                                    crate::styles::button(("artist-tab", i), cx)
                                        .label(self.tr(tab.label()))
                                        .selected(!self.artist_overview && self.artist_tab == tab)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.change_artist_tab(tab, cx);
                                        }))
                                }),
                        ),
                );
                if let Some(info) = &self.artist_info {
                    if !self.artist_overview
                        && (!info.biography.is_empty() || !item.comment.is_empty())
                    {
                        container = container.child(
                            gpui::div()
                                .id("artist-biography")
                                .max_h(px(150.0))
                                .overflow_y_scroll()
                                .text_sm()
                                .v_flex()
                                .child(crate::artist_overview::biography_view(
                                    info,
                                    &item.comment,
                                    window,
                                    cx,
                                )),
                        );
                    }
                    if !info.homepage.is_empty() {
                        let url = info.homepage.clone();
                        container = container.child(
                            crate::styles::button("artist-homepage", cx)
                                .label(crate::appearance::text("Artist page", cx))
                                .on_click(move |_, _, cx| cx.open_url(&url)),
                        );
                    }
                }
            }
        }
        if self.artist_overview_active()
            || matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album)
        {
            return container;
        }
        if self.playlist_tracks_editing() {
            return self.library_list_controls(cx);
        }
        container.child(self.library_list_controls(cx))
    }

    fn detail_search_control(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.direct_detail_search() {
            gpui::div()
                .debug_selector(|| "detail-search-control".into())
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .gap_2()
                .child(crate::styles::input(&self.local_search, cx))
                .child(
                    crate::styles::button("clear-list-filter", cx)
                        .label(crate::appearance::text("Clear filter", cx))
                        .disabled(self.local_filter.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.local_search
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            this.submit_local_filter(String::new(), cx);
                        })),
                )
                .into_any_element()
        } else {
            self.list_search_control(false, false, cx)
        }
    }

    pub(crate) fn library_list_controls(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let config = self.list_config();
        if self.has_standard_list_header() || self.has_playlist_header() {
            return div().id("library-track-controls").w_full().when(
                config.display == "table" && config.table.enable_header,
                |div| div.child(self.configured_header(cx)),
            );
        }
        let is_grid = config.display == "poster";
        let mut container = div()
            .id("library-track-controls")
            .debug_selector(|| "library-track-controls".into())
            .v_flex()
            .w_full()
            .gap_3();
        if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album) {
            return self.album_track_controls(cx);
        }
        if let Route::Search(_, selected) = self.route {
            container = container.child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_1()
                    .children(
                        [
                            (ItemKind::Song, "entity.track", "search-tracks"),
                            (ItemKind::Album, "entity.album", "search-albums"),
                            (ItemKind::AlbumArtist, "entity.artist", "search-artists"),
                        ]
                        .into_iter()
                        .map(|(kind, label, id)| {
                            crate::styles::button(id, cx)
                                .debug_selector(move || id.into())
                                .label(
                                    feishin_core::i18n::translate_with(
                                        &self.state.settings.language,
                                        label,
                                        &serde_json::json!({"count": 2}),
                                    )
                                    .unwrap_or_else(|| self.tr(label)),
                                )
                                .selected(selected == kind)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.change_search_kind(kind, cx)
                                }))
                        }),
                    ),
            );
        }
        container = container.child(
            gpui::div()
                .h_flex().when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .flex_wrap()
                .gap_2()
                .when(self.has_local_filter() && !matches!(self.route, Route::Detail(_)), |div| {
                    div.child(self.list_search_control(false, false, cx))
                })
                .child(
                    crate::styles::button("play-visible", cx)
                        .disabled(!self.library_fetches.is_empty())
                        .primary()
                        .debug_selector(|| "play-visible".into())
                        .label(if !self.has_library_selection() {
                            "Play all".into()
                        } else {
                            format!("Play {} selected", self.selected_library_items().len())
                        })
                        .tooltip(format!("{}\n{}", self.tr("Play all"), self.tr("Hold to shuffle")))
                        .on_long_press(cx.listener(|view, _, _, cx| {
                            view.enqueue_visible_shuffled(view.state.settings.play_button_behavior.mode(), cx)
                        }))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.play_visible(cx)),
                        ),
                )
                .child(
                    crate::styles::button("next-visible", cx)
                        .debug_selector(|| "next-visible".into())
                        .label(self.tr("Play next"))
                        .disabled(!self.library_fetches.is_empty())
                        .tooltip(format!("{}\n{}", self.tr("Play next"), self.tr("Hold to shuffle")))
                        .on_long_press(cx.listener(|view, _, _, cx| view.enqueue_visible_shuffled(Enqueue::Next, cx)))
                        .on_click(cx.listener(|view, _, _, cx| view.enqueue_visible(Enqueue::Next, cx))),
                )
                .child(
                    crate::styles::button("add-visible", cx).debug_selector(|| "add-visible".into()).disabled(!self.library_fetches.is_empty()).label(self.tr("player.addLast")).tooltip(format!("{}\n{}", self.tr("Add to queue"), self.tr("Hold to shuffle"))).on_long_press(cx.listener(|view, _, _, cx| view.enqueue_visible_shuffled(Enqueue::End, cx))).on_click(
                        cx.listener(|this, _, _, cx| this.enqueue_visible(Enqueue::End, cx)),
                    ),
                )
                .child(
                    crate::styles::button("refresh", cx)
                        .ghost()
                        .debug_selector(|| "refresh-library".into())
                        .label(self.tr("Refresh"))
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_library(cx))),
                )
                .when(self.collection_kind().is_some(), |div| div.child(crate::styles::button("save-collection", cx).label(crate::appearance::text("Save collection", cx)).on_click(cx.listener(|this, _, window, cx| this.edit_collection(None, true, window, cx)))))
                .when(
                    self.route == Route::Library(ItemKind::Artist)
                        && self.api().is_some_and(|api| api.features().artist_roles),
                    |div| {
                        let view = cx.entity().downgrade();
                        div.child(
                            crate::styles::button("artist-role", cx)
                                .label(format!(
                                    "Role: {}",
                                    self.artist_role.as_deref().unwrap_or("all")
                                ))
                                .dropdown_menu(move |mut menu, _, cx| {
                                    let roles = [
                                        None,
                                        Some("arranger"),
                                        Some("artist"),
                                        Some("composer"),
                                        Some("conductor"),
                                        Some("director"),
                                        Some("djmixer"),
                                        Some("engineer"),
                                        Some("lyricist"),
                                        Some("mixer"),
                                        Some("performer"),
                                        Some("producer"),
                                        Some("remixer"),
                                    ];
                                    for role in roles {
                                        let view = view.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(crate::appearance::text(role.unwrap_or("All artists"), cx))
                                                .on_click(move |_, _, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.artist_role = role.map(str::to_owned);
                                                        this.reload(false, cx);
                                                    })
                                                    .ok();
                                                }),
                                        );
                                    }
                                    menu
                                }),
                        )
                    },
                )
                .when(matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist) && !self.selected.is_empty(), |div| {
                    div.child(crate::styles::button("remove-selected-playlist", cx).label(crate::appearance::text("Remove selected", cx)).on_click(cx.listener(|this, _, _, cx| this.playlist_selection_action(feishin_core::api::PlaylistSelectionAction::Remove, cx))))
                    .child(crate::styles::button("move-selected-playlist", cx).label(crate::appearance::text("Move selected", cx)).dropdown_menu({
                        let view = cx.weak_entity(); move |mut menu, _, _| {
                            for (label, before) in [("To top", Some(0)), ("To end", None)] {
                                let view = view.clone(); menu = menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| { view.update(cx, |this, cx| this.playlist_selection_action(feishin_core::api::PlaylistSelectionAction::MoveBefore(before), cx)).ok(); }));
                            }
                            menu
                        }
                    }))
                })
                .when(
                    self.items.iter().any(|item| item.kind == ItemKind::Song),
                    |div| {
                        div.child(
                            crate::styles::button("edit-tags", cx)
                                .label(self.tr("Edit tags"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.edit_selected_tags(window, cx)
                                })),
                        )
                    },
                )
                .when(
                    matches!(self.route, Route::Library(ItemKind::Playlist)),
                    |div| {
                        div.child(
                            crate::styles::button("new-playlist", cx)
                                .label(self.tr("New playlist"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.edit_playlist(None, vec![], window, cx)
                                })),
                        )
                    },
                )
                .when(
                    matches!(self.route, Route::Library(ItemKind::Radio)),
                    |div| {
                        div.child(
                            crate::styles::button("new-radio", cx)
                                .label(self.tr("Add station"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.edit_radio(None, window, cx)
                                })),
                        )
                    },
                )
                .when(matches!(self.route, Route::Favorites(_)), |div| {
                    div.children(
                        [ItemKind::Song, ItemKind::Album, ItemKind::Artist]
                            .into_iter()
                            .enumerate()
                            .map(|(i, kind)| {
                                crate::styles::button(("favorite-type", i), cx)
                                    .small()
                                    .label(format!("{kind:?}"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.navigate(Route::Favorites(kind), cx)
                                    }))
                            }),
                    )
                })
                .child(gpui::div().flex_1())
                .child(
                    crate::styles::button("sort", cx)
                        .ghost()
                        .label(self.sort.label())
                        .dropdown_menu({let view=cx.weak_entity();move |mut menu,_,cx| {for sort in Sort::ALL {let view=view.clone();menu=menu.item(PopupMenuItem::new(crate::appearance::text(sort.label(), cx)).on_click(move |_,_,cx| {view.update(cx,|this,cx| {this.sort=sort;this.reload(false,cx);}).ok();}));}menu}}),
                )
                .child(
                    crate::styles::button("sort-order", cx)
                        .ghost()
                        .icon(if self.descending {
                            IconName::SortDescending
                        } else {
                            IconName::SortAscending
                        })
                        .tooltip(crate::appearance::text("Sort order", cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.descending = !this.descending;
                            this.reload(false, cx);
                        })),
                )
                .child(
                    crate::styles::button("view", cx)
                        .ghost()
                        .icon(if is_grid {
                            IconName::Menu
                        } else {
                            IconName::GalleryVerticalEnd
                        })
                        .tooltip(crate::appearance::text("Switch layout", cx))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let key = this.list_key();
                            let mut config = this.list_config();
                            config.display = if config.display == "poster" { "table" } else { "poster" }.into();
                            this.state.settings.lists.insert(key, config);
                            this.save(); cx.notify();
                        })),
                ),
        );
        if !matches!(self.route, Route::Home | Route::Search(..) | Route::Explore) {
            container = container.child(
                crate::styles::button("library-filter-settings", cx)
                    .label(if self.active_filters.is_empty() {
                        "Filters"
                    } else {
                        "Filters active"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.edit_filters(window, cx))),
            );
        }
        container = container.child(
            crate::styles::button("library-list-settings", cx)
                .label(crate::appearance::text("List layout", cx))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.edit_list_config(this.list_key(), window, cx)
                })),
        );
        if self.playlist_tracks_editing() {
            container = gpui::div()
                .id("playlist-edit-controls")
                .v_flex()
                .gap_3()
                .child(self.playlist_edit_toolbar(cx));
        } else if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist && item.rules.is_none() && self.can_edit_playlist(item))
        {
            container = container.child(
                crate::styles::button("edit-playlist-tracks", cx)
                    .debug_selector(|| "edit-playlist-tracks".into())
                    .label(self.tr("common.edit").to_uppercase())
                    .on_click(cx.listener(|view, _, _, cx| view.start_playlist_tracks_edit(cx))),
            );
        }
        if !is_grid
            && config.table.enable_header
            && !matches!(config.display.as_str(), "list" | "detail")
        {
            container = container.child(self.configured_header(cx));
        }
        container
    }

    fn library_footer(&self, cx: &mut Context<Self>) -> Div {
        let config = self.list_config();
        gpui::div()
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .justify_between()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(
                self.total
                    .map_or(format!("{} items loaded", self.items.len()), |total| {
                        format!("{} of {total} items", self.items.len())
                    }),
            )
            .when(config.pagination == "paginated", |div| {
                div.child(
                    crate::styles::button("library-prev-page", cx)
                        .label(crate::appearance::text("Previous page", cx))
                        .disabled(self.loading || self.library_page == 0)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.change_list_page(this.library_page.saturating_sub(1), cx)
                        })),
                )
                .child(format!("Page {}", self.library_page + 1))
                .child(
                    crate::styles::button("library-next-page", cx)
                        .label(crate::appearance::text("Next page", cx))
                        .disabled(self.loading || !self.has_more)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.change_list_page(this.library_page + 1, cx)
                        })),
                )
            })
            .when(self.has_more && config.pagination != "paginated", |div| {
                div.child(
                    crate::styles::button("load-more", cx)
                        .label(if self.loading {
                            "Loading..."
                        } else {
                            "Load more"
                        })
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| this.reload(true, cx))),
                )
            })
    }

    pub(crate) fn scrolling_detail_header(&self) -> bool {
        !self.playlist_tracks_editing()
            && matches!(&self.route, Route::Detail(item) if matches!(item.kind,
                ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Playlist))
    }

    fn detail_page_rows(&self, columns: usize) -> Vec<DetailPageRow> {
        let mut rows = vec![DetailPageRow::Header];
        if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album) {
            rows.extend(
                self.album_page_rows(columns)
                    .into_iter()
                    .map(DetailPageRow::Album),
            );
        } else if self.grouped_song_albums() {
            rows.extend(
                feishin_core::album_metadata::groups(&self.items)
                    .into_iter()
                    .map(DetailPageRow::GroupedSongs),
            );
        } else if self.list_config().display == "detail"
            && self.items.iter().all(|item| item.kind == ItemKind::Album)
        {
            rows.extend(
                self.album_group_rows(1)
                    .unwrap_or_else(|| {
                        (0..self.items.len())
                            .map(|index| crate::album_groups::AlbumRow {
                                group: None,
                                indexes: vec![index],
                            })
                            .collect()
                    })
                    .into_iter()
                    .map(DetailPageRow::AlbumDetail),
            );
        } else if let Some(albums) = self.album_group_rows(columns) {
            rows.extend(albums.into_iter().map(DetailPageRow::Albums));
        } else {
            rows.extend(
                (0..self.items.len())
                    .collect::<Vec<_>>()
                    .chunks(columns.max(1))
                    .map(|items| DetailPageRow::Items(items.to_vec())),
            );
        }
        if self.items.is_empty() {
            rows.push(DetailPageRow::Empty);
        }
        rows
    }

    pub(crate) fn detail_row_position(&self, item: usize) -> Option<usize> {
        self.detail_page_rows(self.detail_columns)
            .iter()
            .position(|row| match row {
                DetailPageRow::Album(crate::details::AlbumPageRow::Tracks(items))
                | DetailPageRow::Items(items) => items.contains(&item),
                DetailPageRow::GroupedSongs(items) => items.contains(&item),
                DetailPageRow::AlbumDetail(row) | DetailPageRow::Albums(row) => {
                    row.indexes.contains(&item)
                }
                _ => false,
            })
    }

    fn detail_page_view(
        &mut self,
        columns: usize,
        grid: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.detail_columns = columns;
        let rows = self.detail_page_rows(columns);
        let count = self.album_details_scroll.item_count();
        if count < rows.len() {
            self.album_details_scroll
                .splice(count..count, rows.len() - count);
        } else if count != rows.len() {
            self.album_details_scroll.reset(rows.len());
        }
        let config = self.list_config();
        let album = matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album);
        let controls_index = if album { 2 } else { 0 };
        let album_layout = self.album_content_layout(cx);
        let album_end = rows.iter().position(|row| {
            matches!(
                row,
                DetailPageRow::Album(crate::details::AlbumPageRow::GridEnd)
            )
        });
        let table_end =
            album_end.and_then(|index| self.album_details_scroll.bounds_for_item(index));
        let sticky_top = table_end.map_or(px(0.), |end| {
            (end.top()
                - self.album_details_scroll.viewport_bounds().top()
                - px(40. * self.state.settings.zoom))
            .min(px(0.))
        });
        let sticky_header = config.display == "table"
            && album_end
                .is_none_or(|end| self.album_details_scroll.logical_scroll_top().item_ix < end)
            && config.table.enable_header
            && (self.album_details_scroll.logical_scroll_top().item_ix > controls_index
                || self
                    .album_details_scroll
                    .bounds_for_item(controls_index)
                    .is_some_and(|header| {
                        header.bottom() - cx.theme().font_size * 0.75
                            <= self.album_details_scroll.viewport_bounds().top()
                                + px(40. * self.state.settings.zoom)
                    }));
        let row_height = px(config.table.size.row_height() * self.state.settings.zoom);
        let sticky_disc_top = if sticky_header {
            px(41. * self.state.settings.zoom)
        } else {
            px(0.)
        };
        let logical = self.album_details_scroll.logical_scroll_top();
        let sticky_disc = if album
            && config.display == "table"
            && album_end.is_some_and(|end| logical.item_ix < end)
        {
            let table_top = self
                .album_details_scroll
                .bounds_for_item(controls_index)
                .map(|controls| {
                    controls.bottom()
                        - cx.theme().font_size * 0.75
                        - if config.table.enable_header {
                            px(40. * self.state.settings.zoom)
                        } else {
                            px(0.)
                        }
                })
                .or_else(|| {
                    (logical.item_ix > controls_index).then(|| {
                        self.album_details_scroll.viewport_bounds().top()
                            - logical.offset_in_item
                            - if config.table.enable_header {
                                px(40. * self.state.settings.zoom)
                            } else {
                                px(0.)
                            }
                            - rows[controls_index + 1..logical.item_ix]
                                .iter()
                                .map(|row| match row {
                                    DetailPageRow::Album(crate::details::AlbumPageRow::Disc(
                                        ..,
                                    )) => px(40. * self.state.settings.zoom),
                                    DetailPageRow::Album(crate::details::AlbumPageRow::Tracks(
                                        ..,
                                    )) => row_height,
                                    _ => px(0.),
                                })
                                .fold(px(0.), |total, height| total + height)
                    })
                });
            let groups: Vec<_> = rows
                .iter()
                .filter_map(|row| match row {
                    DetailPageRow::Album(crate::details::AlbumPageRow::Disc(disc, items)) => {
                        Some((*disc, items))
                    }
                    _ => None,
                })
                .collect();
            table_top
                .and_then(|top| {
                    crate::details::sticky_disc_index(
                        &groups
                            .iter()
                            .map(|(_, items)| items.len())
                            .collect::<Vec<_>>(),
                        row_height,
                        px(40. * self.state.settings.zoom),
                        config.table.enable_header,
                        top - self.album_details_scroll.viewport_bounds().top(),
                        sticky_disc_top,
                    )
                })
                .map(|index| (groups[index].0, groups[index].1.clone()))
        } else {
            None
        };
        let sticky_disc_top = table_end.map_or(sticky_disc_top, |end| {
            sticky_disc_top
                .min(end.top() - self.album_details_scroll.viewport_bounds().top() - row_height)
        });
        let entity = cx.entity();
        let page = list(
            self.album_details_scroll.clone(),
            move |index, window, cx| {
                entity.update(cx, |view, cx| match &rows[index] {
                    DetailPageRow::Header => view
                        .library_controls(window, cx)
                        .when(!view.has_detail_page_header(), |header| header.pb_3())
                        .when(
                            view.album_group_rows(columns).is_some()
                                && view.list_config().display != "detail",
                            |div| {
                                div.child(
                                    crate::styles::button("configure-release-groups", cx)
                                        .label(crate::appearance::text(
                                            "Release group settings",
                                            cx,
                                        ))
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.navigate(Route::AlbumGroups, cx)
                                        })),
                                )
                            },
                        )
                        .into_any_element(),
                    DetailPageRow::Album(row) => view.album_page_row(row, grid, window, cx),
                    DetailPageRow::GroupedSongs(items) => view.grouped_song_row(items.clone(), cx),
                    DetailPageRow::AlbumDetail(row) => div()
                        .v_flex()
                        .when_some(row.group.clone(), |div, group| {
                            div.child(
                                gpui::div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .h(px(28.))
                                    .child(group),
                            )
                        })
                        .child(view.album_detail_row(row.indexes[0], cx))
                        .into_any_element(),
                    DetailPageRow::Albums(row) => div()
                        .v_flex()
                        .gap_1()
                        .child(
                            div()
                                .h(px(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(row.group.clone().unwrap_or_default()),
                        )
                        .child(view.detail_item_row(&row.indexes, columns, grid, cx))
                        .into_any_element(),
                    DetailPageRow::Items(items) => view.detail_item_row(items, columns, grid, cx),
                    DetailPageRow::Empty => div()
                        .text_color(cx.theme().muted_foreground)
                        .p_8()
                        .child(if view.loading {
                            "Loading your library..."
                        } else {
                            "No items to display"
                        })
                        .into_any_element(),
                })
            },
        )
        .flex_1()
        .min_h_0();
        let rows = div()
            .id("library-list-rows")
            .debug_selector(|| "library-list-rows".into())
            .key_context("LibraryList")
            .track_focus(&self.library_focus)
            .on_key_down(cx.listener(Self::library_grid_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, window, _| {
                    if view
                        .album_details_scroll
                        .bounds_for_item(0)
                        .is_none_or(|bounds| !bounds.contains(&event.position))
                    {
                        window.focus(&view.library_focus);
                    }
                }),
            )
            .v_flex()
            .relative()
            .overflow_hidden()
            .flex_1()
            .min_h_0()
            .child(
                canvas(
                    {
                        let view = cx.entity();
                        move |bounds, _, cx| {
                            view.update(cx, |view, cx| {
                                if view.detail_header_width != bounds.size.width {
                                    view.detail_header_width = bounds.size.width;
                                    cx.notify();
                                }
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .child(page)
            .when(sticky_header, |rows| {
                rows.child(
                    div()
                        .id("detail-sticky-header")
                        .debug_selector(|| "detail-sticky-header".into())
                        .absolute()
                        .top(sticky_top)
                        .left_0()
                        .right_0()
                        .when(album, |header| {
                            header
                                .pl(if album_layout.rtl {
                                    album_layout.songs_right_padding()
                                } else {
                                    album_layout.padding()
                                })
                                .pr(if album_layout.rtl {
                                    album_layout.padding()
                                } else {
                                    album_layout.songs_right_padding()
                                })
                        })
                        .child(self.configured_header(cx)),
                )
            })
            .when_some(sticky_disc, |rows, (disc, items)| {
                rows.child(
                    div()
                        .id("album-sticky-disc")
                        .debug_selector(|| "album-sticky-disc".into())
                        .absolute()
                        .top(sticky_disc_top)
                        .left_0()
                        .right_0()
                        .child(
                            album_layout
                                .songs_frame()
                                .child(self.album_disc_row(disc, &items, row_height, true, cx)),
                        ),
                )
            })
            .child(crate::scroll::Scrollbar::vertical(
                "library-scrollbar",
                &self.album_details_scroll,
            ))
            .when(self.list_config().display == "table", |div| {
                div.child(
                    crate::scroll::Scrollbar::horizontal(
                        "library-horizontal-scrollbar",
                        &self.list_horizontal_scroll,
                    )
                    .in_viewport(&self.album_details_scroll),
                )
            });
        div()
            .id("library-drop")
            .debug_selector(|| "library-drop".into())
            .on_drop(cx.listener(|view, dragged: &crate::queue::Dragged, _, cx| {
                if matches!(&view.route, Route::Detail(item) if item.kind == ItemKind::Playlist) {
                    view.drop_playlist_tracks(dragged, None, cx);
                    cx.stop_propagation();
                }
            }))
            .v_flex()
            .relative()
            .size_full()
            .min_h_0()
            .gap_3()
            .child(if album_layout.large && album {
                self.album_sticky_metadata(
                    rows.size_full().into_any_element(),
                    2,
                    album_end.unwrap(),
                    cx,
                )
            } else {
                rows.into_any_element()
            })
            .child(self.library_footer(cx))
            .when(self.has_library_selection(), |div| {
                div.child(
                    gpui::div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(16.))
                        .h_flex()
                        .justify_center()
                        .child(self.library_selection_actions(cx)),
                )
            })
            .into_any_element()
    }

    pub(crate) fn detail_item_row(
        &self,
        items: &[usize],
        columns: usize,
        grid: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if grid {
            return div()
                .v_flex()
                .w_full()
                .when(items.first() == Some(&0), |row| {
                    row.pt(cx.theme().font_size * 0.25)
                })
                .when(
                    items
                        .last()
                        .is_some_and(|index| *index + 1 == self.items.len()),
                    |row| row.pb(cx.theme().font_size * 0.25),
                )
                .child(self.library_grid_row(items, cx))
                .into_any_element();
        }
        div()
            .h_flex()
            .w_full()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .gap_3()
            .children(items.iter().map(|index| {
                if grid {
                    self.card(&self.items[*index], *index, cx)
                } else {
                    self.row(&self.items[*index], *index, cx)
                }
            }))
            .children((items.len()..columns).map(|_| div().flex_1().min_w_0()))
            .into_any_element()
    }

    fn library(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.library_width <= px(0.) {
            self.library_width = window.viewport_size().width;
        }
        let view = cx.weak_entity();
        div()
            .relative()
            .size_full()
            .min_h_0()
            .child(self.library_content(window, cx))
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let view = view.clone();
                        window.defer(cx, move |_, cx| {
                            view.update(cx, |view, cx| {
                                if view.library_width != bounds.size.width {
                                    view.library_width = bounds.size.width;
                                    cx.notify();
                                }
                            })
                            .ok();
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    fn library_content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Song) {
            return self.missing_album_view(window, cx);
        }
        let config = self.list_config();
        let is_grid = config.display == "poster";
        let available_width = f32::from(window.viewport_size().width)
            - if self.is_full_player_view() {
                40.
            } else {
                self.sidebar_pixel_width() + 40.
            };
        let grid_layout = self.library_grid_layout(cx);
        let grid_count = grid_layout.columns;
        if self.scrolling_detail_header() && !self.artist_overview_active() {
            return self.detail_page_view(if is_grid { grid_count } else { 1 }, is_grid, cx);
        }
        let controls = self.library_controls(window, cx);
        if self.artist_overview_active() {
            return self.artist_overview_view(controls.into_any_element(), window, cx);
        }
        let mut container = gpui::div()
            .id("library-list-rows")
            .debug_selector(|| "library-list-rows".into())
            .key_context("LibraryList")
            .track_focus(&self.library_focus)
            .on_key_down(cx.listener(Self::library_grid_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, _| window.focus(&view.library_focus)),
            )
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_3();
        let entity = cx.entity();
        let album_detail =
            matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Album);
        if album_detail {
            let rows = self.album_page_rows(if is_grid { grid_count } else { 1 });
            let count = self.album_details_scroll.item_count();
            if count < rows.len() {
                self.album_details_scroll
                    .splice(count..count, rows.len() - count);
            } else if count != rows.len() {
                self.album_details_scroll.reset(rows.len());
            }
            container = container.child(
                list(
                    self.album_details_scroll.clone(),
                    move |index, window, cx| {
                        entity.update(cx, |this, cx| {
                            this.album_page_row(&rows[index], is_grid, window, cx)
                        })
                    },
                )
                .flex_1()
                .min_h_0(),
            );
        } else if self.grouped_song_albums() {
            let groups = feishin_core::album_metadata::groups(&self.items);
            let count = self.album_details_scroll.item_count();
            if count != groups.len() {
                self.album_details_scroll.reset(groups.len());
            }
            container = container.child(
                list(self.album_details_scroll.clone(), move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.grouped_song_row(groups[index].clone(), cx)
                    })
                })
                .flex_1()
                .min_h_0(),
            );
        } else if config.display == "detail"
            && self.items.iter().all(|item| item.kind == ItemKind::Album)
        {
            let rows = self.album_group_rows(1).unwrap_or_else(|| {
                (0..self.items.len())
                    .map(|index| crate::album_groups::AlbumRow {
                        group: None,
                        indexes: vec![index],
                    })
                    .collect()
            });
            if self.album_details_scroll.item_count() < rows.len() {
                let count = self.album_details_scroll.item_count();
                self.album_details_scroll
                    .splice(count..count, rows.len() - count);
            } else if self.album_details_scroll.item_count() != rows.len() {
                self.album_details_scroll.reset(rows.len());
            }
            container = container.child(
                list(self.album_details_scroll.clone(), move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        let row = &rows[index];
                        div()
                            .v_flex()
                            .when_some(row.group.clone(), |div, group| {
                                div.child(
                                    gpui::div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .h(px(28.))
                                        .child(group),
                                )
                            })
                            .child(this.album_detail_row(row.indexes[0], cx))
                            .into_any_element()
                    })
                })
                .flex_1()
                .min_h_0(),
            );
        } else if let Some(rows) = self.album_group_rows(if is_grid { grid_count } else { 1 }) {
            let height = if is_grid {
                f32::from(grid_layout.height) + 28.
            } else {
                (config.table.size.row_height() + 28.) * self.state.settings.zoom
            };
            container = container
                .child(
                    crate::styles::button("configure-release-groups", cx)
                        .label(crate::appearance::text("Release group settings", cx))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.navigate(Route::AlbumGroups, cx)),
                        ),
                )
                .child(
                    uniform_list("artist-release-groups", rows.len(), move |range, _, cx| {
                        entity.update(cx, |this, cx| {
                            range
                                .map(|index| {
                                    let row = &rows[index];
                                    div()
                                        .v_flex()
                                        .h(px(height))
                                        .gap_1()
                                        .child(
                                            div()
                                                .h(px(24.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(row.group.clone().unwrap_or_default()),
                                        )
                                        .child(this.detail_item_row(
                                            &row.indexes,
                                            grid_count,
                                            is_grid,
                                            cx,
                                        ))
                                })
                                .collect()
                        })
                    })
                    .track_scroll(self.library_scroll.clone())
                    .flex_1()
                    .min_h_0(),
                );
        } else if is_grid {
            container = container.child(
                uniform_list(
                    "library-grid",
                    self.items.len().div_ceil(grid_count),
                    move |range, _, cx| {
                        entity.update(cx, |this, cx| {
                            range
                                .map(|row| {
                                    this.library_grid_row(
                                        &(row * grid_count
                                            ..((row + 1) * grid_count).min(this.items.len()))
                                            .collect::<Vec<_>>(),
                                        cx,
                                    )
                                })
                                .collect()
                        })
                    },
                )
                .py(cx.theme().font_size * 0.25)
                .track_scroll(self.library_scroll.clone())
                .flex_1()
                .min_h_0(),
            );
        } else {
            container = container.child(
                uniform_list("library-table", self.items.len(), move |range, _, cx| {
                    entity.update(cx, |this, cx| {
                        range.map(|i| this.row(&this.items[i], i, cx)).collect()
                    })
                })
                .track_scroll(self.library_scroll.clone())
                .flex_1()
                .min_h_0(),
            );
        }
        container = container.relative().child(
            if album_detail
                || self.grouped_song_albums()
                || (config.display == "detail"
                    && self.items.iter().all(|item| item.kind == ItemKind::Album))
            {
                crate::scroll::Scrollbar::vertical("library-scrollbar", &self.album_details_scroll)
            } else {
                crate::scroll::Scrollbar::vertical("library-scrollbar", &self.library_scroll)
            },
        );
        if config.display == "table" {
            let scrollbar = crate::scroll::Scrollbar::horizontal(
                "library-horizontal-scrollbar",
                &self.list_horizontal_scroll,
            );
            container = container.child(if album_detail || self.grouped_song_albums() {
                scrollbar.in_viewport(&self.album_details_scroll)
            } else {
                scrollbar.in_viewport(&self.library_scroll)
            });
        }
        if self.items.is_empty() {
            container = container.child(
                gpui::div()
                    .text_color(cx.theme().muted_foreground)
                    .p_8()
                    .child(if self.loading {
                        "Loading your library..."
                    } else {
                        "No items to display"
                    }),
            );
        }
        let content = container
            .child(self.library_footer(cx))
            .when(self.has_library_selection(), |div| {
                div.child(
                    gpui::div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(16.))
                        .h_flex()
                        .justify_center()
                        .child(self.library_selection_actions(cx)),
                )
            })
            .into_any_element();
        let content = div()
            .id("library-drop")
            .debug_selector(|| "library-drop".into())
            .on_drop(cx.listener(|this, dragged: &crate::queue::Dragged, _, cx| {
                if matches!(&this.route, Route::Detail(item) if item.kind == ItemKind::Playlist) {
                    this.drop_playlist_tracks(dragged, None, cx);
                    cx.stop_propagation();
                }
            }))
            .v_flex()
            .size_full()
            .min_h_0()
            .gap_3()
            .when(
                self.has_standard_list_header() || self.has_detail_page_header(),
                |div| div.gap_0(),
            )
            .child(controls)
            .child(content)
            .into_any_element();
        if self.folder_route() {
            self.folder_browser_view(content, available_width, cx)
        } else {
            content
        }
    }

    fn servers(&self, cx: &mut Context<Self>) -> AnyElement {
        gpui::div()
            .id("servers-page")
            .v_flex()
            .gap_4()
            .native_vertical_scroll()
            .size_full()
            .children(self.state.servers.iter().map(|server| {
                let id = server.id;
                gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().secondary)
                    .child(
                        gpui::div()
                            .v_flex()
                            .flex_1()
                            .child(server.name.clone())
                            .child(
                                gpui::div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("{} - {}", server.kind.label(), server.url)),
                            ),
                    )
                    .child(
                        crate::styles::button(SharedString::from(format!("server-{id}")), cx)
                            .label(if self.sessions.contains_key(&id) {
                                "Connect"
                            } else {
                                "Sign in"
                            })
                            .disabled(self.signing_out.contains(&id))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.sessions.contains_key(&id) {
                                    this.choose_server(id, cx);
                                } else {
                                    this.edit_server(Some(id), window, cx);
                                }
                            })),
                    )
                    .child(
                        crate::styles::button(SharedString::from(format!("edit-server-{id}")), cx)
                            .label(crate::appearance::text("Edit", cx))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_server(Some(id), window, cx)
                            })),
                    )
                    .when(self.sessions.contains_key(&id), |div| {
                        div.child(
                            crate::styles::button(
                                SharedString::from(format!("sign-out-server-{id}")),
                                cx,
                            )
                            .label(self.tr("page.appMenu.logout"))
                            .on_click(cx.listener(
                                move |this, _, window, cx| this.sign_out_server(id, window, cx),
                            )),
                        )
                    })
                    .child(
                        crate::styles::button(
                            SharedString::from(format!("remove-server-{id}")),
                            cx,
                        )
                        .label(self.tr("Remove"))
                        .on_click(cx.listener(
                            move |this, _, window, cx| this.remove_server(id, window, cx),
                        )),
                    )
            }))
            .child(
                gpui::div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mt_4()
                    .child(self.tr("Connect to a server")),
            )
            .child(self.server_discovery_controls(cx))
            .child(
                gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .children(ServerType::ALL.into_iter().enumerate().map(|(i, kind)| {
                        crate::styles::button(("server-type", i), cx)
                            .label(kind.label())
                            .selected(self.server_kind == kind)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.cancel_quick_connect();
                                this.quick_connect.selected = false;
                                this.server_kind = kind;
                                cx.notify();
                            }))
                    })),
            )
            .child(
                gpui::div()
                    .max_w(px(520.0))
                    .v_flex()
                    .gap_3()
                    .child(self.server_input(
                        "form.addServer.input_name",
                        crate::styles::input(&self.server_name, cx),
                    ))
                    .child(self.server_input(
                        "form.addServer.input_url",
                        crate::styles::input(&self.server_url, cx),
                    ))
                    .when(self.server_kind == ServerType::Jellyfin, |div| {
                        div.child(self.jellyfin_sign_in_method(cx))
                    })
                    .when(
                        self.server_kind != ServerType::Jellyfin || !self.quick_connect.selected,
                        |div| {
                            div.child(self.server_input(
                                "form.addServer.input_username",
                                crate::styles::input(&self.username, cx),
                            ))
                            .child(self.server_input(
                                "form.addServer.input_password",
                                crate::styles::input(&self.password, cx).mask_toggle(),
                            ))
                            .when(
                                self.server_kind == ServerType::Navidrome,
                                |div| {
                                    div.child(
                                        gpui::div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.tr("form.addServer.input_passwordNoSSO")),
                                    )
                                },
                            )
                        },
                    )
                    .child(self.server_options_controls(cx))
                    .child(self.server_tls_control(cx))
                    .child(
                        crate::styles::button("new-server", cx)
                            .label(crate::appearance::text("New server", cx))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit_server(None, window, cx)
                            })),
                    )
                    .when(self.server_kind == ServerType::Subsonic, |div| {
                        div.child(
                            Checkbox::new("legacy")
                                .debug_selector(|| "server-legacy".into())
                                .label(self.tr("form.addServer.input_legacyAuthentication"))
                                .when(crate::appearance::rtl(cx), |checkbox| {
                                    checkbox.flex_row_reverse()
                                })
                                .checked(self.legacy_auth)
                                .on_click(cx.listener(|this, value: &bool, _, cx| {
                                    this.cancel_quick_connect();
                                    this.legacy_auth = *value;
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        crate::styles::button("remember", cx)
                            .ghost()
                            .label(if self.remember {
                                "✓ Remember credentials in OS password store"
                            } else {
                                "Remember credentials"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel_quick_connect();
                                this.remember = !this.remember;
                                cx.notify();
                            })),
                    )
                    .when(
                        self.server_kind == ServerType::Jellyfin && self.quick_connect.selected,
                        |div| div.child(self.quick_connect_controls(cx)),
                    )
                    .when(
                        self.server_kind != ServerType::Jellyfin || !self.quick_connect.selected,
                        |div| {
                            div.child(
                                crate::styles::button("login", cx)
                                    .primary()
                                    .label(if self.loading {
                                        "Connecting..."
                                    } else {
                                        "Sign in"
                                    })
                                    .disabled(self.loading)
                                    .on_click(
                                        cx.listener(|this, _, window, cx| this.login(window, cx)),
                                    ),
                            )
                        },
                    ),
            )
            .into_any_element()
    }

    fn main_queue_row(
        &mut self,
        entry: &feishin_core::queue::Entry,
        i: usize,
        config: &feishin_core::lists::Config,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let this = self;
        let key = entry.key;
        let item = entry.item.clone();
        this.fetch_item_cover(&item, cx);
        gpui::div()
            .id(("queue-row", i))
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                this.hover_table_row(
                    crate::lists::TableRow::Queue {
                        key,
                        sidebar: false,
                    },
                    i,
                    *hovered,
                    cx,
                )
            }))
            .debug_selector(move || format!("queue-drag-row-{i}"))
            .relative()
            .on_drag(
                this.drag_queue_entry(key, false),
                crate::context_preview::drag_constructor(cx),
            )
            .on_drop_with_bounds(crate::queue::row_drop_listener(
                cx,
                move |this, dragged, after, _, cx| {
                    this.drop_queue_at(dragged, Some(key), after, cx);
                    cx.stop_propagation();
                },
            ))
            .child(
                crate::styles::button(("select-queue", i), cx)
                    .w(px(32. * this.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .ghost()
                    .label(if this.queue_selected.contains(&key) {
                        "✓"
                    } else {
                        "□"
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_queue_row(
                            key,
                            false,
                            Modifiers {
                                control: true,
                                ..Modifiers::none()
                            },
                            cx,
                        );
                    })),
            )
            .h_flex()
            .w_full()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .h(px(config.table.size.row_height() * this.state.settings.zoom))
            .gap_3()
            .px_3()
            .rounded_md()
            .when(config.table.enable_horizontal_borders, |div| {
                div.border_b_1().border_color(cx.theme().border)
            })
            .bg(crate::styles::row_background(
                config.table.enable_alternate_row_colors && i % 2 == 1,
                this.queue_selected.contains(&key),
                false,
                cx,
            ))
            .when(config.table.enable_row_hover_highlight, |div| {
                div.hover(|style| {
                    style.bg(crate::styles::row_background(
                        config.table.enable_alternate_row_colors && i % 2 == 1,
                        this.queue_selected.contains(&key),
                        true,
                        cx,
                    ))
                })
            })
            .child(this.metadata_cells(
                &item,
                i,
                config,
                this.configured_columns(),
                crate::lists::TableRow::Queue {
                    key,
                    sidebar: false,
                },
                cx,
            ))
            .child(
                crate::styles::button(("queue-play", i), cx)
                    .w(px(32. * this.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .ghost()
                    .label(this.tr("▶"))
                    .tooltip(crate::appearance::text("Play track", cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.play_request_generation += 1;
                        this.state.queue.current = Some(key);
                        this.state.queue.position = 0.0;
                        this.play_current(cx);
                    })),
            )
            .child(
                crate::styles::button(("queue-up", i), cx)
                    .w(px(32. * this.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .ghost()
                    .icon(IconName::ArrowUp)
                    .tooltip(crate::appearance::text("Move up", cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.state.queue.move_entry(
                            key,
                            this.state
                                .queue
                                .order
                                .iter()
                                .position(|entry| *entry == key)
                                .unwrap_or(i)
                                .saturating_sub(1),
                        );
                        this.arm_next();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                crate::styles::button(("queue-remove", i), cx)
                    .w(px(32. * this.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .ghost()
                    .icon(IconName::Close)
                    .tooltip(crate::appearance::text("Remove track", cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_queue_entries(&[key], cx);
                    })),
            )
            .child(
                crate::styles::button(("queue-more", i), cx)
                    .ghost()
                    .w(px(32. * this.state.settings.zoom))
                    .px_0()
                    .flex_shrink_0()
                    .debug_selector(move || format!("queue-more-{i}"))
                    .label("⋯")
                    .tooltip(crate::appearance::text("More", cx))
                    .dropdown_menu(this.queue_menu_builder(entry, false, cx)),
            )
            .context_menu(this.queue_menu_builder(entry, false, cx))
            .into_any_element()
    }

    pub fn queue_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = self.list_config();
        let all_entries = self.queue_rows();
        let total_entries = all_entries.len();
        let entries = if !self.is_full_player_view() && config.pagination == "paginated" {
            all_entries
                .into_iter()
                .skip(self.library_page * config.items_per_page)
                .take(config.items_per_page)
                .collect::<Vec<_>>()
        } else {
            all_entries
        };
        let count = entries.len();
        let entity = cx.entity();
        let grouped = crate::queue::album_grouping(&config) || self.queue_has_sections(false);
        let rows = if grouped {
            let display =
                self.queue_display_rows(&entries.iter().collect::<Vec<_>>(), &config, false);
            if self.queue_group_scroll.item_count() != display.len() {
                self.queue_group_scroll.reset(display.len());
            }
            list(self.queue_group_scroll.clone(), move |row, _, cx| {
                entity.update(cx, |this, cx| match &display[row] {
                    crate::queue::GroupedQueueRow::Section { up_next, count } => {
                        this.queue_section_header(*up_next, *count, row, false, cx)
                    }
                    crate::queue::GroupedQueueRow::Album(range) => {
                        let songs = entries[range.clone()]
                            .iter()
                            .map(|entry| entry.item.clone())
                            .collect::<Vec<_>>();
                        this.queue_album_header(
                            &songs,
                            entries[range.start].key,
                            range.start,
                            false,
                            cx,
                        )
                    }
                    crate::queue::GroupedQueueRow::Track(index) => {
                        this.main_queue_row(&entries[*index], *index, &config, cx)
                    }
                })
            })
            .flex_1()
            .min_h_0()
            .into_any_element()
        } else {
            uniform_list("queue", count, move |range, _, cx| {
                entity.update(cx, |this, cx| {
                    range
                        .map(|i| this.main_queue_row(&entries[i], i, &config, cx))
                        .collect()
                })
            })
            .track_scroll(self.queue_scroll.clone())
            .flex_1()
            .min_h_0()
            .into_any_element()
        };
        let rows = gpui::div()
            .id("main-queue-list-rows")
            .key_context("MainQueueList")
            .track_focus(&self.queue_focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, window, _| window.focus(&view.queue_focus)),
            )
            .relative()
            .v_flex()
            .flex_1()
            .min_h_0()
            .child(rows)
            .child(if grouped {
                crate::scroll::Scrollbar::vertical("queue-scrollbar", &self.queue_group_scroll)
            } else {
                crate::scroll::Scrollbar::vertical("queue-scrollbar", &self.queue_scroll)
            })
            .child(if grouped {
                crate::scroll::Scrollbar::horizontal(
                    "queue-horizontal-scrollbar",
                    &self.list_horizontal_scroll,
                )
                .in_viewport(&self.queue_group_scroll)
            } else {
                crate::scroll::Scrollbar::horizontal(
                    "queue-horizontal-scrollbar",
                    &self.list_horizontal_scroll,
                )
                .in_viewport(&self.queue_scroll)
            });
        let config = self.list_config();
        gpui::div()
            .id("queue-drop")
            .on_drop(cx.listener(|this, dragged: &crate::queue::Dragged, _, cx| {
                this.drop_queue(dragged, None, cx);
                cx.stop_propagation();
            }))
            .v_flex()
            .size_full()
            .gap_3()
            .when(!self.is_full_player_view(), |div| {
                div.child(self.queue_toolbar(false, cx))
            })
            .when(
                !self.is_full_player_view() && !self.queue_selected.is_empty(),
                |div| {
                    div.child(
                        gpui::div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(
                                crate::styles::button("remove-queue-selection", cx)
                                    .label(crate::appearance::text("Remove selected", cx))
                                    .disabled(self.queue_selected.is_empty())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.remove_queue_selection(false, cx)
                                    })),
                            )
                            .child(
                                crate::styles::button("move-queue-selection", cx)
                                    .label(crate::appearance::text("Move selected", cx))
                                    .disabled(self.queue_selected.is_empty())
                                    .dropdown_menu({
                                        let view = cx.entity();
                                        let first = self.state.queue.order.first().copied();
                                        let next =
                                            self.state.queue.up_next.first().copied().or_else(
                                                || {
                                                    self.state
                                                        .queue
                                                        .current
                                                        .and_then(|key| {
                                                            self.state
                                                                .queue
                                                                .order
                                                                .iter()
                                                                .position(|entry| *entry == key)
                                                        })
                                                        .and_then(|index| {
                                                            self.state.queue.order.get(index + 1)
                                                        })
                                                        .copied()
                                                },
                                            );
                                        move |mut menu, _, _| {
                                            for (label, before) in [
                                                ("To top", first),
                                                ("Play next", next),
                                                ("To end", None),
                                            ] {
                                                let view = view.clone();
                                                menu =
                                                    menu.item(PopupMenuItem::new(label).on_click(
                                                        move |_, _, cx| {
                                                            view.update(cx, |this, cx| {
                                                                this.move_queue_selection(
                                                                    before, cx,
                                                                )
                                                            })
                                                        },
                                                    ));
                                            }
                                            menu
                                        }
                                    }),
                            ),
                    )
                },
            )
            .when(
                !self.is_full_player_view() && config.pagination == "paginated",
                |div| {
                    div.child(
                        gpui::div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_2()
                            .child(
                                crate::styles::button("queue-prev-page", cx)
                                    .label(crate::appearance::text("Previous page", cx))
                                    .disabled(self.library_page == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.change_list_page(
                                            this.library_page.saturating_sub(1),
                                            cx,
                                        )
                                    })),
                            )
                            .child(format!("Page {}", self.library_page + 1))
                            .child(
                                crate::styles::button("queue-next-page", cx)
                                    .label(crate::appearance::text("Next page", cx))
                                    .disabled(
                                        (self.library_page + 1) * config.items_per_page
                                            >= total_entries,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.change_list_page(this.library_page + 1, cx)
                                    })),
                            ),
                    )
                },
            )
            .when(config.table.enable_header, |div| {
                div.child(self.configured_header(cx))
            })
            .child(rows)
            .into_any_element()
    }

    pub fn lyrics_surface(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.lyrics_surface_for("default", window, cx)
    }
    pub fn lyrics_surface_for(
        &self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar = key == "sidebar";
        let position = self.lyric_position();
        let synchronized = self.lyrics.iter().any(|line| line.time.is_some());
        let karaoke = self.state.settings.lyric_words
            && self.lyrics.iter().any(|line| {
                !line.words.is_empty() || line.cue_lines.iter().any(|cue| !cue.words.is_empty())
            });
        let display = self.state.settings.lyric_display_for(key);
        let zoom = self.state.settings.zoom;
        let rem = f32::from(cx.theme().font_size);
        let portrait = window.viewport_size().height > window.viewport_size().width;
        let viewport_height = f32::from(window.viewport_size().height);
        let top = viewport_height * if portrait { 0.05 } else { 0.1 }
            + if sidebar && synchronized {
                rem * 2.
            } else {
                0.
            };
        let bottom = viewport_height
            * if portrait {
                0.05
            } else if synchronized {
                0.5
            } else {
                0.06
            }
            + if sidebar && synchronized {
                rem * 4.
            } else {
                0.
            };
        let font_size = if synchronized {
            display.font_size
        } else {
            display.font_size_unsync
        };
        let scroll = if sidebar {
            &self.sidebar_lyric_scroll
        } else {
            &self.lyric_scroll
        };
        let information = self.lyric_information();
        let signature = crate::lyrics_row::gutter_signature(self, key, &information, window, cx);
        let (has_gutter, gutter_state) =
            crate::lyrics_row::gutter_state(signature, sidebar, window, cx);
        let content =
            gpui::div()
                .id(if sidebar { "lyrics-sidebar" } else { "lyrics" })
                .min_w_0()
                .debug_selector(move || {
                    if sidebar {
                        "lyrics-sidebar".into()
                    } else {
                        "lyrics-player".into()
                    }
                })
                .track_scroll(if sidebar {
                    &self.sidebar_lyric_scroll
                } else {
                    &self.lyric_scroll
                })
                .v_flex()
                .gap_0()
                .flex_1()
                .h_full()
                .overflow_y_scroll()
                .scrollbar_width(px(if has_gutter { 12. * zoom } else { 0. }))
                .vertical_alpha_mask(0.15)
                .on_scroll_reveal({
                    let listener = cx
                        .listener(move |this, _: &(), _, cx| this.pause_lyric_follow(sidebar, cx));
                    move |window, cx| listener(&(), window, cx)
                })
                .on_scroll_wheel(cx.listener(move |this, _, _, cx| {
                    this.pause_lyric_follow(sidebar, cx);
                }))
                .relative()
                .when(self.lyrics.is_empty(), |div| {
                    div.child(self.tr("No lyrics available"))
                })
                .child(gpui::div().h(px(top)).flex_shrink_0())
                .children(information.into_iter().enumerate().map(|(i, text)| {
                    crate::lyrics_row::Row {
                        index: self.lyrics.len() + i,
                        information: true,
                        sidebar,
                        synchronized,
                        karaoke,
                        bright: !synchronized,
                        font_size: font_size * zoom,
                        rem,
                        gap_after: (if synchronized {
                            display.gap
                        } else {
                            display.gap_unsync
                        }) * zoom,
                        padding_left: display.padding_left,
                        padding_right: display.padding_right,
                        opacity: display.opacity_non_active,
                        scale: display.scale_non_active,
                        alignment: self.state.settings.lyric_alignment.clone(),
                        content: crate::lyrics_markup::Text(text).into_any_element(),
                        click: None,
                    }
                }))
                .children(self.lyrics.iter().enumerate().map(|(i, line)| {
                    let time = line.time;
                    let bright = !synchronized
                        || crate::lyrics_row::is_bright(
                            &self.lyrics,
                            i,
                            position,
                            self.playing,
                            karaoke,
                            self.state.settings.lyric_lead_time,
                        );
                    crate::lyrics_row::Row {
                        index: i,
                        information: false,
                        sidebar,
                        synchronized,
                        karaoke,
                        bright,
                        font_size: font_size * zoom,
                        rem,
                        padding_left: display.padding_left,
                        padding_right: display.padding_right,
                        gap_after: if i + 1 < self.lyrics.len() {
                            (if synchronized {
                                display.gap
                            } else {
                                display.gap_unsync
                            }) * zoom
                        } else {
                            0.
                        },
                        opacity: display.opacity_non_active,
                        scale: display.scale_non_active,
                        alignment: self.state.settings.lyric_alignment.clone(),
                        content: self.lyric_line(line, i, key, cx),
                        click: Some(std::rc::Rc::new(cx.listener(move |this, _, _, cx| {
                            if let Some(time) = time {
                                this.seek_lyric(time, cx);
                            }
                        }))),
                    }
                }))
                .child(gpui::div().h(px(bottom)).flex_shrink_0())
                .child(crate::lyrics_row::gutter_monitor(scroll, gutter_state))
                .child(crate::scroll::Scrollbar::vertical(
                    if sidebar {
                        "sidebar-lyrics-scrollbar"
                    } else {
                        "lyrics-scrollbar"
                    },
                    if sidebar {
                        &self.sidebar_lyric_scroll
                    } else {
                        &self.lyric_scroll
                    },
                ))
                .into_any_element();
        self.lyrics_frame(content, sidebar, window, cx)
    }

    fn now_playing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let entry = match self.current_entry().cloned() {
            Some(entry) => entry,
            None if self.is_full_player_view() => feishin_core::queue::Entry {
                key: uuid::Uuid::nil(),
                item: Item::default(),
            },
            None => {
                return gpui::div()
                    .p_8()
                    .child(self.tr("Play a song to see its lyrics"))
                    .into_any_element();
            }
        };
        let metadata = self
            .current_metadata()
            .unwrap_or_else(|| entry.item.clone());
        let tab = self.state.settings.player_display.active_tab.clone();
        let show_panel = !self.is_full_player_view()
            || matches!(tab.as_str(), "queue" | "lyrics" | "related")
            || tab == "visualizer" && self.state.settings.mpv_path.is_empty();
        let (tab, panel_opacity) = if self.is_full_player_view() {
            let target = if show_panel { tab.as_str() } else { "" };
            let motion = window.use_keyed_state("expanded-player-panel-motion", cx, |_, _| {
                crate::player_layout::PanelMotion::new(target)
            });
            let (tab, opacity, active) = motion.update(cx, |motion, cx| {
                motion.frame(target, cx.background_executor().now())
            });
            if active {
                window.request_animation_frame();
            }
            (tab, opacity)
        } else {
            (tab, 1.)
        };
        let image = if self.is_full_player_view() {
            self.full_player_image(&entry, &metadata, window, cx)
        } else {
            gpui::div()
                .id("player-sidebar")
                .v_flex()
                .w(px(
                    self.state.settings.player_display.cover_art_size * 4. + 20.
                ))
                .h_full()
                .overflow_y_scroll()
                .gap_4()
                .child(self.player_artwork(&entry.item, cx))
                .child(self.player_metadata(&metadata, cx))
                .when(!self.is_full_player_view() || tab == "lyrics", |div| {
                    div.child(self.lyric_controls(cx))
                })
                .child(
                    crate::styles::button("now-similar", cx)
                        .label(crate::appearance::text("Similar songs", cx))
                        .disabled(entry.item.kind != ItemKind::Song)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(item) = this.current_entry().map(|entry| entry.item.clone())
                            {
                                this.open_similar(item, cx);
                            }
                        })),
                )
                .when(self.state.settings.show_favorites, |div| {
                    div.child(
                        crate::styles::button("now-favorite", cx)
                            .disabled(entry.item.kind != ItemKind::Song)
                            .label(format!(
                                "{} {}",
                                if entry.item.favorite { "♥" } else { "♡" },
                                self.tr(if entry.item.favorite {
                                    "filter.favorited"
                                } else {
                                    "common.favorite"
                                })
                            ))
                            .aria_label(self.tr(if entry.item.favorite {
                                "Unfavorite"
                            } else {
                                "Favorite"
                            }))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(item) = this.current_entry().map(|e| e.item.clone()) {
                                    this.toggle_favorite(item, cx);
                                }
                            })),
                    )
                })
                .into_any_element()
        };
        let panel = if !self.is_full_player_view() || !tab.is_empty() {
            gpui::div()
                .relative()
                .left(relative((1. - panel_opacity) * 0.1))
                .opacity(panel_opacity)
                .map(|div| {
                    if self.is_full_player_view() {
                        div.w_full()
                    } else {
                        div.flex_1()
                    }
                })
                .min_w_0()
                .h_full()
                .when(
                    self.is_full_player_view() && matches!(tab.as_str(), "queue" | "related"),
                    |div| {
                        if self.list_config().table.enable_header {
                            div.vertical_bottom_alpha_mask(0.05)
                        } else {
                            div.vertical_alpha_mask(0.05)
                        }
                    },
                )
                .id("full-player-active-panel")
                .role(gpui::Role::Group)
                .aria_label(self.tr(if self.is_full_player_view() {
                    match tab.as_str() {
                        "queue" => "page.fullscreenPlayer.upNext",
                        "visualizer" => "page.fullscreenPlayer.visualizer",
                        "related" => "page.fullscreenPlayer.related",
                        _ => "page.fullscreenPlayer.lyrics",
                    }
                } else {
                    "page.fullscreenPlayer.lyrics"
                }))
                .child(if self.is_full_player_view() && tab == "queue" {
                    self.queue_view(cx)
                } else if self.is_full_player_view() && tab == "visualizer" {
                    self.visualizer_surface(true, true, window, cx)
                } else if self.is_full_player_view() && tab == "related" {
                    self.player_related_view(cx)
                } else {
                    self.lyrics_surface(window, cx)
                })
                .into_any_element()
        } else {
            Empty.into_any_element()
        };
        if self.is_full_player_view() {
            let viewport = window.viewport_size();
            return crate::player_layout::PlayerLayout {
                open: show_panel,
                portrait: viewport.height >= viewport.width,
                font: cx.theme().font_size,
                zoom: self.state.settings.zoom,
                image,
                panel,
            }
            .into_any_element();
        }
        gpui::div()
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .size_full()
            .gap_8()
            .child(image)
            .child(panel)
            .into_any_element()
    }

    fn settings_import_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(preview) = &self.settings_import else {
            return gpui::div()
                .v_flex()
                .gap_3()
                .child(self.tr("setting.exportImportSettings_importModalTitle"))
                .child(
                    gpui::div()
                        .id("settings-import-drop")
                        .debug_selector(|| "settings-import-drop".into())
                        .v_flex()
                        .items_center()
                        .gap_3()
                        .p_3()
                        .border_2()
                        .border_dashed()
                        .border_color(if self.settings_import_error.is_empty() {
                            cx.theme().border
                        } else {
                            cx.theme().danger
                        })
                        .rounded_md()
                        .on_click(cx.listener(|this, _, window, cx| {
                            if !this.settings_import_loading {
                                this.choose_settings_import(window, cx);
                            }
                        }))
                        .child(
                            gpui_component::Icon::default()
                                .path("native-icons/file-json.svg")
                                .size(rems(2.)),
                        )
                        .child(
                            crate::styles::button("settings-import-choose", cx)
                                .debug_selector(|| "settings-import-choose".into())
                                .label(self.tr("dragDropZone.mainText"))
                                .loading(self.settings_import_loading)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.choose_settings_import(window, cx);
                                })),
                        )
                        .when(!self.settings_import_error.is_empty(), |div| {
                            div.child(
                                gpui::div()
                                    .text_color(cx.theme().danger)
                                    .child(self.settings_import_error.clone()),
                            )
                        })
                        .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                            this.read_settings_import(paths.paths(), cx);
                            cx.stop_propagation();
                        })),
                )
                .child(
                    crate::styles::button("cancel-settings-import", cx)
                        .debug_selector(|| "cancel-settings-import".into())
                        .label(self.tr("common.cancel"))
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_settings_import(cx))),
                )
                .into_any_element();
        };
        gpui::div()
            .id("settings-import-page")
            .v_flex()
            .gap_3()
            .size_full()
            .overflow_y_scroll()
            .child(format!(
                "{} preferences will change. Applying creates a backup of the current state.",
                preview.changes.len()
            ))
            .when(!preview.data.summary().is_empty(), |div| div.child(preview.data.summary()))
            .when(!self.save_enabled, |div| div.child(crate::appearance::text("The current state file is invalid. Applying replaces it after backing up the original file.", cx)))
            .children(preview.changes.iter().map(|(key, before, after)| {
                gpui::div()
                    .v_flex()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().secondary)
                    .child(
                        gpui::div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(key.clone()),
                    )
                    .child(gpui::div().text_sm().child(format!("Current: {before}")))
                    .child(gpui::div().text_sm().child(format!("Imported: {after}")))
            }))
            .when(!preview.omitted.is_empty(), |div| {
                div.child(crate::appearance::text("These fields are not imported:", cx)).children(
                    preview
                        .omitted
                        .iter()
                        .map(|key| gpui::div().text_sm().child(key.clone())),
                )
            })
            .child(
                crate::styles::button("apply-settings-import", cx)
                    .debug_selector(|| "apply-settings-import".into())
                    .primary()
                    .label(crate::appearance::text("Apply imported preferences", cx))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.apply_settings_import(window, cx)),
                    ),
            )
            .child(
                crate::styles::button("cancel-settings-import", cx)
                    .debug_selector(|| "cancel-settings-import".into())
                    .label(crate::appearance::text("Cancel", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_settings_import(cx))),
            )
            .into_any_element()
    }

    pub(crate) fn audio_presets(&self, compressor: bool, cx: &mut Context<Self>) -> AnyElement {
        let custom = if compressor {
            self.state
                .settings
                .compressor_presets
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        } else {
            self.state
                .settings
                .equalizer_presets
                .keys()
                .cloned()
                .collect::<Vec<_>>()
        };
        let input = if compressor {
            &self.compressor_preset_name
        } else {
            &self.preset_name
        };
        let kind = if compressor { "comp" } else { "eq" };
        let delete_view = cx.entity().downgrade();
        let saved = custom.clone();
        div()
            .v_flex()
            .gap_3()
            .child(self.tr("setting.equalizerPreset"))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                    .gap_3()
                    .child(
                        div()
                            .w(px(180.))
                            .h(rems(2.25))
                            .debug_selector(move || format!("{kind}-presets"))
                            .child(
                                crate::styles::select(
                                    &self.audio_preset_selects[usize::from(compressor)],
                                )
                                .placeholder(self.tr("setting.equalizerPresetSelectPlaceholder"))
                                .search_placeholder(self.tr("common.search"))
                                .cleanable(true),
                            ),
                    )
                    .when(!saved.is_empty(), |row| {
                        row.child(
                            crate::styles::button(
                                SharedString::from(format!("{kind}-delete-preset")),
                                cx,
                            )
                            .debug_selector(move || format!("{kind}-delete-preset"))
                            .label(self.tr("setting.equalizerPresetDeletePlaceholder"))
                            .dropdown_menu(move |mut menu, _, _| {
                                for name in &saved {
                                    let view = delete_view.clone();
                                    let name = name.clone();
                                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                                        move |_, window, cx| {
                                            view.update(cx, |this, cx| {
                                                this.delete_audio_preset(
                                                    compressor, &name, window, cx,
                                                )
                                            })
                                            .ok();
                                        },
                                    ));
                                }
                                menu
                            }),
                        )
                    }),
            )
            .child(self.tr("setting.equalizerSavePreset"))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                    .gap_3()
                    .child(div().w(px(180.)).child(crate::styles::input(input, cx)))
                    .child(
                        crate::styles::button(
                            SharedString::from(format!("save-{kind}-preset")),
                            cx,
                        )
                        .debug_selector(move || format!("save-{kind}-preset"))
                        .label(self.tr("common.save"))
                        .disabled(input.read(cx).value().trim().is_empty())
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.save_audio_preset(compressor, window, cx)
                            },
                        )),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.settings_search.read(cx).value().to_string();
        let content = crate::settings::List::new(
            &self.state.settings.language,
            &query,
            self.state.settings.settings_tab,
        );
        // Keep each settings group's builder temporaries off the parent render stack.
        let content = self.settings_general_controls(content, &query, cx);
        let content = self.settings_playback_controls(content, &query, cx);
        let content = self.settings_library_controls(content, cx).render(cx);
        gpui::div()
            .id("settings-page")
            .debug_selector(|| "settings-page".into())
            .v_flex()
            .size_full()
            .child(self.settings_header(cx))
            .child(
                gpui::div()
                    .v_flex()
                    .size_full()
                    .min_h_0()
                    .child(self.settings_tabs(cx))
                    .child(content),
            )
            .into_any_element()
    }

    fn settings_general_controls(
        &self,
        content: crate::settings::List,
        query: &str,
        cx: &mut Context<Self>,
    ) -> crate::settings::List {
        let matches = |label: &str| {
            feishin_core::settings_search::matches(&self.state.settings.language, query, &[label])
        };
        content
            .setting(
                &["Updates", "update-settings"],
                || crate::styles::button("native-update-settings", cx)
                    .label(self.tr("Updates"))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate(Route::UpdateSettings, cx)),
                    ),
            )
            .when(cfg!(target_os = "linux"), |div| {
                div.setting(&["setting.passwordStore"], || self.password_store_control(cx))
            })
            .setting(
                &["Track info in Window Title", "setting.windowBarTrackinfo"],
                || self.setting_checkbox("window-title-track-info", "setting.windowBarTrackinfo", self.state.settings.window_title_track_info, cx)
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.state.settings.window_title_track_info = *value;
                        this.save();
                        cx.notify();
                    })),
            )
            .setting(
                &["Window bar style", "", "setting.windowBarStyle"],
                || crate::styles::button("window-style", cx)
                    .debug_selector(|| "window-style".into())
                    .tooltip(self.tr("setting.windowBarStyle_description"))
                    .label(format!(
                        "{}: {}",
                        self.tr("Window bar style"),
                        match self.state.settings.window_bar_style.as_str() { "windows" => "Windows", "macos" => "macOS", "web" => "Web (hidden)", _ => "Native" }
                    ))
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        let selected = self.state.settings.window_bar_style.clone();
                        move |mut menu, _, _| {
                            for (value, label) in [
                                ("web", "Web (hidden)"),
                                ("windows", "Windows"),
                                ("macos", "macOS"),
                                ("linux", "Native"),
                            ] {
                                let view = view.clone();
                                menu = menu.item(PopupMenuItem::new(label).checked(selected == value).on_click(
                                    move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.change_window_bar_style(value, cx);
                                        })
                                        .ok();
                                    },
                                ));
                            }
                            menu
                        }
                    }),
            )
            .settings_children("media-session-settings",
                || ([
                    ("setting.mediaSession", true, self.state.settings.media_session),
                    (
                        "setting.globalMediaHotkeys",
                        false,
                        self.state.settings.global_media_hotkeys,
                    ),
                ]
                .into_iter()
                .filter(|(label, _, _)| matches(label))
                .map(|(label, session, value)| {
                    let id = if session {
                        "media-session"
                    } else {
                        "global-media-hotkeys"
                    };
                    Checkbox::new(id)
                    .debug_selector(move || id.into())
                    .label(self.tr(label))
                    .checked(value)
                    .when(crate::appearance::rtl(cx), |checkbox| checkbox.flex_row_reverse())
                    .on_click(cx.listener(
                        move |this, value: &bool, window, cx| {
                            if session {
                                this.state.settings.media_session = *value;
                                if *value {
                                    this.state.settings.global_media_hotkeys = false;
                                }
                            } else {
                                this.state.settings.global_media_hotkeys = *value;
                                if *value {
                                    this.state.settings.media_session = false;
                                }
                            }
                            this.media_attempted = false;
                            this.sync_media_controls(window);
                            this.install_shortcuts(cx);
                            this.save();
                            cx.notify();
                        },
                    ))
                })).collect::<Vec<_>>(),
            )
            .setting(
                &["Private mode", "page.appMenu.privateModeOn", "page.appMenu.privateModeOff"],
                || crate::styles::button("private-mode", cx)
                    .label(self.tr(if self.state.settings.private_mode {
                        "page.appMenu.privateModeOff"
                    } else {
                        "page.appMenu.privateModeOn"
                    }))
                    .selected(self.state.settings.private_mode)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.private_mode = !this.state.settings.private_mode;
                        this.save();
                        this.update_presence(cx);
                        cx.notify();
                    })),
            )
            .setting(
                &["Discord Rich Presence", "setting.discordRichPresence"],
                || Checkbox::new("discord-rpc")
                    .label(self.tr("setting.discordRichPresence"))
                    .checked(self.state.settings.discord_enabled)
                    .when(crate::appearance::rtl(cx), |checkbox| {
                        checkbox.flex_row_reverse()
                    })
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.state.settings.discord_enabled = *value;
                        this.save();
                        this.update_presence(cx);
                        cx.notify();
                    })),
            )
            .setting(
                &["Discord Rich Presence settings", "discord-settings"],
                || crate::styles::button("presence-settings", cx)
                    .label(self.tr("Discord Rich Presence settings"))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate(Route::PresenceSettings, cx)),
                    ),
            )
            .setting(
                &[
                    "Tray icon",
                    "Close to tray",
                    "Minimize to tray",
                    "Start minimized",
                    "setting.trayEnabled",
                    "setting.exitToTray",
                    "setting.minimizeToTray",
                    "setting.startMinimized",
                ],
                || gpui::div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_wrap()
                    .gap_3()
                    .child(
                        self.setting_checkbox("native-tray", "setting.trayEnabled", self.state.settings.tray, cx)
                            .debug_selector(|| "native-tray".into())
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.state.settings.tray = *value;
                                if !*value {
                                    this.state.settings.exit_to_tray = false;
                                    this.state.settings.minimize_to_tray = false;
                                    this.state.settings.start_minimized = false;
                                }
                                this.tray_error = None;
                                this.save();
                                cx.notify();
                            })),
                    )
                    .when(self.state.settings.tray, |row| row.child(
                        self.setting_checkbox("native-close-to-tray", "setting.exitToTray", self.state.settings.exit_to_tray, cx)
                            .debug_selector(|| "native-close-to-tray".into())
                            .disabled(!self.state.settings.tray || !self.native_hide_supported)
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.state.settings.exit_to_tray = *value;
                                this.save();
                                cx.notify();
                            })),
                    ))
                    .when(self.state.settings.tray, |row| row.child(
                        self.setting_checkbox("native-minimize-to-tray", "setting.minimizeToTray", self.state.settings.minimize_to_tray, cx)
                            .debug_selector(|| "native-minimize-to-tray".into())
                            .disabled(!self.state.settings.tray || !self.native_hide_supported)
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.state.settings.minimize_to_tray = *value;
                                this.save();
                                cx.notify();
                            })),
                    ))
                    .when(self.state.settings.tray, |row| row.child(
                        self.setting_checkbox("native-start-minimized", "setting.startMinimized", self.state.settings.start_minimized, cx)
                            .debug_selector(|| "native-start-minimized".into())
                            .on_click(cx.listener(|this, value: &bool, _, cx| {
                                this.state.settings.start_minimized = *value;
                                this.save();
                                cx.notify();
                            })),
                    )),
            )
            .settings_children("theme-settings", || self.theme_settings_rows(query, cx))
            .settings_children("application-settings", || self.image_settings_rows(query, cx))
            .setting(
                &[
                    "Fonts",
                    "setting.fontType",
                    "setting.font",
                    "setting.customFontPath",
                    "setting.zoom",
                ],
                || crate::styles::button("font-settings", cx)
                    .label(self.tr("Fonts"))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::FontPicker, cx))),
            )
            .setting(
                &[
                    "Customize home page",
                    "setting.homeFeature",
                    "setting.homeFeatureStyle",
                ],
                || crate::styles::button("home-settings", cx)
                    .label(self.tr("Customize home page"))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::HomeSettings, cx))),
            )
            .setting(
                &["setting.language"],
                || crate::styles::button("language", cx)
                    .debug_selector(|| "language".into())
                    .label(format!(
                        "{}: {}",
                        self.tr("setting.language"),
                        self.state.settings.language
                    ))
                    .on_click(
                        cx.listener(|this, _, _, cx| this.navigate(Route::LanguagePicker, cx)),
                    ),
            )
            .setting(
                &["Playlist folders", "setting.sidebarPlaylistFolders"],
                || self.setting_checkbox("playlist-folders-enabled", "setting.sidebarPlaylistFolders", self.state.settings.playlist_folders, cx)
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.state.settings.playlist_folders = *value;
                        this.load_playlist_options(cx);
                        this.save();
                        cx.notify();
                    })),
            )
            .when(self.state.settings.playlist_folders, |content| content
                .setting(
                    &["Playlist folder view", "setting.sidebarPlaylistFolderView"],
                    || gpui::div().v_flex().gap_2()
                        .child(crate::styles::button("playlist-folder-view", cx)
                            .debug_selector(|| "playlist-folder-view".into())
                            .label(format!("{}: {}", self.tr("setting.sidebarPlaylistFolderView"), self.tr(match self.state.settings.playlist_folder_view.as_str() {
                                "single" => "setting.sidebarPlaylistFolderView_optionSingle",
                                "navigation" => "setting.sidebarPlaylistFolderView_optionNavigation",
                                _ => "setting.sidebarPlaylistFolderView_optionTree",
                            })))
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                move |mut menu, _, cx| {
                                    for (value, label) in [
                                        ("single", "setting.sidebarPlaylistFolderView_optionSingle"),
                                        ("tree", "setting.sidebarPlaylistFolderView_optionTree"),
                                        ("navigation", "setting.sidebarPlaylistFolderView_optionNavigation"),
                                    ] {
                                        let selected = view.upgrade().is_some_and(|view| view.read(cx).state.settings.playlist_folder_view == value);
                                        let view = view.clone();
                                        menu = menu.item(PopupMenuItem::new(crate::appearance::text(label, cx)).checked(selected).on_click(move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.state.settings.playlist_folder_view = value.into();
                                                this.playlist_folder_path.clear();
                                                this.shared_playlist_folder_path.clear();
                                                this.save();
                                                cx.notify();
                                            }).ok();
                                        }));
                                    }
                                    menu
                                }
                            }))
                        .child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.sidebarPlaylistFolderView_description"))),
                )
                .settings_children("sidebar-settings", || ([
                    ("setting.sidebarPlaylistFolderSeparator", &self.playlist_separator, false),
                    ("setting.sidebarPlaylistFolderTreeIndent", &self.playlist_folder_indent_input, true),
                    ("setting.sidebarPlaylistFolderTreeLineColor", &self.playlist_folder_color, true),
                ].into_iter().filter(|(key, _, tree)| (!tree || self.state.settings.playlist_folder_view == "tree") && feishin_core::settings_search::matches(&self.state.settings.language, query, &[key])).map(|(key, input, _)| {
                    gpui::div().max_w(px(500.)).v_flex().gap_2()
                        .child(self.tr(key))
                        .child(if key == "setting.sidebarPlaylistFolderTreeIndent" {
                            crate::styles::number_input(input, cx).into_any_element()
                        } else {
                            crate::styles::input(input, cx).into_any_element()
                        })
                        .child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr(&format!("{key}_description"))))
                })).collect::<Vec<_>>())
                .setting(
                    &["Apply playlist folder style", "setting.sidebarPlaylistFolderSeparator", "setting.sidebarPlaylistFolderTreeIndent", "setting.sidebarPlaylistFolderTreeLineColor"],
                    || crate::styles::button("playlist-folder-style", cx)
                        .debug_selector(|| "playlist-folder-style".into())
                        .label(self.tr("common.save"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            let mut settings = this.state.settings.clone();
                            settings.playlist_separator = this.playlist_separator.read(cx).value().to_string();
                            let result = (|| -> anyhow::Result<()> {
                                if settings.playlist_folder_view == "tree" {
                                    settings.playlist_folder_color = this.playlist_folder_color.read(cx).value().to_string();
                                    let raw = this.playlist_folder_indent_input.read(cx).value();
                                    let indent = if raw.trim().is_empty() { Ok(0.) } else { raw.parse::<f64>() };
                                    let value = indent.ok().filter(|value| value.is_finite()).ok_or_else(|| anyhow::anyhow!("Use a finite folder indent"))?;
                                    settings.playlist_folder_indent = value.round().clamp(0., 64.) as u32;
                                }
                                feishin_core::migration::validate(&mut settings)
                            })();
                            match result {
                                Ok(()) => {
                                    this.playlist_folder_indent_input.update(cx, |input, cx| input.set_value(settings.playlist_folder_indent.to_string(), window, cx));
                                    this.state.settings = settings;
                                    this.playlist_folder_path.clear();
                                    this.shared_playlist_folder_path.clear();
                                    this.message.clear();
                                    this.save();
                                }
                                Err(error) => this.message = error.to_string(),
                            }
                            cx.notify();
                        })),
                ))
    }

    fn settings_playback_controls(
        &self,
        content: crate::settings::List,
        query: &str,
        cx: &mut Context<Self>,
    ) -> crate::settings::List {
        let matches = |label: &str| {
            feishin_core::settings_search::matches(&self.state.settings.language, query, &[label])
        };
        content
            .setting(&["setting.mpvExecutablePath"], || gpui::div().max_w(px(650.0)).child(crate::styles::input(&self.mpv_path, cx)))
            .setting(&["setting.audioDevice"], || gpui::div().max_w(px(650.0)).child(crate::styles::input(&self.mpv_device, cx)))
            .setting(&["MPV arguments and properties (JSON)", "setting.mpvExtraParameters"], || gpui::div().max_w(px(650.0)).v_flex().gap_2().child(crate::appearance::text("MPV arguments and properties (JSON)", cx)).child(crate::styles::input(&self.mpv_options, cx).h(px(220.0))))
            .setting(&["Apply MPV options", "setting.mpvExtraParameters"], || crate::styles::button("save-mpv-options", cx).label(crate::appearance::text("Apply MPV options", cx)).on_click(cx.listener(|this,_,_,cx|{match this.state.settings.apply_mpv_options(&this.mpv_options.read(cx).value()){Ok(())=>{this.apply_audio_settings(cx);this.message="MPV properties applied. Arguments and removed properties apply on the next track.".into();},Err(error)=>this.message=error.to_string()};cx.notify();})))
            .setting(&["FFmpeg"], || gpui::div().max_w(px(650.0)).child(crate::styles::input(&self.ffmpeg_path, cx)))
            .setting(&["setting.audioPlayer", "setting.audioDevice", "Jukebox"], || crate::styles::button("audio-outputs", cx).label(if self.state.settings.jukebox { "Audio output: Jukebox".into() } else { self.cast_selected.as_ref().map_or("Audio outputs: local".into(),|device|format!("Audio output: {}",device.name)) }).on_click(cx.listener(|this,_,_,cx|this.navigate(Route::Outputs,cx))))
            .setting(&["Apply player path", "setting.mpvExecutablePath", "setting.audioDevice", "FFmpeg"], || crate::styles::button("save-mpv", cx).label(self.tr("common.save")).on_click(cx.listener(|this,_,_,cx|{this.state.settings.set_mpv_path(this.mpv_path.read(cx).value().to_string());this.state.settings.mpv_audio_device=this.mpv_device.read(cx).value().to_string();this.state.settings.ffmpeg_path=this.ffmpeg_path.read(cx).value().to_string();this.apply_audio_settings(cx);this.message="Player path saved. It applies to the next track.".into();cx.notify();})))
            .setting(&["Native audio streams common music formats and live radio. MPV also supports ReplayGain and audio filters."], || gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("Native audio streams common music formats and live radio. MPV also supports ReplayGain and audio filters.")))
            .setting(&["Playback speed", "Apply"], || gpui::div().h_flex().gap_2().items_center().child(self.tr("Playback speed")).child(gpui::div().w(px(120.)).child(crate::styles::number_input(&self.speed_input, cx))).child(crate::styles::button("apply-speed", cx).label(self.tr("common.confirm")).on_click(cx.listener(|this,_,window,cx| {
                match this.speed_input.read(cx).value().parse::<f32>() {
                    Ok(speed) => this.set_playback_speed(speed,window,cx),
                    Err(_) => {this.message="Enter a playback speed from 0.5 to 2".into();cx.notify();}
                }
            }))))
            .setting(&["pitch-adjust-{i}"], || gpui::div().h_flex().gap_2().items_center()
                .children([-1., -0.1, 0.1, 1.].into_iter().enumerate().filter(|(_, step): &(usize, f32)| self.state.settings.microtonal_pitch_controls || step.abs() == 1.).map(|(i, step)| {
                    gpui::div().debug_selector(move || format!("pitch-adjust-{i}"))
                        .child(crate::styles::button(("pitch-adjust", i), cx).label(if step.abs() == 1. {format!("{step:+.0}st")} else {format!("{:+.0}ct",step*100.)})
                            .on_click(cx.listener(move |this,_,window,cx| this.adjust_musical_speed(step,window,cx))))
                }))
                .child(format!("{:.2}x {:+.2}st", self.state.settings.speed,12.*self.state.settings.speed.log2())))
            .setting(&["Microtonal pitch controls"], || Checkbox::new("microtonal-pitch").label(self.tr("Microtonal pitch controls")).checked(self.state.settings.microtonal_pitch_controls).on_click(cx.listener(|this,value:&bool,_,cx| {this.state.settings.microtonal_pitch_controls = *value;this.save();cx.notify();})))
            .setting(&["Track transition", "Crossfade", "setting.playbackStyle", "setting.crossfadeDuration", "setting.crossfadeStyle"], || gpui::div().id("settings-playback-config").debug_selector(|| "settings-playback-config".into()).h_flex().gap_2().items_center().when(crate::appearance::rtl(cx), |div| div.flex_row_reverse()).child(self.tr("setting.playbackStyle")).child(self.player_config_button(None, cx)))
            .setting(&["Fade on play, pause and stop", "setting.audioFadeOnStatusChange"], || crate::styles::button("status-fade", cx).label(self.tr("setting.audioFadeOnStatusChange")).selected(self.state.settings.fade_on_status).on_click(cx.listener(|this,_,_,cx|{this.state.settings.fade_on_status= !this.state.settings.fade_on_status;this.apply_audio_settings(cx);})))
            .setting(&["Preserve pitch", "setting.preservePitch"], || crate::styles::button("pitch", cx).label(self.tr("setting.preservePitch")).selected(self.state.settings.preserve_pitch).on_click(cx.listener(|this,_,_,cx|{this.state.settings.preserve_pitch= !this.state.settings.preserve_pitch;this.apply_audio_settings(cx);})))


            .setting(&["Confirm fetching an entire library list"], || Checkbox::new("large-library-confirm").label(self.tr("Confirm fetching an entire library list")).checked(self.state.settings.confirm_large_fetch).on_click(cx.listener(|this, value: &bool, _, cx| { this.state.settings.confirm_large_fetch = *value; this.save(); cx.notify(); })))
            .map(|content| self.scrobble_controls(content, cx))
            .setting(&["Follow current song", "setting.followCurrentSong"], || self.setting_checkbox("follow-current-song", "setting.followCurrentSong", self.state.settings.follow_current_song, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.follow_current_song = *value;this.save();cx.notify();})))
            .settings_children("application-settings", || ([(true, "setting.albumBackground"), (false, "setting.artistBackground")].into_iter().enumerate().filter(|(_,(_,label))| matches(label)).map(|(i, (album, label))| {
                let enabled=if album {self.state.settings.album_background} else {self.state.settings.artist_background};
                let blur=if album {self.state.settings.album_background_blur} else {self.state.settings.artist_background_blur};
                gpui::div().h_flex().gap_2()
                    .child(self.setting_checkbox(("detail-background-enabled", i), label, enabled, cx).on_click(cx.listener(move |this,checked:&bool,_,cx| {
                        let value=if album {&mut this.state.settings.album_background} else {&mut this.state.settings.artist_background};
                        *value = *checked; this.save(); cx.notify();
                    })))
                    .child(crate::styles::button(("detail-background-blur", i), cx).label(format!("{}: {:.1} rem", self.tr("Background blur"),blur)).on_click(cx.listener(move |this,_,_,cx| {
                        let value=if album {&mut this.state.settings.album_background_blur} else {&mut this.state.settings.artist_background_blur};
                        *value=if *value >= 6. {0.} else {(*value + 0.5).min(6.)}; this.save(); cx.notify();
                    })))
            })).collect::<Vec<_>>())
            .settings_children("application-settings", || ([("setting.showFavorites", 0usize), ("setting.showRatings", 1), ("setting.enableGridMultiSelect", 2), ("setting.blurExplicitImages", 3)].into_iter().filter(|(label,_)| matches(label)).map(|(label, i)| {
                let settings=&self.state.settings;
                let value=match i {0=>settings.show_favorites,1=>settings.show_ratings,2=>settings.enable_grid_multiselect,_=>settings.blur_explicit_images};
                self.setting_checkbox(("library-preference", i), label, value, cx).on_click(cx.listener(move |this,checked:&bool,_,cx|{
                    let settings=&mut this.state.settings;
                    let value=match i {0=>&mut settings.show_favorites,1=>&mut settings.show_ratings,2=>&mut settings.enable_grid_multiselect,_=>&mut settings.blur_explicit_images};
                    *value = *checked;
                    if i == 2 { this.selected.clear(); this.grid_selection_order.clear(); }
                    if i == 3 { this.player_background=None; this.player_background_key.clear(); this.fetch_covers(cx); }
                    this.save();cx.notify();
                }))
            })).collect::<Vec<_>>())

            .setting(&["Open expanded player after idle minutes (0 disables)", "setting.fullscreenAutoOpenTimeout"], || gpui::div().max_w(px(250.0)).v_flex().gap_2().child(self.tr("Open expanded player after idle minutes (0 disables)")).child(crate::styles::number_input(&self.fullscreen_idle_input, cx)))
            .setting(&["Apply idle timeout", "setting.fullscreenAutoOpenTimeout"], || crate::styles::button("fullscreen-idle-apply", cx).label(self.tr("common.save")).on_click(cx.listener(|this,_,_,cx| {match this.fullscreen_idle_input.read(cx).value().parse::<f64>() {Ok(value) if value.is_finite() && (0. ..=120.).contains(&value)=>{this.state.settings.fullscreen_auto_open_minutes=value;this.save();}, _=>this.message="Use a timeout from 0 to 120 minutes".into()};cx.notify();})))
            .setting(&["Click player bar to expand", "setting.playerbarOpenDrawer"], || self.setting_checkbox("playerbar-open-drawer", "setting.playerbarOpenDrawer", self.state.settings.playerbar_open_drawer, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.playerbar_open_drawer = *value;this.save();cx.notify();})))
            .settings_children("control-settings", || self.waveform_controls(query, cx))
            .setting(&["Volume wheel step", "setting.volumeWheelStep"], || gpui::div().v_flex().gap_2()
                .child(self.tr("setting.volumeWheelStep"))
                .child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.volumeWheelStep_description")))
                .child(gpui::div().w(px(100.)).debug_selector(|| "volume-wheel-step".into()).child(crate::styles::slider(&self.volume_wheel_slider, cx).aria_label(self.tr("setting.volumeWheelStep")))))
            .settings_children("control-settings", || ([("setting.volumeWidth", 0), ("setting.artistRadioCount", 1), ("setting.buttonSize", 2)].into_iter().filter(|(key,_)| matches(key)).map(|(key, index)| gpui::div().max_w(px(250.)).v_flex().gap_2()
                .child(self.tr(key))
                .child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr(&format!("{key}_description"))))
                .child(crate::styles::number_input(&self.volume_settings_inputs[index], cx)))).collect::<Vec<_>>())

            .setting(&["Play button behavior"], || crate::styles::button("play-button-behavior", cx).label(format!("{}: {}",self.tr("setting.playButtonBehavior"),self.tr(match self.state.settings.play_button_behavior {feishin_core::queue::PlayBehavior::Now=>"setting.playButtonBehavior_optionPlay",feishin_core::queue::PlayBehavior::Next=>"setting.playButtonBehavior_optionAddNext",feishin_core::queue::PlayBehavior::Last=>"setting.playButtonBehavior_optionAddLast",feishin_core::queue::PlayBehavior::Shuffle=>"setting.playButtonBehavior_optionPlayShuffled"}))).dropdown_menu({
                let view=cx.weak_entity();
                move |mut menu,_,cx| {
                    use feishin_core::queue::PlayBehavior;
                    for (value,label) in [(PlayBehavior::Now,"setting.playButtonBehavior_optionPlay"),(PlayBehavior::Next,"setting.playButtonBehavior_optionAddNext"),(PlayBehavior::Last,"setting.playButtonBehavior_optionAddLast"),(PlayBehavior::Shuffle,"setting.playButtonBehavior_optionPlayShuffled")] {
                        let selected = view.upgrade().is_some_and(|view| view.read(cx).state.settings.play_button_behavior == value);
                        let view=view.clone();
                        menu=menu.item(PopupMenuItem::new(crate::appearance::text(label,cx)).checked(selected).on_click(move |_,_,cx|{view.update(cx,|this,cx|{this.state.settings.play_button_behavior=value;this.save();cx.notify();}).ok();}));
                    }
                    menu
                }
            }))
            .setting(&["Up next queue", "setting.upNextQueue"], || gpui::div().v_flex().gap_1().child(self.setting_checkbox("up-next-queue", "setting.upNextQueue", self.state.settings.up_next_queue, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.up_next_queue = *value;this.save();cx.notify();}))).child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.upNextQueue_description"))))
            .setting(&["Restore queue on startup", "setting.savePlayQueue"], || self.setting_checkbox("resume-queue", "setting.savePlayQueue", self.state.settings.resume_queue, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.resume_queue = *value;this.save();cx.notify();})))
            .settings_children("external-links-settings", || ([("setting.externalLinks", 0), ("setting.lastfm", 1), ("setting.listenbrainz", 2), ("setting.musicbrainz", 3), ("setting.qobuz", 4), ("setting.spotify", 5), ("setting.nativeSpotify", 6)].into_iter().filter(|(label,i)| matches(label) && (*i == 0 || self.state.settings.external_links.enabled) && (*i != 6 || self.state.settings.external_links.spotify)).map(|(label, i)| {
                let config=&self.state.settings.external_links;
                let value=match i {0=>config.enabled,1=>config.last_fm,2=>config.listen_brainz,3=>config.music_brainz,4=>config.qobuz,5=>config.spotify,_=>config.native_spotify};
                self.setting_checkbox(("external-link-setting", i as usize), label, value, cx).on_click(cx.listener(move |this,checked:&bool,_,cx|{
                    let config=&mut this.state.settings.external_links;
                    let value=match i {0=>&mut config.enabled,1=>&mut config.last_fm,2=>&mut config.listen_brainz,3=>&mut config.music_brainz,4=>&mut config.qobuz,5=>&mut config.spotify,_=>&mut config.native_spotify};
                    *value = *checked;this.save();cx.notify();
                }))
            })).collect::<Vec<_>>())
            .setting(&["Autosave queue to server", "setting.autosave"], || self.setting_checkbox("queue-autosave", "setting.autosave", self.state.settings.queue_autosave, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.queue_autosave = *value;this.save();cx.notify();})))
            .setting(&["Tracks between queue saves", "setting.autosaveCount"], || gpui::div().max_w(px(250.0)).v_flex().gap_2().child(self.tr("Tracks between queue saves")).child(crate::styles::number_input(&self.queue_autosave_input, cx)))
            .setting(&["Apply autosave interval", "setting.autosaveCount"], || crate::styles::button("queue-autosave-apply", cx).label(self.tr("common.save")).on_click(cx.listener(|this,_,_,cx|{match this.queue_autosave_input.read(cx).value().parse::<u32>() {Ok(value) if value > 0 => {this.state.settings.queue_autosave_count=value;this.save();}, _=>this.message="Use a positive whole number of tracks".into()}; cx.notify();})))
            .setting(&["Confirm queue changes", "setting.confirmQueueChanges"], || self.setting_checkbox("confirm-queue", "setting.confirmQueueChanges", self.state.settings.confirm_queue_changes, cx).on_click(cx.listener(|this,value:&bool,_,cx|{this.state.settings.confirm_queue_changes = *value;this.save();cx.notify();})))
            .setting(&["Auto DJ", "auto-dj-settings"], || crate::styles::button("auto-dj", cx).label(self.tr("setting.autoDJ_enabled")).selected(self.state.settings.auto_dj).on_click(cx.listener(|this,_,_,cx|{this.state.settings.auto_dj= !this.state.settings.auto_dj;this.reset_auto_dj();this.save();cx.notify();})))
            .setting(&["setting.autoDJ", "auto-dj-settings"], || self.auto_dj_controls(cx))
            .setting(&["Player Filters", "setting.playerFilters"], || crate::styles::button("playback-filters", cx).label(self.tr("Player Filters")).on_click(cx.listener(|this,_,_,cx|this.navigate(Route::PlaybackFilters,cx))))
            .setting(&["setting.transcode"], || crate::styles::button("transcode", cx).label(self.tr("setting.transcode")).selected(self.state.settings.transcode).on_click(cx.listener(|this,_,_,cx|{this.state.settings.transcode= !this.state.settings.transcode;this.save();cx.notify();})))
            .setting(&["setting.transcodeSampleRate"], || gpui::div().max_w(px(300.)).v_flex().gap_2().child(self.tr("setting.transcodeSampleRate")).child(crate::styles::number_input(&self.sample_rate_input, cx)).child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.transcodeSampleRate_description"))))
            .setting(&["setting.transcodeBitrate"], || gpui::div().max_w(px(300.)).v_flex().gap_2().child(self.tr("setting.transcodeBitrate")).child(crate::styles::number_input(&self.bitrate_input, cx)).child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.transcodeBitrate_description"))))
            .setting(&["setting.transcodeFormat"], || gpui::div().max_w(px(300.)).v_flex().gap_2().child(self.tr("setting.transcodeFormat")).child(crate::styles::input(&self.transcode_format_input, cx)).child(gpui::div().text_sm().text_color(cx.theme().muted_foreground).child(self.tr("setting.transcodeFormat_description"))))
            .setting(&["transcode-settings"], || crate::styles::button("transcode-save", cx).debug_selector(|| "transcode-save".into()).label(self.tr("common.save")).on_click(cx.listener(|this, _, _, cx| this.apply_transcoding_settings(cx))))
            .setting(&["ReplayGain", "setting.replayGainMode", "setting.replayGainPreamp", "setting.replayGainClipping", "setting.replayGainFallback"], || crate::styles::button("replaygain", cx).label(format!("{}: {}", self.tr("setting.replayGainMode"), self.tr(match self.state.settings.replay_gain.as_str() { "track" => "setting.replayGainMode_optionTrack", "album" => "setting.replayGainMode_optionAlbum", _ => "setting.replayGainMode_optionNone" }))).on_click(cx.listener(|this,_,_,cx|{this.state.settings.replay_gain=match this.state.settings.replay_gain.as_str(){"no"=>"track","track"=>"album",_=>"no"}.into();this.apply_audio_settings(cx);})))
            .setting(&["Equalizer", "setting.equalizer"], || crate::styles::button("eq-enabled", cx).debug_selector(|| "equalizer-toggle".into()).label(self.tr("setting.equalizer")).selected(self.state.settings.equalizer_enabled).on_click(cx.listener(|this,_,window,cx|{this.state.settings.equalizer_enabled= !this.state.settings.equalizer_enabled;if !this.state.settings.equalizer_enabled { this.audio_preset_selects[0].update(cx, |select,cx| select.reset(window,cx)); }this.apply_audio_settings(cx);})))
            .setting(&["eq-settings"], || self.equalizer_controls(cx))
            .setting(&["Compressor", "setting.compressor"], || crate::styles::button("compressor", cx).debug_selector(|| "compressor-toggle".into()).label(self.tr("setting.compressor")).selected(self.state.settings.compressor.enabled).on_click(cx.listener(|this,_,window,cx|{this.state.settings.compressor.enabled= !this.state.settings.compressor.enabled;if !this.state.settings.compressor.enabled { this.audio_preset_selects[1].update(cx, |select,cx| select.reset(window,cx)); }this.apply_audio_settings(cx);})))
            .setting(&["eq-settings"], || self.compressor_controls(cx))
            .setting(&["lyric-settings"], || self.lyric_settings(cx))
            .setting(&["Keep display awake during playback", "setting.preventSleepOnPlayback"], || self.setting_checkbox("prevent-sleep", "setting.preventSleepOnPlayback", self.state.settings.prevent_sleep, cx).debug_selector(|| "prevent-sleep".into()).disabled(self.state.settings.prevent_suspend).on_click(cx.listener(|this, value: &bool, _, cx| { this.state.settings.prevent_sleep = *value; this.save(); cx.notify(); })))
            .setting(&["Prevent idle sleep during playback", "setting.preventSuspendOnPlayback"], || self.setting_checkbox("prevent-suspend", "setting.preventSuspendOnPlayback", self.state.settings.prevent_suspend, cx).debug_selector(|| "prevent-suspend".into()).disabled(self.state.settings.prevent_sleep).on_click(cx.listener(|this, value: &bool, _, cx| { this.state.settings.prevent_suspend = *value; this.save(); cx.notify(); })))
            .setting(&["Sleep timer"], || self.sleep_controls(cx))
    }

    fn settings_library_controls(
        &self,
        content: crate::settings::List,
        cx: &mut Context<Self>,
    ) -> crate::settings::List {
        content
            .setting(&["Restart"], || {
                crate::styles::button("restart-native", cx)
                    .debug_selector(|| "restart-native".into())
                    .label(self.tr("Restart"))
                    .on_click(cx.listener(|this, _, window, cx| this.restart_native(window, cx)))
            })
            .setting(&["Developer tools"], || {
                crate::styles::button("native-inspector", cx)
                    .label(self.tr("Developer tools"))
                    .on_click(|_, window, cx| {
                        window.dispatch_action(Box::new(gpui_component::ToggleInspector), cx)
                    })
            })
            .setting(&["setting.remoteUsername"], || {
                gpui::div()
                    .max_w(px(650.))
                    .v_flex()
                    .gap_2()
                    .child(self.tr("setting.remoteUsername"))
                    .child(crate::styles::input(&self.remote_username, cx))
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tr("setting.remoteUsername_description")),
                    )
            })
            .setting(&["setting.remotePassword"], || {
                gpui::div()
                    .max_w(px(650.))
                    .v_flex()
                    .gap_2()
                    .child(self.tr("setting.remotePassword"))
                    .child(crate::styles::input(&self.remote_password, cx).mask_toggle())
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tr("setting.remotePassword_description")),
                    )
            })
            .setting(&["setting.remotePort"], || {
                gpui::div()
                    .max_w(px(650.))
                    .v_flex()
                    .gap_2()
                    .child(self.tr("setting.remotePort"))
                    .child(crate::styles::number_input(&self.remote_port, cx))
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tr("setting.remotePort_description")),
                    )
            })
            .setting(&["Save remote settings", "remote-settings"], || {
                crate::styles::button("remote-apply", cx)
                    .debug_selector(|| "remote-apply".into())
                    .label(crate::appearance::text("Save remote settings", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.apply_remote_config(cx)))
            })
            .setting(&["setting.enableRemote"], || {
                gpui::div()
                    .v_flex()
                    .gap_2()
                    .child(
                        Checkbox::new("remote-toggle")
                            .debug_selector(|| "remote-toggle".into())
                            .label(self.tr("setting.enableRemote"))
                            .checked(self.remote.is_some() || self.state.settings.remote_enabled)
                            .when(crate::appearance::rtl(cx), |checkbox| {
                                checkbox.flex_row_reverse()
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_remote(cx))),
                    )
                    .child(
                        gpui::div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(self.tr("setting.enableRemote_description")),
                    )
                    .child({
                        let url = format!(
                            "http://localhost:{}",
                            self.remote
                                .as_ref()
                                .map_or(self.state.settings.remote_port, |remote| remote.port)
                        );
                        crate::styles::button("remote-open", cx)
                            .label(url.clone())
                            .on_click(move |_, _, cx| cx.open_url(&url))
                    })
            })
            .setting(&["cache-settngs"], || self.cache_controls(cx))
            .setting(&["Edit local audio tags"], || {
                crate::styles::button("edit-local-tags", cx)
                    .label(self.tr("Edit local audio tags"))
                    .on_click(cx.listener(|this, _, window, cx| this.choose_tag_files(window, cx)))
            })
            .setting(&["path-settings", "setting.pathReplace"], || {
                crate::path_settings::view(cx.weak_entity(), None)
            })
            .setting(&["Scan server library"], || {
                crate::styles::button("scan", cx)
                    .debug_selector(|| "scan-library".into())
                    .label(self.library_scan_label())
                    .disabled(self.library_scan_busy() || !self.can_scan_library())
                    .on_click(cx.listener(|this, _, _, cx| this.start_library_scan(cx)))
            })
            .setting(&["Export diagnostics"], || {
                crate::styles::button("export-diagnostics", cx)
                    .label(crate::appearance::text("Export diagnostics", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.export_diagnostics(cx)))
            })
            .setting(&["Log level", "setting.logLevel"], || {
                crate::styles::button("log-level", cx)
                    .label(format!(
                        "{}: {}",
                        self.tr("setting.logLevel"),
                        self.tr(if self.state.settings.log_level == "debug" {
                            "setting.logLevel_optionDebug"
                        } else {
                            "setting.logLevel_optionInfo"
                        })
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.change_log_level(cx)))
            })
            .setting(&["Open log folder"], || {
                crate::styles::button("open-log-folder", cx)
                    .label(crate::appearance::text("Open log folder", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let dir = this.data_dir.join("logs");
                        if let Some(path) = dir.to_str() {
                            cx.reveal_path(std::path::Path::new(path));
                        }
                    }))
            })
            .setting(&["Collections"], || {
                crate::styles::button("collection-settings", cx)
                    .label(crate::appearance::text("Collections", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Collections, cx)))
            })
            .setting(&["Visualizer"], || {
                crate::styles::button("visualizer-settings", cx)
                    .label(crate::appearance::text("Visualizer", cx))
                    .on_click(cx.listener(|this, _, window, cx| this.edit_visualizer(window, cx)))
            })
            .setting(
                &[
                    "Playback panels",
                    "setting.sidePlayQueueStyle",
                    "setting.sidePlayQueueLayout",
                    "setting.showQueueInSidebar",
                    "setting.showLyricsInSidebar",
                    "setting.showVisualizerInSidebar",
                    "setting.combinedLyricsAndVisualizer",
                ],
                || {
                    crate::styles::button("playback-panel-settings", cx)
                        .label(crate::appearance::text("Playback panels", cx))
                        .on_click(
                            cx.listener(|this, _, _, cx| this.navigate(Route::PanelSettings, cx)),
                        )
                },
            )
            .setting(&["Album groups"], || {
                crate::styles::button("release-group-settings", cx)
                    .label(crate::appearance::text("Album groups", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::AlbumGroups, cx)))
            })
            .setting(
                &[
                    "Export settings and queue",
                    "setting.exportImportSettings_control_title",
                ],
                || {
                    crate::styles::button("export-settings", cx)
                        .label(self.tr("Export settings and queue"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let picker = cx
                                .prompt_for_new_path(&this.data_dir, Some("feishin-settings.json"));
                            cx.spawn(async move |this, cx| {
                                if let Ok(Ok(Some(path))) = picker.await {
                                    this.update(cx, |this, cx| {
                                        this.message = match storage::save(&path, &this.state) {
                                            Ok(()) => {
                                                "Settings exported without credentials".into()
                                            }
                                            Err(e) => e.to_string(),
                                        };
                                        cx.notify();
                                    })
                                    .ok();
                                }
                            })
                            .detach();
                        }))
                },
            )
            .setting(
                &[
                    "Import settings",
                    "setting.exportImportSettings_control_title",
                ],
                || {
                    crate::styles::button("import-settings", cx)
                        .debug_selector(|| "import-settings".into())
                        .label(self.tr("setting.exportImportSettings_control_importText"))
                        .on_click(cx.listener(|this, _, _, cx| this.open_settings_import(cx)))
                },
            )
            .setting(&["Open application data folder"], || {
                crate::styles::button("open-data", cx)
                    .label(self.tr("Open application data folder"))
                    .on_click(cx.listener(|this, _, _, cx| cx.reveal_path(&this.data_dir)))
            })
            .setting(
                &[
                    "Custom query builder fields",
                    "setting.queryBuilderCustomFields",
                ],
                || {
                    crate::styles::button("query-field-settings", cx)
                        .label(self.tr("Custom query builder fields"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_query_fields(window, cx)),
                        )
                },
            )
            .setting(&["Keyboard shortcuts"], || {
                crate::styles::button("shortcut-settings", cx)
                    .label(crate::appearance::text("Keyboard shortcuts", cx))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Shortcuts, cx)))
            })
    }

    fn editor(&self, cx: &mut Context<Self>) -> AnyElement {
        gpui::div()
            .id("playlist-editor")
            .native_vertical_scroll()
            .h_full()
            .v_flex()
            .gap_4()
            .max_w(px(720.0))
            .child(crate::styles::input(&self.form_name, cx))
            .child(crate::styles::input(&self.form_comment, cx))
            .when(self.route == Route::PlaylistEditor, |div| {
                div.child(
                    crate::styles::button("public-playlist", cx)
                        .label(self.tr("form.createPlaylist.input_public"))
                        .selected(self.form_public)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.form_public = !this.form_public;
                            cx.notify();
                        })),
                )
                .when(
                    self.api()
                        .is_some_and(|api| api.server.kind == ServerType::Navidrome),
                    |div| {
                        div.child(self.playlist_owner_controls(cx))
                            .child(
                                gpui::div()
                                    .h_flex()
                                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                    .gap_2()
                                    .child(
                                        crate::styles::button("visual-smart", cx)
                                            .label(crate::appearance::text(
                                                "Visual rule editor",
                                                cx,
                                            ))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.enable_smart(window, cx)
                                            })),
                                    )
                                    .child(
                                        crate::styles::button("raw-smart", cx)
                                            .label(crate::appearance::text("Edit JSON", cx))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.raw_smart(window, cx)
                                            })),
                                    )
                                    .child(
                                        crate::styles::button("standard-playlist", cx)
                                            .label(crate::appearance::text("Standard playlist", cx))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.smart_editor = None;
                                                this.form_rules.update(cx, |input, cx| {
                                                    input.set_value("", window, cx)
                                                });
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .child(if let Some(editor) = &self.smart_editor {
                                editor.render(
                                    &self.playlist_options,
                                    self.form_target.as_ref().map(|item| item.id.as_str()),
                                    cx,
                                )
                            } else {
                                crate::styles::input(&self.form_rules, cx)
                                    .h(px(220.0))
                                    .into_any_element()
                            })
                    },
                )
                .child(
                    gpui::div()
                        .text_sm()
                        .child(format!("{} tracks to add", self.form_songs.len())),
                )
                .child(
                    crate::styles::button("save-playlist", cx)
                        .primary()
                        .disabled(self.playlist_saving)
                        .loading(self.playlist_saving)
                        .label(self.tr("Save playlist"))
                        .on_click(cx.listener(|this, _, _, cx| this.save_playlist(cx))),
                )
            })
            .when(self.route == Route::RadioEditor, |div| {
                div.child(crate::styles::input(&self.form_homepage, cx))
                    .child(
                        crate::styles::button("save-radio", cx)
                            .primary()
                            .label(self.tr("Save station"))
                            .on_click(cx.listener(|this, _, _, cx| this.save_radio(cx))),
                    )
            })
            .into_any_element()
    }

    pub(crate) fn player_bar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let metadata = self.current_metadata();
        let item = metadata.as_ref();
        let radio = self.radio_item.is_some();
        let compact = f32::from(window.viewport_size().width) / self.state.settings.zoom <= 768.;
        let rtl = crate::appearance::rtl(cx);
        gpui::div()
            .id("player-bar")
            .debug_selector(|| "player-bar".into())
            .when(
                !compact && self.state.settings.playerbar_open_drawer,
                |div| {
                    div.aria_label(self.tr(if self.full_player {
                        "Close expanded player"
                    } else {
                        "Open expanded player"
                    }))
                    .aria_expanded(self.full_player)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_full_player(cx)))
                },
            )
            .h_flex()
            .when(!compact, |bar| bar.grid().grid_cols(3).grid_rows(1))
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .h(px(90. * self.state.settings.zoom))
            .w_full()
            .flex_shrink_0()
            .gap(rems(1.))
            .when(compact, |bar| {
                bar.px(rems(1.))
                    .py(rems(0.5))
                    .gap(rems(0.5))
                    .overflow_hidden()
            })
            .when(!compact, |bar| bar.border_t(px(self.state.settings.zoom)))
            .border_color(cx.theme().border.opacity(0.5))
            .bg(cx.theme().background)
            .when(true, |div| {
                crate::styles::apply(
                    div,
                    "div",
                    "player-bar",
                    &["fs-player-bar-module-container"],
                    None,
                    cx,
                )
            })
            .child(
                gpui::div()
                    .id("player-bar-control-group-0")
                    .role(gpui::Role::GenericContainer)
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .debug_selector(|| "player-left-controls".into())
                    .gap(rems(if compact { 0.5 } else { 0.75 }))
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .when(!compact, |group| {
                        group
                            .pl(rems(1.))
                            .col_start(if rtl { 3 } else { 1 })
                            .row_start(1)
                    })
                    .when(
                        if compact {
                            item.is_some_and(|item| !item.id.is_empty())
                        } else {
                            !(self.state.settings.sidebar.image_enabled
                                && self.state.settings.sidebar.image_shown
                                && !self.state.settings.sidebar.collapsed)
                        },
                        |div| {
                            let item = item.cloned().unwrap_or_default();
                            let artwork =
                                gpui::div()
                                    .id("playerbar-artwork")
                                    .debug_selector(|| "playerbar-artwork".into())
                                    .relative()
                                    .flex_shrink_0()
                                    .role(Role::Button)
                                    .focusable()
                                    .tab_stop(true)
                                    .aria_label(self.tr("player.toggleFullscreenPlayer"))
                                    .child(self.artwork_at(
                                        &item,
                                        60. * self.state.settings.zoom,
                                        feishin_core::images::Placement::Table,
                                        cx,
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        if compact {
                                            this.toggle_compact_player(cx);
                                        } else {
                                            this.toggle_full_player(cx);
                                        }
                                    }))
                                    .when(
                                        !compact
                                            && self.state.settings.sidebar.image_enabled
                                            && !self.state.settings.sidebar.collapsed,
                                        |div| {
                                            div.child(
                                                crate::styles::button(
                                                    "playerbar-show-sidebar-artwork",
                                                    cx,
                                                )
                                                .debug_selector(|| {
                                                    "playerbar-show-sidebar-artwork".into()
                                                })
                                                .ghost()
                                                .xsmall()
                                                .icon(IconName::ChevronUp)
                                                .tooltip(self.tr("common.expand"))
                                                .absolute()
                                                .right_0()
                                                .top_0()
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.state.settings.sidebar.image_shown = true;
                                                    this.fetch_covers(cx);
                                                    this.save();
                                                    cx.stop_propagation();
                                                    cx.notify();
                                                })),
                                            )
                                        },
                                    );
                            div.child(if item.kind == ItemKind::Song && !item.id.is_empty() {
                                artwork
                                    .context_menu(self.item_menu_builder(
                                        &item,
                                        vec![item.clone()],
                                        false,
                                        cx,
                                    ))
                                    .into_any_element()
                            } else {
                                artwork.into_any_element()
                            })
                        },
                    )
                    .child(
                        gpui::div()
                            .id("current-track")
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(rems(if compact { 0. } else { 0.25 }))
                            .when(compact, |stack| stack.max_w(relative(0.5)))
                            .text_size(rems(if compact { 11. / 16. } else { 14. / 16. }))
                            .line_height(rems(if compact { 14. / 16. } else { 18. / 16. }))
                            .child(
                                gpui::div()
                                    .h_flex()
                                    .min_w_0()
                                    .gap(rems(0.25))
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child({
                                        let title = gpui::div()
                                            .id("playerbar-title")
                                            .debug_selector(|| "playerbar-title".into())
                                            .when(!radio, |div| {
                                                div.role(Role::Link)
                                                    .focusable()
                                                    .tab_stop(true)
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.navigate_playerbar(
                                                            Route::Queue,
                                                            compact,
                                                            cx,
                                                        )
                                                    }))
                                            })
                                            .min_w_0()
                                            .text_size(rems(if compact {
                                                13. / 16.
                                            } else {
                                                14. / 16.
                                            }))
                                            .line_height(rems(if compact { 1. } else { 18. / 16. }))
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(item.map_or_else(
                                                || TitleText::from("Nothing playing".to_owned()),
                                                |item| track_title(item, !radio && !compact, cx),
                                            ));
                                        if let Some(item) =
                                            item.filter(|item| !radio && !item.id.is_empty())
                                        {
                                            title
                                                .context_menu(self.item_menu_builder(
                                                    item,
                                                    vec![item.clone()],
                                                    false,
                                                    cx,
                                                ))
                                                .into_any_element()
                                        } else {
                                            title.into_any_element()
                                        }
                                    })
                                    .when_some(
                                        item.filter(|item| !radio && !item.id.is_empty()),
                                        |row, item| {
                                            row.child(
                                                crate::playerbar::button(
                                                    "track-menu",
                                                    false,
                                                    window,
                                                    cx,
                                                )
                                                .with_size(window.rem_size() * 1.15)
                                                .size(rems(1.15))
                                                .icon(
                                                    gpui_component::Icon::new(
                                                        IconName::EllipsisVertical,
                                                    )
                                                    .with_size(window.rem_size() * 0.875),
                                                )
                                                .tooltip(self.tr("More"))
                                                .dropdown_menu(self.item_menu_builder(
                                                    item,
                                                    vec![item.clone()],
                                                    false,
                                                    cx,
                                                )),
                                            )
                                        },
                                    ),
                            )
                            .child(
                                gpui::div()
                                    .truncate()
                                    .text_color(cx.theme().muted_foreground)
                                    .when_some(item, |div, item| {
                                        if radio {
                                            div.child(item.artist.clone())
                                        } else {
                                            div.child(self.joined_artists(
                                                item,
                                                false,
                                                if compact {
                                                    "playerbar-compact-artists"
                                                } else {
                                                    "playerbar-artists"
                                                },
                                                cx,
                                            ))
                                        }
                                    }),
                            )
                            .when_some(item.filter(|_| radio), |div, item| {
                                div.child(
                                    gpui::div()
                                        .id("playerbar-station")
                                        .debug_selector(|| "playerbar-station".into())
                                        .text_sm()
                                        .truncate()
                                        .text_color(cx.theme().muted_foreground)
                                        .role(Role::Link)
                                        .focusable()
                                        .tab_stop(true)
                                        .cursor_pointer()
                                        .child(item.album.clone())
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.navigate(Route::Library(ItemKind::Radio), cx)
                                        })),
                                )
                            })
                            .when_some(
                                item.filter(|item| !radio && !item.album.is_empty()),
                                |div, item| {
                                    let album = Item {
                                        id: item.album_id.clone(),
                                        name: item.album.clone(),
                                        server_id: item.server_id,
                                        kind: ItemKind::Album,
                                        ..Default::default()
                                    };
                                    div.child(
                                        gpui::div()
                                            .id("playerbar-album")
                                            .debug_selector(|| "playerbar-album".into())
                                            .truncate()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(item.album.clone())
                                            .when(!album.id.is_empty(), |div| {
                                                div.role(Role::Link)
                                                    .focusable()
                                                    .tab_stop(true)
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.navigate_playerbar(
                                                            Route::Detail(Box::new(album.clone())),
                                                            compact,
                                                            cx,
                                                        )
                                                    }))
                                            }),
                                    )
                                },
                            ),
                    ),
            )
            .child(if compact {
                self.playerbar_transport(true, window, cx)
            } else {
                gpui::div()
                    .id("player-bar-control-group-1")
                    .debug_selector(|| "player-center-controls".into())
                    .role(Role::GenericContainer)
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .v_flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .col_start(2)
                    .row_start(1)
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .gap(rems(0.5))
                    .child(self.playerbar_transport(false, window, cx))
                    .when(!radio, |group| group.child(self.seek_row(window, cx)))
                    .into_any_element()
            })
            .when(!compact, |bar| {
                bar.child(self.playerbar_right_controls(window, cx))
            })
            .into_any_element()
    }
}

impl Feishin {
    fn expanded_player_view(
        &mut self,
        slide: f32,
        color: Rgba,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::styles::expanded_player_scope(cx, |cx| {
            let content = self.now_playing(window, cx);
            let hover = window.use_keyed_state("expanded-player-hover-controls", cx, |_, _| {
                gpui_component::animation::CssTransition::new(
                    0.2,
                    std::time::Duration::from_millis(200),
                    [0.25, 0.1, 0.25, 1.],
                )
            });
            let (opacity, animating) = hover.read(cx).value(cx.background_executor().now());
            if animating {
                window.request_animation_frame();
            }
            gpui::div()
                .id("expanded-player")
                .debug_selector(|| "expanded-player".into())
                .absolute()
                .left_0()
                .top(relative(slide))
                .size_full()
                .overflow_hidden()
                .occlude()
                .map(|div| {
                    crate::styles::apply(
                        div,
                        "div",
                        "expanded-player",
                        &["fs-full-screen-player-module-container"],
                        None,
                        cx,
                    )
                })
                .on_hover(move |hovered, _, cx| {
                    hover.update(cx, |hover, cx| {
                        hover.retarget(
                            if *hovered { 1. } else { 0.2 },
                            cx.background_executor().now(),
                        );
                        cx.notify();
                    })
                })
                .child(self.player_background_view(color, window, cx))
                .child(content)
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .opacity(opacity)
                        .child(self.player_tabs(cx))
                        .child(self.player_header_controls(cx)),
                )
                .into_any_element()
        })
    }
}

impl Render for Feishin {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.route != Route::Settings
            || self.state.settings.settings_tab != feishin_core::settings_search::Tab::General
        {
            self.image_resolution_open = false;
        }
        if !cx.has_active_drag() {
            self.sidebar_folder_drag_expanded.clear();
        }
        self.sync_progress(window, cx);
        self.sync_song_lyric_offset(window, cx);
        self.restore_navigation_scroll(window, cx);
        let route = std::mem::discriminant(&self.route);
        if self.secret_form_route != Some(route) {
            for input in [
                &self.password,
                &self.remote_password,
                &self.presence_settings_inputs[1],
                &self.lyric_translation_key,
            ] {
                input.update(cx, |input, cx| input.set_masked(true, window, cx));
            }
            self.secret_form_route = Some(route);
        }
        if cx
            .try_global::<crate::appearance::Language>()
            .is_none_or(|locale| locale.0 != self.state.settings.language)
        {
            gpui_component::set_locale(&self.state.settings.language);
            cx.set_global(crate::appearance::Language(
                self.state.settings.language.clone(),
            ));
        }
        if let Some(editor) = &mut self.smart_editor {
            editor.refresh_choices(
                &self.state.settings.query_fields,
                &self.state.settings.language,
                window,
                cx,
            );
        }
        let title = self.window_caption();
        if window.window_title() != title {
            window.set_window_title(&title);
        }
        self.sync_queue_confirmation(window, cx);
        self.sync_library_fetch_confirmation(window, cx);
        self.sync_css_environment(window, cx);
        self.sync_player_related(cx);
        self.sync_visualizer_display(window, cx);
        self.animate_lyric_scroll(window, cx);
        self.sync_style_context(window, cx);
        self.sync_queue_follow(cx);
        self.sync_player_background(cx);
        self.sync_detail_background(cx);
        self.sync_home_backgrounds(cx);
        if self.playing
            && self.state.settings.lyric_words
            && (self.route == Route::Lyrics
                || self.full_player
                || (self.state.settings.panels.expanded
                    && self
                        .active_panels()
                        .contains(&feishin_core::panels::Panel::Lyrics)))
            && self
                .lyrics
                .iter()
                .any(|line| !line.words.is_empty() || !line.cue_lines.is_empty())
        {
            window.request_animation_frame();
        }
        let visualizer_visible = self.route == Route::Visualizer
            || self.visualizer_display.mounted
            || (self.full_player
                && self.state.settings.player_display.active_tab == "visualizer"
                && self.state.settings.mpv_path.is_empty())
            || self.panels_visualizer_visible();
        self.player.capture.enable(visualizer_visible);
        self.sync_system_audio(visualizer_visible);
        if (!visualizer_visible || self.state.settings.visualizer.kind != "butterchurn")
            && let Some(renderer) = self.milkdrop.take()
            && let Some(image) = renderer.borrow_mut().take_image()
        {
            let _ = window.drop_image(image);
        }
        let pinned = self.filters_pinned() && !matches!(self.route, Route::ListFilters(_));
        if pinned
            && self
                .filter_editor
                .as_ref()
                .is_none_or(|editor| editor.key != self.view_key())
        {
            self.prepare_filters(false, window, cx);
        }
        let pinned = pinned
            && self
                .filter_editor
                .as_ref()
                .is_some_and(|editor| editor.key == self.view_key());
        if self.filter_input_sync {
            self.filter_input_sync = false;
            if let Route::Search(query, _) = &self.route {
                self.search
                    .update(cx, |input, cx| input.set_value(query.clone(), window, cx));
            }
            self.local_search.update(cx, |input, cx| {
                input.set_value(self.local_filter.clone(), window, cx)
            });
        }
        let connection_page = self.server_check.busy.is_some() || self.route == Route::NoNetwork;
        let visualizer_overlay = (!connection_page)
            .then(|| self.expanded_visualizer_view(window, cx))
            .flatten();
        self.rendering_full_player = Some(false);
        let content = match self.route {
            _ if self.server_check.busy.is_some() => self.server_connection_view(cx),
            Route::NoNetwork => self.server_connection_view(cx),
            Route::Servers => self.servers(cx),
            Route::Collections => self.collections_view(cx),
            Route::CollectionEditor => self.collection_editor_view(cx),
            Route::Settings => self.settings_view(cx),
            Route::PlayerSettings => self.player_settings_view(cx),
            Route::Home => self.home_view(window, cx),
            Route::HomeSettings => self.home_settings_view(cx),
            Route::Random => self.random_view(window, cx),
            Route::FontPicker => self.font_picker(cx),
            Route::PresenceSettings => self.presence_settings(cx),
            Route::QueryFields => self.query_fields_view(cx),
            Route::PlaybackFilters => self.playback_filters_view(window, cx),
            Route::SidebarSettings => self.sidebar_settings_view(cx),
            Route::PanelSettings => self.panel_settings_view(cx),
            Route::UpdateSettings => self.update_settings_view(cx),
            Route::SettingsImport => self.settings_import_view(cx),
            Route::AlbumGroups => self.album_group_settings(cx),
            Route::ListSettings(_) => self.list_settings_view(cx),
            Route::ListFilters(_) => self.filters_view(cx),
            Route::Shortcuts => self.shortcut_view(cx),
            Route::PlaylistFolderEditor(_) => self.playlist_folder_editor(cx),
            Route::TagEditor => self.tag_editor_view(cx),
            Route::Share => self.share_view(cx),
            Route::Outputs => self.output_view(cx),
            Route::Info => self.info_view(cx),
            Route::Visualizer if self.visualizer_display.mounted => Empty.into_any_element(),
            Route::Visualizer => self.visualizer_view(window, cx),
            Route::VisualizerSettings => self.visualizer_settings_view(cx),
            Route::LyricEditor => self.lyric_editor_view(cx),
            Route::LanguagePicker => self.appearance_picker(cx),
            Route::Queue => self.queue_view(cx),
            Route::Lyrics => self.now_playing(window, cx),
            Route::PlaylistEditor | Route::RadioEditor => self.editor(cx),
            _ => self.library(window, cx),
        };
        let content = if self.expanded_album.is_some() {
            div()
                .v_flex()
                .size_full()
                .child(div().flex_1().min_h_0().w_full().child(content))
                .child(self.expanded_album_view(window, cx))
                .into_any_element()
        } else {
            content
        };
        let content = if pinned {
            let app = cx.weak_entity();
            let wide = self.filter_container_width >= cx.theme().font_size * 36.;
            gpui::div()
                .relative()
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .size_full()
                .when(wide, |row| {
                    row.child(
                        gpui::div()
                            .id("pinned-filter-panel")
                            .debug_selector(|| "pinned-filter-panel".into())
                            .flex_shrink_0()
                            .w(px(300. * self.state.settings.zoom))
                            .h_full()
                            .border_r_1()
                            .border_color(cx.theme().border)
                            .p_4()
                            .child(self.filters_view(cx)),
                    )
                })
                .child(gpui::div().flex_1().min_w_0().h_full().child(content))
                .child(
                    canvas(
                        move |bounds, window, cx| {
                            let app = app.clone();
                            window.defer(cx, move |_, cx| {
                                app.update(cx, |view, cx| {
                                    if view.filter_container_width != bounds.size.width {
                                        view.filter_container_width = bounds.size.width;
                                        cx.notify();
                                    }
                                })
                                .ok();
                            });
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .into_any_element()
        } else {
            content
        };
        let content = if self.has_standard_list_header() && !connection_page {
            div()
                .v_flex()
                .size_full()
                .child(self.standard_list_header(cx))
                .child(div().flex_1().min_h_0().w_full().child(content))
                .into_any_element()
        } else {
            content
        };
        let content = gpui::div()
            .id("themed-content")
            .debug_selector(|| "themed-content".into())
            .w_full()
            .max_w(px(self.theme_length("content-max-width", 1800.0)))
            .mx_auto()
            .h_full()
            .min_h_0()
            .child(content)
            .when(true, |div| {
                crate::styles::apply(
                    div,
                    "div",
                    "themed-content",
                    &["fs-main-content-module-main-content-container"],
                    None,
                    cx,
                )
            })
            .into_any_element();
        let content = self.wrap_playback_panels(content, window, cx);
        self.rendering_full_player = Some(true);
        let background = self.player_background_color(cx);
        let motion = window.use_keyed_state("expanded-player-overlay-motion", cx, |_, _| {
            crate::player_layout::OverlayMotion::new(self.full_player, background)
        });
        let (slide, color, mounted, active) = motion.update(cx, |motion, cx| {
            motion.frame(self.full_player, background, cx.background_executor().now())
        });
        if active {
            window.request_animation_frame();
        }
        let player_overlay = (mounted && !connection_page)
            .then(|| self.expanded_player_view(slide, color, window, cx));
        self.rendering_full_player = None;
        let activity = cx.weak_entity();
        gpui::div()
            .id("native-root")
            .role(gpui::Role::Application)
            .aria_label("Feishin")
            .font(crate::fonts::app_font(cx))
            .text_spacing_trim(TextSpacingTrim::Normal)
            .text_align(TextAlign::Start)
            .text_direction(if crate::appearance::rtl(cx) {
                TextDirection::Rtl
            } else {
                TextDirection::Ltr
            })
            .debug_selector(|| "native-root".into())
            .relative()
            .when(
                self.state.settings.fullscreen_auto_open_minutes > 0.,
                |div| {
                    div.child(
                        canvas(
                            |_, _, _| (),
                            move |_, _, window, _| {
                                let moved = activity.clone();
                                window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, cx| {
                                    if phase == DispatchPhase::Capture {
                                        moved.update(cx, |this, _| this.record_activity()).ok();
                                    }
                                });
                                window.on_mouse_event(move |_: &ScrollWheelEvent, phase, _, cx| {
                                    if phase == DispatchPhase::Capture {
                                        activity.update(cx, |this, _| this.record_activity()).ok();
                                    }
                                });
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                },
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.column_resize_origin = None;
                    this.column_resize_preview = None;
                }),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Back),
                cx.listener(|this, _, _, cx| this.back(cx)),
            )
            .on_mouse_down(
                MouseButton::Navigate(NavigationDirection::Forward),
                cx.listener(|this, _, _, cx| this.forward(cx)),
            )
            .on_drag_move(cx.listener(
                |this, event: &DragMoveEvent<crate::lists::ColumnDrag>, _, cx| {
                    let drag = event.drag(cx).clone();
                    this.resize_column(&drag, event.event.position.x, cx);
                },
            ))
            .on_drop(cx.listener(|this, drag: &crate::lists::ColumnDrag, _, cx| {
                this.drop_column(drag, None, cx)
            }))
            .key_context(if self.record_shortcut.is_some() {
                "Feishin RecordingShortcut"
            } else {
                "Feishin"
            })
            .capture_key_down(cx.listener(|this, event, _, cx| {
                this.record_activity();
                this.capture_shortcut(event, cx)
            }))
            .capture_any_mouse_down(cx.listener(|this, _, _, _| this.record_activity()))
            .on_action(
                cx.listener(|this, action: &crate::shortcuts::RunShortcut, window, cx| {
                    this.run_shortcut(action.action, window, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &crate::shortcuts::SelectListAll, window, cx| {
                    view.toggle_list_selection(window, cx)
                }),
            )
            .on_action(
                cx.listener(|view, _: &gpui_component::input::SelectAll, window, cx| {
                    view.toggle_list_selection(window, cx)
                }),
            )
            .on_action(cx.listener(
                |view, _: &crate::shortcuts::RemoveQueueSelection, window, cx| {
                    if let Some(scope) = view.focused_list(window, cx)
                        && matches!(
                            scope,
                            crate::shortcuts::ListScope::Queue
                                | crate::shortcuts::ListScope::SideQueue
                        )
                    {
                        view.remove_queue_selection(
                            scope == crate::shortcuts::ListScope::SideQueue,
                            cx,
                        );
                    }
                },
            ))
            .track_focus(&self.focus)
            .v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_base()
            .text_align(if crate::appearance::rtl(cx) {
                TextAlign::Right
            } else {
                TextAlign::Left
            })
            .when(true, |div| {
                crate::styles::apply(div, "body", "native-root", &[], None, cx)
            })
            // Single-line inputs emit PressEnter, then propagate the action.
            // Consume it here so Return cannot be inserted as a newline.
            .on_action(|_: &gpui_component::input::Enter, _, _| {})
            .on_action(
                cx.listener(|_, _: &ToggleFullScreenWindow, window, _| window.toggle_fullscreen()),
            )
            .on_action(
                cx.listener(|this, _: &ToggleFullScreenPlayer, _, cx| this.toggle_full_player(cx)),
            )
            .on_action(cx.listener(|this, _: &LeaveFullPlayer, window, cx| {
                if this.visualizer_display.expanded {
                    if window.is_fullscreen() {
                        window.toggle_fullscreen();
                        cx.notify();
                    } else {
                        this.toggle_expanded_visualizer(window, cx);
                    }
                } else if this.full_player {
                    this.full_player = false;
                    this.persist_player_modes();
                    cx.notify();
                } else if this.route == Route::Random {
                    this.close_random_form(cx);
                } else if this.route == Route::SettingsImport {
                    this.cancel_settings_import(cx);
                } else if window.is_fullscreen() {
                    window.toggle_fullscreen();
                }
            }))
            .on_action(cx.listener(|this, _: &TogglePlay, _, cx| this.toggle_play(cx)))
            .on_action(cx.listener(|this, _: &NextTrack, _, cx| this.next(false, cx)))
            .on_action(cx.listener(|this, _: &PreviousTrack, _, cx| this.previous(cx)))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.back(cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.open_command_palette(window, cx)
            }))
            .when(
                !window.is_fullscreen()
                    && matches!(self.window_bar_style_active.as_str(), "macos" | "windows"),
                |div| div.child(self.native_titlebar(window, cx)),
            )
            .child(
                gpui::div()
                    .relative()
                    .overflow_hidden()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(self.sidebar_view(cx))
                    .child(
                        gpui::div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .relative()
                            .p_6()
                            .when(pinned, |div| div.px_0())
                            .gap_4()
                            .when(
                                matches!(self.route, Route::Home | Route::Settings)
                                    || self.has_standard_list_header()
                                    || self.has_detail_page_header(),
                                |div| div.p_0().gap_0(),
                            )
                            .when(
                                !matches!(self.route, Route::Home | Route::Settings)
                                    && !self.has_standard_list_header()
                                    && !self.has_detail_page_header(),
                                |div| {
                                    div.child(
                                        gpui::div()
                                            .h_flex()
                                            .when(pinned, |div| div.px_6())
                                            .when(crate::appearance::rtl(cx), |div| {
                                                div.flex_row_reverse()
                                            })
                                            .gap_3()
                                            .child(
                                                crate::styles::button("back", cx)
                                                    .ghost()
                                                    .icon(IconName::ArrowLeft)
                                                    .tooltip(self.tr("Back"))
                                                    .disabled(self.history.is_empty())
                                                    .on_click(
                                                        cx.listener(|this, _, _, cx| this.back(cx)),
                                                    ),
                                            )
                                            .child(
                                                gpui::div()
                                                    .text_2xl()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .flex_1()
                                                    .child(self.tr(&self.route.title())),
                                            )
                                            .child(
                                                gpui::div()
                                                    .w(px(300.0))
                                                    .debug_selector(|| "global-search-input".into())
                                                    .child(crate::styles::input(&self.search, cx)),
                                            )
                                            .child(
                                                crate::styles::button("command-palette", cx)
                                                    .debug_selector(|| {
                                                        "command-palette-open".into()
                                                    })
                                                    .ghost()
                                                    .icon(IconName::Search)
                                                    .tooltip(self.tr("page.globalSearch.title"))
                                                    .on_click(cx.listener(
                                                        |this, _, window, cx| {
                                                            this.open_command_palette(window, cx)
                                                        },
                                                    )),
                                            )
                                            .when(true, |div| {
                                                div.child(
                                                    gpui::div()
                                                        .debug_selector(|| {
                                                            "toggle-playback-panels".into()
                                                        })
                                                        .child(
                                                            crate::styles::button(
                                                                "toggle-playback-panels",
                                                                cx,
                                                            )
                                                            .ghost()
                                                            .label(crate::appearance::text(
                                                                "Panels", cx,
                                                            ))
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.toggle_playback_panels(cx)
                                                                },
                                                            )),
                                                        ),
                                                )
                                            }),
                                    )
                                },
                            )
                            .when(!self.library_fetches.is_empty(), |div| {
                                div.child(
                                    gpui::div()
                                        .h_flex()
                                        .gap_3()
                                        .items_center()
                                        .p_2()
                                        .child(format!(
                                            "Fetching tracks ({})",
                                            self.library_fetches.len()
                                        ))
                                        .child(
                                            crate::styles::button("cancel-library-fetch", cx)
                                                .debug_selector(|| "cancel-library-fetch".into())
                                                .label(self.tr("Cancel"))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.cancel_library_fetches(cx)
                                                })),
                                        ),
                                )
                            })
                            .when(!self.message.is_empty(), |div| {
                                div.child(
                                    gpui::div()
                                        .id("message-banner")
                                        .role(gpui::accesskit::Role::Status)
                                        .aria_label(self.message.clone())
                                        .aria_live(gpui::accesskit::Live::Polite)
                                        .h_flex()
                                        .when(crate::appearance::rtl(cx), |div| {
                                            div.flex_row_reverse()
                                        })
                                        .gap_2()
                                        .p_3()
                                        .bg(cx.theme().secondary)
                                        .rounded_md()
                                        .child(
                                            gpui::div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_sm()
                                                .child(self.message.clone()),
                                        )
                                        .child(
                                            crate::styles::button("dismiss-message", cx)
                                                .ghost()
                                                .icon(IconName::Close)
                                                .tooltip(crate::appearance::text("Dismiss", cx))
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.message.clear();
                                                    cx.notify();
                                                })),
                                        ),
                                )
                            })
                            .child(content),
                    )
                    .children(player_overlay)
                    .children(visualizer_overlay),
            )
            .when(
                !(self.visualizer_display.mounted && window.is_fullscreen()),
                |div| div.child(self.player_bar(window, cx)),
            )
            .child(self.queue_drawer_view(cx))
            .children(gpui_component::Root::render_dialog_layer(window, cx))
    }
}

#[cfg(test)]
mod inline_title_tests {
    use super::track_title;
    use feishin_core::model::{Item, ItemKind};
    use gpui::{
        AppContext, Context, IntoElement, Render, TestAppContext, TextDirection, TextLayout,
        TextStyleRefinement, Window, div, prelude::*, px,
    };
    use gpui_component::ActiveTheme;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn inline_title_fonts_and_wrap_match_original_components(cx: &mut TestAppContext) {
        struct Probe {
            case: serde_json::Value,
            layout: Rc<RefCell<Option<TextLayout>>>,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let case = &self.case;
                let zoom = case["zoom"].as_f64().unwrap() as f32;
                let rem = case["rem"].as_f64().unwrap() as f32 * zoom;
                let style = TextStyleRefinement {
                    font_family: Some("Inter".into()),
                    font_size: Some(px(rem * 14. / 16.).into()),
                    line_height: Some(px(rem * 18. / 16.).into()),
                    text_direction: Some(if case["rtl"] == true {
                        TextDirection::Rtl
                    } else {
                        TextDirection::Ltr
                    }),
                    ..Default::default()
                };
                let item = Item {
                    name: case["title"].as_str().unwrap().into(),
                    kind: ItemKind::Song,
                    column_data: [(
                        "trackSubtitle".into(),
                        case["subtitle"].as_str().unwrap().into(),
                    )]
                    .into(),
                    ..Default::default()
                };
                let mut title = track_title(&item, true, cx);
                if case["column"] == 2 {
                    title = title.inherit_subtitle_line_height();
                }
                title.mark = match case["status"].as_str() {
                    Some("EXPLICIT") => Some(("🅴", "Explicit")),
                    Some("CLEAN") => Some(("🅲", "Clean")),
                    _ => None,
                };
                let text =
                    window.with_text_style(Some(style.clone()), |window| title.styled(window, cx));
                self.layout.replace(Some(text.layout().clone()));
                let mut container = div()
                    .w(px(case["availableWidth"].as_f64().unwrap() as f32 * zoom))
                    .whitespace_normal();
                container.style().text = Some(style);
                container
                    .when_some(case["clamp"].as_u64(), |div, clamp| {
                        div.line_clamp(clamp as usize)
                    })
                    .child(text)
            }
        }
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/inline-title-layout-linux-reference.json")
        } else {
            include_str!("../assets/inline-title-layout-darwin-reference.json")
        })
        .unwrap();
        let cases = reference["cases"].as_array().unwrap();
        let layout = Rc::new(RefCell::new(None));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|_| Probe {
                case: cases[0].clone(),
                layout: layout.clone(),
            });
            captured.replace(Some(view.clone()));
            gpui_component::Root::new(view, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        for case in cases {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            cx.update(|window, cx| {
                gpui_component::Theme::global_mut(cx).font_size =
                    px(case["rem"].as_f64().unwrap() as f32 * zoom);
                window.set_rem_size(cx.theme().font_size);
                window.text_system().set_optical_size_scale(zoom);
                probe.update(cx, |view, cx| {
                    view.case = case.clone();
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            let saved = layout.borrow();
            let wrapped = saved.as_ref().unwrap().line_layout_for_index(0).unwrap();
            let rows = wrapped.unwrapped_layout.fragments(&wrapped.wrap_boundaries);
            let layout = &wrapped.unwrapped_layout;
            let cells = case["cells"].as_array().unwrap();
            let line_height = case["rem"].as_f64().unwrap() * 18. / 16.;
            let mut centers = Vec::<f64>::new();
            for expected in cells {
                let start = expected["start"].as_u64().unwrap() as usize;
                if expected["text"].as_str().unwrap().trim().is_empty() {
                    continue;
                }
                let row = rows
                    .iter()
                    .position(|row| row.range.contains(&start))
                    .unwrap();
                let center = expected["bounds"][1].as_f64().unwrap()
                    + expected["bounds"][3].as_f64().unwrap() * 0.5;
                let expected_row = centers
                    .iter()
                    .position(|known| (known - center).abs() < line_height * 0.5)
                    .unwrap_or_else(|| {
                        centers.push(center);
                        centers.len() - 1
                    });
                assert_eq!(row, expected_row, "Wrapped character {expected}; {case}");
                let run = layout
                    .runs
                    .iter()
                    .find(|run| run.glyphs.iter().any(|glyph| glyph.index == start))
                    .unwrap();
                assert!(
                    (f32::from(run.font_size) - expected["font"].as_f64().unwrap() as f32 * zoom)
                        .abs()
                        < 0.001,
                    "Font size: {expected}; {case}"
                );
                let cell = layout
                    .cells
                    .iter()
                    .find(|cell| cell.range.start == start)
                    .unwrap();
                let metrics = wrapped.row_metrics(row, px(line_height as f32 * zoom));
                let hit = gpui::point(
                    (cell.start_x + cell.end_x) / 2. - rows[row].x,
                    metrics.offset + metrics.height / 2.,
                );
                assert_eq!(
                    wrapped.index_for_position(hit, px(line_height as f32 * zoom)),
                    Ok(start),
                    "Inline row hit: {case}"
                );
                let selected =
                    wrapped.selection_rects(cell.range.clone(), px(line_height as f32 * zoom));
                assert!(
                    selected.iter().all(|rect| rect.origin.y == metrics.offset
                        && rect.size.height == metrics.height),
                    "Inline selection: {case}"
                );
                // Source Range bounds include caret-edge quantization; the badge's
                // right padding belongs to its inline box, outside its glyph range.
                let padding = if start == 0 && case["status"].is_string() {
                    case["rem"].as_f64().unwrap() as f32 * 0.25 * zoom
                } else {
                    0.
                };
                let actual = f32::from((cell.end_x - cell.start_x).abs()) - padding;
                let wanted = expected["bounds"][2].as_f64().unwrap() as f32 * zoom;
                assert!(
                    (actual - wanted).abs() < 0.13 * zoom,
                    "Advance {actual} vs {wanted}; {expected}; {case}"
                );
            }
        }
        // Clamping paints an ellipsis using the visible title run, without letting
        // a hidden subtitle change the final row's height.
        let mut case = cases
            .iter()
            .find(|case| {
                case["column"] == 0
                    && case["rem"] == 16
                    && case["zoom"] == 1
                    && case["rtl"] == false
                    && case["size"] == "compact"
                    && case["status"] == "EXPLICIT"
            })
            .unwrap()
            .clone();
        case["clamp"] = 1.into();
        let original_len = case["text"].as_str().unwrap().len();
        cx.update(|window, cx| {
            gpui_component::Theme::global_mut(cx).font_size = px(16.);
            window.set_rem_size(px(16.));
            window.text_system().set_optical_size_scale(1.);
            probe.update(cx, |view, cx| {
                view.case = case;
                cx.notify();
            });
            window.refresh();
        });
        cx.run_until_parked();
        let saved = layout.borrow();
        let wrapped = saved.as_ref().unwrap().line_layout_for_index(0).unwrap();
        assert!(wrapped.len() < original_len);
        assert!(wrapped.wrap_boundaries.is_empty());
        let ellipsis = cx.update(|window, _| {
            let run = gpui::TextStyle {
                font_family: "Inter".into(),
                ..Default::default()
            }
            .to_run("…".len());
            window
                .text_system()
                .shape_line("…".into(), px(14.), &[run], None)
                .runs[0]
                .glyphs[0]
                .id
        });
        assert!(
            wrapped.runs().iter().any(|run| run.font_size == px(14.)
                && run.glyphs.iter().any(|glyph| glyph.id == ellipsis))
        );
    }
}
