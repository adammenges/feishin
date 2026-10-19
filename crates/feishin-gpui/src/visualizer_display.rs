use crate::app::{Feishin, Route};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, StyledExt, WindowExt, animation::CssTransition,
    button::ButtonVariants,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct State {
    pub expanded: bool,
    pub mounted: bool,
    pub return_to_player: bool,
    pub settings_open: bool,
    pub settings_close_requested: bool,
    exit_fullscreen_requested: bool,
    route: Option<Route>,
    current: Option<Uuid>,
    slide: CssTransition,
    song: CssTransition,
    song_deadline: Option<Instant>,
    close_deadline: Option<Instant>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            expanded: false,
            mounted: false,
            return_to_player: false,
            settings_open: false,
            settings_close_requested: false,
            exit_fullscreen_requested: false,
            route: None,
            current: None,
            slide: CssTransition::new(0., Duration::from_millis(500), [0.42, 0., 0.58, 1.]),
            song: CssTransition::new(0., Duration::from_millis(500), [0.42, 0., 0.58, 1.]),
            song_deadline: None,
            close_deadline: None,
        }
    }
}
fn tween(
    transition: &mut CssTransition,
    target: f32,
    duration: Duration,
    delay: Duration,
    now: Instant,
) {
    if transition.target() == target {
        return;
    }
    let from = transition.value(now).0;
    *transition = CssTransition::new(from, duration, [0.42, 0., 0.58, 1.]);
    transition.retarget(target, now + delay);
}
impl State {
    pub fn restored(expanded: bool) -> Self {
        Self {
            expanded,
            slide: CssTransition::new(
                if expanded { 1. } else { 0. },
                Duration::from_millis(500),
                [0.42, 0., 0.58, 1.],
            ),
            ..Default::default()
        }
    }
    fn open(&mut self, player: bool, current: Option<Uuid>, route: Route, now: Instant) {
        let fresh = !self.mounted;
        self.expanded = true;
        self.mounted = true;
        self.close_deadline = None;
        self.exit_fullscreen_requested = false;
        self.return_to_player = player;
        self.route = Some(route);
        if fresh {
            self.current = current;
            self.song = CssTransition::new(0., Duration::from_millis(500), [0.42, 0., 0.58, 1.]);
            self.song_deadline = None;
        }
        tween(
            &mut self.slide,
            1.,
            Duration::from_millis(500),
            Duration::from_millis(100),
            now,
        );
    }
    pub fn dismiss(&mut self, now: Instant) -> bool {
        let player = self.return_to_player;
        if self.slide.target() != 0. {
            self.close_deadline =
                (self.slide.value(now).0 > 0.).then_some(now + Duration::from_millis(500));
        }
        self.expanded = false;
        self.return_to_player = false;
        self.exit_fullscreen_requested = self.mounted;
        tween(
            &mut self.slide,
            0.,
            Duration::from_millis(500),
            Duration::ZERO,
            now,
        );
        player
    }
    fn toggle(&mut self, player: bool, current: Option<Uuid>, route: Route, now: Instant) -> bool {
        if self.expanded {
            self.dismiss(now)
        } else {
            self.open(player, current, route, now);
            false
        }
    }
    fn frame(&mut self, current: Option<Uuid>, now: Instant) -> (f32, f32, bool) {
        if let Some(deadline) = self.song_deadline.filter(|deadline| now >= *deadline) {
            tween(
                &mut self.song,
                0.,
                Duration::from_millis(1500),
                Duration::ZERO,
                deadline,
            );
            self.song_deadline = None;
        }
        if self.mounted && current != self.current {
            self.current = current;
            if current.is_some() {
                tween(
                    &mut self.song,
                    1.,
                    Duration::from_millis(500),
                    Duration::ZERO,
                    now,
                );
                self.song_deadline = Some(now + Duration::from_secs(3));
            }
        }
        let (slide, sliding) = self.slide.value(now);
        let (song, fading) = self.song.value(now);
        let closing = self.close_deadline.is_some_and(|deadline| now <= deadline);
        if !self.expanded && !sliding && !closing {
            self.mounted = false;
            self.route = None;
            self.current = None;
            self.song_deadline = None;
            self.close_deadline = None;
        }
        (
            slide,
            song,
            sliding || closing || fading || self.song_deadline.is_some(),
        )
    }
}
impl Feishin {
    pub fn persist_player_modes(&mut self) {
        self.state.settings.player_display.expanded = self.full_player;
        self.state.settings.player_display.visualizer_expanded = self.visualizer_display.expanded;
        self.save();
    }
    pub fn restore_player_modes(&mut self) {
        let expanded = self.state.settings.player_display.visualizer_expanded;
        let leave = self.visualizer_display.mounted && !expanded;
        self.full_player = self.state.settings.player_display.expanded && !expanded;
        self.visualizer_display = State::restored(expanded);
        self.visualizer_display.exit_fullscreen_requested = leave;
    }
    pub fn toggle_expanded_visualizer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.current_entry().map(|entry| entry.key);
        self.full_player = self.visualizer_display.toggle(
            self.full_player,
            current,
            self.route.clone(),
            cx.background_executor().now(),
        );
        if self.full_player {
            self.restore_player_tab(cx);
        }
        self.persist_player_modes();
        window.focus(&self.focus);
        cx.notify();
    }
    pub fn sync_visualizer_display(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        if self.visualizer_display.expanded && !self.visualizer_display.mounted {
            self.visualizer_display.open(
                false,
                self.current_entry().map(|entry| entry.key),
                self.route.clone(),
                now,
            );
        }
        if self.visualizer_display.expanded
            && self.visualizer_display.route.as_ref() != Some(&self.route)
        {
            self.visualizer_display.dismiss(now);
            self.persist_player_modes();
        }
        if self.visualizer_display.exit_fullscreen_requested && !self.visualizer_display.mounted {
            self.visualizer_display.exit_fullscreen_requested = false;
            if window.is_fullscreen() {
                window.toggle_fullscreen();
            }
        }
        if self.visualizer_display.settings_close_requested {
            self.visualizer_display.settings_close_requested = false;
            window.close_dialog(cx);
        }
    }
    pub fn expanded_visualizer_view(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.visualizer_display.mounted {
            return None;
        }
        let current = self.current_entry().cloned();
        let (slide, song, animating) = self.visualizer_display.frame(
            current.as_ref().map(|entry| entry.key),
            cx.background_executor().now(),
        );
        if !self.visualizer_display.mounted {
            self.sync_visualizer_display(window, cx);
            return None;
        }
        if animating {
            window.request_animation_frame();
        }
        let rem = f32::from(cx.theme().font_size);
        let width = f32::from(window.viewport_size().width);
        let surface = self.visualizer_surface(true, true, window, cx);
        let overlay = current.filter(|_| song > 0.).map(|entry| {
            let title = (width * 0.08).clamp(rem * 2., rem * 6.);
            let artist = (width * 0.05).clamp(rem * 1.25, rem * 3.5);
            div()
                .absolute()
                .inset_0()
                .h_flex()
                .items_center()
                .justify_center()
                .opacity(song)
                .child(div().absolute().inset_0().bg(black().opacity(0.6)))
                .child(
                    div()
                        .id("visualizer-song-info")
                        .debug_selector(|| "visualizer-song-info".into())
                        .relative()
                        .v_flex()
                        .items_center()
                        .w(relative(0.6))
                        .p(px(rem))
                        .gap(px(rem))
                        .text_align(TextAlign::Center)
                        .text_color(rgb(0xdddddd))
                        .text_shadows(vec![TextShadow {
                            offset: point(px(0.), px(2. * self.state.settings.zoom)),
                            blur_radius: px(8. * self.state.settings.zoom),
                            color: Some(black().opacity(0.5)),
                        }])
                        .child(
                            div()
                                .id("visualizer-song-title")
                                .debug_selector(|| "visualizer-song-title".into())
                                .w_full()
                                .text_size(px(title))
                                .line_height(relative(1.2))
                                .font_weight(FontWeight::EXTRA_BOLD)
                                .child(entry.item.name),
                        )
                        .when(!entry.item.artist.is_empty(), |div| {
                            div.child(
                                gpui::div()
                                    .id("visualizer-song-artist")
                                    .debug_selector(|| "visualizer-song-artist".into())
                                    .w_full()
                                    .text_size(px(artist))
                                    .line_height(relative(1.3))
                                    .child(entry.item.artist),
                            )
                        }),
                )
        });
        Some(
            div()
                .id("expanded-visualizer")
                .debug_selector(|| "expanded-visualizer".into())
                .absolute()
                .left_0()
                .right_0()
                .top(relative(1. - slide))
                .h_full()
                .overflow_hidden()
                .occlude()
                .bg(cx.theme().background)
                .child(surface)
                .children(overlay)
                .into_any_element(),
        )
    }
}

struct SettingsDialog {
    parent: Entity<Feishin>,
    _subscription: Subscription,
}
impl Render for SettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.parent.read(cx).message.clone();
        let content = self
            .parent
            .update(cx, |parent, cx| parent.visualizer_settings_view(cx));
        div()
            .v_flex()
            .gap_2()
            .h(px(f32::from(window.viewport_size().height) * 0.75))
            .w_full()
            .when(error.starts_with("Invalid visualizer settings:"), |div| {
                div.child(
                    gpui::div()
                        .id("visualizer-settings-error")
                        .role(Role::Status)
                        .aria_live(gpui::accesskit::Live::Polite)
                        .child(error),
                )
            })
            .child(div().flex_1().min_h_0().w_full().child(content))
    }
}
impl Feishin {
    pub fn open_visualizer_settings_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let parent = cx.entity();
        self.visualizer_display.settings_open = true;
        let content = cx.new(|cx| SettingsDialog {
            parent: parent.clone(),
            _subscription: cx.observe(&parent, |_, _, cx| cx.notify()),
        });
        let parent = parent.downgrade();
        let title = feishin_core::i18n::translate_with(
            &self.state.settings.language,
            "common.setting",
            &serde_json::json!({"count": 2}),
        )
        .unwrap_or_else(|| "Settings".into());
        let zoom = self.state.settings.zoom;
        window.open_dialog(cx, move |dialog, window, _| {
            let parent = parent.clone();
            dialog
                .title(title.clone())
                .aria_label(title.clone())
                .width(rems(48.75))
                .overlay(false)
                .pop_transition(px(10. * zoom))
                .max_w(px(1400. * zoom))
                .h(window.viewport_size().height * 0.81)
                .min_h(px(600. * zoom))
                .child(content.clone())
                .on_close(move |_, _, cx| {
                    parent
                        .update(cx, |parent, cx| {
                            parent.visualizer_display.settings_open = false;
                            parent.visualizer_settings = None;
                            parent.visualizer_subscriptions.clear();
                            parent.visualizer_numbers.clear();
                            cx.notify();
                        })
                        .ok();
                })
        });
        cx.notify();
    }
    pub fn visualizer_compact_frame(
        &self,
        surface: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hover = window.use_keyed_state("visualizer-hover-controls", cx, |_, _| {
            CssTransition::new(0., Duration::from_millis(200), [0.42, 0., 0.58, 1.])
        });
        let (opacity, animating) = hover.read(cx).value(cx.background_executor().now());
        if animating {
            window.request_animation_frame();
        }
        let rem = f32::from(cx.theme().font_size);
        let expanded = self.visualizer_display.expanded;
        div()
            .id("visualizer-surface")
            .debug_selector(|| "visualizer-surface".into())
            .relative()
            .size_full()
            .overflow_hidden()
            .on_hover(move |hovered, _, cx| {
                hover.update(cx, |hover, cx| {
                    hover.retarget(f32::from(*hovered), cx.background_executor().now());
                    cx.notify();
                })
            })
            .child(surface)
            .child(
                div()
                    .absolute()
                    .top(px(rem * 0.5))
                    .right(px(rem * 0.5))
                    .h_flex()
                    .gap(px(rem * 0.25))
                    .opacity(opacity)
                    .child(
                        crate::styles::button(
                            if expanded {
                                "shrink-expanded-visualizer"
                            } else {
                                "expand-visualizer"
                            },
                            cx,
                        )
                        .debug_selector(move || {
                            if expanded {
                                "shrink-expanded-visualizer"
                            } else {
                                "expand-visualizer"
                            }
                            .into()
                        })
                        .ghost()
                        .icon(
                            Icon::default()
                                .path(if expanded {
                                    "native-icons/shrink.svg"
                                } else {
                                    "native-icons/expand.svg"
                                })
                                .with_size(px(rem)),
                        )
                        .tooltip(self.tr(if expanded {
                            "common.minimize"
                        } else {
                            "common.maximize"
                        }))
                        .size(px(rem * 2.25))
                        .p_0()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_expanded_visualizer(window, cx)
                        })),
                    )
                    .child(
                        crate::styles::button("expanded-visualizer-settings", cx)
                            .debug_selector(|| "expanded-visualizer-settings".into())
                            .ghost()
                            .icon(
                                Icon::default()
                                    .path("native-icons/settings.svg")
                                    .with_size(px(rem)),
                            )
                            .tooltip(self.tr("common.configure"))
                            .size(px(rem * 2.25))
                            .p_0()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.edit_visualizer(window, cx)),
                            ),
                    ),
            )
            .when(
                self.state.settings.visualizer.kind == "butterchurn",
                |div| {
                    div.child(
                        gpui::div()
                            .absolute()
                            .bottom(px(rem * 0.5))
                            .right(px(rem * 0.5))
                            .h_flex()
                            .gap(px(rem * 0.25))
                            .opacity(opacity)
                            .child(
                                crate::styles::button("compact-milkdrop-previous", cx)
                                    .debug_selector(|| "compact-milkdrop-previous".into())
                                    .ghost()
                                    .icon(
                                        Icon::default()
                                            .path("native-icons/arrow-left.svg")
                                            .with_size(px(rem)),
                                    )
                                    .size(px(rem * 2.25))
                                    .p_0()
                                    .tooltip(self.tr("Previous preset"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.advance_milkdrop(true, cx)
                                    })),
                            )
                            .child(
                                crate::styles::button("compact-milkdrop-next", cx)
                                    .debug_selector(|| "compact-milkdrop-next".into())
                                    .ghost()
                                    .icon(
                                        Icon::default()
                                            .path("native-icons/arrow-right.svg")
                                            .with_size(px(rem)),
                                    )
                                    .size(px(rem * 2.25))
                                    .p_0()
                                    .tooltip(self.tr("Next preset"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.advance_milkdrop(false, cx)
                                    })),
                            ),
                    )
                },
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::State;
    use crate::app::Route;
    use std::time::{Duration, Instant};
    #[test]
    fn expanded_visualizer_modes_match_original_navigation_handlers() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/expanded-visualizer-reference.json"))
                .unwrap();
        assert_eq!(cases.len(), 32);
        for case in cases {
            let source = &case["state"];
            let now = Instant::now();
            let mut player = source["expanded"].as_bool().unwrap();
            let mut state = State {
                expanded: source["visualizerExpanded"].as_bool().unwrap(),
                mounted: source["visualizerExpanded"].as_bool().unwrap(),
                return_to_player: source["visualizerReturnToPlayer"].as_bool().unwrap(),
                route: Some(Route::Home),
                ..Default::default()
            };
            match case["action"].as_str().unwrap() {
                "visualizer" => player = state.toggle(player, None, Route::Home, now),
                "player" => {
                    player = !player;
                    state.dismiss(now);
                }
                "escape" => {
                    if state.expanded {
                        if case["fullscreen"] != true {
                            player = state.dismiss(now);
                        }
                    } else if player {
                        player = false;
                    }
                }
                "route" => {
                    if state.expanded {
                        state.dismiss(now);
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(
                serde_json::json!({"expanded":player,"visualizerExpanded":state.expanded,"visualizerReturnToPlayer":state.return_to_player}),
                case["expected"],
                "{case}"
            );
        }
    }
    #[test]
    fn song_changes_reset_three_second_hold_and_preserve_delayed_slide_and_fade() {
        let start = Instant::now();
        let first = uuid::Uuid::new_v4();
        let second = uuid::Uuid::new_v4();
        let third = uuid::Uuid::new_v4();
        let mut state = State::default();
        state.open(true, Some(first), Route::Home, start);
        for milliseconds in [0, 50, 100] {
            let (slide, song, _) =
                state.frame(Some(first), start + Duration::from_millis(milliseconds));
            assert_eq!(slide, 0.);
            assert_eq!(song, 0.);
        }
        let (slide, _, _) = state.frame(Some(first), start + Duration::from_millis(350));
        assert!((slide - 0.5).abs() < 0.0001);
        assert_eq!(
            state
                .frame(Some(first), start + Duration::from_millis(600))
                .0,
            1.
        );
        state.frame(Some(second), start + Duration::from_millis(1000));
        assert!(
            (state
                .frame(Some(second), start + Duration::from_millis(1250))
                .1
                - 0.5)
                .abs()
                < 0.0001
        );
        assert_eq!(
            state
                .frame(Some(second), start + Duration::from_millis(1500))
                .1,
            1.
        );
        state.frame(Some(third), start + Duration::from_millis(3000));
        assert_eq!(
            state
                .frame(Some(third), start + Duration::from_millis(5999))
                .1,
            1.
        );
        assert!(
            (state
                .frame(Some(third), start + Duration::from_millis(6750))
                .1
                - 0.5)
                .abs()
                < 0.0001
        );
        assert_eq!(
            state
                .frame(Some(third), start + Duration::from_millis(7500))
                .1,
            0.
        );
        assert!(state.dismiss(start + Duration::from_millis(8000)));
        assert!(
            (state
                .frame(Some(third), start + Duration::from_millis(8250))
                .0
                - 0.5)
                .abs()
                < 0.0001
        );
        state.frame(Some(third), start + Duration::from_millis(8501));
        assert!(!state.mounted);
        assert!(!state.return_to_player);
    }

    #[test]
    fn expanded_visualizer_matches_original_motion_frames_and_interrupted_song_lifetime() {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/expanded-visualizer-motion-reference.json"
        ))
        .unwrap();
        for scenario in reference["scenarios"].as_array().unwrap() {
            let start = Instant::now();
            let first = uuid::Uuid::new_v4();
            let second = uuid::Uuid::new_v4();
            let third = uuid::Uuid::new_v4();
            let name = scenario["name"].as_str().unwrap();
            let mut state = if name == "restored" {
                State::restored(true)
            } else {
                State::default()
            };
            state.open(false, Some(first), Route::Home, start);
            let mut current = first;
            match name {
                "close" => {
                    state.frame(Some(first), start + Duration::from_millis(1000));
                    state.dismiss(start + Duration::from_millis(1000));
                }
                "interrupt-close" => {
                    let time = Duration::from_millis(scenario["interruptAt"].as_u64().unwrap());
                    state.frame(Some(first), start + time);
                    state.dismiss(start + time);
                }
                "song" | "song-reset" | "reopen-song" => {
                    state.frame(Some(second), start + Duration::from_millis(700));
                    current = second;
                    if name == "song-reset" {
                        state.frame(Some(third), start + Duration::from_millis(2700));
                        current = third;
                    }
                    if name == "reopen-song" {
                        state.dismiss(start + Duration::from_millis(1000));
                        state.frame(Some(second), start + Duration::from_millis(1100));
                        state.open(
                            false,
                            Some(second),
                            Route::Home,
                            start + Duration::from_millis(1100),
                        );
                    }
                }
                _ => {}
            }
            for frame in scenario["frames"].as_array().unwrap() {
                let now = start + Duration::from_millis(frame["milliseconds"].as_u64().unwrap());
                let (slide, song, _) = state.frame(Some(current), now);
                assert_eq!(
                    state.mounted,
                    frame["present"].as_bool().unwrap(),
                    "{name}: {frame}"
                );
                if state.mounted {
                    assert!(
                        (1. - f64::from(slide) - frame["y"].as_f64().unwrap()).abs() < 0.001,
                        "{name}: slide={slide}, {frame}"
                    );
                    for alpha in frame["info"].as_array().unwrap() {
                        assert!(
                            (f64::from(song) - alpha.as_f64().unwrap()).abs() < 0.001,
                            "{name}: song={song}, {frame}"
                        );
                    }
                }
                assert_eq!(
                    usize::from(state.song_deadline.is_some()),
                    frame["timers"].as_u64().unwrap() as usize,
                    "{name}: {frame}"
                );
            }
        }
    }

    #[gpui::test]
    fn expanded_visualizer_pointer_escape_modal_song_changes_and_restart_preserve_queue(
        cx: &mut gpui::TestAppContext,
    ) {
        use crate::app::{Feishin, LeaveFullPlayer, ToggleFullScreenWindow};
        use feishin_core::{model::Item, queue::Enqueue, storage::State as SavedState};
        use gpui::{AppContext, KeyBinding, Modifiers, Role, px, size};
        use gpui_component::{Root, WindowExt};
        use std::{cell::RefCell, rc::Rc};
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        cx.update(|cx| {
            cx.bind_keys([
                KeyBinding::new("escape", LeaveFullPlayer, Some("Feishin")),
                KeyBinding::new("f11", ToggleFullScreenWindow, Some("Feishin")),
            ])
        });
        let dir = std::env::temp_dir().join(format!(
            "feishin-expanded-visualizer-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = SavedState::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.panels.expanded = true;
            state.settings.panels.queue = false;
            state.settings.panels.lyrics = false;
            state.settings.panels.visualizer = true;
            state.settings.panels.order = vec![feishin_core::panels::Panel::Visualizer];
            state.settings.player_display.active_tab = "queue".into();
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        name: "First track".into(),
                        artist: "First artist".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        name: "Second track".into(),
                        artist: "Second artist".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            state.queue.position = 42.;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1280.), px(900.)));
        cx.run_until_parked();
        let click = |cx: &mut gpui::VisualTestContext, id: &str| {
            let bounds = cx
                .debug_bounds(id)
                .unwrap_or_else(|| panic!("Missing {id}"));
            cx.simulate_click(bounds.center(), Modifiers::none());
            cx.run_until_parked();
        };
        let keys = app.read_with(cx, |app, _| app.state.queue.order.clone());
        click(cx, "expand-visualizer");
        app.read_with(cx, |app, _| {
            assert!(app.visualizer_display.expanded);
            assert!(!app.full_player);
            assert!(app.active_panels().is_empty());
            assert_eq!(app.state.queue.position, 42.);
        });
        let start = cx.debug_bounds("expanded-visualizer").unwrap();
        let bar = cx.debug_bounds("player-bar").unwrap();
        assert!(start.origin.y >= bar.origin.y);
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let expanded = cx.debug_bounds("expanded-visualizer").unwrap();
        assert!((f32::from(expanded.size.width) - 1280.).abs() < 1.);
        let bar = cx.debug_bounds("player-bar").unwrap();
        assert!(
            (f32::from(expanded.bottom() - bar.origin.y)).abs() < 1.,
            "expanded={expanded:?}, bar={bar:?}"
        );
        assert!(app.read_with(cx, |app, _| {
            app.state.settings.player_display.visualizer_expanded
        }));
        app.update(cx, |app, _| app.state.settings.language = "fr".into());
        click(cx, "expanded-visualizer-settings");
        cx.executor().advance_clock(Duration::from_millis(310));
        cx.refresh().unwrap();
        cx.run_until_parked();
        assert!(cx.update(WindowExt::has_active_dialog));
        let title = feishin_core::i18n::translate_with(
            "fr",
            "common.setting",
            &serde_json::json!({"count":2}),
        )
        .unwrap();
        assert_ne!(title, "Settings");
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| {
                    node.label() == Some(title.as_str()) || node.value() == Some(title.as_str())
                })
        }));
        let history = app.read_with(cx, |app, _| app.history.clone());
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                let input = app.visualizer_settings.clone().unwrap();
                input.update(cx, |input, cx| input.set_value("invalid json", window, cx));
            })
        });
        click(cx, "save-visualizer-settings");
        assert!(cx.update(WindowExt::has_active_dialog));
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.role() == Role::Label
                && node
                    .value()
                    .is_some_and(|value| value.starts_with("Invalid visualizer settings:"))
        }));
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                let mut config = app.state.settings.visualizer.clone();
                config.audiomotionanalyzer["mode"] = serde_json::json!(3);
                let input = app.visualizer_settings.clone().unwrap();
                input.update(cx, |input, cx| {
                    input.set_value(serde_json::to_string(&config).unwrap(), window, cx)
                });
            })
        });
        click(cx, "save-visualizer-settings");
        assert!(!cx.update(WindowExt::has_active_dialog));
        app.update(cx, |app, _| app.state.settings.language = "en".into());
        app.read_with(cx, |app, _| {
            assert!(app.visualizer_display.expanded);
            assert_eq!(app.history, history);
            assert_eq!(app.state.settings.visualizer.audiomotionanalyzer["mode"], 3);
        });
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.queue.current = Some(keys[1]);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(500));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.value() == Some("Second track"))
        );
        assert!(cx.debug_bounds("visualizer-song-info").is_some());
        cx.simulate_keystrokes("f11");
        cx.run_until_parked();
        assert!(cx.update(|window, _| window.is_fullscreen()));
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button
                    && node.label() == Some("Expand player (Alt+Enter)"))
        );
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!cx.update(|window, _| window.is_fullscreen()));
        assert!(app.read_with(cx, |app, _| app.visualizer_display.expanded));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert!(!app.visualizer_display.expanded);
            assert!(!app.full_player);
        });
        cx.executor().advance_clock(Duration::from_millis(501));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert!(!app.read_with(cx, |app, _| app.visualizer_display.mounted));
        assert!(cx.debug_bounds("playback-panel-Visualizer").is_some());
        cx.update(|_, cx| app.update(cx, |app, cx| app.toggle_full_player(cx)));
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        if app.read_with(cx, |app, _| {
            app.state.settings.player_display.active_tab != "visualizer"
        }) {
            click(cx, "full-player-tab-visualizer");
        }
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        click(cx, "expand-visualizer");
        assert!(
            app.read_with(cx, |app, _| app.visualizer_display.return_to_player
                && !app.full_player)
        );
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        click(cx, "shrink-expanded-visualizer");
        assert!(app.read_with(cx, |app, _| app.full_player
            && !app.visualizer_display.expanded));
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert!(saved.settings.player_display.expanded);
        assert!(!saved.settings.player_display.visualizer_expanded);
        assert_eq!(saved.queue.order, keys);
        cx.executor().advance_clock(Duration::from_millis(501));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        click(cx, "expand-visualizer");
        cx.executor().advance_clock(Duration::from_millis(600));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.visualizer.kind = "butterchurn".into();
                cx.notify();
            })
        });
        cx.run_until_parked();
        click(cx, "compact-milkdrop-next");
        let preset = app.read_with(cx, |app, _| {
            app.state.settings.visualizer.butterchurn["currentPreset"].clone()
        });
        click(cx, "compact-milkdrop-previous");
        assert_ne!(
            preset,
            app.read_with(cx, |app, _| {
                app.state.settings.visualizer.butterchurn["currentPreset"].clone()
            })
        );
        app.read_with(cx, |app, _| {
            assert_eq!(app.state.queue.order, keys);
            assert_eq!(app.state.queue.current, Some(keys[1]));
            assert!(!app.playing);
        });
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert!(saved.settings.player_display.visualizer_expanded);
        assert!(!saved.settings.player_display.expanded);
        cx.update(|window, _| window.remove_window());
        cx.run_until_parked();
        // Return-to-player intent is transient; restarting retains only expanded mode.
        let captured = Rc::new(RefCell::new(None));
        let (_, restarted) = cx.add_window_view(|window, cx| {
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(saved), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let restored = captured.borrow_mut().take().unwrap();
        restarted.run_until_parked();
        restored.read_with(restarted, |app, _| {
            assert!(app.visualizer_display.expanded);
            assert!(!app.visualizer_display.return_to_player);
            assert_eq!(app.state.queue.order, keys);
            assert!(!app.full_player);
        });
        restarted.simulate_keystrokes("f11");
        restarted.run_until_parked();
        assert!(restarted.update(|window, _| window.is_fullscreen()));
        restarted.update(|_, cx| restored.update(cx, |app, cx| app.navigate(Route::Home, cx)));
        restarted.run_until_parked();
        restarted
            .executor()
            .advance_clock(Duration::from_millis(501));
        restarted.update(|window, _| window.refresh());
        restarted.run_until_parked();
        assert!(!restarted.update(|window, _| window.is_fullscreen()));
        restored.read_with(restarted, |app, _| {
            assert!(!app.visualizer_display.expanded);
            assert!(!app.visualizer_display.mounted);
            assert_eq!(app.route, Route::Home);
            assert_eq!(app.state.queue.order, keys);
        });
        std::fs::remove_dir_all(dir).ok();
    }
}
