use crate::app::Feishin;
use feishin_core::storage::WindowPlacement;
use gpui::{
    AnyElement, App, Bounds, Context, MouseButton, Pixels, TitlebarOptions, Window, WindowBounds,
    div, point, prelude::*, px, rgb, size,
};
use gpui_component::{ActiveTheme, InteractiveElementExt, Sizable, StyledExt};

pub fn titlebar_options(style: &str) -> TitlebarOptions {
    TitlebarOptions {
        title: Some("Feishin".into()),
        appears_transparent: style != "linux",
        traffic_light_position: (style != "linux").then_some(point(px(-200.), px(-200.))),
    }
}

fn restore(
    saved: Option<WindowPlacement>,
    displays: &[Bounds<Pixels>],
    fallback: Bounds<Pixels>,
) -> WindowBounds {
    let Some(saved) = saved.filter(|saved| {
        [saved.x, saved.y, saved.width, saved.height]
            .iter()
            .all(|n| n.is_finite())
            && saved.width > 0.0
            && saved.height > 0.0
    }) else {
        return WindowBounds::Windowed(fallback);
    };
    let display = displays
        .iter()
        .find(|bounds| bounds.contains(&point(px(saved.x), px(saved.y))))
        .or_else(|| displays.first());
    let mut bounds = Bounds::new(
        point(px(saved.x), px(saved.y)),
        size(px(saved.width.max(960.0)), px(saved.height.max(640.0))),
    );
    if let Some(display) = display {
        bounds.size.width = bounds.size.width.min(display.size.width);
        bounds.size.height = bounds.size.height.min(display.size.height);
        bounds.origin.x = bounds
            .origin
            .x
            .max(display.origin.x)
            .min(display.right() - bounds.size.width);
        bounds.origin.y = bounds
            .origin
            .y
            .max(display.origin.y)
            .min(display.bottom() - bounds.size.height);
    }
    if saved.fullscreen {
        WindowBounds::Fullscreen(bounds)
    } else if saved.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

pub fn initial_bounds(saved: Option<WindowPlacement>, cx: &App) -> WindowBounds {
    let displays = cx
        .displays()
        .iter()
        .map(|display| display.bounds())
        .collect::<Vec<_>>();
    restore(
        saved,
        &displays,
        Bounds::centered(None, size(px(1320.0), px(840.0)), cx),
    )
}

impl Feishin {
    pub(crate) fn change_window_bar_style(&mut self, style: &str, cx: &mut Context<Self>) {
        if !matches!(style, "linux" | "windows" | "macos" | "web") {
            return;
        }
        let requires_restart =
            (self.state.settings.window_bar_style == "linux") != (style == "linux");
        self.state.settings.window_bar_style = style.into();
        if self.window_bar_style_active != "linux" && style != "linux" {
            self.window_bar_style_active = style.into();
        }
        if requires_restart {
            self.message = self.tr("Restart required");
        }
        self.save();
        cx.notify();
    }

    pub fn window_caption(&self) -> String {
        let paused =
            feishin_core::i18n::translate(&self.state.settings.language, "page.windowBar.paused")
                .unwrap_or_default();
        let private = if self.state.settings.private_mode {
            format!(
                " {}",
                feishin_core::i18n::translate(
                    &self.state.settings.language,
                    "page.windowBar.privateMode"
                )
                .unwrap_or_default()
            )
        } else {
            String::new()
        };
        let mut title = "Feishin".to_owned();
        if self.state.settings.window_title_track_info {
            if let Some(radio) = &self.radio_item {
                title = format!(
                    "{}{}",
                    if self.playing { "" } else { &paused },
                    radio.item.name
                );
                for text in [&self.radio_metadata.artist, &self.radio_metadata.title] {
                    if !text.is_empty() {
                        title.push_str(&format!(" - {text}"));
                    }
                }
                title.push_str(" - Feishin");
            } else if let Some(entry) = self.state.queue.current() {
                let index = self
                    .state
                    .queue
                    .order
                    .iter()
                    .position(|key| *key == entry.key)
                    .unwrap_or(0);
                title = format!(
                    "{}({} / {}) {}",
                    if self.loaded && !self.playing {
                        &paused
                    } else {
                        ""
                    },
                    index + 1,
                    self.state.queue.order.len(),
                    entry.item.name
                );
                if !entry.item.artist.is_empty() {
                    title.push_str(&format!(" - {}", entry.item.artist));
                    title.push_str(" - Feishin");
                }
            }
        }
        title.push_str(&private);
        title
    }
    pub fn close_native_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.prepare_window_close(window, cx) {
            window.remove_window();
        }
    }
    pub fn prepare_window_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.remember_window(window);
        self.save();
        if (cfg!(target_os = "macos") && !cfg!(test))
            || (self.state.settings.tray
                && self.state.settings.exit_to_tray
                && self.tray.is_some()
                && self.native_hide_supported)
        {
            self.hide_native_window(window, cx);
            !self.window_hidden
        } else {
            true
        }
    }
    pub fn native_titlebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mac = self.window_bar_style_active == "macos";
        let rtl = crate::appearance::rtl(cx);
        let zoom = self.state.settings.zoom;
        let font = cx.theme().font_size;
        let width = window.viewport_size().width;
        let narrow = width < px(768. * zoom);
        let bar = feishin_core::styles::Target::new(
            "div",
            "native-window-bar",
            &["fs-window-bar-module-window-bar"],
        );
        let container = feishin_core::styles::Target::new(
            "div",
            "",
            &[if mac {
                "fs-window-bar-module-macos-container"
            } else {
                "fs-window-bar-module-windows-container"
            }],
        );
        let group = if mac {
            "fs-window-bar-module-macos-button-group"
        } else {
            "fs-window-bar-module-windows-button-group"
        };
        let controls = div()
            .debug_selector(|| "window-control-group".into())
            .flex()
            .when(rtl, |div| div.flex_row_reverse())
            .flex_shrink_0()
            .h_full()
            .w(px(if mac {
                60.
            } else if narrow {
                120.
            } else {
                130.
            } * zoom))
            .when(mac, |div| {
                div.absolute().top(px(5. * zoom)).left(font * 0.5)
            })
            .map(|div| {
                crate::styles::apply_in(
                    div,
                    feishin_core::styles::Target::new("div", "", &[group]),
                    [bar.clone(), container.clone()],
                    cx,
                )
            })
            .children((0..3usize).map(|slot| {
                let action = if mac { [2, 0, 1][slot] } else { slot };
                let focus = window
                    .use_keyed_state(("window-control-focus", action), cx, |_, cx| {
                        cx.focus_handle()
                    })
                    .read(cx)
                    .clone();
                let label = self.tr(match action {
                    0 => "Minimize",
                    1 => "Maximize",
                    _ => "Close",
                });
                let mut target = feishin_core::styles::Target::new(
                    "div",
                    "",
                    &[if mac {
                        "fs-window-bar-module-macos-button"
                    } else {
                        "fs-window-bar-module-windows-button"
                    }],
                );
                if mac && action < 2 {
                    target.classes.push(format!(
                        "fs-window-bar-module-{}-button",
                        if action == 0 { "min" } else { "max" }
                    ));
                }
                if !mac && action == 2 {
                    target.classes.push("fs-window-bar-module-exit".into());
                }
                let parents = [
                    bar.clone(),
                    container.clone(),
                    feishin_core::styles::Target::new("div", "", &[group]),
                ];
                let mut hovered = target.clone();
                hovered.states.push("hover".into());
                let hover_style = cx
                    .try_global::<crate::styles::NativeStyles>()
                    .map(|styles| styles.style_in(hovered, parents.clone(), f32::from(font)))
                    .unwrap_or_default();
                let control = div()
                    .id(("native-window-control", action))
                    .debug_selector(move || format!("native-window-control-{action}"))
                    .key_context("FormControl")
                    .role(gpui::Role::Button)
                    .aria_label(label)
                    .track_focus(&focus.clone().tab_stop(true))
                    .relative()
                    .flex_shrink_0()
                    .when(mac, |div| div.w(px(20. * zoom)).h_full())
                    .when(!mac, |div| {
                        div.flex_1()
                            .map(|mut div| {
                                div.style().flex_shrink = Some(1.);
                                div
                            })
                            .min_w_0()
                            .w(px(50. * zoom))
                            .h(px(30. * zoom))
                            .flex()
                            .items_center()
                            .justify_center()
                    })
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&focus);
                        cx.stop_propagation();
                    })
                    .on_double_click(|_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        match action {
                            0 => window.minimize_window(),
                            1 => window.zoom_window(),
                            _ => this.close_native_window(window, cx),
                        }
                    }))
                    .map(|div| crate::styles::apply_in(div, target, parents, cx));
                if mac {
                    let group = ["window-min", "window-max", "window-close"][action];
                    let stem = ["min", "max", "close"][action];
                    control
                        .group(group)
                        .child(
                            div()
                                .debug_selector(move || format!("native-window-icon-{action}"))
                                .absolute()
                                .top_0()
                                .left(px(if rtl { 2. * zoom } else { 0. }))
                                .size(px(18. * zoom))
                                .child(
                                    div()
                                        .size_full()
                                        .group_hover(group, |style| style.opacity(0.))
                                        .child(
                                            gpui::img(gpui::SharedString::from(format!(
                                                "native-window/{stem}.png"
                                            )))
                                            .size_full(),
                                        ),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .opacity(0.)
                                        .group_hover(group, |style| style.opacity(1.))
                                        .child(
                                            gpui::img(gpui::SharedString::from(format!(
                                                "native-window/{stem}-hover.png"
                                            )))
                                            .size_full(),
                                        ),
                                ),
                        )
                        .hover(move |style| style.refine_style(&hover_style))
                } else {
                    let background = if action == 2 {
                        cx.theme().danger
                    } else {
                        gpui::Hsla::from(rgb(0x7d7d7d)).opacity(0.3)
                    };
                    let icon_size = px(if action == 1 { 13. } else { 19. } * zoom);
                    control
                        .hover(move |style| style.bg(background).refine_style(&hover_style))
                        .child(
                            div()
                                .debug_selector(move || format!("native-window-icon-{action}"))
                                .size(icon_size)
                                .child(
                                    gpui_component::Icon::default()
                                        .path(
                                            [
                                                "native-icons/window-minimize.svg",
                                                "native-icons/window-maximize.svg",
                                                "native-icons/window-close.svg",
                                            ][action],
                                        )
                                        .with_size(icon_size),
                                ),
                        )
                }
            }));
        let caption = div()
            .debug_selector(|| "window-caption-container".into())
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .when(rtl, |div| div.flex_row_reverse())
            .when(mac, |div| {
                div.max_w((width - px(140. * zoom)).max(px(0.)))
                    .px(font * 0.5)
                    .mx_auto()
                    .justify_center()
            })
            .when(!mac, |div| {
                div.pl(font * if narrow { 0.5 } else { 1. }).gap(font * 0.5)
            })
            .when(!mac, |div| {
                div.child(
                    gpui::div()
                        .debug_selector(|| "window-app-icon".into())
                        .size(px(16. * zoom))
                        .flex_shrink_0()
                        .child(gpui::img("native-window/app.png").size_full()),
                )
            })
            .child(
                div()
                    .debug_selector(|| "window-caption-text".into())
                    .min_w_0()
                    .max_w_full()
                    .truncate()
                    .text_size(font * 13. / 16.)
                    .line_height(font)
                    .text_color(cx.theme().foreground)
                    .when(mac, |div| div.text_center())
                    .when(!mac, |div| div.flex_1().w_0())
                    .map(|div| {
                        crate::styles::apply_in(
                            div,
                            feishin_core::styles::Target::new(
                                "div",
                                "",
                                &[
                                    "fs-window-bar-module-player-status-text",
                                    "fs-text-module-root",
                                ],
                            ),
                            [
                                bar.clone(),
                                container.clone(),
                                feishin_core::styles::Target::new(
                                    "div",
                                    "",
                                    &["fs-window-bar-module-player-status-container"],
                                ),
                            ],
                            cx,
                        )
                    })
                    .child(self.window_caption()),
            )
            .map(|div| {
                crate::styles::apply_in(
                    div,
                    feishin_core::styles::Target::new(
                        "div",
                        "",
                        &["fs-window-bar-module-player-status-container"],
                    ),
                    [bar.clone(), container.clone()],
                    cx,
                )
            });
        let contents = div()
            .id("window-bar-container")
            .debug_selector(|| "window-bar-container".into())
            .relative()
            .flex()
            .items_center()
            .size_full()
            .when(rtl, |div| div.flex_row_reverse())
            .when(!mac, |div| div.gap(font * 0.5))
            .when(mac, |div| div.justify_center())
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_direction(if rtl {
                gpui::TextDirection::Rtl
            } else {
                gpui::TextDirection::Ltr
            })
            .border_b(crate::styles::css_border_width(px(zoom), cx))
            .border_color(cx.theme().border.opacity(0.5))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .on_double_click(|_, window, _| window.zoom_window())
            .map(|div| crate::styles::apply_in(div, container, [bar.clone()], cx))
            .child(caption)
            .child(controls);
        div()
            .id("native-window-bar")
            .debug_selector(|| "native-window-bar".into())
            .h(px(30. * zoom))
            .w_full()
            .flex_shrink_0()
            .child(contents)
            .map(|div| crate::styles::apply_in(div, bar, [], cx))
            .into_any_element()
    }
    pub fn media_controls_enabled(&self) -> bool {
        cfg!(target_os = "linux") || self.state.settings.media_session
    }
    pub fn sync_media_controls(&mut self, _window: &Window) {
        if !self.media_controls_enabled() {
            self.media = None;
            self.media_metadata = None;
            self.media_attempted = false;
            return;
        }
        if self.media.is_some() || self.media_attempted {
            return;
        }
        self.media_attempted = true;
        #[cfg(not(test))]
        {
            #[cfg(not(windows))]
            let hwnd = None;
            #[cfg(windows)]
            let hwnd = {
                raw_window_handle::HasWindowHandle::window_handle(_window)
                    .ok()
                    .and_then(|handle| match handle.as_raw() {
                        raw_window_handle::RawWindowHandle::Win32(handle) => {
                            Some(handle.hwnd.get() as *mut std::ffi::c_void)
                        }
                        _ => None,
                    })
            };
            let sender = self.media_sender.clone();
            match souvlaki::MediaControls::new(souvlaki::PlatformConfig {
                dbus_name: "Feishin",
                display_name: "Feishin",
                hwnd,
            })
            .and_then(|mut controls| {
                controls.attach(move |event| {
                    let _ = sender.send(event);
                })?;
                Ok(controls)
            }) {
                Ok(controls) => self.media = Some(controls),
                Err(error) => log::warn!("Media controls unavailable: {error}"),
            }
        }
    }
    pub fn remember_window(&mut self, window: &Window) {
        let placement = window.window_bounds();
        let bounds = placement.get_bounds();
        self.state.window = Some(WindowPlacement {
            x: bounds.origin.x.into(),
            y: bounds.origin.y.into(),
            width: bounds.size.width.into(),
            height: bounds.size.height.into(),
            fullscreen: matches!(placement, WindowBounds::Fullscreen(_)),
            maximized: matches!(placement, WindowBounds::Maximized(_)),
        });
    }
    pub fn toggle_full_player(&mut self, cx: &mut Context<Self>) {
        self.full_player = !(self.full_player || self.visualizer_display.expanded);
        self.visualizer_display
            .dismiss(cx.background_executor().now());
        self.persist_player_modes();
        self.restore_player_tab(cx);
        self.fetch_covers(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[gpui::test]
    fn window_bar_matches_original_controls_titles_zoom_and_direction(
        cx: &mut gpui::TestAppContext,
    ) {
        use gpui::{AppContext, Entity, Render, Subscription};
        use gpui_component::{Root, Theme};
        use std::{cell::RefCell, rc::Rc};
        struct Probe {
            app: Entity<Feishin>,
            _watch: Subscription,
        }
        impl Render for Probe {
            fn render(
                &mut self,
                window: &mut Window,
                cx: &mut Context<Self>,
            ) -> impl gpui::IntoElement {
                div().font(crate::fonts::app_font(cx)).child(
                    self.app
                        .update(cx, |view, cx| view.native_titlebar(window, cx)),
                )
            }
        }
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let dir = std::env::temp_dir().join(format!("feishin-window-bar-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(Default::default()), window, cx));
            captured.replace(Some(app.clone()));
            let probe = cx.new(|cx| Probe {
                app: app.clone(),
                _watch: cx.observe(&app, |_, _, cx| cx.notify()),
            });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_scale_factor(if cfg!(target_os = "macos") { 2. } else { 1. });
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(if cfg!(target_os = "macos") {
                include_str!("../assets/window-bar-layout-darwin-reference.json")
            } else {
                include_str!("../assets/window-bar-layout-linux-reference.json")
            })
            .unwrap();
        for case in reference {
            let zoom = case["zoom"].as_f64().unwrap() as f32;
            let rem = case["rem"].as_f64().unwrap() as f32;
            let mac = case["mac"] == true;
            let title = case["title"].as_str().unwrap();
            cx.simulate_resize(size(
                px(case["width"].as_f64().unwrap() as f32 * zoom),
                px(200. * zoom),
            ));
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.zoom = zoom;
                    view.state.settings.language =
                        if case["rtl"] == true { "ar" } else { "en" }.into();
                    cx.set_global(crate::appearance::Language(
                        view.state.settings.language.clone(),
                    ));
                    view.window_bar_style_active = if mac { "macos" } else { "windows" }.into();
                    view.state.queue = Default::default();
                    if title != "Feishin" {
                        view.state.queue.add(
                            vec![feishin_core::model::Item {
                                name: title.strip_prefix("(1 / 1) ").unwrap().into(),
                                ..Default::default()
                            }],
                            feishin_core::queue::Enqueue::Now,
                        );
                    }
                    view.apply_theme(window, cx);
                    Theme::global_mut(cx).font_size = px(rem * zoom);
                    window.set_rem_size(px(rem * zoom));
                    window.text_system().set_optical_size_scale(zoom);
                    view.sync_css_environment(window, cx);
                    view.sync_style_context(window, cx);
                    assert_eq!(view.window_caption(), title);
                    cx.notify();
                });
            });
            cx.run_until_parked();
            let mut pairs = vec![
                ("native-window-bar".to_owned(), &case["bar"]),
                ("window-bar-container".into(), &case["container"]),
                ("window-caption-container".into(), &case["caption"]),
                ("window-caption-text".into(), &case["text"]),
                ("window-control-group".into(), &case["group"]),
            ];
            for action in 0..3 {
                pairs.push((
                    format!("native-window-control-{action}"),
                    &case["buttons"][action]["bounds"],
                ));
                pairs.push((
                    format!("native-window-icon-{action}"),
                    &case["buttons"][action]["icon"],
                ));
            }
            if !mac {
                pairs.push(("window-app-icon".into(), &case["app"]));
            }
            for (selector, expected) in pairs {
                let bounds = cx.debug_bounds(selector.as_str()).unwrap();
                for (actual, expected) in [
                    bounds.origin.x,
                    bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ]
                .into_iter()
                .zip(expected.as_array().unwrap())
                {
                    let expected = expected.as_f64().unwrap() as f32 * zoom;
                    assert!(
                        (f32::from(actual) - expected).abs() <= 1.,
                        "{selector}: {actual:?} vs {expected}; {case}"
                    );
                }
            }
        }
        for mac in [false, true] {
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.window_bar_style_active = if mac { "macos" } else { "windows" }.into();
                    view.state.settings.zoom = 1.;
                    view.state.settings.language = "en".into();
                    cx.set_global(crate::appearance::Language("en".into()));
                    view.apply_theme(window, cx);
                    view.sync_css_environment(window, cx);
                    cx.notify();
                })
            });
            cx.run_until_parked();
            for action in 0..3 {
                let bounds = cx
                    .debug_bounds(&format!("native-window-control-{action}"))
                    .unwrap();
                cx.simulate_mouse_move(bounds.center(), None, gpui::Modifiers::none());
                cx.run_until_parked();
                if !mac {
                    let expected = cx.update(|_, cx| {
                        if action == 2 {
                            cx.theme().danger
                        } else {
                            gpui::Hsla::from(rgb(0x7d7d7d)).opacity(0.3)
                        }
                    });
                    assert!(cx.update(|window, _| {
                        window
                            .painted_quad_opacities_for_test()
                            .iter()
                            .any(|(actual, color)| *actual == bounds && *color == expected)
                    }));
                }
                cx.simulate_mouse_down(bounds.center(), MouseButton::Left, gpui::Modifiers::none());
                let label = ["Minimize", "Maximize", "Close"][action];
                assert!(cx.update(|window, _| {
                    let tree = window.accessibility_tree_for_test().unwrap();
                    tree.nodes.iter().any(|(id, node)| {
                        *id == tree.focus
                            && node.role() == gpui::Role::Button
                            && node.label() == Some(label)
                    })
                }));
                // Cancel mouse activation: OS minimize/maximize remain deferred.
                cx.simulate_mouse_up(
                    point(px(300.), px(150.)),
                    MouseButton::Left,
                    gpui::Modifiers::none(),
                );
                cx.run_until_parked();
            }
        }
        // Keyboard activation uses the focused close control on this headless window.
        cx.simulate_keystrokes("space");
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("space").unwrap(),
        });
        assert!(cx.windows().is_empty());
        drop(app);
        std::fs::remove_dir_all(dir).ok();
    }
    #[gpui::test]
    fn window_preferences_match_original_dependencies_and_restart_boundaries(
        cx: &mut gpui::TestAppContext,
    ) {
        use feishin_core::{migration, model::Item, queue::Enqueue, storage};
        use gpui::{AppContext, Modifiers, Role};
        use gpui_component::Root;
        use std::{cell::RefCell, rc::Rc};
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/window-settings-reference.json")).unwrap();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-window-settings-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.queue.add(
                vec![
                    Item {
                        id: "same".into(),
                        ..Default::default()
                    };
                    2
                ],
                Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1600.), px(1600.)));
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let fields = [
            "window_bar_style",
            "window_title_track_info",
            "tray",
            "exit_to_tray",
            "minimize_to_tray",
            "start_minimized",
            "prevent_sleep",
            "prevent_suspend",
        ];
        for language in ["en", "fr", "ar"] {
            // These fixtures change window preferences, not theme/font settings.
            // Resolve the language once instead of rebuilding fonts for every control.
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.state.settings.language = language.into();
                    view.apply_theme(window, cx);
                })
            });
            for (case_index, case) in reference.iter().enumerate() {
                // All dependency combinations in English, then every tray/sleep combination in both other locales.
                if language != "en" && case_index >= 8 {
                    continue;
                }
                for control in case["controls"].as_array().unwrap() {
                    let key = control["title"].as_str().unwrap();
                    let label = feishin_core::i18n::translate(language, key).unwrap();
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.state.settings = migration::preview(
                                &view.state.settings,
                                &serde_json::to_vec(
                                    &serde_json::json!({"general":{},"window":case["settings"]}),
                                )
                                .unwrap(),
                            )
                            .unwrap()
                            .settings;
                            view.state.settings.language = language.into();
                            view.window_bar_style_active =
                                case["settings"]["windowBarStyle"].as_str().unwrap().into();
                            view.native_hide_supported = true;
                            view.message.clear();
                            view.route = crate::app::Route::Settings;
                            view.state.settings.settings_tab =
                                feishin_core::settings_search::Tab::Window;
                            view.settings_search
                                .update(cx, |input, cx| input.set_value(label.clone(), window, cx));
                            cx.notify();
                        })
                    });
                    cx.run_until_parked();
                    let before = app.read_with(cx, |view, _| view.state.settings.clone());
                    if control["type"] == "select" {
                        for change in control["changes"].as_array().unwrap() {
                            let Some(style) = change["value"].as_str() else {
                                continue;
                            };
                            app.update(cx, |view, cx| {
                                view.state.settings = before.clone();
                                view.window_bar_style_active = before.window_bar_style.clone();
                                view.message.clear();
                                view.change_window_bar_style(style, cx);
                                assert_eq!(view.state.settings.window_bar_style, style);
                                assert_eq!(
                                    view.message == view.tr("Restart required"),
                                    change["restarts"] == 1
                                );
                                assert_eq!(
                                    view.window_bar_style_active,
                                    if before.window_bar_style == "linux" || style == "linux" {
                                        before.window_bar_style.as_str()
                                    } else {
                                        style
                                    }
                                );
                            });
                        }
                        continue;
                    }
                    let node = cx.update(|window, _| {
                        window
                            .accessibility_tree_for_test()
                            .unwrap()
                            .nodes
                            .iter()
                            .find(|(_, node)| {
                                node.role() == Role::CheckBox
                                    && node.label() == Some(label.as_str())
                            })
                            .cloned()
                    });
                    assert_eq!(
                        node.is_none(),
                        control["hidden"].as_bool().unwrap(),
                        "{language}: {case_index}: {key}"
                    );
                    let Some((_, node)) = node else {
                        continue;
                    };
                    let disabled = control["disabled"].as_bool().unwrap();
                    assert_eq!(node.is_disabled(), disabled, "{language}: {key}");
                    let next = node.toggled() != Some(gpui::Toggled::True);
                    let bounds = node.bounds().unwrap();
                    let position = cx.update(|window, _| {
                        let scale = f64::from(window.scale_factor());
                        point(
                            px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                            px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
                        )
                    });
                    cx.simulate_click(position, Modifiers::none());
                    cx.run_until_parked();
                    let change = control["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|change| change["value"] == next)
                        .unwrap();
                    let expected = if disabled {
                        before
                    } else {
                        migration::preview(
                            &before,
                            &serde_json::to_vec(&serde_json::json!({"general":{},"window":change["settings"]["window"]})).unwrap(),
                        )
                        .unwrap()
                        .settings
                    };
                    app.read_with(cx, |view, _| {
                        let actual = serde_json::to_value(&view.state.settings).unwrap();
                        let expected = serde_json::to_value(expected).unwrap();
                        for field in fields {
                            assert_eq!(
                                actual[field], expected[field],
                                "{language}: {case_index}: {key}: {field}"
                            );
                        }
                        if !disabled {
                            let saved = serde_json::to_value(
                                storage::load(&dir.join("state.json")).unwrap().settings,
                            )
                            .unwrap();
                            for field in fields {
                                assert_eq!(
                                    saved[field], expected[field],
                                    "{language}: saved {key}: {field}"
                                );
                            }
                        }
                        assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
                        assert!(!view.playing);
                    });
                }
            }
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.settings_search.update(cx, |input, cx| {
                        input.set_value(
                            feishin_core::i18n::translate(language, "setting.windowBarStyle")
                                .unwrap(),
                            window,
                            cx,
                        )
                    });
                    cx.notify();
                })
            });
            cx.run_until_parked();
            for (choice, label) in [
                ("macos", "macOS"),
                ("web", "Web (hidden)"),
                ("windows", "Windows"),
                ("linux", "Native"),
            ] {
                let button = cx.debug_bounds("window-style").unwrap().center();
                cx.simulate_click(button, Modifiers::none());
                cx.run_until_parked();
                let bounds = cx.update(|window, _| {
                    let tree = window.accessibility_tree_for_test().unwrap();
                    let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
                    tree.nodes
                        .iter()
                        .find(|(id, node)| {
                            node.role() == Role::MenuItem
                                && consumer
                                    .state()
                                    .node_by_tree_local_id(*id, gpui::accesskit::TreeId::ROOT)
                                    .unwrap()
                                    .label()
                                    .as_deref()
                                    == Some(label)
                        })
                        .unwrap()
                        .1
                        .bounds()
                        .unwrap()
                });
                let position = cx.update(|window, _| {
                    let scale = f64::from(window.scale_factor());
                    point(
                        px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                        px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
                    )
                });
                cx.simulate_click(position, Modifiers::none());
                cx.run_until_parked();
                assert_eq!(
                    app.read_with(cx, |view, _| view.state.settings.window_bar_style.clone()),
                    choice
                );
                assert_eq!(
                    storage::load(&dir.join("state.json"))
                        .unwrap()
                        .settings
                        .window_bar_style,
                    choice
                );
            }
            for style in ["macos", "windows"] {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.window_bar_style_active = style.into();
                        view.state.settings.language = language.into();
                        view.apply_theme(window, cx);
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                for key in ["Minimize", "Maximize", "Close"] {
                    let label = app.read_with(cx, |view, _| view.tr(key));
                    assert!(cx.update(|window, _| {
                        window
                            .accessibility_tree_for_test()
                            .unwrap()
                            .nodes
                            .iter()
                            .any(|(_, node)| {
                                node.role() == Role::Button && node.label() == Some(label.as_str())
                            })
                    }));
                }
            }
        }
        let saved = storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(serde_json::to_value(saved.queue).unwrap(), queue);
        let state_path = dir.join("state.json");
        std::fs::remove_file(&state_path).unwrap();
        std::fs::create_dir(&state_path).unwrap();
        app.update(cx, |view, cx| {
            view.change_window_bar_style("windows", cx);
            assert!(view.message.starts_with("Could not save settings:"));
        });
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn restored_windows_remain_visible_after_display_changes() {
        let display = Bounds::new(point(px(-1440.0), px(0.0)), size(px(1440.0), px(900.0)));
        let saved = WindowPlacement {
            x: 6000.0,
            y: 4000.0,
            width: 4000.0,
            height: 2000.0,
            fullscreen: false,
            maximized: true,
        };
        assert_eq!(
            restore(Some(saved), &[display], display),
            WindowBounds::Maximized(display)
        );
        let saved = WindowPlacement {
            x: -1000.0,
            y: 50.0,
            width: 1000.0,
            height: 700.0,
            fullscreen: true,
            maximized: false,
        };
        let restored = restore(Some(saved), &[display], display);
        assert!(matches!(restored, WindowBounds::Fullscreen(_)));
        assert_eq!(restored.get_bounds().origin.x, px(-1000.0));
        assert_eq!(
            restore(
                Some(WindowPlacement {
                    width: f32::NAN,
                    ..saved
                }),
                &[display],
                display
            ),
            WindowBounds::Windowed(display)
        );
        assert_eq!(
            restore(None, &[display], display),
            WindowBounds::Windowed(display)
        );
    }
}
