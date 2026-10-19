use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{model::ItemKind, sidebar::Config};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
};

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "sidebar_action_tests.rs"]
mod action_tests;

fn route(id: &str) -> Option<(Route, IconName)> {
    Some(match id {
        "Now Playing" => (Route::Queue, IconName::PanelRight),
        "Search" => (
            Route::Search(String::new(), ItemKind::Song),
            IconName::Search,
        ),
        "Home" => (Route::Home, IconName::LayoutDashboard),
        "Favorites" => (Route::Favorites(ItemKind::Song), IconName::Heart),
        "Albums" => (
            Route::Library(ItemKind::Album),
            IconName::GalleryVerticalEnd,
        ),
        "Tracks" => (Route::Library(ItemKind::Song), IconName::Menu),
        "Artists" => (Route::Library(ItemKind::AlbumArtist), IconName::User),
        "Artists-all" => (Route::Library(ItemKind::Artist), IconName::User),
        "Genres" => (Route::Library(ItemKind::Genre), IconName::Palette),
        "Folders" => (Route::Library(ItemKind::Folder), IconName::Folder),
        "Playlists" => (Route::Library(ItemKind::Playlist), IconName::BookOpen),
        "Collections" => (Route::Collections, IconName::Folder),
        "Radio" => (Route::Library(ItemKind::Radio), IconName::Globe),
        "Settings" => (Route::Settings, IconName::Settings),
        "Explore" => (Route::Explore, IconName::Globe),
        "Queue" => (Route::Queue, IconName::PanelRight),
        "Visualizer" => (Route::Visualizer, IconName::Palette),
        _ => return None,
    })
}
fn label_key(id: &str) -> Option<&'static str> {
    Some(match id {
        "Now Playing" => "page.sidebar.nowPlaying",
        "Search" => "page.sidebar.search",
        "Home" => "page.sidebar.home",
        "Favorites" => "page.sidebar.favorites",
        "Albums" => "page.sidebar.albums",
        "Tracks" => "page.sidebar.tracks",
        "Artists" => "page.sidebar.albumArtists",
        "Artists-all" => "page.sidebar.artists",
        "Genres" => "page.sidebar.genres",
        "Folders" => "page.sidebar.folders",
        "Playlists" => "page.sidebar.playlists",
        "Collections" => "page.sidebar.collections",
        "Radio" => "page.sidebar.radio",
        "Settings" => "page.sidebar.settings",
        "Menu" => "common.menu",
        _ => return None,
    })
}
pub(crate) fn navigation_icon(id: &str, selected: bool) -> Option<&'static str> {
    let names = match id {
        "Home" => Some(("RiHome6Line", "RiHome6Fill")),
        "Favorites" => Some(("RiHeartLine", "RiHeartFill")),
        "Albums" => Some(("RiAlbumLine", "RiAlbumFill")),
        "Tracks" => Some(("RiMusic2Line", "RiMusic2Fill")),
        "Artists" | "Artists-all" => Some(("RiUserVoiceLine", "RiUserVoiceFill")),
        "Genres" => Some(("RiFlag2Line", "RiFlag2Fill")),
        "Folders" => Some(("RiFolder3Line", "RiFolder3Fill")),
        "Now Playing" | "Queue" => Some(("RiPlayLine", "RiPlayFill")),
        "Search" => Some(("RiSearchLine", "RiSearchFill")),
        "Playlists" => Some(("RiPlayListLine", "RiPlayListFill")),
        "Radio" => Some(("RiRadioLine", "RiRadioFill")),
        "Settings" => Some(("RiSettings2Line", "RiSettings2Fill")),
        "Menu" => Some(("menu", "menu")),
        "Collections" => Some(("collection", "collection")),
        _ => None,
    };
    names.map(|(normal, active)| if selected { active } else { normal })
}

impl Feishin {
    pub(crate) fn sidebar_pixel_width(&self) -> f32 {
        (if self.state.settings.sidebar.collapsed {
            80.
        } else {
            self.sidebar_resize
                .as_ref()
                .map_or(self.state.settings.sidebar.width, |resize| resize.width)
        }) * self.state.settings.zoom
    }
    pub(crate) fn resize_sidebar(
        &mut self,
        position: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(resize) = &mut self.sidebar_resize else {
            return;
        };
        let position = if crate::appearance::rtl(cx) {
            window.viewport_size().width - position
        } else {
            position
        };
        let was_collapsed = self.state.settings.sidebar.collapsed;
        resize.update(
            &mut self.state.settings.sidebar,
            f32::from(position) / self.state.settings.zoom,
        );
        if was_collapsed != self.state.settings.sidebar.collapsed {
            self.sidebar_closed_sections = std::collections::HashSet::from(["shared-playlists"]);
        }
        cx.notify();
    }
    pub(crate) fn finish_sidebar_resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resize) = self.sidebar_resize.take() else {
            return;
        };
        let was_collapsed = self.state.settings.sidebar.collapsed;
        resize.finish(&mut self.state.settings.sidebar);
        if was_collapsed != self.state.settings.sidebar.collapsed {
            self.sidebar_closed_sections = std::collections::HashSet::from(["shared-playlists"]);
        }
        self.sidebar_width.update(cx, |input, cx| {
            input.set_value(self.state.settings.sidebar.width.to_string(), window, cx)
        });
        self.save();
        cx.notify();
    }
    pub(crate) fn sidebar_section_toggle(
        &self,
        section: &'static str,
        label: &str,
        cx: &mut Context<Self>,
    ) -> Button {
        self.sidebar_section_toggle_with_controls(section, label, None, None, cx)
    }
    pub(crate) fn sidebar_section_toggle_with_controls(
        &self,
        section: &'static str,
        label: &str,
        leading: Option<AnyElement>,
        trailing: Option<AnyElement>,
        cx: &mut Context<Self>,
    ) -> Button {
        let open = !self.sidebar_closed_sections.contains(section);
        let rem = cx.theme().font_size;
        let with_controls = leading.is_some() || trailing.is_some();
        let with_trailing = trailing.is_some();
        let contents = div()
            .h_flex()
            .flex_1()
            .min_w_0()
            .when(with_trailing, |div| div.pr(rem * 0.75))
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .when_some(leading, |div, leading| div.child(leading))
            .child(
                div()
                    .debug_selector(move || format!("sidebar-section-label-{section}"))
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(rem * 0.875)
                    .line_height(rem * 1.125)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .when(crate::appearance::rtl(cx), |div| div.text_right())
                    .child(self.tr(label)),
            )
            .when_some(trailing, |div, trailing| div.child(trailing));
        Button::new(SharedString::from(format!("sidebar-section-{section}")))
            .label_full_width()
            .debug_selector(move || format!("sidebar-section-{section}"))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(transparent_black())
                    .foreground(cx.theme().foreground)
                    .hover(cx.theme().background)
                    .active(cx.theme().background),
            )
            .w_full()
            .h(rem * if with_controls { 2.875 } else { 2.5 })
            .flex_shrink_0()
            .min_w_0()
            .px(rem * 0.75)
            .py_0()
            .border_0()
            .rounded(cx.theme().radius)
            .aria_label(self.tr(label))
            .aria_expanded(open)
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .child(contents)
                    .child(
                        div()
                            .debug_selector(move || format!("sidebar-section-chevron-{section}"))
                            .size(rem)
                            .flex_shrink_0()
                            .child(crate::sidebar_motion::SectionChevron {
                                id: format!("sidebar-section-motion-{section}").into(),
                                open,
                                size: rem,
                            }),
                    ),
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                if !view.sidebar_closed_sections.remove(section) {
                    view.sidebar_closed_sections.insert(section);
                }
                cx.notify();
            }))
    }
    fn sidebar_history_button(&self, forward: bool, cx: &mut Context<Self>) -> Button {
        let id = if forward {
            "sidebar-forward"
        } else {
            "sidebar-back"
        };
        let button = if self.state.settings.sidebar.collapsed {
            let mut target = feishin_core::styles::Target::new(
                "button",
                id,
                &[
                    "fs-action-icon-module-root",
                    "mantine-ActionIcon-root",
                    "fs-collapsed-sidebar-button-module-button",
                ],
            );
            target
                .attributes
                .insert("data-variant".into(), "subtle".into());
            let background = cx.theme().background.to_rgb();
            let mix = if cx.theme().is_dark() { 0.05 } else { 0. };
            let hover = Rgba {
                r: background.r * 0.95 + mix,
                g: background.g * 0.95 + mix,
                b: background.b * 0.95 + mix,
                a: background.a,
            }
            .into();
            Button::new(id)
                .action_icon(self.state.settings.zoom)
                .state_style(crate::styles::state_style_path(target, Vec::new()))
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(transparent_black())
                        .foreground(cx.theme().foreground)
                        .hover(hover)
                        .active(hover),
                )
                .border_1()
                .border_color(transparent_black())
                .rounded(cx.theme().font_size * (3. / 16.))
        } else {
            crate::styles::button(id, cx)
        };
        button
            .debug_selector(move || id.into())
            .tooltip(self.tr(if forward { "Forward" } else { "Back" }))
            .small()
            .p_0()
            .min_w_0()
            .icon(
                Icon::default()
                    .path(if forward {
                        "native-icons/arrow-right.svg"
                    } else {
                        "native-icons/arrow-left.svg"
                    })
                    .with_size(cx.theme().font_size),
            )
            .disabled(if forward {
                self.forward_history.is_empty()
            } else {
                self.history.is_empty()
            })
            .on_click(cx.listener(move |view, _, _, cx| {
                if forward {
                    view.forward(cx)
                } else {
                    view.back(cx)
                }
            }))
    }

    fn collapsed_sidebar_history(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rem = f32::from(cx.theme().font_size);
        div()
            .h_flex()
            .flex_wrap()
            .flex_shrink_0()
            .w_full()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .children([false, true].map(|forward| {
                self.sidebar_history_button(forward, cx)
                    .flex_1()
                    .min_w(px(rem * 2.25))
                    .max_w(px(40. * self.state.settings.zoom))
                    .h(px(rem * 2.925 + 2. * self.state.settings.zoom))
                    .icon(
                        Icon::default()
                            .path(if forward {
                                "native-icons/arrow-right.svg"
                            } else {
                                "native-icons/arrow-left.svg"
                            })
                            .with_size(px(rem * 1.125)),
                    )
            }))
    }

    pub(crate) fn sidebar_action_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let rem = f32::from(cx.theme().font_size);
        let width = self.sidebar_pixel_width();
        // Original Grid columns are 7/12 and 5/12, including their half gutters.
        let search_width = ((width - rem) * 7. / 12. - rem * 0.5).max(0.);
        let button_width = ((width - rem * 3. - search_width) / 3.).max(0.);
        let button_height = rem * 2.25;
        div()
            .id("sidebar-action-bar")
            .debug_selector(|| "sidebar-action-bar".into())
            .h_flex()
            .flex_shrink_0()
            .w(px(width))
            .h(px(65. * self.state.settings.zoom))
            .px(px(rem * 0.75))
            .gap(px(rem * 0.5))
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .child(
                crate::styles::button("sidebar-command-palette", cx)
                    .debug_selector(|| "sidebar-command-palette".into())
                    .focus_handle(&self.sidebar_search_focus)
                    .key_context("FormControl")
                    .small()
                    .w(px(search_width))
                    .h(px(button_height))
                    .min_w_0()
                    .flex_shrink_0()
                    .px(px((rem * 0.6875 - self.state.settings.zoom).max(0.)))
                    .justify_start()
                    .when(crate::appearance::rtl(cx), |button| button.justify_end())
                    .font_weight(FontWeight::NORMAL)
                    .text_color(cx.theme().muted_foreground)
                    .aria_label(self.tr("common.search"))
                    .child(
                        div()
                            .h_flex()
                            .gap(px(rem * 0.5625))
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .child(
                                div()
                                    .debug_selector(|| "sidebar-search-icon".into())
                                    .size(px(rem * 0.875))
                                    .flex_shrink_0()
                                    .child(
                                        Icon::default()
                                            .path("native-icons/search.svg")
                                            .with_size(px(rem * 0.875)),
                                    ),
                            )
                            .child(
                                div()
                                    .max_w(px((search_width - rem * 2.875).max(0.)))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(self.tr("common.search")),
                            ),
                    )
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.sidebar_search_focus.focus(window);
                        view.open_command_palette(window, cx)
                    })),
            )
            .child(self.application_menu_button(Some(button_width), cx))
            .children([false, true].map(|forward| {
                self.sidebar_history_button(forward, cx)
                    .w(px(button_width))
                    .h(px(button_height))
                    .flex_shrink_0()
            }))
    }

    pub(crate) fn sidebar_navigation_control(
        &self,
        id: &str,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Button {
        let rem = f32::from(cx.theme().font_size);
        let zoom = self.state.settings.zoom;
        let collapsed = self.state.settings.sidebar.collapsed;
        let menu_tile = matches!(id, "Menu" | "Collections");
        let label = self.tr(label_key(id).unwrap_or(id));
        let icon = match navigation_icon(id, selected) {
            Some(name) => Icon::default().path(format!("native-icons/{name}.svg")),
            None => Icon::new(route(id).map_or(IconName::Folder, |(_, icon)| icon)),
        };
        let icon_size = if menu_tile {
            rem * 1.5
        } else if collapsed {
            25. * zoom
        } else {
            rem * 0.875
        };
        let icon_selector = format!("sidebar-navigation-icon-{id}");
        let label_selector = format!("sidebar-navigation-label-{id}");
        let button_selector = format!("sidebar-navigation-control-{id}");
        let marker_selector = format!("sidebar-navigation-marker-{id}");
        let button_id = SharedString::from(format!("nav-{id}"));
        let button = if collapsed {
            Button::new(button_id)
        } else {
            crate::styles::button(button_id, cx)
        }
        .debug_selector(move || button_selector.clone())
        .ghost()
        .small()
        .w_full()
        .selected(selected)
        .tooltip(label.clone())
        .aria_label(label.clone())
        .bg(cx.theme().transparent);
        let icon = div()
            .debug_selector(move || icon_selector.clone())
            .size(px(icon_size))
            .flex_shrink_0()
            .child(if id == "Menu" {
                self.application_menu_icon(px(icon_size))
            } else {
                icon.with_size(px(icon_size)).into_any_element()
            });
        if collapsed {
            let label = if id == "Artists" {
                label.replacen(' ', "\n", 1)
            } else {
                label
            };
            button
                .relative()
                .w(px(80. * zoom))
                .h_auto()
                .border_0()
                .rounded_none()
                .p_0()
                .text_color(cx.theme().muted_foreground)
                .state_style(std::rc::Rc::new(|states, cx| {
                    div()
                        .bg(cx.theme().transparent)
                        .when(states.contains(&"hover"), |div| {
                            div.text_color(cx.theme().foreground)
                        })
                        .style()
                        .clone()
                }))
                .child(
                    div()
                        .v_flex()
                        .items_center()
                        .w(px(80. * zoom))
                        .px(px(if menu_tile { 0. } else { rem * 0.3 }))
                        .py(px(rem * if menu_tile { 0.75 } else { 0.9 }))
                        .child(icon.when(selected, |icon| icon.text_color(cx.theme().primary)))
                        .child(
                            div()
                                .debug_selector(move || label_selector.clone())
                                .w_full()
                                .overflow_hidden()
                                .text_center()
                                .text_size(px(rem * 0.6875))
                                .line_height(px(rem * 0.875))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(if selected {
                                    cx.theme().foreground
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .child(label),
                        ),
                )
                .when(selected, |button| {
                    button.child(
                        div()
                            .debug_selector(move || marker_selector.clone())
                            .absolute()
                            .map(|div| {
                                if crate::appearance::rtl(cx) {
                                    div.right_0()
                                } else {
                                    div.left(px(3. * zoom))
                                }
                            })
                            .top(relative(0.1))
                            .h(relative(0.8))
                            .w(px(2. * zoom))
                            .bg(cx.theme().primary),
                    )
                })
        } else {
            button
                .h(px(rem * 2.25))
                .px(px(rem * 0.75))
                .justify_start()
                .when(crate::appearance::rtl(cx), |button| button.justify_end())
                .text_size(px(rem * 0.875))
                .text_color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().foreground
                })
                .child(
                    div()
                        .debug_selector(move || label_selector.clone())
                        .h_flex()
                        .line_height(relative(1.))
                        .text_color(if selected {
                            cx.theme().primary
                        } else {
                            cx.theme().foreground
                        })
                        .gap(px(rem * 0.75))
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .child(icon)
                        .child(div().whitespace_nowrap().child(label)),
                )
        }
    }

    pub fn sidebar_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.sidebar;
        let collapsed = config.collapsed;
        let padding = if collapsed {
            0.
        } else {
            f32::from(cx.theme().font_size) * 0.75
        };
        let content =
            div()
                .id("sidebar")
                .role(gpui::Role::Navigation)
                .aria_label(self.tr("Navigation"))
                .debug_selector(|| "sidebar".into())
                .v_flex()
                .h_full()
                .gap_1()
                .when(collapsed, |div| div.gap_0())
                .w_full()
                .flex_shrink_0()
                .bg(cx.theme().sidebar)
                .when(!collapsed, |div| div.child(self.sidebar_action_bar(cx)))
                .child(
                    div()
                        .id("sidebar-scroll")
                        .debug_selector(|| "sidebar-scroll".into())
                        .native_vertical_scroll()
                        .v_flex()
                        .flex_1()
                        .min_h_0()
                        .gap_0()
                        .px(px(padding))
                        .pb(px(padding))
                        .when(collapsed, |div| {
                            div.gap_0()
                                .when(config.collapsed_navigation, |div| div.child(self.collapsed_sidebar_history(cx)))
                                .child(self.application_menu_for_button(
                                    self.sidebar_navigation_control("Menu", false, cx)
                                        .debug_selector(|| "application-menu".into()), true, cx))
                        })
                        .when(!collapsed, |div| {
                            div.child(self.sidebar_section_toggle(
                                "library",
                                "page.sidebar.myLibrary",
                                cx,
                            ))
                        })
                        .child({
                            let library = div().v_flex().flex_shrink_0()
                            .when(!collapsed, |div| div.pb(px(padding)))
                            .children(
                            config
                                .items
                                .iter()
                                .filter(|entry| {
                                    !entry.disabled
                                        && (collapsed || entry.id != "Collections")
                                })
                                .filter_map(|entry| {
                                    if collapsed && entry.id == "Collections" {
                                        return (!self.state.settings.collections.is_empty())
                                            .then(|| self.collapsed_sidebar_collections(cx));
                                    }
                                    let (target, _) = route(&entry.id)?;
                                    let selected = matches!((&self.route, &target), (Route::Collection(collection), Route::Library(kind)) if collection.kind == *kind)
                                        || (std::mem::discriminant(&self.route) == std::mem::discriminant(&target)
                                            && match (&self.route, &target) {
                                                (Route::Library(a), Route::Library(b)) => a == b,
                                                _ => true,
                                            });
                                    let queue = target == Route::Queue;
                                    let selector = entry.id.clone();
                                    Some(
                                div()
                                    .id(SharedString::from(format!("nav-drop-{}", entry.id)))
                                    .debug_selector(move || format!("sidebar-nav-{selector}"))
                                    .on_drop(cx.listener(
                                        move |this, dragged: &crate::queue::Dragged, _, cx| {
                                            if queue {
                                                this.drop_queue(dragged, None, cx);
                                                cx.stop_propagation();
                                            }
                                        },
                                    ))
                                    .child(
                                        self.sidebar_navigation_control(&entry.id, selected, cx)
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                if matches!(target, Route::Search(..)) {
                                                    this.navigate(target.clone(), cx);
                                                    this.search.update(cx, |input, cx| {
                                                        input.focus(window, cx)
                                                    });
                                                } else {
                                                    this.navigate(target.clone(), cx);
                                                }
                                            }),
                                        ),
                                    )
                                    .into_any_element(),
                            )
                                }),
                            );
                            if collapsed {
                                library.into_any_element()
                            } else {
                                crate::sidebar_motion::SectionCollapse {
                                    id: "sidebar-panel-library".into(),
                                    open: !self.sidebar_closed_sections.contains("library"),
                                    content: library.into_any_element(),
                                }.into_any_element()
                            }
                        })
                        .when(
                            !collapsed && !self.state.settings.collections.is_empty(),
                            |div| {
                                div.child(self.sidebar_section_toggle(
                                    "collections",
                                    "page.sidebar.collections",
                                    cx,
                                ))
                                .child(crate::sidebar_motion::SectionCollapse {
                                    id: "sidebar-panel-collections".into(),
                                    open: !self.sidebar_closed_sections.contains("collections"),
                                    content: self.sidebar_collections(cx),
                                })
                            },
                        )
                        .when(config.playlist_list, |div| {
                            div.child(gpui::div().when(collapsed, |div| div.px(px(6.))).child(self.sidebar_playlists(cx)))
                        }),
                )
                .when(
                    !collapsed,
                    |div| {
                        let item = self.current_metadata().unwrap_or_default();
                        let context = item.clone();
                        let artwork = gpui::div()
                            .id("sidebar-artwork")
                            .debug_selector(|| "sidebar-artwork".into())
                            .relative()
                            .group("sidebar-artwork")
                            .size(px(self.sidebar_pixel_width()))
                            .p(px(padding))
                            .flex_shrink_0()
                            .role(gpui::Role::Button)
                            .focusable()
                            .tab_stop(true)
                            .aria_label(self.tr("player.toggleFullscreenPlayer"))
                            .child(gpui::div().debug_selector(|| "sidebar-artwork-picture".into())
                                .size(px(self.sidebar_pixel_width() - padding * 2.))
                                .child(self.artwork_at(
                                &item,
                                self.sidebar_pixel_width() - padding * 2.,
                                feishin_core::images::Placement::Sidebar,
                                cx,
                            )))
                            .on_click(cx.listener(|this, _, _, cx| this.toggle_full_player(cx)))
                            .child(gpui::div().absolute().top(cx.theme().font_size).right(cx.theme().font_size)
                                .invisible().group_hover("sidebar-artwork", |style| style.visible())
                                .child(Button::new("sidebar-hide-artwork")
                                    .debug_selector(|| "sidebar-hide-artwork".into())
                                    .action_icon(self.state.settings.zoom)
                                    .size(cx.theme().font_size * 2.25)
                                    .p_0()
                                    .rounded(cx.theme().radius)
                                    .opacity(0.8)
                                    .icon(Icon::default().path("native-icons/arrow-down.svg").with_size(cx.theme().font_size))
                                    .tooltip(self.tr("common.collapse"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.settings.sidebar.image_shown = false;
                                        this.save();
                                        cx.stop_propagation();
                                        cx.notify();
                                    }))),
                            );
                        div.child(crate::sidebar_artwork::ArtworkPresence {
                            shown: config.image_enabled && config.image_shown,
                            width: px(self.sidebar_pixel_width()),
                            distance: px(200. * self.state.settings.zoom),
                            content: if item.kind == ItemKind::Song && !item.id.is_empty() {
                            use gpui_component::menu::ContextMenuExt;
                            artwork
                                .context_menu(self.item_menu_builder(
                                    &context,
                                    vec![context.clone()],
                                    false,
                                    cx,
                                ))
                                .into_any_element()
                        } else {
                            artwork.into_any_element()
                        }})
                    },
                )
                .into_any_element();
        div()
            .relative()
            .h_full()
            .w(px(self.sidebar_pixel_width()))
            .flex_shrink_0()
            .child(content)
            .child(
                div()
                    .id("sidebar-resize-handle")
                    .debug_selector(|| "sidebar-resize-handle".into())
                    .role(Role::Splitter)
                    .aria_label(self.tr("Sidebar width"))
                    .aria_orientation(gpui::accesskit::Orientation::Vertical)
                    .aria_numeric_value(f64::from(
                        self.sidebar_pixel_width() / self.state.settings.zoom,
                    ))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .w(px(4. * self.state.settings.zoom))
                    .when(crate::appearance::rtl(cx), |div| div.left_0())
                    .when(!crate::appearance::rtl(cx), |div| div.right_0())
                    .cursor(CursorStyle::ResizeLeftRight)
                    .occlude()
                    .bg(cx.theme().border)
                    .opacity(if self.sidebar_resize.is_some() {
                        1.
                    } else {
                        0.
                    })
                    .hover(|style| style.opacity(0.6))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.sidebar_resize = Some(feishin_core::sidebar::Resize::new(
                                &this.state.settings.sidebar,
                            ));
                            window.prevent_default();
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .when(self.sidebar_resize.is_some(), |div| {
                let moving = cx.entity().downgrade();
                let releasing = moving.clone();
                div.child(
                    canvas(
                        |_, _, _| {},
                        move |_, _, window, _| {
                            window.on_mouse_event(
                                move |event: &MouseMoveEvent, phase, window, cx| {
                                    if phase == DispatchPhase::Bubble
                                        && event.pressed_button == Some(MouseButton::Left)
                                    {
                                        moving
                                            .update(cx, |view, cx| {
                                                view.resize_sidebar(event.position.x, window, cx)
                                            })
                                            .ok();
                                        cx.stop_propagation();
                                    }
                                },
                            );
                            window.on_mouse_event(
                                move |event: &MouseUpEvent, phase, window, cx| {
                                    if phase == DispatchPhase::Bubble
                                        && event.button == MouseButton::Left
                                    {
                                        releasing
                                            .update(cx, |view, cx| {
                                                view.finish_sidebar_resize(window, cx)
                                            })
                                            .ok();
                                        cx.stop_propagation();
                                    }
                                },
                            );
                        },
                    )
                    .absolute()
                    .size_0(),
                )
            })
            .into_any_element()
    }
    pub fn open_sidebar_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_width.update(cx, |input, cx| {
            input.set_value(self.state.settings.sidebar.width.to_string(), window, cx)
        });
        self.sidebar_regex.update(cx, |input, cx| {
            input.set_value(
                self.state.settings.sidebar.playlist_filter_regex.clone(),
                window,
                cx,
            )
        });
        self.navigate(Route::SidebarSettings, cx);
    }
    pub fn apply_sidebar(&mut self, cx: &mut Context<Self>) {
        let mut candidate = self.state.settings.sidebar.clone();
        let result = (|| {
            candidate.width = self.sidebar_width.read(cx).value().trim().parse()?;
            candidate.playlist_filter_regex = self.sidebar_regex.read(cx).value().to_string();
            candidate.validate()?;
            Ok::<_, anyhow::Error>(())
        })();
        match result {
            Ok(()) => {
                self.state.settings.sidebar = candidate;
                self.save();
            }
            Err(error) => self.message = format!("Cannot apply sidebar settings: {error}"),
        }
        cx.notify();
    }
    pub fn sidebar_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.sidebar;
        div()
            .id("sidebar-settings")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(self.tr("Sidebar width (160-640 pixels)"))
            .child(crate::styles::input(&self.sidebar_width, cx))
            .child(self.tr("Hide playlist names matching this regular expression"))
            .child(crate::styles::input(&self.sidebar_regex, cx))
            .child(
                crate::styles::button("apply-sidebar-settings", cx)
                    .label(self.tr("common.save"))
                    .on_click(cx.listener(|this, _, _, cx| this.apply_sidebar(cx))),
            )
            .children(
                [
                    ("Playlist list", config.playlist_list),
                    ("Playlist sorting", config.playlist_sorting),
                    ("Collapsed navigation buttons", config.collapsed_navigation),
                    ("Sidebar image enabled", config.image_enabled),
                    ("Show sidebar image", config.image_shown),
                    (
                        "Collapse shared playlists",
                        self.sidebar_closed_sections.contains("shared-playlists"),
                    ),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (label, checked))| {
                    Checkbox::new(("sidebar-option", index))
                        .label(self.tr(label))
                        .checked(checked)
                        .on_click(cx.listener(move |this, value: &bool, _, cx| {
                            let config = &mut this.state.settings.sidebar;
                            match index {
                                0 => config.playlist_list = *value,
                                1 => config.playlist_sorting = *value,
                                2 => config.collapsed_navigation = *value,
                                3 => config.image_enabled = *value,
                                4 => config.image_shown = *value,
                                _ => {
                                    config.collapse_shared = *value;
                                    if *value {
                                        this.sidebar_closed_sections.insert("shared-playlists");
                                    } else {
                                        this.sidebar_closed_sections.remove("shared-playlists");
                                    }
                                }
                            }
                            this.save();
                            cx.notify();
                        }))
                }),
            )
            .child(
                crate::styles::button("sidebar-playlist-mode", cx)
                    .label(format!("Playlist mode: {}", config.playlist_mode))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.sidebar.playlist_mode =
                            if this.state.settings.sidebar.playlist_mode == "compact" {
                                "expanded"
                            } else {
                                "compact"
                            }
                            .into();
                        this.save();
                        cx.notify();
                    })),
            )
            .children(config.items.iter().enumerate().map(|(index, item)| {
                let label = label_key(&item.id).map_or_else(
                    || {
                        route(&item.id)
                            .map_or(item.id.clone(), |(route, _)| self.tr(&route.title()))
                    },
                    |key| self.tr(key),
                );
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(
                        Checkbox::new(("sidebar-visible", index))
                            .label(label)
                            .checked(!item.disabled)
                            .on_click(cx.listener(move |this, value: &bool, _, cx| {
                                this.state.settings.sidebar.items[index].disabled = !*value;
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button(("sidebar-up", index), cx)
                            .label(crate::appearance::text("↑", cx))
                            .disabled(index == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.sidebar.move_entry(index, true);
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button(("sidebar-down", index), cx)
                            .label(crate::appearance::text("↓", cx))
                            .disabled(index + 1 == config.items.len())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.sidebar.move_entry(index, false);
                                this.save();
                                cx.notify();
                            })),
                    )
            }))
            .child(
                crate::styles::button("sidebar-reset", cx)
                    .label(self.tr("Reset to defaults"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.state.settings.sidebar = Config::default();
                        this.save();
                        this.open_sidebar_settings(window, cx);
                    })),
            )
            .into_any_element()
    }
}
