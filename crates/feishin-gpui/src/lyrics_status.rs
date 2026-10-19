use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt, animation::CssTransition,
    button::ButtonVariants, spinner::Spinner,
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Loading,
    Missing,
    Text,
}
struct State {
    mode: Option<Mode>,
    inputs: Option<(bool, bool, bool)>,
    faded: bool,
    deadline: Option<Instant>,
    opacity: CssTransition,
}
impl Default for State {
    fn default() -> Self {
        Self {
            mode: None,
            inputs: None,
            faded: false,
            deadline: None,
            opacity: CssTransition::new(1., Duration::from_millis(500), [0., 0., 0.58, 1.]),
        }
    }
}
impl State {
    fn tween(&mut self, target: f32, now: Instant) {
        self.opacity = CssTransition::new(
            self.opacity.value(now).0,
            Duration::from_millis(500),
            [0., 0., 0.58, 1.],
        );
        self.opacity.retarget(target, now);
    }
    fn frame(
        &mut self,
        busy: bool,
        has_lyrics: bool,
        fade: bool,
        now: Instant,
    ) -> (Mode, f32, bool, Option<Duration>) {
        let old_deadline = self.deadline;
        if let Some(deadline) = self.deadline.filter(|deadline| now >= *deadline) {
            self.faded = true;
            self.deadline = None;
            self.tween(0., deadline);
        }
        let mode = if busy {
            Mode::Loading
        } else if has_lyrics {
            Mode::Text
        } else {
            Mode::Missing
        };
        if self.mode != Some(mode) {
            self.opacity = CssTransition::new(
                if mode == Mode::Text { 0. } else { 1. },
                Duration::from_millis(500),
                [0., 0., 0.58, 1.],
            );
            if mode == Mode::Text {
                self.opacity.retarget(1., now);
            }
            if mode == Mode::Missing && self.faded && fade {
                self.opacity.retarget(0., now);
            }
            self.mode = Some(mode);
        }
        if self.inputs != Some((busy, has_lyrics, fade)) {
            self.deadline = None;
            if !fade {
                if mode == Mode::Missing && self.faded {
                    self.opacity = CssTransition::new(1., Duration::ZERO, [0., 0., 0.58, 1.]);
                }
                self.faded = false;
            } else if !busy && !has_lyrics {
                self.deadline = Some(now + Duration::from_secs(3));
            }
            if has_lyrics {
                self.faded = false;
            }
            self.inputs = Some((busy, has_lyrics, fade));
        }
        let (opacity, animating) = self.opacity.value(now);
        let timer = self
            .deadline
            .filter(|deadline| Some(*deadline) != old_deadline)
            .map(|deadline| deadline.saturating_duration_since(now));
        (mode, opacity, mode != Mode::Loading && animating, timer)
    }
}

struct SettingsDialog {
    previous: String,
    parent: Entity<Feishin>,
    _subscription: Subscription,
}
impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .parent
            .update(cx, |parent, cx| parent.lyric_settings(cx));
        let parent = self.parent.downgrade();
        let previous = self.previous.clone();
        div()
            .v_flex()
            .id("lyrics-settings-dialog-content")
            .debug_selector(|| "lyrics-settings-dialog-content".into())
            .h(px(f32::from(window.viewport_size().height) * 0.75))
            .w_full()
            .child(
                div()
                    .id("lyrics-settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(
                crate::styles::button("lyrics-settings-close", cx)
                    .debug_selector(|| "lyrics-settings-close".into())
                    .label(crate::appearance::text("common.close", cx))
                    .on_click(move |_, window, cx| {
                        parent
                            .update(cx, |parent, cx| {
                                parent.set_lyric_settings_key(&previous, window, cx)
                            })
                            .ok();
                        window.close_dialog(cx);
                    }),
            )
    }
}
impl Feishin {
    pub(crate) fn open_lyrics_settings_dialog(
        &mut self,
        sidebar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = self.lyric_settings_key.clone();
        self.set_lyric_settings_key(if sidebar { "sidebar" } else { "default" }, window, cx);
        let parent = cx.entity();
        let content = cx.new(|cx| SettingsDialog {
            previous: previous.clone(),
            parent: parent.clone(),
            _subscription: cx.observe(&parent, |_, _, cx| cx.notify()),
        });
        let parent = parent.downgrade();
        let zoom = self.state.settings.zoom;
        window.open_dialog(cx, move |dialog, _, _| {
            let parent = parent.clone();
            let previous = previous.clone();
            dialog
                .width(rems(48.75))
                .overlay(false)
                .pop_transition(px(10. * zoom))
                .close_button(false)
                .child(content.clone())
                .on_close(move |_, window, cx| {
                    parent
                        .update(cx, |parent, cx| {
                            parent.set_lyric_settings_key(&previous, window, cx)
                        })
                        .ok();
                })
        });
    }
    pub(crate) fn lyrics_frame(
        &self,
        content: AnyElement,
        sidebar: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = cx.background_executor().now();
        let status = window.use_keyed_state(
            SharedString::from(format!("lyrics-status-{sidebar}")),
            cx,
            |_, _| State::default(),
        );
        let (mode, opacity, animating, timer) = status.update(cx, |state, _| {
            state.frame(
                self.lyric_busy,
                !self.lyrics.is_empty(),
                !sidebar || self.state.settings.panels.combined,
                now,
            )
        });
        if let Some(delay) = timer {
            let timer = cx.background_executor().timer(delay);
            cx.spawn(async move |this, cx| {
                timer.await;
                this.update(cx, |_, cx| cx.notify()).ok();
            })
            .detach();
        }
        if animating {
            window.request_animation_frame();
        }
        let hover = window.use_keyed_state(
            SharedString::from(format!("lyrics-settings-hover-{sidebar}")),
            cx,
            |_, _| CssTransition::new(0., Duration::from_millis(200), [0.25, 0.1, 0.25, 1.]),
        );
        let (settings_opacity, hovering) = hover.read(cx).value(now);
        if hovering {
            window.request_animation_frame();
        }
        let actions = window.use_keyed_state(
            SharedString::from(format!("lyrics-actions-state-{sidebar}")),
            cx,
            crate::lyrics::actions::State::new,
        );
        let action_strip = self.lyric_actions(sidebar, &actions, window, cx);
        let rem = f32::from(cx.theme().font_size);
        let body = match mode {
            Mode::Loading => div()
                .id(SharedString::from(format!("lyrics-loading-{sidebar}")))
                .debug_selector(move || format!("lyrics-loading-{sidebar}"))
                .role(Role::Status)
                .aria_label(self.tr("Loading lyrics..."))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Spinner::new()
                        .icon(Icon::default().path("native-icons/spinner.svg"))
                        .with_size(px(rem * 1.125))
                        .animation(Duration::from_secs(1), |value| value),
                )
                .into_any_element(),
            Mode::Missing => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .id(SharedString::from(format!("lyrics-missing-{sidebar}")))
                        .debug_selector(move || format!("lyrics-missing-{sidebar}"))
                        .opacity(opacity)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().muted_foreground)
                        .child(self.tr("No lyrics available")),
                )
                .into_any_element(),
            Mode::Text => div()
                .size_full()
                .opacity(opacity)
                .child(content)
                .into_any_element(),
        };
        div()
            .id(SharedString::from(format!("lyrics-frame-{sidebar}")))
            .debug_selector(move || format!("lyrics-frame-{sidebar}"))
            .relative()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .size_full()
            .on_hover(move |hovered, window, cx| {
                actions.update(cx, |state, _| state.frame_hover(*hovered, window));
                hover.update(cx, |hover, cx| {
                    let target = f32::from(*hovered);
                    if hover.target() != target {
                        hover.retarget(target, cx.background_executor().now());
                        cx.notify();
                    }
                })
            })
            .child(body)
            .child(action_strip)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .opacity(settings_opacity)
                    .child(
                        crate::styles::button(
                            if sidebar {
                                "lyrics-sidebar-settings"
                            } else {
                                "lyrics-player-settings"
                            },
                            cx,
                        )
                        .debug_selector(move || {
                            if sidebar {
                                "lyrics-sidebar-settings"
                            } else {
                                "lyrics-player-settings"
                            }
                            .into()
                        })
                        .ghost()
                        .icon(
                            Icon::default()
                                .path("native-icons/settings.svg")
                                .with_size(px(rem)),
                        )
                        .tooltip(self.tr("common.configure"))
                        .size(px(rem * 2.25))
                        .p_0()
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.open_lyrics_settings_dialog(sidebar, window, cx)
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Mode, State};
    use std::time::{Duration, Instant};
    #[gpui::test]
    fn native_lyric_loading_and_independent_empty_surfaces_follow_original_timers(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::app::{Feishin, Route};
        use feishin_core::{
            model::{Item, ItemKind},
            queue::Enqueue,
            storage::State,
        };
        use gpui::{AppContext, Role};
        use gpui_component::{ActiveTheme, Root};
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.use_platform_text_system()
            .add_fonts(
                feishin_core::fonts::builtin_fonts()
                    .unwrap()
                    .into_iter()
                    .map(|font| std::borrow::Cow::Owned(font.bytes))
                    .collect(),
            )
            .unwrap();
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-lyric-status-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.queue.add(
                vec![Item {
                    kind: ItemKind::Song,
                    name: "Empty lyric fixture".into(),
                    ..Default::default()
                }],
                Enqueue::Now,
            );
            let app = cx.new(|cx| {
                let mut app = Feishin::new(dir.clone(), Ok(state), window, cx);
                app.route = Route::Lyrics;
                app.lyric_busy = true;
                app
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Status
                    && node.label() == Some("Loading lyrics..."))
        );
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.lyric_busy = false;
                cx.notify();
            })
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let label = cx.debug_bounds("lyrics-missing-false").unwrap();
        let frame = cx.debug_bounds("lyrics-frame-false").unwrap();
        assert!((label.center().x - frame.center().x).abs() <= gpui::px(1.));
        assert!((label.center().y - frame.center().y).abs() <= gpui::px(1.));
        let opacity = |cx: &mut gpui::VisualTestContext, sidebar| {
            let bounds = cx
                .debug_bounds(&format!("lyrics-missing-{sidebar}"))
                .unwrap();
            cx.update(|window, cx| {
                window
                    .painted_glyphs_with_opacity_for_test()
                    .iter()
                    .filter(|(glyph, _, _)| bounds.contains(&glyph.center()))
                    .map(|(_, _, color)| color.a / cx.theme().muted_foreground.a)
                    .fold(0_f32, f32::max)
            })
        };
        assert!((opacity(cx, false) - 1.).abs() < 0.001);
        cx.executor().advance_clock(Duration::from_secs(3));
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!((opacity(cx, false) - 0.315357).abs() < 0.001);
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(opacity(cx, false), 0.);
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.panels.expanded = true;
                app.state.settings.panels.queue = false;
                app.state.settings.panels.visualizer = false;
                app.state.settings.panels.lyrics = true;
                app.state.settings.panels.combined = false;
                cx.notify();
            })
        });
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(4));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!((opacity(cx, true) - 1.).abs() < 0.001);
        assert_eq!(opacity(cx, false), 0.);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn lyric_status_matches_original_missing_loading_and_text_motion() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/lyrics-status-motion-reference.json"
        ))
        .unwrap();
        let now = Instant::now();
        for scenario in reference["scenarios"].as_array().unwrap() {
            let mut state = State::default();
            let mut inputs = scenario["initial"].clone();
            let mut updates = scenario["updates"].as_array().unwrap().iter().peekable();
            state.frame(
                inputs["busy"] == true,
                inputs["hasLyrics"] == true,
                inputs["fade"] == true,
                now,
            );
            for frame in scenario["frames"].as_array().unwrap() {
                let elapsed = frame["milliseconds"].as_u64().unwrap();
                while updates
                    .peek()
                    .is_some_and(|event| event[0].as_u64().unwrap() <= elapsed)
                {
                    let event = updates.next().unwrap();
                    inputs = event[1].clone();
                    state.frame(
                        inputs["busy"] == true,
                        inputs["hasLyrics"] == true,
                        inputs["fade"] == true,
                        now + Duration::from_millis(event[0].as_u64().unwrap()),
                    );
                }
                let (mode, opacity, _, _) = state.frame(
                    inputs["busy"] == true,
                    inputs["hasLyrics"] == true,
                    inputs["fade"] == true,
                    now + Duration::from_millis(elapsed),
                );
                assert_eq!(
                    frame["mode"].as_str().unwrap(),
                    match mode {
                        Mode::Loading => "loading",
                        Mode::Missing => "missing",
                        Mode::Text => "text",
                    }
                );
                if let Some(expected) = frame["opacity"].as_f64() {
                    assert!(
                        (opacity - expected as f32).abs() < 0.0001,
                        "{scenario}: {frame}, actual={opacity}"
                    );
                }
                assert_eq!(
                    u64::from(state.deadline.is_some()),
                    frame["timers"].as_u64().unwrap(),
                    "{scenario}: {frame}"
                );
            }
        }
    }
}
