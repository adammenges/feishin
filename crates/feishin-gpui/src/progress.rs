use std::time::{Duration, Instant};

#[cfg(test)]
pub(crate) fn layout_reference() -> serde_json::Value {
    serde_json::from_str(if cfg!(target_os = "linux") {
        include_str!("../assets/playerbar-slider-layout-linux-reference.json")
    } else {
        include_str!("../assets/playerbar-slider-layout-darwin-reference.json")
    })
    .unwrap()
}

#[derive(Default)]
pub(crate) struct SeekPreview(Option<(f64, Option<Instant>)>);
impl SeekPreview {
    pub(crate) fn change(&mut self, target: f64) {
        self.0 = Some((target, None));
    }
    pub(crate) fn release(&mut self, target: f64, current: f64, now: Instant) {
        self.0 = ((current - target).abs() >= 0.5).then_some((target, Some(now)));
    }
    pub(crate) fn observe(&mut self, current: f64, now: Instant) {
        if let Some((target, Some(released))) = self.0
            && ((current - target).abs() < 0.5
                || now.duration_since(released) > Duration::from_secs(1))
        {
            self.0 = None;
        }
    }
    pub(crate) fn value(&self, current: f64) -> f64 {
        self.0.map_or(current, |(target, _)| target)
    }
    pub(crate) fn clear(&mut self) {
        self.0 = None;
    }
}

impl crate::app::Feishin {
    pub(crate) fn seek_row(
        &self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> gpui::AnyElement {
        use gpui::{prelude::*, *};
        use gpui_component::{ActiveTheme, StyledExt};
        let radio = self.radio_item.is_some();
        let rtl = crate::appearance::rtl(cx);
        let times = f32::from(window.viewport_size().width) / self.state.settings.zoom >= 768.;
        div()
            .id("player-seek-row")
            .debug_selector(|| "player-seek-row".into())
            .h_flex()
            .w(relative(0.95))
            .h(px(20. * self.state.settings.zoom))
            .min_w_0()
            .flex_shrink_0()
            .mx_auto()
            .gap(rems(0.5))
            .when(rtl, |row| row.flex_row_reverse())
            .text_size(rems(11. / 16.))
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .when(times, |row| {
                row.child(
                    div()
                        .h_flex()
                        .w(rems(4.75))
                        .min_w_0()
                        .flex_shrink_0()
                        .justify_end()
                        .when(rtl, |cell| cell.justify_start())
                        .child(self.scrobble_status(cx)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .h_flex()
                    .items_center()
                    .child(
                        if !radio
                            && self.state.settings.playerbar.kind == "waveform"
                            && self.waveform_data.is_some()
                        {
                            self.waveform_view(cx)
                        } else {
                            crate::styles::slider(&self.progress, cx)
                                .playerbar(self.state.settings.zoom, "player-seek")
                                .aria_label(self.tr("Progress"))
                                .value_label(feishin_core::model::duration_label(f64::from(
                                    self.progress.read(cx).value().end(),
                                )))
                                .disabled(
                                    radio
                                        || self
                                            .current_entry()
                                            .is_none_or(|entry| entry.item.duration <= 0.),
                                )
                                .into_any_element()
                        },
                    ),
            )
            .when(times, |row| {
                row.child(
                    div()
                        .h_flex()
                        .w(rems(4.75))
                        .min_w_0()
                        .flex_shrink_0()
                        .when(rtl, |cell| cell.justify_end())
                        .child(self.duration_control(cx)),
                )
            })
            .into_any_element()
    }

    pub(crate) fn sync_progress(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let entry = self.current_entry();
        let key = entry.map(|entry| entry.key);
        let duration = entry.map_or(0., |entry| entry.item.duration).max(0.) as f32;
        let changed = self.progress_entry != key;
        if changed {
            self.progress_entry = key;
            self.seek_preview.clear();
        }
        let position = self.seek_preview.value(self.current_position()) as f32;
        self.progress.update(cx, |slider, cx| {
            if changed {
                slider.cancel_pointer_change();
            }
            slider.set_range(0., duration, cx);
            if slider.value().end() != position {
                slider.set_value(position, window, cx);
            }
        });
    }

    pub(crate) fn duration_control(&self, cx: &mut gpui::Context<Self>) -> gpui::AnyElement {
        use gpui::{prelude::*, *};
        let radio = self.radio_item.is_some();
        let duration = self.current_entry().map_or(0., |entry| entry.item.duration);
        let seconds = if self.state.settings.show_time_remaining {
            self.current_position() - duration
        } else {
            duration
        };
        let label = if radio {
            "LIVE".into()
        } else {
            signed_duration(seconds)
        };
        div()
            .id("player-duration")
            .debug_selector(|| "player-duration".into())
            .role(Role::Button)
            .tab_stop(!radio)
            .aria_disabled(radio)
            .aria_label(label.clone())
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                if !radio {
                    this.state.settings.show_time_remaining =
                        !this.state.settings.show_time_remaining;
                    this.save();
                    cx.notify();
                }
            }))
            .into_any_element()
    }
}

fn signed_duration(seconds: f64) -> String {
    format!(
        "{}{}",
        if seconds <= -1. { "-" } else { "" },
        feishin_core::model::duration_label(seconds.abs())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duration_labels_match_original_total_remaining_and_subsecond_formatting() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/playerbar-time-reference.json")).unwrap();
        for case in cases {
            let duration = case["duration"].as_f64().unwrap();
            let remaining = case["remaining"].as_bool().unwrap();
            let value = if remaining {
                case["current"].as_f64().unwrap() - duration
            } else {
                duration
            };
            assert_eq!(
                signed_duration(value),
                case["label"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(!remaining, case["next"].as_bool().unwrap());
        }
    }
    #[test]
    fn seek_previews_match_original_hold_release_catch_up_and_timeout() {
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/seek-slider-reference.json")).unwrap();
        for case in cases {
            let mut preview = SeekPreview::default();
            let start = Instant::now();
            let mut seeks = Vec::new();
            for frame in case["frames"].as_array().unwrap() {
                let value = frame["value"].as_f64().unwrap();
                let current = frame["currentTime"].as_f64().unwrap();
                let now = start + Duration::from_millis(frame["clock"].as_u64().unwrap());
                match frame["event"].as_str().unwrap() {
                    "change" => preview.change(value),
                    "release" => {
                        preview.release(value, current, now);
                        seeks.push(value);
                    }
                    "poll" => preview.observe(current, now),
                    _ => unreachable!(),
                }
                assert_eq!(
                    preview.value(current),
                    frame["displayed"].as_f64().unwrap(),
                    "{}: {frame}",
                    case["name"]
                );
                assert_eq!(
                    seeks,
                    frame["seeks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_f64().unwrap())
                        .collect::<Vec<_>>()
                );
            }
            preview.change(80.);
            preview.clear();
            assert_eq!(preview.value(0.), 0.);
        }
    }
}

#[gpui::test]
fn regular_seek_drag_survives_polls_commits_on_release_and_preserves_duration_choice(
    cx: &mut gpui::TestAppContext,
) {
    use crate::app::Feishin;
    use feishin_core::{
        model::{Item, ItemKind},
        playback::Event,
        queue::Enqueue,
        storage::State,
    };
    use gpui::{prelude::*, *};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};
    cx.update(gpui_component::init);
    let dir = std::env::temp_dir().join(format!("feishin-seek-ui-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (sender, receiver) = std::sync::mpsc::channel();
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        window.activate_accessibility_for_test();
        let mut state = State::default();
        state.settings.scrobble = false;
        state.settings.confirm_large_fetch = false;
        state.settings.playerbar_open_drawer = true;
        state.queue.add(
            vec![
                Item {
                    id: "song".into(),
                    kind: ItemKind::Song,
                    duration: 100.,
                    ..Default::default()
                };
                2
            ],
            Enqueue::Now,
        );
        let app = cx.new(|cx| {
            let mut view = Feishin::new(dir.clone(), Ok(state), window, cx);
            view.player.events = receiver;
            view
        });
        captured.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.run_until_parked();
    for language in ["en", "ar"] {
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.settings.language = language.into();
                view.state.queue.current = Some(view.state.queue.entries[0].key);
                view.state.queue.position = 10.;
                view.seek_preview.clear();
                view.apply_theme(window, cx);
                cx.notify();
            })
        });
        cx.run_until_parked();
        let slider_id = cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let (id, _) = tree
                .nodes
                .iter()
                .find(|(_, node)| {
                    node.role() == Role::Slider
                        && node.max_numeric_value() == Some(100.)
                        && node.numeric_value() == Some(10.)
                })
                .unwrap();
            *id
        });
        let bounds = cx.debug_bounds("player-seek-track").unwrap();
        let target = point(
            bounds.left() + bounds.size.width * if language == "ar" { 0.25 } else { 0.75 },
            bounds.center().y,
        );
        cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, cx| {
            assert_eq!(view.state.queue.position, 10.);
            assert_eq!(view.progress.read(cx).value().end(), 75.);
        });
        let generation = app.read_with(cx, |view, _| view.player_generation);
        for position in [74.9, 30.] {
            sender.send(Event::Position(generation, position)).unwrap();
            cx.update(|window, cx| app.update(cx, |view, cx| view.tick(window, cx)));
            cx.run_until_parked();
            app.read_with(cx, |view, cx| {
                assert_eq!(view.state.queue.position, position);
                assert_eq!(
                    view.progress.read(cx).value().end(),
                    75.,
                    "Engine polls cannot cancel a held edit"
                );
            });
        }
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.position, 75.);
            assert!(!view.full_player, "Seeking must not open the player drawer");
        });
        sender.send(Event::Position(generation, 40.)).unwrap();
        cx.update(|window, cx| app.update(cx, |view, cx| view.tick(window, cx)));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, cx| view.progress.read(cx).value().end()),
            75.
        );
        sender.send(Event::Position(generation, 75.1)).unwrap();
        cx.update(|window, cx| app.update(cx, |view, cx| view.tick(window, cx)));
        cx.run_until_parked();
        assert!(
            (app.read_with(cx, |view, cx| view.progress.read(cx).value().end()) - 75.1).abs()
                < 0.001
        );
        cx.update(|window, cx| {
            window.dispatch_accessibility_action_for_test(
                accesskit::ActionRequest {
                    action: accesskit::Action::Focus,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: slider_id,
                    data: None,
                },
                cx,
            )
        });
        cx.simulate_keystrokes(if language == "ar" { "left" } else { "right" });
        cx.run_until_parked();
        assert_eq!(app.read_with(cx, |view, _| view.state.queue.position), 76.);
        let duration = cx.debug_bounds("player-duration").unwrap();
        cx.simulate_click(duration.center(), Modifiers::none());
        cx.run_until_parked();
        let saved = feishin_core::storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(saved.settings.show_time_remaining, language == "en");
        assert!(!app.read_with(cx, |view, _| view.full_player));
        cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.state.queue.current = Some(view.state.queue.entries[1].key);
                view.state.queue.entries[1].item.duration = 300.;
                view.state.queue.position = 0.;
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, cx| {
            assert_eq!(view.state.queue.position, 0.);
            assert_eq!(view.progress.read(cx).value().end(), 0.);
        });
        cx.update(|window, _| {
            assert!(
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.role() == Role::Slider
                        && node.max_numeric_value() == Some(300.))
            )
        });
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[gpui::test]
fn playerbar_layout_and_emphasis_match_original_responsive_slider(cx: &mut gpui::TestAppContext) {
    use crate::app::Feishin;
    use feishin_core::{
        model::{Item, ItemKind},
        queue::Enqueue,
        storage::State,
    };
    use gpui::{prelude::*, *};
    use gpui_component::{Root, Theme};
    use std::{cell::RefCell, rc::Rc};
    struct Probe {
        app: Entity<Feishin>,
        zoom: f32,
    }
    impl Render for Probe {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .font(crate::fonts::app_font(cx))
                .w(px(600. * self.zoom))
                .pt(px(80. * self.zoom))
                .child(self.app.update(cx, |app, cx| app.seek_row(window, cx)))
        }
    }
    cx.use_platform_text_system();
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::fonts::register_builtins(cx).unwrap();
    });
    let dir = std::env::temp_dir().join(format!("feishin-seek-layout-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        window.activate_accessibility_for_test();
        let mut state = State::default();
        state.settings.scrobble = false;
        state.queue.add(
            vec![Item {
                id: "song".into(),
                kind: ItemKind::Song,
                duration: 100.,
                ..Default::default()
            }],
            Enqueue::Now,
        );
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        let probe = cx.new(|_| Probe { app, zoom: 1. });
        captured.replace(Some(probe.clone()));
        Root::new(probe, window, cx)
    });
    let probe = captured.borrow_mut().take().unwrap();
    let reference = layout_reference();
    for case in reference["cases"].as_array().unwrap() {
        let zoom = case["zoom"].as_f64().unwrap() as f32;
        let rem = case["rem"].as_f64().unwrap() as f32;
        let rtl = case["rtl"].as_bool().unwrap();
        let value = case["value"].as_f64().unwrap() as f32;
        cx.simulate_mouse_move(point(px(1100.), px(500.)), None, Modifiers::none());
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
            Theme::global_mut(cx).primary = rgb(0x00ff00).into();
            probe.update(cx, |probe, cx| {
                probe.zoom = zoom;
                probe.app.update(cx, |app, cx| {
                    app.state.settings.zoom = zoom;
                    app.state.queue.position = value as f64;
                    app.progress.update(cx, |slider, cx| {
                        slider.set_range(0., 100., cx);
                        slider.set_value(value, window, cx);
                    });
                });
                cx.notify();
            });
        });
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
        let origin = cx.debug_bounds("player-seek-row").unwrap().origin;
        for (key, selector) in [
            ("row", "player-seek-row"),
            ("slider", "player-seek-slider"),
            ("track", "player-seek-track"),
            ("bar", "player-seek-fill"),
            ("thumb", "player-seek-thumb"),
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            let actual = [
                f32::from(bounds.left() - origin.x),
                f32::from(bounds.top() - origin.y),
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
            ];
            for (actual, expected) in actual
                .into_iter()
                .zip(case["elements"][key]["bounds"].as_array().unwrap())
            {
                let expected = expected.as_f64().unwrap() as f32 * zoom;
                assert!(
                    (actual - expected).abs() <= 1.1,
                    "{key} {actual} != {expected}: {case}"
                );
            }
        }
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let visible = tree
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button && node.label() == Some("1:40"));
            assert_eq!(
                visible,
                case["viewport"].as_u64().unwrap() >= 768,
                "Time labels at the viewport breakpoint"
            );
        });
    }
    // The last reference uses RTL and zoom. Paint must interpolate actual colors,
    // and showing the label must not move the track or surrounding layout.
    let track = cx.debug_bounds("player-seek-track").unwrap();
    let thumb = cx.debug_bounds("player-seek-thumb").unwrap();
    cx.simulate_mouse_move(track.center(), None, Modifiers::none());
    cx.run_until_parked();
    let mut previous = 0;
    for frame in reference["motion"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["name"] == "thumb")
    {
        let ms = frame["elapsed"].as_u64().unwrap();
        cx.executor()
            .advance_clock(Duration::from_millis(ms - previous));
        previous = ms;
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(cx.debug_bounds("player-seek-track").unwrap(), track);
        let expected: f32 = frame["value"].as_str().unwrap().parse().unwrap();
        cx.update(|window, _| {
            let painted = window.painted_quad_opacities_for_test();
            let actual = painted
                .iter()
                .filter(|(bounds, _)| {
                    f32::from(bounds.center().x - thumb.center().x).abs() < 1.
                        && f32::from(bounds.size.width - thumb.size.width).abs() < 1.
                        && f32::from(bounds.center().y - thumb.center().y).abs() < 1.
                })
                .map(|(_, color)| color.a)
                .fold(0., f32::max);
            assert!(
                (actual - expected).abs() < 0.015,
                "Opacity at {ms}ms: {actual} != {expected}"
            );
            let bar = reference["motion"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["name"] == "bar" && f["elapsed"] == ms)
                .unwrap();
            let expected: Vec<f32> = bar["value"]
                .as_str()
                .unwrap()
                .split(|c: char| !c.is_ascii_digit())
                .filter(|v| !v.is_empty())
                .map(|v| v.parse().unwrap())
                .collect();
            let (_, color) = painted
                .iter()
                .rev()
                .find(|(bounds, _)| {
                    f32::from(bounds.size.width - track.size.width - window.rem_size() * 0.75).abs()
                        < 1.1
                        && f32::from(bounds.size.height - track.size.height).abs() < 1.1
                })
                .unwrap();
            let color: Rgba = (*color).into();
            for (actual, expected) in [color.r, color.g, color.b].into_iter().zip(expected) {
                assert!(
                    (actual * 255. - expected).abs() < 1.1,
                    "Bar color at {ms}ms: {actual} != {expected}"
                );
            }
        });
    }
    let label = cx.debug_bounds("player-seek-label").unwrap();
    let origin = cx.debug_bounds("player-seek-row").unwrap().origin;
    let case = reference["cases"].as_array().unwrap().last().unwrap();
    for (actual, expected) in [
        f32::from(label.left() - origin.x),
        f32::from(label.top() - origin.y),
        f32::from(label.size.width),
        f32::from(label.size.height),
    ]
    .into_iter()
    .zip(case["elements"]["label"]["bounds"].as_array().unwrap())
    {
        let expected = expected.as_f64().unwrap() as f32 * 1.25;
        assert!(
            (actual - expected).abs() < 1.1,
            "Label {actual} != {expected}"
        );
    }
    let _ = std::fs::remove_dir_all(dir);
}
