use crate::{
    app::{Feishin, Route},
    list_header::action,
};
use feishin_core::{
    model::{Item, ItemKind, Sort},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
};

impl Feishin {
    pub(crate) fn has_playlist_header(&self) -> bool {
        matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist)
    }

    pub(crate) fn playlist_header_title(&self, item: &Item) -> String {
        if self.state.settings.playlist_folders {
            feishin_core::playlist_folders::leaf(
                &item.name,
                &self.state.settings.playlist_separator,
            )
            .into()
        } else {
            item.name.clone()
        }
    }

    pub(crate) fn playlist_page_header(
        &self,
        item: &Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut page = div()
            .id("library-controls")
            .v_flex()
            .w_full()
            .flex_shrink_0();
        if self.state.settings.playlist_header_collapsed {
            page = page.child(self.standard_list_header(cx));
        } else {
            let rem = cx.theme().font_size;
            let width = if self.detail_header_width > px(0.) {
                self.detail_header_width
            } else {
                window.viewport_size().width - px(self.sidebar_pixel_width())
            };
            let layout = crate::library_header::Layout::new(width, true, cx);
            let artwork = self.detail_header_artwork(item, layout.artwork_size(), cx);
            let kind = layout
                .item_type(
                    crate::library_header::type_label(
                        ItemKind::Playlist,
                        &self.state.settings.language,
                    )
                    .to_uppercase(),
                )
                .id("playlist-header-type")
                .role(Role::Link)
                .key_context("FormControl")
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .on_click(cx.listener(|view, _, _, cx| {
                    view.navigate(Route::Library(ItemKind::Playlist), cx)
                }))
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" {
                        cx.stop_propagation();
                        view.navigate(Route::Library(ItemKind::Playlist), cx);
                    }
                }));
            let content = div()
                .v_flex()
                .w_full()
                .gap(rem * 0.75)
                // The original narrow menu retains the gap before its empty secondary group.
                .when(width < rem * 48., |content| content.pb(rem * 0.5))
                .when(!item.comment.trim().is_empty(), |content| {
                    content.child(crate::library_header::description(
                        &item.comment,
                        true,
                        window,
                        cx,
                    ))
                })
                .child(
                    div()
                        .debug_selector(|| "library-detail-actions".into())
                        .h_flex()
                        .gap(rem * 0.75)
                        .when(width < rem * 48., |row| row.justify_center())
                        .children(
                            [
                                (Enqueue::Now, "play-detail"),
                                (Enqueue::Next, "play-detail-next"),
                                (Enqueue::End, "play-detail-last"),
                            ]
                            .map(|(mode, id)| self.detail_header_play(id, mode, width, cx)),
                        ),
                );
            let header = layout
                .frame()
                .child(artwork)
                .child(
                    layout
                        .metadata()
                        .child(kind)
                        .child(layout.title(self.playlist_header_title(item), window))
                        .child(content),
                )
                .child(
                    div()
                        .absolute()
                        .top(rem)
                        .right(rem * 0.75)
                        .child(self.list_search_control(false, false, cx)),
                );
            page = page.child(header).child(self.playlist_filter_bar(cx));
        }
        let config = self.list_config();
        page.when(
            config.display == "table" && config.table.enable_header,
            |page| page.child(self.configured_header(cx)),
        )
    }

    pub(crate) fn playlist_filter_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let Route::Detail(item) = &self.route else {
            return div().into_any_element();
        };
        let rem = cx.theme().font_size;
        let editing = self.playlist_tracks_editing();
        let rule_edit = item.rules.is_some();
        let divider = || {
            div()
                .w(px(self.state.settings.zoom))
                .h(rem * 2.25)
                .bg(cx.theme().border)
        };
        let controls = div()
            .h_flex()
            .flex_wrap()
            .min_w_0()
            .flex_1()
            .gap(rem * 0.5)
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .child(
                crate::styles::button("playlist-target", cx)
                    .debug_selector(|| "playlist-target".into())
                    .ghost()
                    .icon(
                        Icon::default()
                            .path("native-icons/arrow-left-right.svg")
                            .with_size(rem * 0.875),
                    )
                    .label(
                        feishin_core::i18n::translate_with(
                            &self.state.settings.language,
                            if self.state.settings.playlist_albums {
                                "entity.album"
                            } else {
                                "entity.track"
                            },
                            &serde_json::json!({"count":2}),
                        )
                        .unwrap_or_else(|| {
                            self.tr(if self.state.settings.playlist_albums {
                                "Albums"
                            } else {
                                "Tracks"
                            })
                        }),
                    )
                    .disabled(editing)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.remember_view();
                        view.state.settings.playlist_albums = !view.state.settings.playlist_albums;
                        view.restore_view();
                        view.save();
                        view.reload(false, cx);
                    })),
            )
            .child(divider())
            .child(
                crate::styles::button("sort", cx)
                    .debug_selector(|| "library-sort".into())
                    .ghost()
                    .label(if self.sort == Sort::Id {
                        "ID".into()
                    } else {
                        self.tr(self.sort.label())
                    })
                    .disabled(editing)
                    .dropdown_menu({
                        let app = cx.weak_entity();
                        move |mut menu, _, cx| {
                            for sort in Sort::ALL {
                                let app = app.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(sort.label(), cx))
                                        .on_click(move |_, _, cx| {
                                            app.update(cx, |view, cx| {
                                                view.sort = sort;
                                                view.reload(false, cx);
                                            })
                                            .ok();
                                        }),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(divider())
            .child(
                action(
                    "sort-order",
                    if self.descending {
                        "sort-descending"
                    } else {
                        "sort-ascending"
                    },
                    self.tr("Sort order"),
                    cx,
                )
                .disabled(editing)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.descending = !view.descending;
                    view.reload(false, cx);
                })),
            )
            .child(divider())
            .child(
                action("library-filter-settings", "filter", self.tr("Filters"), cx)
                    .on_click(cx.listener(|view, _, window, cx| view.edit_filters(window, cx))),
            )
            .child(
                action("refresh-library", "refresh", self.tr("Refresh"), cx)
                    .disabled(self.loading || editing)
                    .on_click(cx.listener(|view, _, _, cx| view.refresh_library(cx))),
            )
            .child(
                action("playlist-more", "more", self.tr("More"), cx)
                    .dropdown_menu(self.playlist_page_menu(item, cx)),
            );
        let mut tools = div()
            .h_flex()
            .gap(rem * 0.5)
            .flex_shrink_0()
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse());
        if editing {
            tools = tools.child(self.playlist_edit_toolbar(cx));
        } else if self.can_edit_playlist(item)
            && (self.detail_header_width - rem * 0.5 >= px(480. * self.state.settings.zoom)
                || self.state.settings.playlist_albums)
        {
            let playlist = item.as_ref().clone();
            tools = tools.child(
                crate::styles::button("edit-playlist-tracks", cx)
                    .debug_selector(|| "edit-playlist-tracks".into())
                    .ghost()
                    .label(self.tr("common.edit").to_uppercase())
                    .on_click(cx.listener(move |view, _, window, cx| {
                        if rule_edit {
                            view.edit_playlist(Some(playlist.clone()), vec![], window, cx);
                        } else {
                            view.start_playlist_tracks_edit(cx);
                        }
                    })),
            );
        }
        let display = self.list_config().display;
        let icon = match display.as_str() {
            "poster" => "layout-grid",
            "detail" => "layout-detail",
            _ => "layout-table",
        };
        tools = tools
            .child(
                action(
                    "playlist-header-toggle",
                    if self.state.settings.playlist_header_collapsed {
                        "arrow-down"
                    } else {
                        "arrow-up"
                    },
                    self.tr(if self.state.settings.playlist_header_collapsed {
                        "common.expand"
                    } else {
                        "common.collapse"
                    }),
                    cx,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.state.settings.playlist_header_collapsed =
                        !view.state.settings.playlist_header_collapsed;
                    view.save();
                    cx.notify();
                })),
            )
            .child(
                action("view", icon, self.tr("Switch layout"), cx).on_click(cx.listener(
                    |view, _, _, cx| {
                        let mut config = view.list_config();
                        config.display = match config.display.as_str() {
                            "poster" if view.state.settings.playlist_albums => "detail",
                            "poster" | "detail" => "table",
                            _ => "poster",
                        }
                        .into();
                        view.state.settings.lists.insert(view.list_key(), config);
                        view.save();
                        cx.notify();
                    },
                )),
            )
            .child(
                action(
                    "library-list-settings",
                    "settings",
                    self.tr("List layout"),
                    cx,
                )
                .on_click(cx.listener(|view, _, window, cx| {
                    view.edit_list_config(view.list_key(), window, cx)
                })),
            );
        let bar = div()
            .id("library-filter-bar")
            .debug_selector(|| "library-filter-bar".into())
            .h_flex()
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .w_full()
            .flex_shrink_0()
            .p(rem * 0.25)
            .border_b(px(self.state.settings.zoom))
            .border_color(cx.theme().border)
            .child(controls)
            .child(tools);
        crate::styles::apply(
            bar,
            "div",
            "library-filter-bar",
            &["fs-filter-bar-module-filter-bar"],
            None,
            cx,
        )
        .into_any_element()
    }

    fn playlist_page_menu(
        &self,
        item: &Item,
        cx: &Context<Self>,
    ) -> impl Fn(
        gpui_component::menu::PopupMenu,
        &mut Window,
        &mut Context<gpui_component::menu::PopupMenu>,
    ) -> gpui_component::menu::PopupMenu
    + Clone
    + 'static {
        let base = self.library_menu_builder(item, usize::MAX, cx);
        let item = item.clone();
        let app = cx.weak_entity();
        let disabled = !self.can_edit_playlist(&item);
        let partial = self.has_more && self.selected.is_empty();
        move |menu, window, cx| {
            let mut menu = base(menu, window, cx).separator();
            for (queue, label) in [
                (false, "Replace with selected / visible tracks"),
                (true, "Replace with queue"),
            ] {
                let app = app.clone();
                let item = item.clone();
                menu = menu.item(
                    PopupMenuItem::new(crate::appearance::text(label, cx))
                        .disabled(disabled || !queue && partial)
                        .on_click(move |_, window, cx| {
                            app.update(cx, |view, cx| {
                                view.replace_playlist(item.clone(), queue, window, cx)
                            })
                            .ok();
                        }),
                );
            }
            menu
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Feishin, Item, ItemKind, Route, Sort};
    use feishin_core::{
        api::Api,
        model::{Server, ServerType, Session},
        storage::State,
    };
    use gpui::{AppContext, Modifiers, Role, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn complete_playlist_header_matches_original_frames_and_keeps_actions(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("feishin-playlist-header-{}", uuid::Uuid::new_v4()));
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
            state.settings.confirm_queue_changes = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        app.update(cx, |view, cx| {
            let server = Server {
                id: uuid::Uuid::new_v4(),
                name: "Fixture".into(),
                username: "fixture".into(),
                url: "http://127.0.0.1:1".into(),
                kind: ServerType::Navidrome,
                legacy_auth: false,
                options: Default::default(),
            };
            let item = Item {
                id: "playlist".into(),
                server_id: server.id,
                name: "Test playlist".into(),
                comment: "Disposable fixture".into(),
                kind: ItemKind::Playlist,
                duration: 120.,
                ..Default::default()
            };
            view.sessions
                .insert(server.id, Api::new(server, Session::default()).unwrap());
            view.route = Route::Detail(Box::new(item.clone()));
            view.sort = Sort::Id;
            view.descending = false;
            view.items = (0..3)
                .map(|index| Item {
                    id: index.to_string(),
                    name: format!("Track {index}"),
                    kind: ItemKind::Song,
                    server_id: item.server_id,
                    ..Default::default()
                })
                .collect();
            view.total = Some(3);
            view.message.clear();
            cx.notify();
        });
        cx.run_until_parked();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/playlist-page-header-reference.json"
        ))
        .unwrap();
        let check = |selector: &str, expected: &serde_json::Value, cx: &mut VisualTestContext| {
            let actual = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"));
            for (actual, expected) in [
                actual.origin.x,
                actual.origin.y,
                actual.size.width,
                actual.size.height,
            ]
            .into_iter()
            .zip(expected.as_array().unwrap())
            {
                assert!(
                    (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "{selector}: {actual:?} != {expected}"
                );
            }
        };
        let click = |selector: &str, cx: &mut VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
        };
        for (selector, key) in [
            ("library-detail-header", "header"),
            ("library-detail-title", "title"),
            ("library-detail-artwork", "artwork"),
            ("library-detail-metadata", "metadata"),
            ("library-detail-type", "type"),
            ("library-search", "search"),
            ("library-filter-bar", "filter"),
        ] {
            check(selector, &reference["expanded"][key], cx);
        }
        for (index, selector) in ["play-detail", "play-detail-next", "play-detail-last"]
            .into_iter()
            .enumerate()
        {
            check(selector, &reference["expanded"]["play"][index], cx);
        }
        for (index, selector) in [
            "playlist-target",
            "library-sort",
            "sort-order",
            "library-filter-settings",
            "refresh-library",
            "playlist-more",
            "edit-playlist-tracks",
            "playlist-header-toggle",
            "view",
            "library-list-settings",
        ]
        .into_iter()
        .enumerate()
        {
            check(
                selector,
                &reference["expanded"]["buttons"][index]["bounds"],
                cx,
            );
        }
        click("playlist-header-toggle", cx);
        check("library-page-header", &reference["collapsed"]["header"], cx);
        check("library-page-title", &reference["collapsed"]["title"], cx);
        check(
            "playlist-duration-badge",
            &reference["collapsed"]["badges"][0]["bounds"],
            cx,
        );
        check(
            "library-page-count",
            &reference["collapsed"]["badges"][1]["bounds"],
            cx,
        );
        assert!(
            feishin_core::storage::load(&directory.join("state.json"))
                .unwrap()
                .settings
                .playlist_header_collapsed
        );
        click("library-play-menu-trigger", cx);
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| {
                    node.role() == Role::Button && node.label() == Some("Play\nHold to shuffle")
                })
        }));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        click("playlist-header-toggle", cx);
        click("play-detail-next", cx);
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            3
        );
        click("playlist-more", cx);
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.value() == Some("Replace with queue"))
        }));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        let responsive: Vec<serde_json::Value> = serde_json::from_str(include_str!(
            "../assets/playlist-page-responsive-reference.json"
        ))
        .unwrap();
        for frame in responsive {
            cx.simulate_resize(size(
                px(frame["viewport"][0].as_f64().unwrap() as f32),
                px(840.),
            ));
            cx.run_until_parked();
            for (selector, key) in [
                ("library-detail-header", "header"),
                ("library-detail-title", "title"),
                ("library-detail-artwork", "artwork"),
                ("library-detail-metadata", "metadata"),
                ("library-detail-type", "type"),
                ("library-search", "search"),
                ("library-filter-bar", "filter"),
            ] {
                check(selector, &frame[key], cx);
            }
            for (index, selector) in ["play-detail", "play-detail-next", "play-detail-last"]
                .into_iter()
                .enumerate()
            {
                check(selector, &frame["play"][index], cx);
            }
            let buttons = [
                "playlist-target",
                "library-sort",
                "sort-order",
                "library-filter-settings",
                "refresh-library",
                "playlist-more",
                "edit-playlist-tracks",
                "playlist-header-toggle",
                "view",
                "library-list-settings",
            ];
            for (index, selector) in buttons
                .into_iter()
                .filter(|selector| {
                    *selector != "edit-playlist-tracks"
                        || frame["viewport"][0].as_f64().unwrap() > 800.
                })
                .enumerate()
            {
                check(selector, &frame["buttons"][index]["bounds"], cx);
            }
        }
        std::fs::remove_dir_all(directory).ok();
    }
}
