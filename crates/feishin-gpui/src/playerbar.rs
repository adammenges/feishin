use crate::app::{Feishin, Route};
use feishin_core::{queue::Repeat, shortcuts::Action};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
};

pub(crate) fn button(id: &'static str, main: bool, window: &Window, cx: &App) -> Button {
    let mut target = feishin_core::styles::Target::new(
        "button",
        id,
        &[
            "fs-action-icon-module-root",
            "mantine-ActionIcon-root",
            if main {
                "fs-player-button-module-main"
            } else {
                "fs-player-button-module-secondary"
            },
        ],
    );
    target.attributes.insert(
        "data-variant".into(),
        if main { "default" } else { "subtle" }.into(),
    );
    let background = if main {
        cx.theme().foreground
    } else {
        transparent_black()
    };
    let hover = if main {
        background
    } else {
        let color = cx.theme().background.to_rgb();
        let mix = if cx.theme().is_dark() { 0.05 } else { 0. };
        Rgba {
            r: color.r * 0.95 + mix,
            g: color.g * 0.95 + mix,
            b: color.b * 0.95 + mix,
            a: color.a,
        }
        .into()
    };
    Button::new(id)
        .action_icon(
            cx.try_global::<crate::styles::NativeStyles>()
                .map_or(1., |styles| styles.zoom()),
        )
        .state_style(crate::styles::state_style_path(target, Vec::new()))
        .debug_selector(move || format!("player-{id}"))
        .custom(
            ButtonCustomVariant::new(cx)
                .color(background)
                .foreground(if main {
                    cx.theme().background
                } else {
                    cx.theme().foreground
                })
                .hover(hover)
                .active(hover),
        )
        .with_size(window.rem_size() * 2.25)
        .size(rems(2.25))
        .flex_shrink_0()
        .p_0()
        .border_0()
        .rounded(window.rem_size() * if main { 2.25 } else { 3. / 16. })
}

fn icon(name: &'static str, size: Pixels) -> AnyElement {
    div()
        .debug_selector(move || format!("player-icon-{name}"))
        .size(size)
        .child(
            gpui_component::Icon::default()
                .path(format!("native-icons/{name}.svg"))
                .with_size(size),
        )
        .into_any_element()
}

impl Feishin {
    pub(crate) fn playerbar_right_controls(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use feishin_core::model::{ItemKind, ServerType};
        let rtl = crate::appearance::rtl(cx);
        let item = self.current_entry().map(|entry| entry.item.clone());
        let panel_open = if self.state.settings.panels.kind == "sideDrawerQueue" {
            self.queue_drawer
        } else {
            self.state.settings.panels.expanded
        };
        let glyph = window.rem_size();
        let mut top = div()
            .debug_selector(|| "player-right-top".into())
            .h_flex()
            .flex_wrap()
            .h(relative(1. / 3.))
            .gap(rems(0.75))
            .when(rtl, |row| row.flex_row_reverse());
        if let Some(item) = item.as_ref().filter(|item| {
            self.state.settings.show_ratings
                && item.kind == ItemKind::Song
                && self
                    .sessions
                    .get(&item.server_id)
                    .is_some_and(|api| api.server.kind != ServerType::Jellyfin)
        }) {
            top = top.child(
                self.rating("player-rating", item, cx)
                    .size(glyph * (11. / 16.))
                    .horizontal_padding(glyph * 0.25),
            );
        }
        top = top.child(self.auto_dj_button(window, cx));
        let mut controls = div()
            .debug_selector(|| "player-right-middle".into())
            .h_flex()
            .gap(rems(0.25))
            .when(rtl, |row| row.flex_row_reverse())
            .child(self.output_button(window, cx))
            .child(self.sleep_button(window, cx))
            .child(self.player_config_button(Some(window), cx))
            .child(
                button("player-lyrics", false, window, cx)
                    .debug_selector(|| "player-lyrics".into())
                    .icon(
                        gpui_component::Icon::default()
                            .path("native-icons/microphone.svg")
                            .with_size(glyph),
                    )
                    .when(
                        self.full_player
                            && self.state.settings.player_display.active_tab == "lyrics",
                        |button| button.text_color(cx.theme().primary),
                    )
                    .tooltip(self.tr("player.lyrics"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_player_lyrics(cx))),
            );
        if self.state.settings.show_favorites {
            let favorite = item.as_ref().is_some_and(|item| item.favorite);
            controls = controls.child(
                button("player-favorite", false, window, cx)
                    .debug_selector(|| "player-favorite".into())
                    .icon(
                        gpui_component::Icon::default()
                            .path(if favorite {
                                "native-icons/favorite-filled.svg"
                            } else {
                                "native-icons/favorite.svg"
                            })
                            .with_size(glyph),
                    )
                    .when(favorite, |button| button.text_color(cx.theme().primary))
                    .tooltip(self.tr(if favorite {
                        "player.unfavorite"
                    } else {
                        "player.favorite"
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(item) = item.clone().filter(|item| item.kind == ItemKind::Song)
                        {
                            this.toggle_favorite(item, cx);
                        }
                    })),
            );
        }
        controls = controls
            .child(
                button("show-queue", false, window, cx)
                    .debug_selector(|| "player-queue-toggle".into())
                    .icon(
                        gpui_component::Icon::default()
                            .path(if panel_open {
                                "native-icons/panel-close.svg"
                            } else {
                                "native-icons/panel-open.svg"
                            })
                            .with_size(glyph),
                    )
                    .tooltip(self.tr("player.viewQueue"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_playback_panels(cx))),
            )
            .child(self.player_volume_controls(window, cx));
        div()
            .id("player-bar-control-group-2")
            .role(Role::GenericContainer)
            .debug_selector(|| "player-right-controls".into())
            .on_click(|_, _, cx| cx.stop_propagation())
            .v_flex()
            .flex_1()
            .col_start(if rtl { 1 } else { 3 })
            .row_start(1)
            .min_w_0()
            .h_full()
            .items_end()
            .when(rtl, |group| group.items_start())
            .px(rems(1.))
            .py(rems(0.5))
            .child(top)
            .child(controls)
            .child(
                div()
                    .debug_selector(|| "player-right-bottom".into())
                    .h(relative(1. / 3.)),
            )
            .into_any_element()
    }

    pub(crate) fn navigate_playerbar(
        &mut self,
        route: Route,
        compact: bool,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        let open = compact && !self.full_player;
        if compact {
            self.state.settings.player_display.active_tab = "player".into();
        }
        self.navigate(route, cx);
        if open {
            self.toggle_full_player(cx);
        } else if compact {
            self.save();
        }
    }

    pub(crate) fn toggle_compact_player(&mut self, cx: &mut Context<Self>) {
        self.state.settings.player_display.active_tab = "player".into();
        self.toggle_full_player(cx);
    }

    pub(crate) fn playerbar_transport(
        &self,
        compact: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let radio = self.radio_item.is_some();
        let glyph = if compact {
            window.rem_size() * 0.875
        } else {
            px(self.state.settings.button_size * self.state.settings.zoom)
        };
        div()
            .id("player-transport")
            .debug_selector(|| "player-transport".into())
            .role(Role::GenericContainer)
            .h_flex()
            .flex_shrink_0()
            .gap(rems(0.5))
            .when(!compact, |row| row.h(px(35. * self.state.settings.zoom)))
            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
            .on_click(|_, _, cx| cx.stop_propagation())
            .when(!compact, |row| {
                row.child(
                    button("stop", false, window, cx)
                        .child(icon("stop", glyph - px(2. * self.state.settings.zoom)))
                        .tooltip(self.tr("Stop playback"))
                        .on_click(cx.listener(|this, _, _, cx| this.stop_playback(cx))),
                )
                .child(
                    button("shuffle", false, window, cx)
                        .disabled(radio)
                        .when(self.state.queue.shuffle, |button| {
                            button.text_color(cx.theme().primary)
                        })
                        .child(icon("shuffle", glyph))
                        .tooltip(self.tr("Shuffle"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.queue.set_shuffle(!this.state.queue.shuffle);
                            this.save();
                            cx.notify();
                        })),
                )
            })
            .child(
                button("previous", false, window, cx)
                    .disabled(radio)
                    .child(icon("previous", glyph))
                    .tooltip(self.tr("Previous track"))
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        this.run_shortcut(
                            if event.modifiers().alt {
                                Action::PreviousAlbum
                            } else {
                                Action::Previous
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .when(
                !compact && self.state.settings.playerbar.skip_enabled,
                |row| {
                    row.child(
                        button("skip-backward", false, window, cx)
                            .disabled(radio)
                            .child(icon("step-back", glyph))
                            .tooltip(format!(
                                "{} {}s",
                                self.tr("Skip backward"),
                                self.state.settings.playerbar.skip_backward_seconds
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.seek_position(
                                    this.current_position()
                                        - this.state.settings.playerbar.skip_backward_seconds,
                                    window,
                                    cx,
                                )
                            })),
                    )
                },
            )
            .child(
                button("play-pause", true, window, cx)
                    .disabled(
                        !radio
                            && self
                                .current_entry()
                                .is_none_or(|entry| entry.item.id.is_empty()),
                    )
                    .child(icon(
                        if self.buffering || self.playing {
                            "pause"
                        } else {
                            "play"
                        },
                        window.rem_size(),
                    ))
                    .tooltip(self.tr("Play / pause"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_play(cx))),
            )
            .when(
                !compact && self.state.settings.playerbar.skip_enabled,
                |row| {
                    row.child(
                        button("skip-forward", false, window, cx)
                            .disabled(radio)
                            .child(icon("step-forward", glyph))
                            .tooltip(format!(
                                "{} {}s",
                                self.tr("Skip forward"),
                                self.state.settings.playerbar.skip_forward_seconds
                            ))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.seek_position(
                                    this.current_position()
                                        + this.state.settings.playerbar.skip_forward_seconds,
                                    window,
                                    cx,
                                )
                            })),
                    )
                },
            )
            .child(
                button("next", false, window, cx)
                    .disabled(radio)
                    .child(icon("next", glyph))
                    .tooltip(self.tr("Next track"))
                    .on_click(cx.listener(|this, event: &ClickEvent, window, cx| {
                        this.run_shortcut(
                            if event.modifiers().alt {
                                Action::NextAlbum
                            } else {
                                Action::Next
                            },
                            window,
                            cx,
                        )
                    })),
            )
            .when(!compact, |row| {
                row.child(
                    button("repeat", false, window, cx)
                        .disabled(radio)
                        .when(self.state.queue.repeat != Repeat::Off, |button| {
                            button.text_color(cx.theme().primary)
                        })
                        .child(icon(
                            if self.state.queue.repeat == Repeat::One {
                                "repeat-one"
                            } else {
                                "repeat"
                            },
                            glyph,
                        ))
                        .tooltip(self.tr("Repeat mode"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.queue.repeat = match this.state.queue.repeat {
                                Repeat::Off => Repeat::All,
                                Repeat::All => Repeat::One,
                                Repeat::One => Repeat::Off,
                            };
                            this.save();
                            cx.notify();
                        })),
                )
                .child(
                    button("random-library", false, window, cx)
                        .debug_selector(|| "random-library".into())
                        .disabled(radio || self.api().is_none())
                        .child(icon("random", glyph))
                        .tooltip(self.tr("Play random"))
                        .on_click(
                            cx.listener(|view, _, window, cx| view.open_random_form(window, cx)),
                        ),
                )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;
    use feishin_core::{
        model::{Item, ItemKind},
        queue::Enqueue,
        storage::State,
    };
    use gpui_component::{Root, Theme};
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn playerbar_right_controls_match_original_layouts(cx: &mut TestAppContext) {
        struct Probe {
            app: Entity<Feishin>,
            zoom: f32,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(500. * self.zoom))
                    .h(px(90. * self.zoom))
                    .font(crate::fonts::app_font(cx))
                    .child(
                        self.app
                            .update(cx, |app, cx| app.playerbar_right_controls(window, cx)),
                    )
            }
        }
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let dir =
            std::env::temp_dir().join(format!("feishin-player-right-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            let mut state = State::default();
            state.settings.scrobble = false;
            state.queue.add(
                vec![Item {
                    id: "song".into(),
                    name: "A song".into(),
                    kind: ItemKind::Song,
                    ..Default::default()
                }],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                let server = feishin_core::model::Server {
                    id: uuid::Uuid::nil(),
                    kind: feishin_core::model::ServerType::Subsonic,
                    url: "http://127.0.0.1:1".into(),
                    name: "Fixture".into(),
                    username: "fixture".into(),
                    legacy_auth: false,
                    options: Default::default(),
                };
                app.sessions.insert(
                    server.id,
                    feishin_core::api::Api::new(server, feishin_core::model::Session::default())
                        .unwrap(),
                );
            });
            let probe = cx.new(|_| Probe { app, zoom: 1. });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        #[cfg(target_os = "linux")]
        let reference = include_str!("../assets/playerbar-right-layout-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let reference = include_str!("../assets/playerbar-right-layout-darwin-reference.json");
        let cases: Vec<serde_json::Value> = serde_json::from_str(reference).unwrap();
        for case in cases {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            let rtl = case["rtl"] == true;
            cx.simulate_resize(size(px(1200. * zoom), px(600.)));
            cx.update(|window, cx| {
                window.set_rem_size(px(rem * zoom));
                window.text_system().set_optical_size_scale(zoom);
                Theme::global_mut(cx).font_size = px(rem * zoom);
                cx.set_global(crate::appearance::Language(
                    if rtl { "ar" } else { "en" }.into(),
                ));
                probe.update(cx, |probe, cx| {
                    probe.zoom = zoom;
                    probe.app.update(cx, |app, _| {
                        app.state.settings.zoom = zoom;
                        app.state.settings.show_ratings = case["rating"] == true;
                        app.state.settings.show_favorites = case["favorite"] == true;
                        app.state.settings.volume_width = case["volume"].as_f64().unwrap() as f32;
                        // Arabic falls back to this same original Auto-DJ label.
                        app.state.settings.language = if rtl { "ar" } else { "en" }.into();
                        app.last_theme_scan = std::time::Instant::now();
                    });
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let root = cx.debug_bounds("player-right-controls").unwrap();
            let expected_root = case["root"].as_array().unwrap();
            let mut compare = |selector: &str, expected: &serde_json::Value| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("Missing {selector}: {case}"));
                let values = [
                    f32::from(bounds.left() - root.left()),
                    f32::from(bounds.top() - root.top()),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                ];
                for (i, value) in values.into_iter().enumerate() {
                    let origin = if i < 2 {
                        expected_root[i].as_f64().unwrap() as f32
                    } else {
                        0.
                    };
                    let expected = (expected[i].as_f64().unwrap() as f32 - origin) * zoom;
                    assert!(
                        (value - expected).abs() < 1.1,
                        "{selector}[{i}]: {value} vs {expected}: {case}"
                    );
                }
            };
            for (i, name) in [
                "player-right-top",
                "player-right-middle",
                "player-right-bottom",
            ]
            .into_iter()
            .enumerate()
            {
                compare(name, &case["groups"][i]);
            }
            for (key, selector) in [
                ("dj", "player-auto-dj"),
                ("rating", "player-rating"),
                ("cast", "player-output"),
                ("sleep", "player-sleep"),
                ("config", "player-configure"),
                ("lyrics", "player-lyrics"),
                ("favorite", "player-favorite"),
                ("queue", "player-queue-toggle"),
                ("mute", "player-mute"),
                ("slider", "player-volume"),
            ] {
                if !case["controls"][key].is_null() {
                    compare(selector, &case["controls"][key]);
                }
            }
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn playerbar_button_paint_and_pointer_states_match_original(cx: &mut TestAppContext) {
        struct Probe {
            main: bool,
            disabled: bool,
        }
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().size_full().p(px(20.)).child(
                    button("paint-probe", self.main, window, cx)
                        .disabled(self.disabled)
                        .aria_label("Paint probe")
                        .on_click(|_, _, _| {})
                        .child(
                            div().size(px(10.)).child(
                                canvas(
                                    |_, _, _| {},
                                    |bounds, _, window, _| {
                                        window.paint_quad(fill(bounds, window.text_style().color));
                                    },
                                )
                                .size_full(),
                            ),
                        ),
                )
            }
        }
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            let probe = cx.new(|_| Probe {
                main: false,
                disabled: false,
            });
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        #[cfg(target_os = "linux")]
        let reference = include_str!("../assets/playerbar-button-states-linux-reference.json");
        #[cfg(not(target_os = "linux"))]
        let reference = include_str!("../assets/playerbar-button-states-darwin-reference.json");
        let cases: Vec<serde_json::Value> = serde_json::from_str(reference).unwrap();
        for case in cases {
            let disabled = case["disabled"] == true;
            let state = case["state"].as_str().unwrap();
            // Native disabled buttons cannot receive keyboard focus.
            if disabled && state == "focus" {
                continue;
            }
            let dark = case["dark"] == true;
            let outside = point(px(2.), px(2.));
            cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(outside, None, Modifiers::none());
            cx.update(|window, cx| {
                window.blur();
                window.set_rem_size(px(16.));
                let theme = Theme::global_mut(cx);
                theme.mode = if dark {
                    gpui_component::ThemeMode::Dark
                } else {
                    gpui_component::ThemeMode::Light
                };
                theme.background = rgb(if dark { 0x101010 } else { 0xf0f0f0 }).into();
                theme.foreground = rgb(if dark { 0xeeeeee } else { 0x101010 }).into();
                probe.update(cx, |probe, cx| {
                    probe.main = case["kind"] == "main";
                    probe.disabled = disabled;
                    cx.notify();
                });
            });
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(250));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let bounds = cx.debug_bounds("player-paint-probe").unwrap();
            if matches!(state, "hover" | "active") {
                cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
                if state == "active" {
                    cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
                }
            } else if state == "focus" {
                cx.update(|window, _| window.focus_next());
            }
            cx.run_until_parked();
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(250));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let actual_bounds = cx.debug_bounds("player-paint-probe").unwrap();
            let offset = if case["transform"] == "none" { 0. } else { 1. };
            assert_eq!(
                actual_bounds.origin.y - bounds.origin.y,
                px(offset),
                "{case}"
            );
            let painted = cx.update(|window, _| window.painted_quad_opacities_for_test());
            for (key, size) in [("background", 36.), ("icon", 10.)] {
                let rgba = feishin_core::appearance::color(case[key].as_str().unwrap()).unwrap();
                let expected_alpha = rgba[3] * case["opacity"].as_f64().unwrap() as f32;
                let actual = painted
                    .iter()
                    .find(|(bounds, _)| bounds.size == gpui::size(px(size), px(size)))
                    .map_or(transparent_black(), |(_, color)| *color)
                    .to_rgb();
                assert!(
                    (actual.a - expected_alpha).abs() < 0.002,
                    "{key} alpha {actual:?}: {case}"
                );
                if expected_alpha > 0. {
                    for (actual, expected) in [actual.r, actual.g, actual.b].into_iter().zip(rgba) {
                        assert!(
                            (actual - expected).abs() < 0.002,
                            "{key} {actual}, expected {expected}: {case}"
                        );
                    }
                }
            }
        }
    }

    #[gpui::test]
    fn playerbar_frames_and_transports_match_original(cx: &mut TestAppContext) {
        struct Probe(Entity<Feishin>);
        impl Render for Probe {
            fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .font(crate::fonts::app_font(cx))
                    .child(self.0.update(cx, |app, cx| {
                        app.sync_progress(window, cx);
                        app.player_bar(window, cx)
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
            std::env::temp_dir().join(format!("feishin-player-frame-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.scrobble = false;
            state.queue.add(
                vec![Item {
                    id: "song".into(),
                    name: "A song title".into(),
                    album: "Album name".into(),
                    album_id: "album".into(),
                    artist: "Artist name".into(),
                    artist_id: "artist".into(),
                    kind: ItemKind::Song,
                    duration: 100.,
                    ..Default::default()
                }],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|_| Probe(app));
            captured.replace(Some(probe.clone()));
            Root::new(probe, window, cx)
        });
        let probe = captured.borrow_mut().take().unwrap();
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(if cfg!(target_os = "linux") {
                include_str!("../assets/playerbar-frame-layout-linux-reference.json")
            } else {
                include_str!("../assets/playerbar-frame-layout-darwin-reference.json")
            })
            .unwrap();
        for case in &reference {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            let compact = case["compact"].as_bool().unwrap();
            let radio = case["radio"].as_bool().unwrap();
            let rtl = case["rtl"].as_bool().unwrap();
            cx.simulate_resize(size(
                px(case["viewport"].as_f64().unwrap() as f32 * zoom),
                px(600.),
            ));
            cx.update(|window, cx| {
                cx.set_global(crate::appearance::Language(
                    if rtl { "ar" } else { "en" }.into(),
                ));
                window.set_rem_size(px(rem * zoom));
                window.text_system().set_optical_size_scale(zoom);
                Theme::global_mut(cx).font_size = px(rem * zoom);
                Theme::global_mut(cx).foreground = rgb(0xeeeeee).into();
                Theme::global_mut(cx).background = rgb(0x101010).into();
                probe.update(cx, |probe, cx| {
                    probe.0.update(cx, |app, _| {
                        // The matrix supplies a synthetic root font. Keep the
                        // real-time theme watcher from replacing it mid-frame.
                        app.last_theme_scan = std::time::Instant::now();
                        app.state.settings.zoom = zoom;
                        app.state.settings.language = if rtl { "ar" } else { "en" }.into();
                        app.state.settings.playerbar.skip_enabled = case["skip"].as_bool().unwrap();
                        app.state.settings.button_size =
                            case["buttonSize"].as_f64().unwrap() as f32;
                        app.radio_item = radio.then(|| feishin_core::queue::Entry {
                            key: uuid::Uuid::nil(),
                            item: Item {
                                id: "radio".into(),
                                kind: ItemKind::Radio,
                                ..Default::default()
                            },
                        });
                    });
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let mut check = |selector: &str, expected: &serde_json::Value| {
                let bounds = cx
                    .debug_bounds(selector)
                    .unwrap_or_else(|| panic!("Missing {selector}: {case}"));
                let actual = [
                    f32::from(bounds.left()),
                    f32::from(bounds.top()),
                    f32::from(bounds.size.width),
                    f32::from(bounds.size.height),
                ];
                for (actual, expected) in actual.into_iter().zip(expected.as_array().unwrap()) {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32 * zoom).abs() <= 1.1,
                        "{selector}: {actual} != {expected} * {zoom}: {case}"
                    );
                }
            };
            check("player-bar", &case["player"]);
            if !compact {
                for (index, selector) in [
                    "player-left-controls",
                    "player-center-controls",
                    "player-right-controls",
                ]
                .into_iter()
                .enumerate()
                {
                    check(selector, &case["slots"][index]);
                }
            }
            for button in case["buttons"].as_array().unwrap() {
                let (name, glyph) = match button["icon"].as_str().unwrap() {
                    "ellipsisVertical" => continue, // The reference stubs desktop metadata.
                    "mediaPrevious" | "previous" => ("player-previous", "previous"),
                    "mediaNext" | "next" => ("player-next", "next"),
                    "mediaPlay" => ("player-play-pause", "play"),
                    "stop" => ("player-stop", "stop"),
                    "shuffle" => ("player-shuffle", "shuffle"),
                    "repeat" => ("player-repeat", "repeat"),
                    "random" => ("random-library", "random"),
                    "back" => ("player-skip-backward", "step-back"),
                    "forward" => ("player-skip-forward", "step-forward"),
                    other => panic!("Unknown reference transport: {other}"),
                };
                check(name, &button["bounds"]);
                check(&format!("player-icon-{glyph}"), &button["iconBounds"]);
            }
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert_eq!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == Role::Slider
                            && node.max_numeric_value() == Some(100.)
                            && node.numeric_value() == Some(0.)),
                    !compact && !radio,
                    "Seek visibility: {case}"
                );
            });
        }
        // Menus remain available when the cover has moved to the sidebar.
        let app = probe.read_with(cx, |probe, _| probe.0.clone());
        cx.simulate_resize(size(px(1200.), px(600.)));
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.radio_item = None;
                app.state.settings.zoom = 1.;
                app.state.settings.language = "en".into();
                app.state.settings.sidebar.image_enabled = true;
                app.state.settings.sidebar.image_shown = true;
                app.state.settings.sidebar.collapsed = false;
                app.apply_theme(window, cx);
            });
            probe.update(cx, |_, cx| cx.notify());
        });
        cx.run_until_parked();
        for (selector, mouse) in [
            ("player-track-menu", MouseButton::Left),
            ("playerbar-title", MouseButton::Right),
        ] {
            let point = cx.debug_bounds(selector).unwrap().center();
            cx.simulate_mouse_down(point, mouse, Modifiers::none());
            cx.simulate_mouse_up(point, mouse, Modifiers::none());
            cx.run_until_parked();
            cx.update(|window, _| {
                assert!(
                    window
                        .accessibility_tree_for_test()
                        .unwrap()
                        .nodes
                        .iter()
                        .any(|(_, node)| node.role() == Role::MenuItem),
                    "Track menu from {selector}"
                )
            });
            assert!(!app.read_with(cx, |app, _| app.full_player));
            cx.simulate_keystrokes("escape");
            cx.run_until_parked();
        }
        // The compact breakpoint is based on CSS width, so desktop windows can
        // enter it at high zoom. Artwork stays available despite sidebar settings.
        cx.simulate_resize(size(px(960.), px(600.)));
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.zoom = 1.25;
                app.state.settings.player_display.active_tab = "lyrics".into();
                app.apply_theme(window, cx);
            });
            probe.update(cx, |_, cx| cx.notify());
        });
        cx.run_until_parked();
        let cover = cx.debug_bounds("playerbar-artwork").unwrap();
        assert_eq!(cover.size.width, px(75.));
        cx.simulate_click(cover.center(), Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(app.full_player);
            assert_eq!(app.state.settings.player_display.active_tab, "player");
        });
        for selector in [
            "playerbar-title",
            "playerbar-compact-artists",
            "playerbar-album",
        ] {
            for expanded in [false, true] {
                cx.update(|_, cx| {
                    app.update(cx, |app, _| {
                        app.full_player = expanded;
                        app.state.settings.player_display.active_tab = "lyrics".into();
                    });
                    probe.update(cx, |_, cx| cx.notify());
                });
                cx.run_until_parked();
                let bounds = cx.debug_bounds(selector).unwrap();
                cx.simulate_click(
                    point(bounds.left() + px(5.), bounds.center().y),
                    Modifiers::none(),
                );
                cx.run_until_parked();
                app.read_with(cx, |app, _| {
                    assert_eq!(app.full_player, !expanded, "Compact navigation: {selector}");
                    assert_eq!(app.state.settings.player_display.active_tab, "player");
                    match selector {
                        "playerbar-title" => assert_eq!(app.route, Route::Queue),
                        "playerbar-compact-artists" => assert!(
                            matches!(&app.route, Route::Detail(item) if item.id == "artist")
                        ),
                        _ => {
                            assert!(matches!(&app.route, Route::Detail(item) if item.id == "album"))
                        }
                    }
                });
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
