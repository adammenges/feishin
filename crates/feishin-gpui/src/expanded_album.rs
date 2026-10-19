use crate::{app::Feishin, details::AlbumTracks, scroll::Scrollbar};
use feishin_core::{
    images::Placement,
    lists::{self, Align},
    model::{Item, ItemKind, Sort},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Icon, Sizable, StyledExt, menu::ContextMenuExt};
use std::collections::BTreeSet;

fn preview_target(tag: &str, classes: &str) -> feishin_core::styles::Target {
    let classes = classes
        .split_whitespace()
        .map(|class| format!("fs-expanded-album-list-item-module-{class}"))
        .collect::<Vec<_>>();
    feishin_core::styles::Target::new(
        tag,
        "",
        &classes.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn preview_style<T: Styled>(element: T, tag: &str, classes: &str, parents: &[&str], cx: &App) -> T {
    let Some(styles) = cx.try_global::<crate::styles::NativeStyles>() else {
        return element;
    };
    element.refine_style(&styles.style_in(
        preview_target(tag, classes),
        parents.iter().map(|class| preview_target("div", class)),
        f32::from(cx.theme().font_size),
    ))
}

pub struct ExpandedAlbum {
    pub item: Item,
    pub tracks: AlbumTracks,
    pub selected: BTreeSet<usize>,
    pub anchor: Option<usize>,
    pub request: uuid::Uuid,
    pub scroll: UniformListScrollHandle,
    pub background: Option<crate::player_display::Background>,
    pub background_key: String,
    pub width: Pixels,
}

impl ExpandedAlbum {
    fn is_dark(&self) -> bool {
        self.background.as_ref().is_none_or(|art| {
            let rgba: Rgba = art.color.into();
            0.299 * rgba.r + 0.587 * rgba.g + 0.114 * rgba.b < 128. / 255.
        })
    }
}

impl Feishin {
    pub fn toggle_expanded_album(&mut self, item: Item, cx: &mut Context<Self>) {
        if item.kind != ItemKind::Album {
            return;
        }
        if self
            .expanded_album
            .as_ref()
            .is_some_and(|current| Self::album_key(&current.item) == Self::album_key(&item))
        {
            self.expanded_album = None;
            cx.notify();
            return;
        }
        self.open_expanded_album(item, cx);
    }

    fn open_expanded_album(&mut self, item: Item, cx: &mut Context<Self>) {
        let request = uuid::Uuid::new_v4();
        let local = !item.playlist_songs.is_empty();
        let tracks = if local {
            AlbumTracks {
                items: item.playlist_songs.clone(),
                ..Default::default()
            }
        } else {
            AlbumTracks {
                loading: true,
                ..Default::default()
            }
        };
        self.expanded_album = Some(ExpandedAlbum {
            item: item.clone(),
            tracks,
            selected: BTreeSet::new(),
            anchor: None,
            request,
            scroll: UniformListScrollHandle::default(),
            background: None,
            background_key: String::new(),
            width: px(0.),
        });
        self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
        cx.notify();
        if local {
            return;
        }
        let Some(api) = self.sessions.get(&item.server_id).cloned() else {
            self.receive_expanded_album_tracks(
                request,
                Err(anyhow::anyhow!("Sign in to this album's server")),
                cx,
            );
            return;
        };
        let task = cx.background_executor().spawn(async move {
            let mut tracks = api.all_tracks(&item)?;
            lists::sort_items(&mut tracks, Sort::Track, false);
            Ok(tracks)
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                view.receive_expanded_album_tracks(request, result, cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn receive_expanded_album_tracks(
        &mut self,
        request: uuid::Uuid,
        result: anyhow::Result<Vec<Item>>,
        cx: &mut Context<Self>,
    ) {
        let Some(album) = self
            .expanded_album
            .as_mut()
            .filter(|album| album.request == request)
        else {
            return;
        };
        album.tracks = match result {
            Ok(items) => AlbumTracks {
                items,
                ..Default::default()
            },
            Err(error) => {
                log::warn!("Expanded album tracks failed: {error}");
                AlbumTracks {
                    error: Some(error.to_string()),
                    ..Default::default()
                }
            }
        };
        cx.notify();
    }

    fn sync_expanded_album_art(&mut self, cx: &mut Context<Self>) {
        let Some(album) = self.expanded_album.as_ref() else {
            return;
        };
        let key = self.cover_cache_key(&album.item, Placement::ItemCard);
        if !self.covers.contains_key(&key)
            || (self.state.settings.blur_explicit_images
                && album.item.explicit()
                && !self.censored_covers.contains_key(&key))
        {
            let item = album.item.clone();
            self.fetch_item_cover_at(&item, Placement::ItemCard, cx);
        }
        let album = self.expanded_album.as_ref().unwrap();
        let Some(source) = self
            .artwork_path_at(&album.item, Placement::ItemCard)
            .cloned()
        else {
            let album = self.expanded_album.as_mut().unwrap();
            album.background = None;
            album.background_key.clear();
            return;
        };
        let key = format!(
            "expanded-v2-{}-{:x}-{}",
            self.cover_cache_key(&album.item, Placement::ItemCard),
            feishin_core::api::cover_hash(&source.to_string_lossy()),
            self.state.cache_revision
        );
        if album.background_key == key {
            return;
        }
        let request = album.request;
        let revision = self.state.cache_revision;
        let destination = self
            .data_dir
            .join("detail-backgrounds")
            .join(format!("{key}.png"));
        let stale = destination.clone();
        let album = self.expanded_album.as_mut().unwrap();
        album.background_key = key.clone();
        album.background = None;
        let task = cx.background_executor().spawn(async move {
            crate::player_display::prepare_expanded_background(&source, &destination)
        });
        cx.spawn(async move |view, cx| {
            let result = task.await;
            view.update(cx, |view, cx| {
                if view.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale);
                    return;
                }
                let Some(album) = view
                    .expanded_album
                    .as_mut()
                    .filter(|album| album.request == request && album.background_key == key)
                else {
                    return;
                };
                match result {
                    Ok(background) => album.background = Some(background),
                    Err(error) => log::debug!("Expanded album artwork preparation failed: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn expanded_album_track(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(album) = self.expanded_album.as_ref() else {
            return div().into_any_element();
        };
        let Some(song) = album.tracks.items.get(index) else {
            return div().into_any_element();
        };
        let items = if album.selected.contains(&index) {
            album
                .selected
                .iter()
                .map(|index| album.tracks.items[*index].clone())
                .collect()
        } else {
            vec![song.clone()]
        };
        let dragged = crate::queue::Dragged {
            items: items.clone(),
            keys: vec![],
            sidebar_playlist: false,
            playlist_source: None,
        };
        let request = album.request;
        let track_class = if album.is_dark() {
            "tracks dark"
        } else {
            "tracks"
        };
        div()
            .id(("expanded-album-track", index))
            .debug_selector(move || format!("expanded-album-track-{index}"))
            .role(Role::Row)
            .aria_label(song.name.clone())
            .aria_selected(album.selected.contains(&index))
            .h_flex()
            .items_center()
            .h(rems(1.5))
            .text_size(rems(13. / 16.))
            .line_height(rems(1.))
            .gap_2()
            .px_2()
            .cursor_pointer()
            .when(album.selected.contains(&index), |div| {
                div.bg(hsla(0., 0., 0., 0.14))
            })
            .hover(|style| style.bg(hsla(0., 0., 0., 0.2)))
            .map(|div| {
                preview_style(
                    div,
                    "div",
                    if album.selected.contains(&index) {
                        "track-row row-selected"
                    } else {
                        "track-row"
                    },
                    &["container", "expanded", "content", track_class],
                    cx,
                )
            })
            .child(
                div()
                    .w(px(55. * self.state.settings.zoom))
                    .flex_shrink_0()
                    .child(format!("{} - {}", song.disc, song.track))
                    .map(|div| {
                        preview_style(
                            div,
                            "div",
                            "track-number",
                            &["container", "expanded", "content", track_class, "track-row"],
                            cx,
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(crate::ui::item_title(
                        song,
                        song.name.clone(),
                        Align::Start,
                        cx,
                    ))
                    .map(|div| {
                        preview_style(
                            div,
                            "div",
                            "track-name",
                            &["container", "expanded", "content", track_class, "track-row"],
                            cx,
                        )
                    }),
            )
            .child(
                div()
                    .w(px(55. * self.state.settings.zoom))
                    .flex_shrink_0()
                    .text_right()
                    .child(feishin_core::model::duration_label(song.duration))
                    .map(|div| {
                        preview_style(
                            div,
                            "div",
                            "track-duration",
                            &["container", "expanded", "content", track_class, "track-row"],
                            cx,
                        )
                    }),
            )
            .on_drag(dragged, crate::context_preview::drag_constructor(cx))
            .on_click(cx.listener(move |view, event: &ClickEvent, _, cx| {
                let Some(album) = view
                    .expanded_album
                    .as_mut()
                    .filter(|album| album.request == request)
                else {
                    return;
                };
                if index >= album.tracks.items.len() {
                    return;
                }
                if event.click_count() == 2 {
                    let items = album.tracks.items.clone();
                    let song = items[index].clone();
                    view.add_tracks_from(items, Enqueue::Now, Some(&song), cx);
                    return;
                }
                let modifiers = event.modifiers();
                if modifiers.shift {
                    let anchor = album.anchor.unwrap_or(index);
                    if !(modifiers.control || modifiers.platform) {
                        album.selected.clear();
                    }
                    album.selected.extend(anchor.min(index)..=anchor.max(index));
                } else if modifiers.control || modifiers.platform {
                    if !album.selected.insert(index) {
                        album.selected.remove(&index);
                    }
                    album.anchor = Some(index);
                } else {
                    album.selected.clear();
                    album.selected.insert(index);
                    album.anchor = Some(index);
                }
                cx.notify();
            }))
            .context_menu(self.item_menu_builder(song, items, false, cx))
            .into_any_element()
    }

    fn play_expanded_album(
        &mut self,
        request: uuid::Uuid,
        mode: Enqueue,
        shuffle: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(album) = self
            .expanded_album
            .as_ref()
            .filter(|album| album.request == request)
        {
            self.enqueue_items(album.tracks.items.clone(), mode, shuffle, cx);
        }
    }

    fn expanded_play_button(
        &self,
        request: uuid::Uuid,
        id: &'static str,
        mode: Enqueue,
        cx: &mut Context<Self>,
    ) -> gpui_component::button::Button {
        let primary = matches!(mode, Enqueue::Now);
        let mut target =
            feishin_core::styles::Target::new("button", id, &["fs-play-button-module-play-button"]);
        target.classes.push(
            if primary {
                "fs-play-button-module-fill"
            } else {
                "fs-play-button-module-secondary"
            }
            .into(),
        );
        let mut parents = [
            "container",
            "expanded",
            "image-container",
            "play-button-group",
        ]
        .into_iter()
        .map(|class| preview_target("div", class))
        .collect::<Vec<_>>();
        parents.push(feishin_core::styles::Target::new(
            "div",
            "",
            &["fs-play-button-group-module-play-button-group"],
        ));
        crate::styles::play_control(id, mode, self.state.settings.zoom, cx)
            .state_style(crate::styles::state_style_path(target, parents))
            .disabled(
                !self.library_fetches.is_empty()
                    || self
                        .expanded_album
                        .as_ref()
                        .is_none_or(|album| album.tracks.items.is_empty()),
            )
            .on_long_press(cx.listener(move |view, _, _, cx| {
                view.play_expanded_album(request, mode, true, cx);
            }))
            .on_click(cx.listener(move |view, _, _, cx| {
                view.play_expanded_album(request, mode, false, cx);
            }))
    }

    pub fn expanded_album_view(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_expanded_album_art(cx);
        let album = self.expanded_album.as_ref().unwrap();
        let item = &album.item;
        let request = album.request;
        let count = album.tracks.items.len();
        let zoom = self.state.settings.zoom;
        let width = album.width / zoom;
        let art_fraction = if width >= px(1200.) {
            1.
        } else if width >= px(768.) {
            0.9
        } else if width >= px(640.) {
            0.8
        } else {
            0.6
        };
        let track_fraction = if width >= px(1200.) {
            0.4
        } else if width >= px(768.) {
            0.5
        } else {
            0.6
        };
        let background = album
            .background
            .as_ref()
            .map(|art| art.color)
            .unwrap_or(cx.theme().muted_foreground);
        let dark = album.is_dark();
        let foreground = if dark { white() } else { black() };
        let track_class = if dark { "tracks dark" } else { "tracks" };
        let artists = if item.album_artist_names.is_empty() {
            vec![item.album_artist.as_str()]
        } else {
            item.album_artist_names
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        };
        let entity = cx.entity();
        let tracks = div()
            .id("expanded-album-track-area")
            .debug_selector(|| "expanded-album-track-area".into())
            .relative()
            .w(relative(track_fraction))
            .max_w(px(700. * zoom))
            .flex_1()
            .min_h_0()
            .map(|div| {
                preview_style(
                    div,
                    "div",
                    track_class,
                    &["container", "expanded", "content"],
                    cx,
                )
            })
            .child(
                uniform_list("expanded-album-tracks", count, move |range, _, cx| {
                    entity.update(cx, |view, cx| {
                        range
                            .map(|index| view.expanded_album_track(index, cx))
                            .collect()
                    })
                })
                .track_scroll(album.scroll.clone())
                .size_full(),
            )
            .child(Scrollbar::vertical("expanded-album-scroll", &album.scroll));
        let mut content = div()
            .relative()
            .size_full()
            .v_flex()
            .p_3()
            .gap_3()
            .map(|div| preview_style(div, "div", "content", &["container", "expanded"], cx))
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .map(|div| {
                        preview_style(
                            div,
                            "div",
                            "header",
                            &["container", "expanded", "content"],
                            cx,
                        )
                    })
                    .child(
                        div()
                            .pr_6()
                            .text_size(rems(1.25))
                            .line_height(relative(1.3))
                            .font_weight(FontWeight::BOLD)
                            .line_clamp(2)
                            .child(item.name.clone())
                            .map(|div| {
                                preview_style(
                                    div,
                                    "h4",
                                    if dark {
                                        "item-title dark"
                                    } else {
                                        "item-title"
                                    },
                                    &["container", "expanded", "content", "header", "header-title"],
                                    cx,
                                )
                            }),
                    )
                    .child(
                        div()
                            .id("expanded-album-subtitle")
                            .debug_selector(|| "expanded-album-subtitle".into())
                            .h_flex()
                            .flex_wrap()
                            .gap_1()
                            .text_size(rems(14. / 16.))
                            .line_height(rems(18. / 16.))
                            .children(artists.iter().enumerate().flat_map(|(index, name)| {
                                let mut parts = Vec::new();
                                if index > 0 {
                                    parts.push(div().child(" • ").into_any_element());
                                }
                                parts.push(
                                    div()
                                        .id(("expanded-album-artist", index))
                                        .debug_selector(move || {
                                            format!("expanded-album-artist-{index}")
                                        })
                                        .whitespace_nowrap()
                                        .child((*name).to_owned())
                                        .map(|div| {
                                            preview_style(
                                                div,
                                                "p",
                                                if dark {
                                                    "item-subtitle dark"
                                                } else {
                                                    "item-subtitle"
                                                },
                                                &[
                                                    "container",
                                                    "expanded",
                                                    "content",
                                                    "header",
                                                    "item-subtitle",
                                                ],
                                                cx,
                                            )
                                        })
                                        .into_any_element(),
                                );
                                parts
                            }))
                            .map(|div| {
                                preview_style(
                                    div,
                                    "div",
                                    if dark {
                                        "item-subtitle dark"
                                    } else {
                                        "item-subtitle"
                                    },
                                    &["container", "expanded", "content", "header"],
                                    cx,
                                )
                            }),
                    ),
            )
            .when(album.tracks.loading, |div| {
                div.child(self.tr("Loading album tracks..."))
            })
            .when_some(album.tracks.error.as_ref(), |div, error| {
                div.child(error.clone()).child(
                    crate::styles::button("retry-expanded-album", cx)
                        .label(self.tr("Retry"))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if let Some(album) = view
                                .expanded_album
                                .as_ref()
                                .filter(|album| album.request == request)
                            {
                                view.open_expanded_album(album.item.clone(), cx);
                            }
                        })),
                )
            })
            .child(tracks);
        content = content.child(
            div()
                .absolute()
                .top_3()
                .right_3()
                .opacity(0.3)
                .hover(|style| style.opacity(0.6))
                .child(
                    gpui_component::button::Button::new("close-expanded-album")
                        .state_style(crate::styles::state_style_path(
                            preview_target("button", "close-button"),
                            ["container", "expanded", "content", "header", "header-title"]
                                .into_iter()
                                .map(|class| preview_target("div", class))
                                .collect(),
                        ))
                        .debug_selector(|| "close-expanded-album".into())
                        .size(cx.theme().font_size * 2.25)
                        .p_0()
                        .border_0()
                        .shadow_none()
                        .rounded(cx.theme().font_size * 1.125)
                        .child(
                            Icon::default()
                                .path("native-icons/close.svg")
                                .with_size(cx.theme().font_size * 1.125),
                        )
                        .tooltip(self.tr("Close"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.expanded_album = None;
                            cx.notify();
                        })),
                ),
        );
        let measured = cx.entity().downgrade();
        div()
            .id("expanded-album-panel")
            .debug_selector(|| "expanded-album-panel".into())
            .w_full()
            .h(px(300. * zoom))
            .flex_shrink_0()
            .p_2()
            .child(
                div()
                    .id("expanded-album-container")
                    .debug_selector(|| "expanded-album-container".into())
                    .group("expanded-album")
                    .relative()
                    .size_full()
                    .rounded_md()
                    .overflow_hidden()
                    .bg(background)
                    .text_color(foreground)
                    .map(|div| preview_style(div, "div", "container", &[], cx))
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .rounded_md()
                            .overflow_hidden()
                            .map(|div| preview_style(div, "div", "expanded", &["container"], cx))
                            .child(
                                canvas(
                                    move |bounds, window, cx| {
                                        let entity = measured.clone();
                                        window.defer(cx, move |_, cx| {
                                            entity
                                                .update(cx, |view, cx| {
                                                    if let Some(album) = view
                                                        .expanded_album
                                                        .as_mut()
                                                        .filter(|album| album.request == request)
                                                        && album.width != bounds.size.width
                                                    {
                                                        album.width = bounds.size.width;
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
                            .child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .h_full()
                                    .w(relative(0.5))
                                    .map(|div| {
                                        preview_style(
                                            div,
                                            "div",
                                            "image-container",
                                            &["container", "expanded"],
                                            cx,
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("expanded-album-image")
                                            .debug_selector(|| "expanded-album-image".into())
                                            .absolute()
                                            .right_0()
                                            .h_full()
                                            .w(relative(art_fraction))
                                            .map(|div| {
                                                preview_style(
                                                    div,
                                                    "div",
                                                    "background-image",
                                                    &["container", "expanded", "image-container"],
                                                    cx,
                                                )
                                            })
                                            .when_some(album.background.as_ref(), |div, art| {
                                                div.child(
                                                    img(art.image.clone())
                                                        .size_full()
                                                        .object_fit(ObjectFit::Cover),
                                                )
                                            })
                                            .child(div().absolute().inset_0().bg(linear_gradient(
                                                90.,
                                                linear_color_stop(background, 0.),
                                                linear_color_stop(background.opacity(0.), 1.),
                                            )))
                                            .child(
                                                div()
                                                    .absolute()
                                                    .inset_0()
                                                    .backdrop_blur(px(2. * zoom)),
                                            ),
                                    ),
                            )
                            .child(content)
                            .child(
                                div()
                                    .absolute()
                                    .right_0()
                                    .top_0()
                                    .h_full()
                                    .w(relative(0.5))
                                    .h_flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .id("expanded-album-play-buttons")
                                            .debug_selector(|| "expanded-album-play-buttons".into())
                                            .h_flex()
                                            .opacity(0.)
                                            .group_hover("expanded-album", |style| {
                                                style.opacity(1.)
                                            })
                                            .map(|div| {
                                                preview_style(
                                                    div,
                                                    "div",
                                                    "play-button-group",
                                                    &["container", "expanded", "image-container"],
                                                    cx,
                                                )
                                            })
                                            .child(div().h_flex().gap_3().items_center().justify_center().map(|div| {
                                                if let Some(styles) = cx.try_global::<crate::styles::NativeStyles>() {
                                                    div.refine_style(&styles.style_in(feishin_core::styles::Target::new("div", "", &["fs-play-button-group-module-play-button-group"]), ["container", "expanded", "image-container", "play-button-group"].into_iter().map(|class| preview_target("div", class)), f32::from(cx.theme().font_size)))
                                                } else { div }
                                            })
                                            .child(self.expanded_play_button(
                                                request,
                                                "next-expanded-album",
                                                Enqueue::Next,
                                                cx,
                                            ))
                                            .child(self.expanded_play_button(
                                                request,
                                                "play-expanded-album",
                                                Enqueue::Now,
                                                cx,
                                            ))
                                            .child(self.expanded_play_button(
                                                request,
                                                "append-expanded-album",
                                                Enqueue::End,
                                                cx,
                                            ))),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::Feishin;
    use feishin_core::{
        model::{Item, ItemKind},
        storage::State,
    };
    use gpui::{
        AppContext, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, point, px,
    };
    use gpui_component::{ActiveTheme, Root};
    use std::collections::BTreeSet;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn expanded_album_controls_selection_duplicates_and_stale_requests(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-expanded-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.confirm_queue_changes = false;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let view = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(view.clone()));
            Root::new(view, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let songs = [3, 1, 2]
            .into_iter()
            .enumerate()
            .map(|(index, track)| Item {
                id: if index < 2 {
                    "duplicate".into()
                } else {
                    "third".into()
                },
                name: format!("Preview song {index}"),
                kind: ItemKind::Song,
                disc: 1,
                track,
                playlist_position: Some(index),
                duration: 60.,
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let album = Item {
            id: "first".into(),
            name: "Expanded first album".into(),
            album_artist: "Preview artist".into(),
            album_artist_names: vec!["Preview artist".into(), "Guest artist".into()],
            kind: ItemKind::Album,
            playlist_songs: songs.clone(),
            ..Default::default()
        };
        let second = Item {
            id: "second".into(),
            name: "Expanded second album".into(),
            ..album.clone()
        };
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = crate::app::Route::Library(ItemKind::Album);
                view.items = vec![album.clone(), second];
                view.loading = false;
                view.selected.insert(1);
                cx.notify();
            })
        });
        cx.run_until_parked();
        crate::tests::show_card_controls(cx, 0);
        let position = cx.debug_bounds("expand-album-0").unwrap().center();
        cx.simulate_click(position, Modifiers::none());
        cx.run_until_parked();
        let panel = cx.debug_bounds("expanded-album-panel").unwrap();
        assert_eq!(panel.size.height, px(300.));
        assert!(cx.debug_bounds("library-drop").unwrap().bottom() <= panel.top());
        assert_eq!(
            cx.debug_bounds("play-expanded-album").unwrap().size,
            gpui::size(px(48.), px(48.))
        );
        assert_eq!(
            cx.debug_bounds("next-expanded-album").unwrap().size,
            gpui::size(px(32.), px(32.))
        );
        assert_eq!(
            cx.debug_bounds("play-expanded-album-icon").unwrap().size,
            gpui::size(px(16.), px(16.))
        );
        assert_eq!(
            cx.debug_bounds("close-expanded-album").unwrap().size,
            gpui::size(px(36.), px(36.))
        );
        let next = cx.debug_bounds("next-expanded-album").unwrap();
        let play = cx.debug_bounds("play-expanded-album").unwrap();
        let last = cx.debug_bounds("append-expanded-album").unwrap();
        assert_eq!(play.center().y, next.center().y);
        assert_eq!(play.center().y, last.center().y);
        assert_eq!(play.left() - next.right(), px(12.));
        assert_eq!(last.left() - play.right(), px(12.));
        assert!(
            cx.debug_bounds("expanded-album-artist-1").unwrap().left()
                > cx.debug_bounds("expanded-album-artist-0").unwrap().right()
        );
        let container_width = cx
            .debug_bounds("expanded-album-container")
            .unwrap()
            .size
            .width;
        app.read_with(cx, |view, _| {
            let expanded = view.expanded_album.as_ref().unwrap();
            assert_eq!(expanded.width, container_width);
            assert_eq!(
                expanded
                    .tracks
                    .items
                    .iter()
                    .map(|song| song.track)
                    .collect::<Vec<_>>(),
                [3, 1, 2]
            );
            assert_eq!(view.selected, std::collections::HashSet::from([1]));
        });
        #[derive(serde::Deserialize)]
        struct Layout {
            width: f32,
            art_width: f32,
            track_width: f32,
            row_height: f32,
        }
        let layouts: Vec<Layout> = serde_json::from_str(include_str!(
            "../assets/expanded-album-layout-reference.json"
        ))
        .unwrap();
        for layout in layouts {
            let viewport_width = cx.update(|window, _| window.viewport_size().width);
            let container_width = cx
                .debug_bounds("expanded-album-container")
                .unwrap()
                .size
                .width;
            cx.simulate_resize(gpui::size(
                px(layout.width) + viewport_width - container_width,
                px(1100.),
            ));
            cx.run_until_parked();
            let width = cx
                .debug_bounds("expanded-album-container")
                .unwrap()
                .size
                .width;
            assert!((width - px(layout.width)).abs() <= px(1.));
            assert!(
                (cx.debug_bounds("expanded-album-image").unwrap().size.width
                    - px(layout.art_width))
                .abs()
                    <= px(1.)
            );
            assert!(
                (cx.debug_bounds("expanded-album-track-area")
                    .unwrap()
                    .size
                    .width
                    - px(layout.track_width))
                .abs()
                    <= px(1.)
            );
            assert_eq!(
                cx.debug_bounds("expanded-album-track-0")
                    .unwrap()
                    .size
                    .height,
                px(layout.row_height)
            );
        }
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.zoom = 1.5;
                view.apply_theme(window, cx);
            })
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("expanded-album-panel").unwrap().size.height,
            px(450.)
        );
        assert_eq!(
            cx.debug_bounds("expanded-album-track-0")
                .unwrap()
                .size
                .height,
            px(36.)
        );
        assert_eq!(
            cx.debug_bounds("play-expanded-album-icon").unwrap().size,
            gpui::size(px(24.), px(24.))
        );
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.zoom = 1.;
                view.apply_theme(window, cx);
            })
        });
        cx.simulate_resize(gpui::size(px(1024.), px(900.)));
        cx.run_until_parked();
        let artist_width = cx
            .debug_bounds("expanded-album-artist-0")
            .unwrap()
            .size
            .width;
        cx.update(|window, cx| app.update(cx, |view, cx| {
            view.state.settings.css.enabled = true;
            view.state.settings.css.content = ".fs-expanded-album-list-item-module-container .fs-expanded-album-list-item-module-content .fs-expanded-album-list-item-module-tracks .fs-expanded-album-list-item-module-track-row { height: 31px; } .fs-expanded-album-list-item-module-container .fs-expanded-album-list-item-module-item-subtitle { font-size: 28px; } .fs-expanded-album-list-item-module-container .fs-play-button-module-secondary { width: 22px; height: 22px; } .fs-expanded-album-list-item-module-container .fs-play-button-group-module-play-button-group { gap: 7px; }".into();
            view.apply_theme(window, cx);
            cx.notify();
        }));
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("expanded-album-track-0")
                .unwrap()
                .size
                .height,
            px(31.)
        );
        assert!(
            (cx.debug_bounds("expanded-album-artist-0")
                .unwrap()
                .size
                .width
                - artist_width * 2.)
                .abs()
                < px(2.)
        );
        let next = cx.debug_bounds("next-expanded-album").unwrap();
        let play = cx.debug_bounds("play-expanded-album").unwrap();
        assert_eq!(next.size, gpui::size(px(22.), px(22.)));
        assert_eq!(play.left() - next.right(), px(7.));
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.css = Default::default();
                view.apply_theme(window, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("expanded-album-track-0")
                .unwrap()
                .size
                .height,
            px(24.)
        );
        for (index, modifiers) in [
            (0, Modifiers::none()),
            (
                2,
                Modifiers {
                    shift: true,
                    ..Default::default()
                },
            ),
            (
                1,
                Modifiers {
                    platform: true,
                    ..Default::default()
                },
            ),
        ] {
            let position = cx
                .debug_bounds(&format!("expanded-album-track-{index}"))
                .unwrap()
                .center();
            cx.simulate_click(position, modifiers);
            cx.run_until_parked();
        }
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.expanded_album.as_ref().unwrap().selected,
                BTreeSet::from([0, 2])
            );
            assert_eq!(view.selected, std::collections::HashSet::from([1]));
        });
        let row = cx.debug_bounds("expanded-album-track-0").unwrap().center();
        cx.simulate_mouse_down(row, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            row + point(px(24.), px(0.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(cx.update(|_, cx| cx.has_active_drag()));
        assert!(cx.debug_bounds("drag-preview-count").is_some());
        cx.simulate_mouse_up(row, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        for id in ["next-expanded-album", "append-expanded-album"] {
            let position = cx.debug_bounds(id).unwrap().center();
            cx.simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
            cx.simulate_click(position, Modifiers::none());
            cx.run_until_parked();
        }
        app.read_with(cx, |view, _| {
            let queue = view.state.queue.ordered();
            assert_eq!(
                queue
                    .iter()
                    .map(|entry| entry.item.track)
                    .collect::<Vec<_>>(),
                [3, 1, 2, 3, 1, 2]
            );
            assert_eq!(
                queue
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<BTreeSet<_>>()
                    .len(),
                6
            );
        });
        let position = cx.debug_bounds("append-expanded-album").unwrap().center();
        cx.simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(499));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let (painted, color, display_scale) = cx.update(|window, cx| {
            (
                window.painted_lines_and_quads_for_test(),
                cx.theme().primary,
                f64::from(window.scale_factor()),
            )
        });
        let circle = painted
            .into_iter()
            .find(|(bounds, actual, rounded, _)| {
                *rounded
                    && actual.h == color.h
                    && actual.s == color.s
                    && actual.l == color.l
                    && (f64::from(bounds.center().x) - f64::from(position.x) * display_scale).abs()
                        < 1.
                    && (f64::from(bounds.center().y) - f64::from(position.y) * display_scale).abs()
                        < 1.
            })
            .expect("Visible preview hold feedback");
        let original: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/play-button-animation-reference.json"
        ))
        .unwrap();
        let opacity = original
            .iter()
            .find(|frame| frame["milliseconds"] == 499)
            .unwrap()["opacity"]
            .as_f64()
            .unwrap() as f32;
        assert!((circle.1.a - opacity).abs() < 0.00001, "{circle:?}");
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            6
        );
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            9
        );
        cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            9
        );
        cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(
            panel.origin - point(px(0.), px(20.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(600));
        cx.run_until_parked();
        cx.simulate_mouse_up(
            panel.origin - point(px(0.), px(20.)),
            MouseButton::Left,
            Modifiers::none(),
        );
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            9
        );
        let position = cx.debug_bounds("expanded-album-track-1").unwrap().center();
        cx.simulate_event(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_event(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 2,
            ..Default::default()
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.entries.len(), 3);
            assert_eq!(
                view.state.queue.current().unwrap().item.playlist_position,
                Some(1)
            );
            assert!(!view.playing);
        });
        let old = app.read_with(cx, |view, _| view.expanded_album.as_ref().unwrap().request);
        cx.simulate_resize(gpui::size(px(1024.), px(1200.)));
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                let row = 1 / view.library_grid_layout(cx).columns;
                view.library_scroll
                    .scroll_to_item(row, gpui::ScrollStrategy::Bottom);
                cx.notify();
            })
        });
        cx.run_until_parked();
        crate::tests::show_card_controls(cx, 1);
        let position = cx.debug_bounds("expand-album-1").unwrap().center();
        assert!(
            cx.debug_bounds("library-list-rows")
                .unwrap()
                .contains(&position),
            "Expand {position:?}, art {:?}, viewport {:?}",
            cx.debug_bounds("card-open-1"),
            cx.debug_bounds("library-list-rows")
        );
        cx.simulate_click(position, Modifiers::none());
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.receive_expanded_album_tracks(
                    old,
                    Err(anyhow::anyhow!("obsolete response")),
                    cx,
                );
                let expanded = view.expanded_album.as_ref().unwrap();
                assert_eq!(expanded.item.id, "second");
                assert!(expanded.tracks.error.is_none());
                assert_eq!(expanded.tracks.items.len(), 3);
                view.route = crate::app::Route::Info;
                cx.notify();
            })
        });
        cx.run_until_parked();
        let position = cx.debug_bounds("close-expanded-album").unwrap().center();
        cx.simulate_click(position, Modifiers::none());
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| view.expanded_album.is_none()));
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = crate::app::Route::Library(ItemKind::Album);
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("library-drop").unwrap().size.height > px(300.));
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.toggle_expanded_album(album.clone(), cx);
                view.toggle_expanded_album(album, cx);
                view.receive_expanded_album_tracks(old, Ok(songs), cx);
                assert!(view.expanded_album.is_none());
            })
        });
        std::fs::remove_dir_all(dir).unwrap();
    }
}
