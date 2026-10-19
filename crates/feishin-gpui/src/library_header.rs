use crate::list_header::action;
use feishin_core::{
    model::{Item, ItemKind, ServerType},
    queue::Enqueue,
    styles::Target,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    menu::DropdownMenu,
};

pub(crate) fn type_label(kind: feishin_core::model::ItemKind, language: &str) -> String {
    use feishin_core::model::ItemKind;
    let key = match kind {
        ItemKind::Album => "entity.album",
        ItemKind::AlbumArtist => "entity.albumArtist",
        ItemKind::Artist => "entity.artist",
        ItemKind::Playlist => "entity.playlist",
        ItemKind::Song => "entity.track",
        _ => "common.unknown",
    };
    feishin_core::i18n::translate_with(language, key, &serde_json::json!({"count": 1}))
        .unwrap_or_else(|| feishin_core::i18n::text(language, "common.unknown"))
}

pub(crate) struct Layout {
    wide: bool,
    large: bool,
    compact: bool,
    font: Pixels,
    zoom: f32,
    rtl: bool,
}

impl Layout {
    pub fn new(width: Pixels, compact: bool, cx: &App) -> Self {
        let font = cx.theme().font_size;
        Self {
            wide: width >= font * 48.,
            large: width >= font * 75.,
            compact,
            font,
            zoom: cx
                .try_global::<crate::styles::NativeStyles>()
                .map_or(1., |styles| styles.zoom()),
            rtl: crate::appearance::rtl(cx),
        }
    }

    pub fn artwork_size(&self) -> Pixels {
        px(if !self.wide || self.large && !self.compact {
            250.
        } else if self.compact {
            200.
        } else {
            225.
        } * self.zoom)
    }

    pub fn frame(&self) -> Div {
        div()
            .relative()
            .debug_selector(|| "library-detail-header".into())
            .v_flex()
            .items_center()
            .w_full()
            .flex_shrink_0()
            .gap(self.font * 0.75)
            .py(self.font * if self.compact { 0.75 } else { 2. })
            .px(self.font * if self.compact { 0.25 } else { 1. })
            .when(self.wide, |frame| {
                frame
                    .flex_row()
                    .when(self.rtl, |frame| frame.flex_row_reverse())
                    .items_end()
                    .px(self.font
                        * if self.compact {
                            if self.large { 0.75 } else { 0.5 }
                        } else {
                            2.
                        })
                    .when(!self.compact, |frame| {
                        frame.min_h(px(340. * self.zoom)).pt(self.font * 5.)
                    })
            })
    }

    pub fn metadata(&self) -> Div {
        div()
            .debug_selector(|| "library-detail-metadata".into())
            .v_flex()
            .gap(self.font * 0.5)
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .text_center()
            .when(self.wide, |metadata| {
                metadata
                    .w_auto()
                    .flex_1()
                    .map(|mut metadata| {
                        metadata.style().align_self = Some(AlignSelf::Stretch);
                        metadata
                    })
                    .items_start()
                    .justify_end()
                    .text_left()
            })
    }

    pub fn item_type(&self, label: String) -> Div {
        div()
            .debug_selector(|| "library-detail-type".into())
            .text_size(self.font * 0.875)
            .line_height(self.font * 1.125)
            .font_weight(FontWeight::SEMIBOLD)
            .child(label)
    }

    pub fn metadata_row(&self) -> Div {
        div()
            .h_flex()
            .flex_wrap()
            .w_full()
            .min_w_0()
            .justify_center()
            .gap(self.font * 0.25)
            .text_size(self.font * 0.875)
            .line_height(self.font * 1.125)
            .when(self.wide, |row| row.justify_start())
            .when(self.rtl, |row| row.flex_row_reverse())
    }

    pub fn title(&self, title: String, window: &Window) -> Div {
        let size = feishin_core::home::feature_title_size(
            &title,
            f32::from(window.viewport_size().width),
            f32::from(self.font),
        );
        div()
            .debug_selector(|| "library-detail-title".into())
            .min_w_0()
            .max_w_full()
            .text_size(px(size))
            .line_height(px((size * 1.2 * 64.).round() / 64.))
            .font_weight(FontWeight::EXTRA_BOLD)
            .text_wrap(TextWrap::KeepAll)
            .child(title)
    }
}

impl crate::app::Feishin {
    pub(crate) fn has_detail_page_header(&self) -> bool {
        matches!(&self.route, crate::app::Route::Detail(item)
            if matches!(item.kind, ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Playlist))
    }

    pub(crate) fn detail_header_menu(
        &self,
        item: &Item,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> Div {
        let rem = cx.theme().font_size;
        let icon_button = |id, icon: &str, label, cx: &App| {
            let mut target = Target::new(
                "button",
                id,
                &["fs-action-icon-module-root", "mantine-ActionIcon-root"],
            );
            target
                .attributes
                .insert("data-variant".into(), "transparent".into());
            action(id, icon, label, cx)
                .size(rem * 3.125)
                .state_style(crate::styles::state_style_path(target, vec![]))
        };
        let play = div()
            .h_flex()
            .min_w_0()
            .gap(rem * 0.75)
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .children(
                [
                    (Enqueue::Now, "play-detail"),
                    (Enqueue::Next, "play-detail-next"),
                    (Enqueue::End, "play-detail-last"),
                ]
                .map(|(mode, id)| self.detail_header_play(id, mode, width, cx)),
            )
            .when(item.kind == ItemKind::Album, |play| {
                let album = item.clone();
                play.child(
                    icon_button(
                        "detail-album-radio",
                        "radio",
                        self.tr("player.albumRadio"),
                        cx,
                    )
                    .disabled(!self.library_fetches.is_empty())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.enqueue_related_radio(album.clone(), Enqueue::Now, false, false, cx);
                    })),
                )
            });
        let tools = div()
            .h_flex()
            .flex_shrink_0()
            .gap(rem * 0.5)
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .when(
                self.state.settings.show_ratings
                    && self.sessions.get(&item.server_id).is_some_and(|api| {
                        api.server.kind == ServerType::Navidrome
                            && matches!(
                                item.kind,
                                ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                            )
                            || api.server.kind == ServerType::Subsonic
                                && item.kind == ItemKind::Album
                    }),
                |tools| {
                    tools.child(
                        self.rating("detail-rating", item, cx)
                            .size(rem)
                            .read_only(self.rating_requests > 0),
                    )
                },
            )
            .when(
                self.state.settings.show_favorites
                    && matches!(
                        item.kind,
                        ItemKind::Album | ItemKind::Artist | ItemKind::AlbumArtist
                    ),
                |tools| {
                    let favorite = item.clone();
                    tools.child(
                        icon_button(
                            "detail-favorite",
                            if item.favorite {
                                "favorite-filled"
                            } else {
                                "favorite"
                            },
                            self.tr(if item.favorite {
                                "Unfavorite"
                            } else {
                                "Favorite"
                            }),
                            cx,
                        )
                        .when(item.favorite, |button| {
                            button.text_color(cx.theme().primary)
                        })
                        .disabled(self.favorite_requests > 0)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.toggle_favorite(favorite.clone(), cx)
                        })),
                    )
                },
            )
            .child(
                icon_button("detail-more", "more", self.tr("More"), cx)
                    .dropdown_menu(self.library_menu_builder(item, usize::MAX, cx)),
            );
        div()
            .debug_selector(|| "library-detail-actions".into())
            .v_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .gap(rem * 0.5)
            .when(width >= rem * 48., |row| {
                row.flex_row()
                    .justify_between()
                    .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            })
            .child(play)
            .child(tools)
    }

    pub(crate) fn detail_header_artwork(
        &self,
        item: &Item,
        size: Pixels,
        cx: &mut Context<Self>,
    ) -> Div {
        let rem = cx.theme().font_size;
        div()
            .group("library-header-image")
            .relative()
            .flex_shrink_0()
            .child(self.detail_artwork(item, size, cx))
            .when(self.can_upload_library_image(item), |artwork| {
                let upload = item.clone();
                let remove = item.clone();
                artwork.child(
                    div()
                        .absolute()
                        .right(rem * 0.25)
                        .bottom(rem * 0.25)
                        .h_flex()
                        .gap(rem * 0.25)
                        .invisible()
                        .group_hover("library-header-image", |style| style.visible())
                        .child(
                            action("upload-image", "upload", self.tr("Change artwork"), cx)
                                .size(rem * 1.875)
                                .rounded_full()
                                .bg(cx.theme().background)
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.choose_library_image(upload.clone(), window, cx);
                                    cx.stop_propagation();
                                })),
                        )
                        .child(
                            action("remove-image", "delete", self.tr("Remove artwork"), cx)
                                .size(rem * 1.875)
                                .rounded_full()
                                .bg(cx.theme().background)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.change_library_image(remove.clone(), None, cx);
                                    cx.stop_propagation();
                                })),
                        ),
                )
            })
    }

    pub(crate) fn detail_header_play(
        &self,
        id: &'static str,
        mode: Enqueue,
        width: Pixels,
        cx: &mut Context<Self>,
    ) -> Button {
        let rem = cx.theme().font_size;
        let (icon, label) = match mode {
            Enqueue::Now => ("play", "player.play"),
            Enqueue::Next => ("play-next", "player.addNext"),
            Enqueue::End => ("play-last", "player.addLast"),
        };
        let dark = cx.theme().is_dark();
        let hover = rgb(if dark { 0xcccccc } else { 0x1a1a1a }).into();
        let mut target = Target::new(
            "button",
            id,
            &[
                "fs-button-module-root",
                "mantine-Button-root",
                "fs-play-button-module-wide-text-button",
                "fs-play-button-module-unthemed",
            ],
        );
        target
            .attributes
            .insert("data-variant".into(), "subtle".into());
        crate::styles::button(id, cx)
            .debug_selector(move || id.into())
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if dark { white() } else { black() })
                    .foreground(if dark { black() } else { white() })
                    .hover(hover)
                    .active(hover),
            )
            .state_style(crate::styles::state_style_path(target.clone(), vec![]))
            .w_auto()
            .h(rem * 2.25)
            .px(rem * 1.5)
            .rounded(rem)
            .text_size(rem * 0.875)
            .font_weight(FontWeight::SEMIBOLD)
            .icon(
                Icon::default()
                    .path(format!("native-icons/{icon}.svg"))
                    .with_size(rem),
            )
            .when(width > rem * 75., |button| {
                button.child(crate::styles::apply_in(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.tr(label)),
                    Target::new(
                        "span",
                        "",
                        &["fs-play-button-module-wide-text-button-label"],
                    ),
                    [target],
                    cx,
                ))
            })
            .tooltip(format!(
                "{}\n{}",
                self.tr(label),
                self.tr("player.holdToShuffle")
            ))
            .disabled(self.loading || !self.library_fetches.is_empty())
            .on_click(cx.listener(move |view, _, _, cx| view.play_detail(mode, false, cx)))
            .on_long_press(cx.listener(move |view, _, _, cx| view.play_detail(mode, true, cx)))
    }

    pub(crate) fn detail_header_metadata(
        &self,
        item: &feishin_core::model::Item,
        header: &Layout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use feishin_core::{metadata_format, model::ItemKind};
        let album = item.kind == ItemKind::Album;
        if !album && !matches!(item.kind, ItemKind::Artist | ItemKind::AlbumArtist) {
            return div().into_any_element();
        }
        let language = &self.state.settings.language;
        let mut values = if album {
            metadata_format::album_header(item, language)
        } else {
            metadata_format::artist_header(item, language)
        };
        if album && let Some(status) = item.explicit_status() {
            let index = values
                .iter()
                .position(|(id, _)| id == "duration")
                .map_or(values.len(), |index| index + 1);
            values.insert(index, ("explicitStatus".into(), status.into()));
        }
        div()
            .v_flex()
            .w_full()
            .gap(cx.theme().font_size * 0.75)
            .child(
                header
                    .metadata_row()
                    .when(!album, |row| row.gap(cx.theme().font_size * 0.75))
                    .children(
                        values
                            .into_iter()
                            .enumerate()
                            .flat_map(|(index, (id, value))| {
                                let separator = div()
                                    .text_color(cx.theme().muted_foreground)
                                    .when(index > 0, |separator| separator.child("•"))
                                    .when(index == 0 && album, |separator| separator.child("♫"));
                                let value = div()
                                    .debug_selector({
                                        let id = id.clone();
                                        move || format!("detail-header-metadata-{id}")
                                    })
                                    .when(!album && id == "duration", |value| {
                                        value.text_color(cx.theme().muted_foreground)
                                    })
                                    .child(value);
                                [
                                    (index > 0 || album).then(|| separator.into_any_element()),
                                    Some(value.into_any_element()),
                                ]
                                .into_iter()
                                .flatten()
                            }),
                    ),
            )
            .when(album, |metadata| {
                metadata.child(header.metadata_row().child(self.joined_artists(
                    item,
                    true,
                    "detail-header-artists",
                    cx,
                )))
            })
            .into_any_element()
    }
}

struct Description {
    source: String,
    expanded: bool,
    height: Pixels,
    motion: gpui_component::animation::CssTransition,
}
impl Description {
    fn new(text: &str, limit: Pixels) -> Self {
        Self {
            source: text.to_owned(),
            expanded: false,
            height: px(0.),
            motion: gpui_component::animation::CssTransition::new(
                f32::from(limit),
                std::time::Duration::from_millis(200),
                [0.25, 0.1, 0.25, 1.],
            ),
        }
    }
}

pub(crate) fn description(
    text: &str,
    playlist: bool,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let font = cx.theme().font_size;
    let id = if playlist {
        "playlist-description"
    } else {
        "album-description"
    };
    let content = div()
        .w_full()
        .flex_shrink_0()
        .when(!playlist, |text| text.pb(font * 0.75))
        .child(crate::lyrics_markup::Text(
            feishin_core::information::comment_html(text),
        ));
    spoiler(
        id,
        text,
        if playlist { 16. } else { 75. },
        playlist,
        content.into_any_element(),
        window,
        cx,
    )
    .aria_label(crate::appearance::text("filter.comment", cx))
    .text_size(font * if playlist { 0.8125 } else { 0.875 })
    .line_height(font * if playlist { 1. } else { 1.125 })
    .text_color(if playlist {
        cx.theme().muted_foreground
    } else {
        cx.theme().foreground
    })
    .into_any_element()
}

pub(crate) fn spoiler(
    id: &'static str,
    text: &str,
    original_limit: f32,
    blank_label: bool,
    content: AnyElement,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let font = cx.theme().font_size;
    let limit = font * (original_limit / 16.);
    let zoom = cx
        .try_global::<crate::styles::NativeStyles>()
        .map_or(1., |styles| styles.zoom());
    let state = window
        .use_keyed_state(SharedString::from(format!("{id}-state")), cx, |_, _| {
            std::rc::Rc::new(std::cell::RefCell::new(Description::new(text, limit)))
        })
        .read(cx)
        .clone();
    let (expanded, overflows, height, animating) = {
        let mut state = state.borrow_mut();
        if state.source != text {
            *state = Description::new(text, limit);
        }
        // Mantine converts both the configured limit and the measured CSS
        // pixel height through rem(). Account for the user's root font size.
        let target = if state.expanded {
            state.height * (f32::from(font) / zoom / 16.)
        } else {
            limit
        };
        let now = cx.background_executor().now();
        state.motion.retarget(f32::from(target), now);
        let (height, animating) = state.motion.value(now);
        (
            state.expanded,
            state.height > px(original_limit * zoom),
            px(height),
            animating,
        )
    };
    if animating {
        window.request_animation_frame();
    }
    let measure = state.clone();
    let keyboard = state.clone();
    div()
        .id(id)
        .debug_selector(move || id.into())
        .role(Role::Group)
        .v_flex()
        .w_full()
        .min_w_0()
        .text_wrap(TextWrap::BreakWord)
        .cursor_pointer()
        .child(
            div()
                .debug_selector(move || format!("{id}-content"))
                .w_full()
                .overflow_hidden()
                .max_h(height)
                .on_children_prepainted(move |children, window, cx| {
                    let height = children.first().map_or(px(0.), |bounds| bounds.size.height);
                    if measure.borrow().height != height {
                        measure.borrow_mut().height = height;
                        window.defer(cx, |window, _| window.refresh());
                    }
                })
                .child(div().w_full().flex_shrink_0().child(content)),
        )
        .when(overflows, |description| {
            // Empty original show/hide labels still retain the control padding.
            description.child(
                div()
                    .id(SharedString::from(format!("{id}-toggle")))
                    .debug_selector(move || format!("{id}-toggle"))
                    .role(Role::Button)
                    .aria_expanded(expanded)
                    .aria_label(crate::appearance::text(
                        if expanded { "Collapse" } else { "Expand" },
                        cx,
                    ))
                    .key_context("FormControl")
                    .focusable()
                    .tab_stop(true)
                    .w_full()
                    .h_flex()
                    .justify_center()
                    .py(font * 0.75)
                    .when(!blank_label, |button| {
                        button.child(
                            gpui_component::Icon::new(if expanded {
                                gpui_component::IconName::ChevronUp
                            } else {
                                gpui_component::IconName::ChevronDown
                            })
                            .size(font),
                        )
                    })
                    .on_key_down(move |event, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            let expanded = keyboard.borrow().expanded;
                            keyboard.borrow_mut().expanded = !expanded;
                            window.refresh();
                        }
                    }),
            )
        })
        .on_click(move |_, window, _| {
            let expanded = state.borrow().expanded;
            state.borrow_mut().expanded = !expanded;
            window.refresh();
        })
}

#[cfg(test)]
mod tests {
    use super::Layout;
    use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px, size};

    #[gpui::test]
    fn complete_album_artist_headers_match_original_pages(cx: &mut TestAppContext) {
        use crate::app::{Feishin, Route};
        use feishin_core::{
            api::Api,
            model::{Item, ItemKind, Server, ServerType, Session},
            storage::State,
        };
        use gpui::{AppContext, Modifiers, Role};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let directory = std::env::temp_dir().join(format!(
            "feishin-detail-page-header-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.follow_system_theme = false;
            state.settings.window_bar_style = "web".into();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            username: "fixture".into(),
            url: "http://127.0.0.1:1".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let server_id = server.id;
        app.update(cx, |view, _| {
            view.sessions
                .insert(server_id, Api::new(server, Session::default()).unwrap());
        });
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/detail-page-header-reference.json"))
                .unwrap();
        for (kind, name, key) in [
            (ItemKind::Album, "Northern Lights", "album"),
            (ItemKind::AlbumArtist, "Fixture Artist", "artist"),
        ] {
            cx.simulate_resize(size(px(1320.), px(840.)));
            app.update(cx, |view, cx| {
                let album = kind == ItemKind::Album;
                view.route = Route::Detail(Box::new(Item {
                    id: key.into(),
                    server_id,
                    kind,
                    name: name.into(),
                    artist: if album {
                        "Fixture Artist".into()
                    } else {
                        String::new()
                    },
                    album_artist: if album {
                        "Fixture Artist".into()
                    } else {
                        String::new()
                    },
                    album_artist_links: album
                        .then(|| vec![("artist0".into(), "Fixture Artist".into())]),
                    duration: if album { 1280. } else { 0. },
                    year: if album { 2026 } else { 0 },
                    genre_links: album.then(|| vec![("genre0".into(), "Ambient".into())]),

                    column_data: if album {
                        [
                            ("releaseDate".into(), "2026-01-02".into()),
                            ("songCount".into(), "32".into()),
                        ]
                        .into_iter()
                        .collect()
                    } else {
                        Default::default()
                    },
                    ..Default::default()
                }));
                view.artist_overview = true;
                view.sort = feishin_core::model::Sort::Id;
                view.descending = false;
                view.items = if album {
                    (0..32)
                        .map(|index| Item {
                            id: format!("song{index}"),
                            server_id,
                            kind: ItemKind::Song,
                            name: format!("Track {:03}", index + 1),
                            track: index + 1,
                            disc: 1,
                            duration: 40.,
                            ..Default::default()
                        })
                        .collect()
                } else {
                    vec![]
                };
                view.message.clear();
                view.loading = false;
                cx.notify();
            });
            cx.run_until_parked();
            let check =
                |selector: &str, expected: &serde_json::Value, cx: &mut gpui::VisualTestContext| {
                    let bounds = cx
                        .debug_bounds(selector)
                        .unwrap_or_else(|| panic!("Missing {selector}"));
                    for (actual, expected) in [
                        bounds.origin.x,
                        bounds.origin.y,
                        bounds.size.width,
                        bounds.size.height,
                    ]
                    .into_iter()
                    .zip(expected.as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{key} {selector}: {actual:?} != {expected}"
                        );
                    }
                };
            for (selector, field) in [
                ("library-detail-header", "header"),
                ("library-detail-title", "title"),
                ("library-detail-artwork", "artwork"),
                ("library-detail-metadata", "metadata"),
            ] {
                check(selector, &reference[key][field], cx);
            }
            let offset = if kind == ItemKind::Album { 0 } else { 2 };
            let selectors = if kind == ItemKind::Album {
                vec![
                    "play-detail",
                    "play-detail-next",
                    "play-detail-last",
                    "detail-album-radio",
                    "detail-favorite",
                    "detail-more",
                ]
            } else {
                vec![
                    "play-detail",
                    "play-detail-next",
                    "play-detail-last",
                    "detail-favorite",
                    "detail-more",
                ]
            };
            for (index, selector) in selectors.iter().enumerate() {
                check(
                    selector,
                    &reference[key]["buttons"][index + offset]["bounds"],
                    cx,
                );
            }
            let play = cx.debug_bounds("play-detail").unwrap();
            let controls: serde_json::Value = serde_json::from_str(if key == "album" {
                include_str!("../assets/album-detail-controls-reference.json")
            } else {
                include_str!("../assets/artist-detail-controls-reference.json")
            })
            .unwrap();
            if key == "album" {
                let toolbar = &controls["toolbar"];
                check("album-track-toolbar", &toolbar["bounds"], cx);
                check("detail-search-control", &toolbar["search"], cx);
                let search = cx.debug_bounds("detail-search-control").unwrap();
                assert!(
                    cx.update(|window, _| window.painted_boxes_for_test())
                        .iter()
                        .any(|(bounds, _, corners, _)| {
                            *bounds == search && *corners == gpui::Corners::all(px(16.))
                        }),
                    "Album search must retain the original rounded border"
                );
                let table_header = cx.debug_bounds("metadata-header").unwrap();
                let first_track = cx.debug_bounds("library-row-0").unwrap();
                assert_eq!(
                    table_header.top(),
                    cx.debug_bounds("album-track-toolbar").unwrap().bottom() + px(16.)
                );
                assert_eq!(first_track.top(), table_header.bottom());
                for (index, selector) in ["album-track-sort", "sort-order", "library-list-settings"]
                    .iter()
                    .enumerate()
                {
                    check(selector, &toolbar["buttons"][index]["bounds"], cx);
                }
                assert!(
                    cx.debug_bounds("play-visible").is_none(),
                    "Album play actions belong in its header"
                );
            }
            let radius = px(controls["playRadius"].as_f64().unwrap() as f32);
            let paint = cx.update(|window, _| window.painted_boxes_for_test());
            assert!(
                paint.iter().any(|(bounds, _, corners, _)| {
                    *bounds == play && *corners == gpui::Corners::all(radius)
                }),
                "Detail play button must use the original radius {radius:?}: {:?}",
                paint
                    .iter()
                    .filter(|(bounds, _, _, _)| *bounds == play)
                    .collect::<Vec<_>>()
            );
            assert!(
                cx.update(|window, _| {
                    window.painted_lines_and_quads_for_test().iter().any(
                        |(bounds, color, rounded, _)| {
                            *bounds == play.scale(window.scale_factor())
                                && *rounded
                                && *color == gpui::white()
                        },
                    )
                }),
                "Play must retain its original white background"
            );
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.theme = "defaultLight".into();
                    view.apply_theme(window, cx);
                    cx.notify();
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(300));
            cx.refresh().unwrap();
            cx.run_until_parked();
            assert!(
                cx.update(|window, _| {
                    window.painted_lines_and_quads_for_test().iter().any(
                        |(bounds, color, rounded, _)| {
                            *bounds == play.scale(window.scale_factor())
                                && *rounded
                                && *color == gpui::black()
                        },
                    )
                }),
                "Play must retain its original black background in the light theme: {:?}",
                cx.update(|window, _| window
                    .painted_lines_and_quads_for_test()
                    .into_iter()
                    .filter(|(bounds, _, _, _)| *bounds == play.scale(window.scale_factor()))
                    .collect::<Vec<_>>())
            );
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.theme = "defaultDark".into();
                    view.apply_theme(window, cx);
                    cx.notify();
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(300));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let more = cx.debug_bounds("detail-more").unwrap().center();
            cx.simulate_click(more, Modifiers::none());
            cx.run_until_parked();
            assert!(cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.role() == Role::Label && node.value() == Some("Information")
                    })
            }));
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
            let responsive: Vec<serde_json::Value> = serde_json::from_str(if key == "album" {
                include_str!("../assets/album-page-responsive-reference.json")
            } else {
                include_str!("../assets/artist-page-responsive-reference.json")
            })
            .unwrap();
            for frame in responsive {
                cx.simulate_resize(size(
                    px(frame["viewport"][0].as_f64().unwrap() as f32),
                    px(840.),
                ));
                cx.run_until_parked();
                for (selector, field) in [
                    ("library-detail-header", "header"),
                    ("library-detail-title", "title"),
                    ("library-detail-artwork", "artwork"),
                    ("library-detail-metadata", "metadata"),
                ] {
                    check(selector, &frame[field], cx);
                }
                for (index, selector) in selectors.iter().enumerate() {
                    check(selector, &frame["buttons"][index + offset]["bounds"], cx);
                }
            }
        }
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn biography_expansion_matches_original_spoiler_frames(cx: &mut TestAppContext) {
        use std::time::Duration;
        cx.update(gpui_component::init);
        struct Probe {
            font: f32,
            zoom: f32,
            body: f32,
            source: String,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui_component::Theme::global_mut(cx).font_size = px(self.font * self.zoom);
                cx.set_global(
                    crate::styles::NativeStyles::new(Default::default(), Default::default())
                        .with_zoom(self.zoom),
                );
                super::spoiler(
                    "artist-biography",
                    &self.source,
                    56.,
                    false,
                    div().h(px(self.body * self.zoom)).into_any_element(),
                    window,
                    cx,
                )
            }
        }
        let (probe, cx) = cx.add_window_view(|_, _| Probe {
            font: 16.,
            zoom: 1.,
            body: 120.,
            source: String::new(),
        });
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/artist-biography-reference.json"))
                .unwrap();
        for zoom in [1., 1.5] {
            for (index, case) in cases.iter().enumerate() {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        *view = Probe {
                            font: case["font"].as_f64().unwrap() as f32,
                            zoom,
                            body: case["body"].as_f64().unwrap() as f32,
                            source: format!("{index}-{zoom}"),
                        };
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let actual = f32::from(
                    cx.debug_bounds("artist-biography-content")
                        .unwrap()
                        .size
                        .height,
                );
                let expected = case["collapsed"]["height"].as_f64().unwrap() as f32 * zoom;
                assert!(
                    (actual - expected).abs() <= 1.01,
                    "{index} zoom {zoom}: collapsed {actual} vs {expected}"
                );
                if case["frames"].as_array().unwrap().is_empty() {
                    continue;
                }
                for expanded in [true, false] {
                    if expanded {
                        let toggle = cx.debug_bounds("artist-biography-toggle").unwrap().center();
                        cx.simulate_click(toggle, gpui::Modifiers::none());
                    } else {
                        cx.simulate_keystrokes("enter");
                    }
                    cx.run_until_parked();
                    let mut last = 0;
                    for frame in case["frames"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|frame| frame["expanded"] == expanded)
                    {
                        let elapsed = frame["elapsed"].as_u64().unwrap();
                        cx.executor()
                            .advance_clock(Duration::from_millis(elapsed - last));
                        cx.update(|window, _| window.refresh());
                        cx.run_until_parked();
                        let actual = f32::from(
                            cx.debug_bounds("artist-biography-content")
                                .unwrap()
                                .size
                                .height,
                        );
                        let expected = frame["height"].as_f64().unwrap() as f32 * zoom;
                        assert!(
                            (actual - expected).abs() <= 1.01,
                            "{index} zoom {zoom}, {frame}: {actual} vs {expected}"
                        );
                        last = elapsed;
                    }
                }
            }
        }
    }

    #[gpui::test]
    fn playlist_and_album_descriptions_expand_open_links_and_reset(cx: &mut TestAppContext) {
        use std::time::Duration;
        cx.update(gpui_component::init);
        struct Probe(String, f32, f32, bool);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                gpui_component::Theme::global_mut(cx).font_size = px(self.1 * self.2);
                cx.set_global(
                    crate::styles::NativeStyles::new(Default::default(), Default::default())
                        .with_zoom(self.2),
                );
                div()
                    .w(px(400.))
                    .child(super::description(&self.0, self.3, window, cx))
            }
        }
        let (probe, cx) = cx.add_window_view(|_, _| {
            Probe(
                "example.com\nSecond line\nThird line <b>literal</b>".into(),
                16.,
                1.,
                true,
            )
        });
        cx.run_until_parked();
        let content = "playlist-description-content";
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(16.));
        let description = cx.debug_bounds("playlist-description").unwrap();
        assert_eq!(description.size.height, px(40.));
        cx.simulate_click(
            gpui::point(description.center().x, description.bottom() - px(8.)),
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(16.));
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let halfway = cx.debug_bounds(content).unwrap().size.height;
        assert!(
            halfway > px(16.) && halfway < px(48.),
            "Halfway height: {halfway:?}"
        );
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(48.));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(16.));
        cx.simulate_keystrokes("space");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(48.));
        let text = cx.debug_bounds(content).unwrap();
        cx.simulate_click(
            text.origin + gpui::point(px(8.), px(8.)),
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(cx.opened_url().as_deref(), Some("https://example.com"));
        cx.update(|_, cx| {
            probe.update(cx, |view, cx| {
                view.0 = "Short description".into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(
            cx.debug_bounds("playlist-description").unwrap().size.height,
            px(16.)
        );
        cx.update(|_, cx| {
            probe.update(cx, |view, cx| {
                view.0 = "New playlist\nMore information".into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(16.));
        for font in [16., 20.] {
            for zoom in [1., 1.5] {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        view.0 = format!("New text {font} {zoom}\nSecond line");
                        view.1 = font;
                        view.2 = zoom;
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                assert_eq!(
                    cx.debug_bounds(content).unwrap().size.height,
                    px(font * zoom)
                );
                assert_eq!(
                    cx.debug_bounds("playlist-description").unwrap().size.height,
                    px(font * zoom * 2.5)
                );
            }
        }
        cx.update(|_, cx| {
            probe.update(cx, |view, cx| {
                *view = Probe(
                    "example.com\nSecond\nThird\nFourth\nFifth\nSixth".into(),
                    16.,
                    1.,
                    false,
                );
                cx.notify();
            })
        });
        cx.run_until_parked();
        let content = "album-description-content";
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(75.));
        let control = cx.debug_bounds("album-description-toggle").unwrap();
        assert_eq!(control.size.height, px(40.));
        cx.simulate_click(control.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(120.));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds(content).unwrap().size.height, px(75.));
    }

    #[gpui::test]
    fn header_layout_matches_original_responsive_frames(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        struct Probe(serde_json::Value, f32);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let font = self.0["font"].as_f64().unwrap() as f32 * self.1;
                let width = px(self.0["width"].as_f64().unwrap() as f32 * self.1);
                gpui_component::Theme::global_mut(cx).font_size = px(font);
                window.set_rem_size(px(font));
                window.text_system().set_optical_size_scale(self.1);
                cx.set_global(
                    crate::styles::NativeStyles::new(Default::default(), Default::default())
                        .with_zoom(self.1),
                );
                let layout = Layout::new(width, self.0["compact"].as_bool().unwrap(), cx);
                div()
                    .w(width)
                    .font_family("Inter")
                    .font_features(gpui::FontFeatures(std::sync::Arc::new(vec![(
                        "tnum".into(),
                        1,
                    )])))
                    .text_size(px(font))
                    .line_height(px(font * 1.125))
                    .child(
                        layout
                            .frame()
                            .child(
                                div()
                                    .debug_selector(|| "header-artwork".into())
                                    .size(layout.artwork_size())
                                    .flex_shrink_0(),
                            )
                            .child(
                                layout
                                    .metadata()
                                    .child(layout.item_type("ALBUM".into()))
                                    .child(
                                        layout.title(
                                            self.0["title"].as_str().unwrap().into(),
                                            window,
                                        ),
                                    )
                                    .child(
                                        div()
                                            .debug_selector(|| "header-content".into())
                                            .w_full()
                                            .h(px(self.0["metadata"].as_f64().unwrap() as f32
                                                * self.1))
                                            .flex_shrink_0(),
                                    ),
                            ),
                    )
            }
        }
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/library-header-reference.json")).unwrap();
        let (probe, cx) = cx.add_window_view(|_, _| Probe(cases[0].clone(), 1.));
        for zoom in [1., 1.5] {
            cx.simulate_resize(size(px(1440. * zoom), px(1400. * zoom)));
            for case in &cases {
                cx.update(|_, cx| {
                    probe.update(cx, |view, cx| {
                        view.0 = case.clone();
                        view.1 = zoom;
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                for layer in case["layers"].as_array().unwrap() {
                    let selector = match layer["kind"].as_str().unwrap() {
                        "library-header" | "library-header compact" => "library-detail-header",
                        "image-section" | "image" => "header-artwork",
                        "metadata-section" => "library-detail-metadata",
                        "item-type" => "library-detail-type",
                        "title" => "library-detail-title",
                        "metadata" => "header-content",
                        other => panic!("Unknown original header layer {other}"),
                    };
                    let actual = cx.debug_bounds(selector).unwrap();
                    for (value, key) in [
                        (actual.origin.x, "x"),
                        (actual.origin.y, "y"),
                        (actual.size.width, "width"),
                        (actual.size.height, "height"),
                    ] {
                        let expected = layer[key].as_f64().unwrap() as f32 * zoom;
                        assert!(
                            (f32::from(value) - expected).abs() <= 1.1,
                            "{selector} {key}: {value:?} != {expected}; zoom {zoom}; title {:?}; metadata {:?}; {case}",
                            cx.debug_bounds("library-detail-title"),
                            cx.debug_bounds("library-detail-metadata")
                        );
                    }
                }
            }
        }
    }
}
