use crate::app::{Feishin, Route};
use feishin_core::{
    home::Kind,
    model::{Item, ItemKind},
    queue::Enqueue,
    styles::Target,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt, animation::CssTransition, button::ButtonVariants,
};
use std::time::Duration;

fn visible_count(width: f32) -> usize {
    if width >= 1280. {
        18
    } else if width >= 1152. {
        15
    } else if width >= 600. {
        12
    } else if width >= 360. {
        8
    } else {
        6
    }
}
fn target(tag: &str, class: &str) -> Target {
    Target::new(tag, "", &[&format!("fs-featured-genres-module-{class}")])
}
struct Hover {
    fade: CssTransition,
    shadow: CssTransition,
}

impl Feishin {
    pub(crate) fn home_genres_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let measured = window.use_keyed_state("home-genres-width", cx, |_, _| px(0.));
        let width = *measured.read(cx);
        let font = cx.theme().font_size;
        let zoom = self.state.settings.zoom;
        let gap = font * 0.75;
        let columns =
            ((f32::from(width + gap) / (220. * zoom + f32::from(gap))).floor() as usize).max(1);
        let section = self.home_sections.get(&Kind::Genres);
        let items = section.map_or(&[][..], |section| section.items.as_slice());
        let root = target("div", "container");
        let grid_target = target("div", "grid");
        crate::styles::apply_in(
            div()
                .id("home-section-Genres")
                .debug_selector(|| "home-section-Genres".into())
                .relative()
                .v_flex()
                .w_full()
                .gap(gap)
                .child(
                    div()
                        .h_flex()
                        .w_full()
                        .justify_between()
                        .items_end()
                        .when(crate::appearance::rtl(cx), |header| {
                            header.flex_row_reverse()
                        })
                        .child(
                            div()
                                .debug_selector(|| "home-genres-title".into())
                                .text_size(font * 1.5)
                                .line_height(font * 2.)
                                .font_weight(FontWeight::BOLD)
                                .child(
                                    feishin_core::i18n::translate_with(
                                        &self.state.settings.language,
                                        "entity.genre",
                                        &serde_json::json!({"count":2}),
                                    )
                                    .unwrap_or_else(|| "Genres".into()),
                                ),
                        )
                        .child(
                            crate::styles::button("home-genres-more", cx)
                                .debug_selector(|| "home-genres-more".into())
                                .ghost()
                                .with_size(font * 1.625)
                                .h(font * 1.625)
                                .px(font * 0.5)
                                .border_1()
                                .border_color(transparent_black())
                                .text_size(font * 13. / 16.)
                                .text_color(cx.theme().primary)
                                .font_weight(FontWeight::SEMIBOLD)
                                .aria_label(self.tr("action.viewMore"))
                                .child(
                                    div()
                                        .text_size(font * 13. / 16.)
                                        .line_height(relative(1.))
                                        .child(self.tr("action.viewMore")),
                                )
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.navigate(Route::Library(ItemKind::Genre), cx)
                                })),
                        ),
                )
                .when(section.is_some_and(|section| section.loading), |view| {
                    view.child(self.tr("Loading..."))
                })
                .when_some(
                    section.filter(|section| !section.error.is_empty()),
                    |view, section| {
                        view.child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(section.error.clone()),
                        )
                    },
                )
                .child(crate::styles::apply_in(
                    div()
                        .grid()
                        .grid_cols(columns as u16)
                        .gap(gap)
                        .py(font * 0.25)
                        .w_full()
                        .children(
                            items
                                .iter()
                                .take(visible_count(f32::from(width) / zoom))
                                .enumerate()
                                .map(|(index, item)| {
                                    div()
                                        .min_w_0()
                                        .col_start(if crate::appearance::rtl(cx) {
                                            columns - index % columns
                                        } else {
                                            index % columns + 1
                                        } as i16)
                                        .row_start((index / columns + 1) as i16)
                                        .child(self.home_genre_tile(item, index, window, cx))
                                }),
                        ),
                    grid_target,
                    [root.clone()],
                    cx,
                ))
                .child(
                    canvas(
                        move |bounds, window, cx| {
                            let measured = measured.clone();
                            window.defer(cx, move |_, cx| {
                                measured.update(cx, |width, cx| {
                                    if *width != bounds.size.width {
                                        *width = bounds.size.width;
                                        cx.notify();
                                    }
                                })
                            });
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                ),
            root,
            [],
            cx,
        )
        .into_any_element()
    }

    fn home_genre_tile(
        &self,
        item: &Item,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = window.use_keyed_state(("home-genre-hover", index), cx, |_, _| Hover {
            fade: CssTransition::new(0., Duration::from_millis(150), [0., 0., 0.58, 1.]),
            shadow: CssTransition::new(0., Duration::from_millis(200), [0.25, 0.1, 0.25, 1.]),
        });
        let now = cx.background_executor().now();
        let (opacity, moving) = state.read(cx).fade.value(now);
        let (shadow, shadow_moving) = state.read(cx).shadow.value(now);
        if moving || shadow_moving {
            window.request_animation_frame();
        }
        let font = cx.theme().font_size;
        let zoom = self.state.settings.zoom;
        let [r, g, b] = feishin_core::genres::badge_color(&item.name).0;
        let color = rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
        let mut tile_target = target("div", "genre-container");
        tile_target
            .inline_declarations
            .push(("--genre-color".into(), format!("#{r:02x}{g:02x}{b:02x}")));
        let parents = vec![target("div", "container"), target("div", "grid")];
        let mut link_target = target("a", "genre-link");
        if state.read(cx).fade.target() != 0. {
            link_target.states.push("hover".into());
        }
        let mut path = parents.clone();
        path.push(tile_target.clone());
        let mut children_path = path.clone();
        children_path.push(link_target.clone());
        let open = item.clone();
        let key_open = item.clone();
        let genre = item.id.clone();
        crate::styles::apply_in(
            div()
                .id(("home-genre", index))
                .debug_selector(move || format!("home-genre-{index}"))
                .relative()
                .min_h(font * 3.)
                .w_full()
                .overflow_hidden()
                .rounded(font * 5. / 16.)
                .bg(cx.theme().secondary)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .bottom_0()
                        .w(font * 0.5)
                        .bg(color),
                )
                .child(crate::styles::apply_in(
                    div()
                        .id(("home-genre-link", index))
                        .role(Role::Link)
                        .aria_label(item.name.clone())
                        .focusable()
                        .tab_stop(true)
                        .relative()
                        .h_flex()
                        .items_center()
                        .w_full()
                        .min_w_0()
                        .min_h(font * 3.)
                        .px(font * 1.5)
                        .text_size(font * 14. / 16.)
                        .line_height(font * 18. / 16.)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_align(TextAlign::Left)
                        .cursor_pointer()
                        .shadow(vec![BoxShadow {
                            color: black().opacity(0.2 * shadow),
                            offset: point(px(0.), px(4. * zoom * shadow)),
                            blur_radius: px(8. * zoom * shadow),
                            spread_radius: px(0.),
                        }])
                        .on_hover(move |visible, _, cx| {
                            state.update(cx, |state, cx| {
                                let now = cx.background_executor().now();
                                state.fade.retarget(f32::from(*visible), now);
                                state.shadow.retarget(f32::from(*visible), now);
                                cx.notify();
                            })
                        })
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.navigate(Route::Detail(Box::new(open.clone())), cx)
                        }))
                        .on_key_down(cx.listener(move |view, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "enter" {
                                view.navigate(Route::Detail(Box::new(key_open.clone())), cx);
                                cx.stop_propagation();
                            }
                        }))
                        .child(crate::styles::apply_in(
                            div()
                                .debug_selector(move || format!("home-genre-name-{index}"))
                                .flex_1()
                                .min_w_0()
                                .mr(font * 1.5)
                                .overflow_hidden()
                                .text_ellipsis()
                                .font_weight(FontWeight::MEDIUM)
                                .child(item.name.clone()),
                            target("span", "genre-name"),
                            children_path.clone(),
                            cx,
                        ))
                        .child(crate::styles::apply_in(
                            div()
                                .id(("home-genre-play-wrapper", index))
                                .debug_selector(move || format!("home-genre-play-wrapper-{index}"))
                                .absolute()
                                .top(relative(0.5))
                                .mt(px(-16. * zoom))
                                .right(font * 0.75 - px(8. * zoom * (1. - opacity)))
                                .opacity(opacity)
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(|_, _, cx| cx.stop_propagation())
                                .child(
                                    crate::styles::play_control(
                                        SharedString::from(format!("home-genre-play-{index}")),
                                        Enqueue::Now,
                                        zoom,
                                        cx,
                                    )
                                    .with_size(px(32. * zoom))
                                    .size(px(32. * zoom))
                                    .rounded(px(16. * zoom))
                                    .disabled(!self.library_fetches.is_empty())
                                    .loading(!self.library_fetches.is_empty())
                                    .on_long_press(|_, _, cx| cx.stop_propagation())
                                    .on_click(cx.listener(
                                        move |view, _, _, cx| {
                                            view.enqueue_random_config(
                                                feishin_core::shuffle::Config {
                                                    genre: genre.clone(),
                                                    ..Default::default()
                                                },
                                                Enqueue::Now,
                                                false,
                                                cx,
                                            );
                                            cx.stop_propagation();
                                        },
                                    )),
                                ),
                            target("span", "play-button-wrapper"),
                            children_path,
                            cx,
                        )),
                    link_target,
                    path,
                    cx,
                )),
            tile_target,
            parents,
            cx,
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use crate::app::Feishin;
    use gpui::TestAppContext;
    #[gpui::test]
    fn featured_genres_match_original_responsive_tiles_and_hover_controls(cx: &mut TestAppContext) {
        use feishin_core::{
            home::Kind,
            model::{Item, ItemKind},
            storage::State,
        };
        use gpui::{prelude::*, *};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc, time::Duration};
        struct Probe {
            app: Entity<Feishin>,
            width: Pixels,
            home: bool,
            _watch: Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .font(crate::fonts::app_font(cx))
                    .w(self.width)
                    .h_full()
                    .child(self.app.update(cx, |view, cx| {
                        if self.home {
                            view.home_view(window, cx)
                        } else {
                            view.home_genres_view(window, cx)
                        }
                    }))
            }
        }
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let dir =
            std::env::temp_dir().join(format!("feishin-home-genres-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.playlist_list = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| Probe {
                app: app.clone(),
                width: px(320.),
                home: false,
                _watch: cx.observe(&app, |_, _, cx| cx.notify()),
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1800.), px(1800.)));
        cx.run_until_parked();
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/home-genres-reference.json")).unwrap();
        for case in cases {
            cx.simulate_mouse_move(point(px(1750.), px(1750.)), None, Modifiers::none());
            cx.run_until_parked();
            cx.executor().advance_clock(Duration::from_millis(250));
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.language = "en".into();
                    view.apply_theme(window, cx);
                    cx.set_global(crate::appearance::Language(
                        if case["rtl"] == true { "ar" } else { "en" }.into(),
                    ));
                    let font = px(case["font"].as_f64().unwrap() as f32);
                    gpui_component::Theme::global_mut(cx).font_size = font;
                    window.set_rem_size(font);
                    let section = view.home_sections.entry(Kind::Genres).or_default();
                    section.items = (0..30)
                        .map(|index| Item {
                            id: format!("genre-{index}"),
                            name: if index == 0 {
                                "A very long genre name that should truncate".into()
                            } else if index == 1 {
                                "Jazz".into()
                            } else {
                                format!("Genre {index}")
                            },
                            kind: ItemKind::Genre,
                            ..Default::default()
                        })
                        .collect();
                    view.library_fetches.clear();
                    if case["loading"] == true {
                        view.library_fetches.insert(
                            1,
                            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                        );
                    }
                    cx.notify();
                });
                probe.update(cx, |view, cx| {
                    view.width = px(case["width"].as_f64().unwrap() as f32);
                    cx.notify();
                });
            });
            cx.run_until_parked();
            if case["hover"] == true {
                let position = cx.debug_bounds("home-genre-0").unwrap().center();
                cx.simulate_mouse_move(position, None, Modifiers::none());
                cx.run_until_parked();
                cx.executor().advance_clock(Duration::from_millis(250));
                cx.refresh().unwrap();
                cx.run_until_parked();
            }
            let origin = cx.debug_bounds("home-section-Genres").unwrap().origin;
            assert_eq!(
                super::visible_count(case["width"].as_f64().unwrap() as f32),
                case["count"].as_u64().unwrap() as usize
            );
            for (part, selector) in [
                ("container", "home-section-Genres"),
                ("heading", "home-genres-title"),
                ("more", "home-genres-more"),
                ("tiles", "home-genre"),
                ("names", "home-genre-name"),
                ("play", "home-genre-play"),
                ("wrapper", "home-genre-play-wrapper"),
            ] {
                for (index, entry) in case[part].as_array().unwrap().iter().enumerate() {
                    let selector = if matches!(part, "container" | "heading" | "more") {
                        selector.to_owned()
                    } else {
                        format!("{selector}-{index}")
                    };
                    let actual = cx
                        .debug_bounds(&selector)
                        .unwrap_or_else(|| panic!("Missing {selector}"));
                    for (actual, expected) in [
                        actual.origin.x - origin.x,
                        actual.origin.y - origin.y,
                        actual.size.width,
                        actual.size.height,
                    ]
                    .into_iter()
                    .zip(entry["bounds"].as_array().unwrap())
                    {
                        assert!(
                            (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                            "{selector}: {actual:?} vs {expected}; font {}, width {}, rtl {}",
                            case["font"],
                            case["width"],
                            case["rtl"]
                        );
                    }
                }
            }
        }
        let server = feishin_core::model::Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            url: "http://127.0.0.1:9".into(),
            username: "fixture".into(),
            kind: feishin_core::model::ServerType::Subsonic,
            legacy_auth: false,
            options: Default::default(),
        };
        cx.update(|_, cx| {
            app.update(cx, |view, _| {
                view.state.active_server = Some(server.id);
                view.sessions.insert(
                    server.id,
                    feishin_core::api::Api::new(server, Default::default()).unwrap(),
                );
                view.library_fetches.clear();
                for kind in [Kind::Feature, Kind::Genres, Kind::Random, Kind::Playlists] {
                    view.home_sections.entry(kind).or_default().items = vec![Item {
                        id: format!("{kind:?}"),
                        name: "Fixture".into(),
                        kind: match kind {
                            Kind::Genres => ItemKind::Genre,
                            Kind::Playlists => ItemKind::Playlist,
                            _ => ItemKind::Album,
                        },
                        ..Default::default()
                    }];
                }
            })
        });
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/home-page-layout-reference.json"))
                .unwrap();
        for case in cases {
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.language = "en".into();
                    view.apply_theme(window, cx);
                    let font = px(case["font"].as_f64().unwrap() as f32);
                    gpui_component::Theme::global_mut(cx).font_size = font;
                    window.set_rem_size(font);
                    view.window_bar_style_active =
                        if case["web"] == true { "web" } else { "linux" }.into();
                    view.state.settings.home.feature = case["feature"] != "none";
                    view.state.settings.home.feature_style = if case["feature"] == "multiple" {
                        "multiple"
                    } else {
                        "single"
                    }
                    .into();
                    let mut kinds = vec![Kind::Genres, Kind::Random, Kind::Playlists];
                    if case["reverse"] == true {
                        kinds.reverse();
                    }
                    view.state.settings.home.items = kinds
                        .into_iter()
                        .map(|id| feishin_core::home::Section {
                            id,
                            disabled: false,
                        })
                        .chain([feishin_core::home::Section {
                            id: Kind::RecentlyAdded,
                            disabled: true,
                        }])
                        .collect();
                    cx.notify();
                });
                probe.update(cx, |probe, cx| {
                    probe.home = true;
                    probe.width = px(case["width"].as_f64().unwrap() as f32);
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let stack = cx.debug_bounds("home-stack").unwrap();
            let mut bottom = stack.top() + px(case["padding"][0].as_f64().unwrap() as f32);
            for (index, child) in case["children"].as_array().unwrap().iter().enumerate() {
                let kind = match child["id"].as_str().unwrap() {
                    "feature" => "Feature",
                    "genres" => "Genres",
                    "random" => "Random",
                    "playlists" => "Playlists",
                    other => panic!("{other}"),
                };
                let bounds = cx.debug_bounds(&format!("home-section-{kind}")).unwrap();
                let gap = if index == 0 {
                    0.
                } else {
                    case["gap"].as_f64().unwrap() as f32
                };
                for (actual, expected) in [
                    (
                        bounds.left() - stack.left(),
                        child["x"].as_f64().unwrap() as f32,
                    ),
                    (bounds.size.width, child["width"].as_f64().unwrap() as f32),
                    (bounds.top() - bottom, gap),
                ] {
                    assert!(
                        (f32::from(actual) - expected).abs() <= 1.,
                        "{kind}: {actual:?} vs {expected}, {case}"
                    );
                }
                bottom = bounds.bottom();
            }
            assert!(
                (f32::from(stack.bottom() - bottom)
                    - case["marginBottom"].as_f64().unwrap() as f32)
                    .abs()
                    <= 1.,
                "{case}"
            );
        }
        drop(app);
        std::fs::remove_dir_all(dir).ok();
    }
}
