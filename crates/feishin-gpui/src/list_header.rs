use crate::app::{Feishin, Route};
use feishin_core::{
    model::{ItemKind, Sort},
    queue::Enqueue,
    styles::Target,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    popover::{Popover, PopoverState},
};

pub(crate) const ALBUM_SEARCH_STYLE: &str = "
.fs-album-detail-content-module-search-text-input {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: var(--theme-radius-xl);
}
[data-mantine-color-scheme='light'] .fs-album-detail-content-module-search-text-input {
    border-color: rgba(0, 0, 0, 0.05);
}";

pub(crate) fn action(id: &'static str, icon: &str, label: String, cx: &App) -> Button {
    let mut target = Target::new(
        "button",
        id,
        &["fs-action-icon-module-root", "mantine-ActionIcon-root"],
    );
    target
        .attributes
        .insert("data-variant".into(), "subtle".into());
    Button::new(id)
        .debug_selector(move || id.into())
        .ghost()
        .state_style(crate::styles::state_style_path(target, vec![]))
        .size(cx.theme().font_size * 2.25)
        .flex_shrink_0()
        .p_0()
        .icon(
            Icon::default()
                .path(format!("native-icons/{icon}.svg"))
                .with_size(cx.theme().font_size),
        )
        .tooltip(label)
}

fn badge(id: &'static str, zoom: f32, cx: &App) -> Stateful<Div> {
    let rem = cx.theme().font_size;
    div()
        .id(id)
        .debug_selector(move || id.into())
        .h_flex()
        .justify_center()
        .flex_shrink_0()
        .h(px(20. * zoom))
        .px(rem * 0.5)
        .border(px(zoom))
        .border_color(cx.theme().border)
        .rounded(rem * (5. / 16.))
        .bg(cx.theme().secondary)
        .text_size(px(11. * zoom))
        .line_height(px(20. * zoom) - rem * 0.125)
        .letter_spacing(px(0.25 * zoom))
        .font_weight(FontWeight::MEDIUM)
}

impl Feishin {
    pub(crate) fn album_track_controls(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let rem = cx.theme().font_size;
        let target = Target::new(
            "input",
            "album-track-search",
            &[
                "fs-text-input-module-input",
                "mantine-Input-input",
                "fs-album-detail-content-module-search-text-input",
            ],
        );
        let search = div()
            .id("detail-search-control")
            .debug_selector(|| "detail-search-control".into())
            .relative()
            .flex_1()
            .min_w_0()
            .child(
                gpui::Styled::h(
                    crate::styles::input(&self.local_search, cx)
                        .state_style(crate::styles::state_style_path(target, vec![]))
                        .with_size(gpui_component::Size::Small),
                    rem * 2.25,
                )
                .w_full()
                .text_size(rem * (13. / 16.))
                .line_height(rem * 2.125)
                .pl(rem * 0.625)
                .pr(rem * 2.25)
                .gap(rem * 0.625)
                .shadow_none()
                .prefix(
                    Icon::default()
                        .path("native-icons/search.svg")
                        .with_size(rem),
                ),
            )
            .when(!self.local_search.read(cx).value().is_empty(), |search| {
                search.child(
                    action("clear-list-filter", "close", self.tr("common.clear"), cx)
                        .absolute()
                        .top_0()
                        .right_0()
                        .on_click(cx.listener(|view, _, window, cx| {
                            view.local_search.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                            view.submit_local_filter(String::new(), cx);
                        })),
                )
            });
        let label = feishin_core::lists::SONG_SORTS
            .iter()
            .find(|(sort, _)| *sort == self.sort)
            .map_or(self.sort.label(), |(_, label)| label);
        let toolbar = div()
            .id("album-track-toolbar")
            .debug_selector(|| "album-track-toolbar".into())
            .h_flex()
            .w_full()
            .gap(rem * 0.5)
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .child(search)
            .child(
                crate::styles::button("sort", cx)
                    .debug_selector(|| "album-track-sort".into())
                    .ghost()
                    .flex_shrink_0()
                    .label(self.tr(label))
                    .dropdown_menu({
                        let app = cx.weak_entity();
                        let selected = self.sort;
                        move |mut menu, _, cx| {
                            for (sort, label) in feishin_core::lists::SONG_SORTS {
                                let app = app.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(crate::appearance::text(label, cx))
                                        .checked(sort == selected)
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
                .on_click(cx.listener(|view, _, _, cx| {
                    view.descending = !view.descending;
                    view.reload(false, cx);
                })),
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
        let config = self.list_config();
        div()
            .id("library-track-controls")
            .debug_selector(|| "library-track-controls".into())
            .v_flex()
            .w_full()
            .gap(rem)
            .child(toolbar)
            .when(
                config.display == "table" && config.table.enable_header,
                |div| div.child(self.configured_header(cx)),
            )
            .when(
                config.display != "table" || !config.table.enable_header,
                |div| div.pb(rem),
            )
    }

    pub(crate) fn has_standard_list_header(&self) -> bool {
        matches!(self.route, Route::Library(ItemKind::Album | ItemKind::Song))
    }

    pub(crate) fn standard_list_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let rem = cx.theme().font_size;
        let zoom = self.state.settings.zoom;
        let app = cx.weak_entity();
        let mut header_target = Target::new(
            "div",
            "library-page-header",
            &["fs-page-header-module-container"],
        );
        header_target
            .attributes
            .insert("data-visible".into(), "true".into());
        let parents = vec![
            header_target.clone(),
            Target::new("div", "", &["fs-page-header-module-header"]),
            Target::new("div", "", &["fs-page-header-module-title-wrapper"]),
            Target::new(
                "div",
                "",
                &["fs-library-header-bar-module-header-container"],
            ),
        ];
        let mut trigger_target = Target::new(
            "button",
            "library-play-menu-trigger",
            &[
                "fs-action-icon-module-root",
                "mantine-ActionIcon-root",
                "fs-play-button-module-text-button",
            ],
        );
        trigger_target
            .attributes
            .insert("data-variant".into(), "filled".into());
        let play = Popover::new("library-play-menu")
            .anchor(Corner::TopLeft)
            .centered(Axis::Horizontal)
            .offset(px(8. * zoom))
            .appearance(false)
            .trigger(
                action(
                    "library-play-menu-trigger",
                    "play",
                    self.tr("player.play"),
                    cx,
                )
                .primary()
                .state_style(crate::styles::state_style_path(
                    trigger_target,
                    parents.clone(),
                ))
                .size(rem * 3.)
                .rounded_full()
                .opacity(if self.library_fetches.is_empty() {
                    0.8
                } else {
                    0.6
                })
                .loading(!self.library_fetches.is_empty())
                .icon(
                    Icon::default()
                        .path("native-icons/play.svg")
                        .with_size(rem * 1.125),
                ),
            )
            .content(move |_, window, cx| {
                let popover = cx.weak_entity();
                window.use_keyed_state("library-play-actions-view", cx, |_, _| ListPlayMenu {
                    app: app.clone(),
                    popover,
                })
            });
        let playlist = match &self.route {
            Route::Detail(item) if item.kind == ItemKind::Playlist => Some(item.as_ref()),
            _ => None,
        };
        let title = playlist.map_or_else(
            || {
                self.tr(if self.route == Route::Library(ItemKind::Album) {
                    "page.albumList.title"
                } else {
                    "page.trackList.title"
                })
            },
            |item| self.playlist_header_title(item),
        );
        let heading = crate::styles::apply_in(
            div()
                .id("library-page-title")
                .debug_selector(|| "library-page-title".into())
                .role(Role::Heading)
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(rem * 2.25)
                .line_height(rem * 2.75)
                .font_weight(FontWeight::BOLD)
                .child(title),
            Target::new(
                "h1",
                "library-page-title",
                &["mantine-Title-root", "fs-text-title-module-root"],
            ),
            parents.clone(),
            cx,
        );
        let title_bar = div()
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .w_full()
            .h_full()
            .flex_1()
            .min_w_0()
            .px(rem * 0.5)
            .gap(rem)
            .child(play)
            .child(heading)
            .when_some(playlist, |bar, item| {
                bar.when(item.rules.is_some(), |bar| {
                    bar.child(
                        badge("playlist-smart-badge", zoom, cx)
                            .child(self.tr("entity.smartPlaylist").to_uppercase()),
                    )
                })
                .when(item.duration > 0., |bar| {
                    bar.child(
                        badge("playlist-duration-badge", zoom, cx).child(
                            feishin_core::metadata_format::duration_text(
                                item.duration,
                                &self.state.settings.language,
                                false,
                            )
                            .to_uppercase(),
                        ),
                    )
                })
            })
            .child(
                badge("library-page-count", zoom, cx)
                    .when(self.loading, |div| {
                        div.child(
                            gpui::div()
                                .id("library-count-loading")
                                .role(Role::ProgressIndicator)
                                .aria_label(self.tr("Loading..."))
                                .child(crate::speaker_picker::spinner(rem * 0.75)),
                        )
                    })
                    .when(!self.loading, |div| {
                        div.child(self.total.map_or_else(
                            || self.items.len().to_string(),
                            |total| total.to_string(),
                        ))
                    }),
            )
            .map(|bar| crate::styles::apply_in(bar, parents[3].clone(), parents[..3].to_vec(), cx));
        let wrapper = div()
            .h_flex()
            .size_full()
            .px(rem * 0.25)
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .child(title_bar)
            .child(self.list_search_control(false, false, cx))
            .map(|wrapper| {
                crate::styles::apply_in(wrapper, parents[2].clone(), parents[..2].to_vec(), cx)
            });
        let header = div()
            .id("library-page-header")
            .debug_selector(|| "library-page-header".into())
            .w_full()
            .h(px(65. * zoom))
            .flex_shrink_0()
            .overflow_hidden()
            .bg(cx.theme().background)
            .child(crate::styles::apply_in(
                div().relative().size_full().child(wrapper),
                parents[1].clone(),
                parents[..1].to_vec(),
                cx,
            ))
            .map(|header| crate::styles::apply_in(header, header_target, vec![], cx));
        let sort = crate::styles::button("sort", cx)
            .debug_selector(|| "library-sort".into())
            .ghost()
            .label(self.tr(self.sort.label()))
            .dropdown_menu({
                let app = cx.weak_entity();
                move |mut menu, _, cx| {
                    for sort in Sort::ALL {
                        let app = app.clone();
                        menu = menu.item(
                            PopupMenuItem::new(crate::appearance::text(sort.label(), cx)).on_click(
                                move |_, _, cx| {
                                    app.update(cx, |app, cx| {
                                        app.sort = sort;
                                        app.reload(false, cx);
                                    })
                                    .ok();
                                },
                            ),
                        );
                    }
                    menu
                }
            });
        let display = self.list_config().display;
        let (icon, label) = match display.as_str() {
            "poster" => ("layout-grid", "table.config.view.grid"),
            "detail" => ("layout-detail", "table.config.view.detail"),
            _ => ("layout-table", "table.config.view.table"),
        };
        let bar = div()
            .id("library-filter-bar")
            .debug_selector(|| "library-filter-bar".into())
            .h_flex()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .flex_shrink_0()
            .w_full()
            .p(rem * 0.25)
            .gap(rem * 0.5)
            .border_b(px(zoom))
            .border_color(cx.theme().border)
            .child(sort)
            .child(div().w(px(zoom)).h(rem * 2.25).bg(cx.theme().border))
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
                .on_click(cx.listener(|app, _, _, cx| {
                    app.descending = !app.descending;
                    app.reload(false, cx);
                })),
            )
            .child(
                action("library-filter-settings", "filter", self.tr("Filters"), cx)
                    .when(!self.active_filters.is_empty(), |button| {
                        button.text_color(cx.theme().primary)
                    })
                    .on_click(cx.listener(|app, _, window, cx| app.edit_filters(window, cx))),
            )
            .child(
                action("refresh-library", "refresh", self.tr("Refresh"), cx)
                    .disabled(self.loading)
                    .on_click(cx.listener(|app, _, _, cx| app.refresh_library(cx))),
            )
            .child(div().flex_1())
            .child(
                action("view", icon, self.tr(label), cx).on_click(cx.listener(|app, _, _, cx| {
                    let mut config = app.list_config();
                    config.display = match config.display.as_str() {
                        "poster" if app.route == Route::Library(ItemKind::Album) => "detail",
                        "poster" | "detail" => "table",
                        _ => "poster",
                    }
                    .into();
                    app.state.settings.lists.insert(app.list_key(), config);
                    app.save();
                    cx.notify();
                })),
            )
            .child(
                action(
                    "library-list-settings",
                    "settings",
                    self.tr("List layout"),
                    cx,
                )
                .on_click(cx.listener(|app, _, window, cx| {
                    app.edit_list_config(app.list_key(), window, cx)
                })),
            );
        let bar = if self.has_playlist_header() {
            self.playlist_filter_bar(cx)
        } else {
            crate::styles::apply(
                bar,
                "div",
                "library-filter-bar",
                &["fs-filter-bar-module-filter-bar"],
                None,
                cx,
            )
            .into_any_element()
        };
        div()
            .v_flex()
            .w_full()
            .flex_shrink_0()
            .child(header)
            .child(bar)
            .into_any_element()
    }
}

struct ListPlayMenu {
    app: WeakEntity<Feishin>,
    popover: WeakEntity<PopoverState>,
}
impl Render for ListPlayMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (zoom, busy) = self
            .app
            .read_with(cx, |app, _| {
                (app.state.settings.zoom, !app.library_fetches.is_empty())
            })
            .unwrap_or((1., true));
        div()
            .id("library-play-actions")
            .debug_selector(|| "library-play-actions".into())
            .h_flex()
            .h(px(64. * zoom))
            .gap(cx.theme().font_size * 0.5)
            .children(
                [
                    (Enqueue::Next, "next-visible"),
                    (Enqueue::Now, "play-visible"),
                    (Enqueue::End, "add-visible"),
                ]
                .map(|(mode, id)| {
                    let app = self.app.clone();
                    let held_app = app.clone();
                    let popover = self.popover.clone();
                    let held_popover = popover.clone();
                    div()
                        .when(!matches!(mode, Enqueue::Now), |div| {
                            div.relative().top(px(-16. * zoom))
                        })
                        .child(
                            crate::styles::play_control(id, mode, zoom, cx)
                                .disabled(busy)
                                .on_click(move |_, window, cx| {
                                    app.update(cx, |app, cx| {
                                        if app.has_playlist_header() {
                                            app.play_detail(mode, false, cx);
                                        } else {
                                            app.enqueue_visible(mode, cx);
                                        }
                                    })
                                    .ok();
                                    popover
                                        .update(cx, |popover, cx| popover.dismiss(window, cx))
                                        .ok();
                                })
                                .on_long_press(move |_, window, cx| {
                                    held_app
                                        .update(cx, |app, cx| {
                                            if app.has_playlist_header() {
                                                app.play_detail(mode, true, cx);
                                            } else {
                                                app.enqueue_visible_shuffled(mode, cx);
                                            }
                                        })
                                        .ok();
                                    held_popover
                                        .update(cx, |popover, cx| popover.dismiss(window, cx))
                                        .ok();
                                }),
                        )
                }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Enqueue, Feishin, ItemKind, Route};
    use gpui::{AppContext, Modifiers, MouseButton, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc, time::Duration};

    #[gpui::test]
    fn full_library_header_matches_original_and_dispatches_controls(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let dir =
            std::env::temp_dir().join(format!("feishin-list-header-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = feishin_core::storage::State::default();
            state.settings.follow_system_theme = false;
            state.settings.window_bar_style = "web".into();
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        let app = captured.borrow_mut().take().unwrap();
        app.update(cx, |view, cx| {
            view.route = Route::Library(ItemKind::Album);
            view.items.clear();
            view.total = Some(4);
            view.message.clear();
            cx.notify();
        });
        cx.run_until_parked();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/library-page-header-reference.json"))
                .unwrap();
        let check = |selector: &str, expected: &serde_json::Value, cx: &mut VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"));
            for (value, expected) in [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ]
            .into_iter()
            .zip(expected.as_array().unwrap())
            {
                assert!(
                    (f32::from(value) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "{selector}: {bounds:?} != {expected}"
                );
            }
        };
        for (selector, key) in [
            ("library-page-header", "page"),
            ("library-page-title", "title"),
            ("library-play-menu-trigger", "play"),
            ("library-page-count", "badge"),
            ("library-filter-bar", "filter"),
            ("library-search", "search"),
        ] {
            check(selector, &reference[key], cx);
        }
        for (index, selector) in [
            "library-sort",
            "sort-order",
            "library-filter-settings",
            "refresh-library",
            "view",
            "library-list-settings",
        ]
        .into_iter()
        .enumerate()
        {
            check(selector, &reference["buttons"][index]["bounds"], cx);
        }
        let click = |selector: &str, cx: &mut VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
        };
        let button_present = |label: &str, cx: &mut VisualTestContext| {
            cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.role() == gpui::Role::Button && node.label() == Some(label)
                    })
            })
        };
        assert!(!button_present("Panels", cx));
        assert!(!button_present("Play\nHold to shuffle", cx));
        for expected in ["detail", "table", "poster"] {
            click("view", cx);
            assert_eq!(
                app.read_with(cx, |view, _| view.list_config().display),
                expected
            );
            assert_eq!(
                feishin_core::storage::load(&dir.join("state.json"))
                    .unwrap()
                    .settings
                    .lists["album"]
                    .display,
                expected
            );
        }
        app.update(cx, |view, cx| {
            view.route = Route::Library(ItemKind::Song);
            view.state.settings.confirm_large_fetch = true;
            // Retain the request before the normal prompt consumer takes it.
            view.library_fetch_prompt_open = true;
            cx.notify();
        });
        cx.run_until_parked();
        for expected in ["poster", "table"] {
            click("view", cx);
            assert_eq!(
                app.read_with(cx, |view, _| view.list_config().display),
                expected
            );
        }
        for (selector, mode) in [
            ("next-visible", Enqueue::Next),
            ("play-visible", Enqueue::Now),
            ("add-visible", Enqueue::End),
        ] {
            for shuffle in [false, true] {
                click("library-play-menu-trigger", cx);
                assert!(button_present("Play\nHold to shuffle", cx));
                if !shuffle && matches!(mode, Enqueue::Next) {
                    for (index, selector) in ["next-visible", "play-visible", "add-visible"]
                        .into_iter()
                        .enumerate()
                    {
                        check(selector, &reference["playActions"][index], cx);
                    }
                }
                assert!(app.read_with(cx, |view, _| view.pending_library_fetch.is_none()));
                let bounds = cx.debug_bounds(selector).unwrap();
                if shuffle {
                    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
                    cx.run_until_parked();
                    cx.executor().advance_clock(Duration::from_millis(500));
                    cx.run_until_parked();
                    cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());
                    cx.run_until_parked();
                } else {
                    click(selector, cx);
                }
                app.update(cx, |view, _| {
                    let request = view
                        .pending_library_fetch
                        .take()
                        .unwrap_or_else(|| panic!("{selector}, shuffle={shuffle}: action must dispatch a list request; route={:?}, prompt={}", view.route, view.library_fetch_prompt_open));
                    assert_eq!(
                        std::mem::discriminant(&request.mode),
                        std::mem::discriminant(&mode)
                    );
                    assert_eq!(request.shuffle, shuffle);
                    assert!(view.state.queue.entries.is_empty());
                });
                assert!(!button_present("Play\nHold to shuffle", cx));
            }
        }
        click("library-play-menu-trigger", cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!button_present("Play\nHold to shuffle", cx));
        assert!(app.read_with(cx, |view, _| view.pending_library_fetch.is_none()));
        std::fs::remove_dir_all(dir).ok();
    }
}
