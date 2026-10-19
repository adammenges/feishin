use super::*;
use crate::test_speaker::Speaker;
use ::core::prelude::v1::test;
use feishin_core::{dlna::Member, storage::State};
use gpui_component::{Root, Theme, slider::SliderValue};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Instant};

struct Probe(Entity<Feishin>);
impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().font(crate::fonts::app_font(cx)).child(
            div().absolute().left(px(200.)).top(px(400.)).child(
                self.0
                    .update(cx, |app, cx| app.player_volume_controls(window, cx)),
            ),
        )
    }
}
fn group(app: &mut Feishin, speaker: &Speaker, kind: &str) {
    app.cast_selected = Some(speaker.device("0"));
    app.cast_members = (0..3)
        .map(|i| (i.to_string(), speaker.device(&i.to_string())))
        .collect();
    app.cast_member_volumes = (0..3).map(|i| (i.to_string(), 50)).collect();
    app.cast_member_mutes.clear();
    app.cast_groups = vec![Group {
        id: "group".into(),
        coordinator: "0".into(),
        stereo_pair: kind == "pair",
        members: (0..if kind == "solo" { 1 } else { 3 })
            .map(|i| Member {
                id: i.to_string(),
                name: format!("Speaker {i}"),
                location: String::new(),
                satellite: false,
            })
            .collect(),
    }];
}
fn wait(cx: &mut VisualTestContext, condition: impl Fn(&mut VisualTestContext) -> bool) {
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        cx.run_until_parked();
        if condition(cx) {
            return;
        }
        assert!(Instant::now() < end, "Speaker response timed out");
        std::thread::sleep(Duration::from_millis(3));
    }
}
fn wheel(cx: &mut VisualTestContext, position: Point<Pixels>, down: bool, shift: bool) {
    cx.simulate_event(ScrollWheelEvent {
        position,
        delta: ScrollDelta::Lines(point(0., if down { -3. } else { 3. })),
        modifiers: Modifiers {
            shift,
            ..Default::default()
        },
        touch_phase: TouchPhase::Moved,
    });
}
#[gpui::test]
fn network_volume_callbacks_and_delayed_updates_match_original(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let speaker = Speaker::new();
    let dir = std::env::temp_dir().join(format!("feishin-speaker-volume-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let mut state = State::default();
        state.settings.scrobble = false;
        state.settings.volume_wheel_step = 7.;
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        captured.replace(Some(app.clone()));
        let probe = cx.new(|_| Probe(app));
        Root::new(probe, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1200.), px(700.)));
    let reference: Value =
        serde_json::from_str(include_str!("../assets/network-volume-reference.json")).unwrap();
    for case in reference["master"].as_array().unwrap() {
        speaker.calls.lock().unwrap().clear();
        let shift = case["shift"].as_bool().unwrap();
        cx.simulate_modifiers_change(Modifiers {
            shift,
            ..Default::default()
        });
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                group(app, &speaker, case["grouping"].as_str().unwrap());
                app.volume_change_task = None;
                app.state.settings.muted = case["muted"].as_bool().unwrap();
                app.state.settings.volume = 50.;
                app.volume
                    .update(cx, |slider, cx| slider.set_value(50., window, cx));
                cx.notify();
            })
        });
        cx.run_until_parked();
        let position = cx.debug_bounds("player-mute").unwrap().center();
        match case["action"].as_str().unwrap() {
            "mute" => cx.simulate_click(
                position,
                Modifiers {
                    shift,
                    ..Default::default()
                },
            ),
            "slider" => cx.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.volume.update(cx, |slider, cx| {
                        slider.set_value(72., window, cx);
                        cx.emit(SliderEvent::Change(SliderValue::Single(72.)));
                    })
                })
            }),
            action => wheel(cx, position, action == "wheel-down", shift),
        }
        wait(cx, |cx| {
            app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
        });
        let mut expected: Vec<Value> = case["calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call[0] == "volume" || call[0] == "mute")
            .cloned()
            .collect();
        let mut actual = speaker.calls.lock().unwrap().clone();
        expected.sort_by_key(Value::to_string);
        actual.sort_by_key(Value::to_string);
        assert_eq!(actual, expected, "{case}");
        app.read_with(cx, |app, cx| {
            let action = case["action"].as_str().unwrap();
            if action == "mute" {
                assert_eq!(app.state.settings.muted, !case["muted"].as_bool().unwrap());
            } else {
                let value = case["calls"].as_array().unwrap().last().unwrap()[1]
                    .as_f64()
                    .unwrap() as f32;
                assert_eq!(app.volume.read(cx).value().end(), value, "{case}");
                assert_eq!(app.state.settings.volume, value, "{case}");
            }
        });
    }
    for case in reference["member"].as_array().unwrap() {
        speaker.calls.lock().unwrap().clear();
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                group(app, &speaker, "group");
                app.cast_members
                    .insert("member".into(), speaker.device("member"));
                app.cast_member_volumes
                    .insert("member".into(), case["value"].as_u64().unwrap() as u8);
                app.cast_member_mutes
                    .insert("member".into(), case["muted"].as_bool().unwrap());
                let value = case["value"].as_u64().unwrap() as u8;
                match case["action"].as_str().unwrap() {
                    "mute" => app.speaker_member_mute(
                        "member".into(),
                        !case["muted"].as_bool().unwrap(),
                        cx,
                    ),
                    "slider" => app.change_member_volume("member".into(), value, true, cx),
                    "wheel-up" => app.change_member_volume(
                        "member".into(),
                        value.saturating_add(7).min(100),
                        false,
                        cx,
                    ),
                    _ => app.change_member_volume(
                        "member".into(),
                        value.saturating_sub(7),
                        false,
                        cx,
                    ),
                }
            })
        });
        wait(cx, |cx| {
            app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
        });
        assert_eq!(
            *speaker.calls.lock().unwrap(),
            case["calls"].as_array().unwrap().clone(),
            "{case}"
        );
    }
    // Slow replies must not overwrite or drop the last requested volume/mute.
    speaker.calls.lock().unwrap().clear();
    speaker.hold.store(true, Ordering::SeqCst);
    cx.update(|_, cx| app.update(cx, |app, cx| app.speaker_member_volume("1".into(), 10, cx)));
    wait(cx, |_| !speaker.calls.lock().unwrap().is_empty());
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            for value in 20..=90 {
                app.speaker_member_volume("1".into(), value, cx);
            }
            app.speaker_member_mute("1".into(), true, cx);
            app.speaker_member_mute("1".into(), false, cx);
            assert_eq!(app.cast_member_volumes["1"], 90);
        })
    });
    speaker.hold.store(false, Ordering::SeqCst);
    wait(cx, |cx| {
        app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
    });
    assert_eq!(
        *speaker.calls.lock().unwrap(),
        vec![
            json!(["volume", "1", 10]),
            json!(["volume", "1", 90]),
            json!(["mute", "1", false])
        ]
    );
    // A member removed during a request must not be restored by its reply.
    speaker.hold.store(true, Ordering::SeqCst);
    speaker.calls.lock().unwrap().clear();
    cx.update(|_, cx| app.update(cx, |app, cx| app.speaker_member_volume("1".into(), 30, cx)));
    wait(cx, |_| !speaker.calls.lock().unwrap().is_empty());
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.speaker_member_volume("1".into(), 80, cx);
            app.cast_members.remove("1");
            app.cast_member_volumes.remove("1");
        })
    });
    speaker.hold.store(false, Ordering::SeqCst);
    wait(cx, |cx| {
        app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
    });
    app.read_with(cx, |app, _| {
        assert!(!app.cast_member_volumes.contains_key("1"))
    });
    assert_eq!(speaker.calls.lock().unwrap().len(), 1);
    speaker.fail.store(true, Ordering::SeqCst);
    cx.update(|_, cx| app.update(cx, |app, cx| app.speaker_member_volume("2".into(), 30, cx)));
    wait(cx, |cx| {
        app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
    });
    app.read_with(cx, |app, _| assert!(!app.message.is_empty()));
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn network_volume_layouts_and_shift_controls_match_original(cx: &mut TestAppContext) {
    cx.use_platform_text_system();
    cx.set_assets(crate::icons::Assets);
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::fonts::register_builtins(cx).unwrap();
    });
    let speaker = Speaker::new();
    let dir = std::env::temp_dir().join(format!("feishin-speaker-layout-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        window.activate_accessibility_for_test();
        let mut state = State::default();
        state.settings.scrobble = false;
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        captured.replace(Some(app.clone()));
        let probe = cx.new(|_| Probe(app));
        Root::new(probe, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    #[cfg(target_os = "linux")]
    let reference = include_str!("../assets/network-volume-layout-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let reference = include_str!("../assets/network-volume-layout-darwin-reference.json");
    let reference: Vec<Value> = serde_json::from_str(reference).unwrap();
    for case in &reference {
        let zoom = case["zoom"].as_f64().unwrap() as f32;
        let rem = case["rem"].as_f64().unwrap() as f32;
        let shift = case["shift"].as_bool().unwrap();
        let rtl = case["rtl"].as_bool().unwrap();
        cx.simulate_resize(size(
            px(case["viewport"].as_f64().unwrap() as f32 * zoom),
            px(700.),
        ));
        cx.simulate_modifiers_change(Modifiers {
            shift,
            ..Default::default()
        });
        cx.update(|window, cx| {
            window.set_rem_size(px(rem * zoom));
            window.text_system().set_optical_size_scale(zoom);
            Theme::global_mut(cx).font_size = px(rem * zoom);
            cx.set_global(crate::appearance::Language(
                if rtl { "ar" } else { "en" }.into(),
            ));
            app.update(cx, |app, cx| {
                group(app, &speaker, case["grouping"].as_str().unwrap());
                app.state.settings.zoom = zoom;
                app.state.settings.volume_width = 70.;
                app.state.settings.language = if rtl { "ar" } else { "en" }.into();
                app.last_theme_scan = Instant::now();
                cx.notify();
            });
        });
        cx.run_until_parked();
        let root = cx.debug_bounds("network-volume").unwrap();
        cx.simulate_mouse_move(
            root.center(),
            None,
            Modifiers {
                shift,
                ..Default::default()
            },
        );
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
        let root = cx.debug_bounds("network-volume").unwrap();
        let mut comparisons = vec![
            ("network-volume".to_owned(), &case["root"]),
            (
                "network-volume-master".to_owned(),
                case["controls"].as_array().unwrap().last().unwrap(),
            ),
        ];
        if !case["panel"].is_null() {
            comparisons.push(("network-volume-panel".to_owned(), &case["panel"]));
            for (i, member) in case["members"].as_array().unwrap().iter().enumerate() {
                comparisons.push((format!("network-member-{i}"), &member["bounds"]));
                comparisons.push((format!("network-member-controls-{i}"), &case["controls"][i]));
            }
        }
        for (selector, expected) in comparisons {
            let b = cx.debug_bounds(&selector).unwrap();
            let actual = [
                f32::from(b.origin.x - root.origin.x),
                f32::from(b.origin.y - root.origin.y),
                f32::from(b.size.width),
                f32::from(b.size.height),
            ];
            for (actual, expected) in actual.into_iter().zip(expected.as_array().unwrap()) {
                assert!(
                    (actual - expected.as_f64().unwrap() as f32 * zoom).abs() < 1.1,
                    "{selector}: actual {actual}, expected {expected} zoom {zoom}, {case}"
                );
            }
        }
    }
    // Disabled members reject pointer input; Shift enables their mute and wheel controls.
    cx.simulate_resize(size(px(1200.), px(700.)));
    cx.update(|window, cx| {
        window.set_rem_size(px(16.));
        Theme::global_mut(cx).font_size = px(16.);
        cx.set_global(crate::appearance::Language("en".into()));
        app.update(cx, |app, cx| {
            group(app, &speaker, "group");
            app.state.settings.zoom = 1.;
            app.state.settings.language = "en".into();
            cx.notify();
        });
    });
    for shift in [false, true] {
        cx.simulate_modifiers_change(Modifiers {
            shift,
            ..Default::default()
        });
        cx.run_until_parked();
        let root = cx.debug_bounds("network-volume").unwrap();
        cx.simulate_mouse_move(
            root.center(),
            None,
            Modifiers {
                shift,
                ..Default::default()
            },
        );
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(250));
        cx.run_until_parked();
        let position = cx.debug_bounds("network-member-mute-0").unwrap().center();
        cx.simulate_mouse_move(
            position,
            None,
            Modifiers {
                shift,
                ..Default::default()
            },
        );
        cx.run_until_parked();
        cx.simulate_click(
            position,
            Modifiers {
                shift,
                ..Default::default()
            },
        );
        wait(cx, |cx| {
            app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
        });
        assert_eq!(speaker.calls.lock().unwrap().len(), usize::from(shift));
        if shift {
            wheel(cx, position, false, true);
            wait(cx, |cx| {
                app.read_with(cx, |app, _| app.cast_volume_pending.is_empty())
            });
            assert_eq!(
                *speaker.calls.lock().unwrap(),
                vec![
                    json!(["mute", "1", true]),
                    json!(["volume", "1", 55]),
                    json!(["mute", "1", false])
                ]
            );
        }
    }
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn network_output_selector_opens_without_navigation_and_tracks_discovery(cx: &mut TestAppContext) {
    struct Outputs(Entity<Feishin>);
    impl Render for Outputs {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            self.0.update(cx, |app, cx| app.output_button(window, cx))
        }
    }
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let dir =
        std::env::temp_dir().join(format!("feishin-output-selector-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        window.activate_accessibility_for_test();
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
        app.update(cx, |app, _| app.cast_scanning = true);
        captured.replace(Some(app.clone()));
        let view = cx.new(|_| Outputs(app));
        Root::new(view, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1200.), px(700.)));
    cx.run_until_parked();
    let route = app.read_with(cx, |app, _| app.route.clone());
    let position = cx.debug_bounds("player-output").unwrap().center();
    cx.simulate_click(position, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("player-output-panel").is_some());
    app.read_with(cx, |app, _| assert_eq!(app.route, route));
    let speaker = Speaker::new();
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            let mut device = speaker.device("Kitchen");
            device.sonos = false;
            app.cast_devices = vec![device];
            app.cast_scanning = false;
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.update(|window, _| {
        let tree = window.accessibility_tree_for_test().unwrap();
        assert!(
            tree.nodes
                .iter()
                .any(|(_, node)| node.label() == Some("Kitchen")
                    || node.label() == Some("Speaker Kitchen")
                    || node.value() == Some("Speaker Kitchen"))
        );
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, _| {
        let tree = window.accessibility_tree_for_test().unwrap();
        assert!(
            !tree
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Dialog
                    && node.label() == Some("Cast to DLNA device"))
        );
    });
    // A completed disconnect closes the same controlled popup and retains this route.
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.cast_selected = Some(speaker.device("Kitchen"));
            app.cast_scanning = true;
            cx.notify();
        });
    });
    cx.simulate_click(position, Modifiers::none());
    cx.run_until_parked();
    app.read_with(cx, |app, _| assert!(app.cast_output_open));
    let disconnect = cx.debug_bounds("speaker-disconnect").unwrap().center();
    cx.simulate_click(disconnect, Modifiers::none());
    wait(cx, |cx| app.read_with(cx, |app, _| !app.cast_output_open));
    cx.update(|window, cx| {
        assert!(
            !window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Dialog
                    && node.label() == Some("Cast to DLNA device"))
        );
        app.read_with(cx, |app, _| assert_eq!(app.route, route));
    });
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn network_volume_motion_matches_original_and_hidden_members_ignore_pointer(
    cx: &mut TestAppContext,
) {
    use std::cell::Cell;
    struct MotionProbe {
        volume: Entity<SpeakerVolume>,
        clicks: Rc<Cell<usize>>,
    }
    impl Render for MotionProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div()
                .size_full()
                .child(
                    div()
                        .id("behind-speaker-panel")
                        .absolute()
                        .left(px(180.))
                        .top(px(150.))
                        .w(px(180.))
                        .h(px(245.))
                        .on_click(move |_, _, _| clicks.set(clicks.get() + 1)),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(200.))
                        .top(px(400.))
                        .child(self.volume.clone()),
                )
        }
    }
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let speaker = Speaker::new();
    let dir = std::env::temp_dir().join(format!("feishin-speaker-motion-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let clicks = Rc::new(Cell::new(0));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
        app.update(cx, |app, _| group(app, &speaker, "group"));
        let volume = cx.new(|cx| SpeakerVolume::new(app.downgrade(), cx));
        captured.replace(Some((app, volume.clone())));
        let probe = cx.new(|_| MotionProbe {
            volume,
            clicks: clicks.clone(),
        });
        Root::new(probe, window, cx)
    });
    let (_app, volume) = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1200.), px(700.)));
    #[cfg(target_os = "linux")]
    let reference = include_str!("../assets/network-volume-motion-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let reference = include_str!("../assets/network-volume-motion-darwin-reference.json");
    let cases: Vec<Value> = serde_json::from_str(reference).unwrap();
    for frames in cases.chunks(6) {
        let rem = frames[0]["rem"].as_f64().unwrap() as f32;
        let panel = frames[0]["name"] == "panel";
        let reverse = frames[0]["reverse"] == true;
        cx.simulate_mouse_move(point(px(5.), px(5.)), None, Modifiers::none());
        cx.simulate_modifiers_change(Modifiers {
            shift: !panel && reverse,
            ..Default::default()
        });
        cx.update(|window, cx| {
            window.blur();
            window.set_rem_size(px(rem));
            Theme::global_mut(cx).font_size = px(rem);
            volume.update(cx, |view, cx| {
                view.hovered = !panel || reverse;
                view.panel_hovered = false;
                view.visibility = CssTransition::new(
                    f32::from(view.hovered),
                    Duration::from_millis(180),
                    [0.25, 0.1, 0.25, 1.],
                );
                view.member_opacity = CssTransition::new(
                    if !panel && reverse { 1. } else { 0.4 },
                    Duration::from_millis(200),
                    [0.25, 0.1, 0.25, 1.],
                );
                cx.notify();
            });
        });
        cx.run_until_parked();
        if panel {
            cx.update(|_, cx| {
                volume.update(cx, |view, cx| {
                    view.hovered = !reverse;
                    cx.notify();
                })
            });
        } else {
            cx.simulate_modifiers_change(Modifiers {
                shift: !reverse,
                ..Default::default()
            });
        }
        cx.run_until_parked();
        let mut elapsed = 0;
        for frame in frames {
            let time = frame["elapsed"].as_u64().unwrap();
            cx.executor()
                .advance_clock(Duration::from_millis(time - elapsed));
            elapsed = time;
            cx.refresh().unwrap();
            cx.run_until_parked();
            let root = cx.debug_bounds("network-volume").unwrap();
            let bounds = cx.debug_bounds("network-volume-panel").unwrap();
            let expected = frame["opacity"].as_f64().unwrap() as f32;
            let expected_y = frame["translateY"].as_f64().unwrap() as f32;
            cx.update(|window, cx| {
                let now = cx.background_executor().now();
                volume.read_with(cx, |view, _| {
                    let actual = if panel {
                        view.visibility.value(now).0
                    } else {
                        view.member_opacity.value(now).0
                    };
                    assert!(
                        (actual - expected).abs() < 0.001,
                        "motion {frame}: {actual}"
                    );
                });
                if panel {
                    let opacity = window
                        .painted_quad_opacities_for_test()
                        .into_iter()
                        .find(|(painted, _)| {
                            (painted.origin - bounds.origin).magnitude() < 1.
                                && (painted.size.width - bounds.size.width).abs() < px(1.)
                                && (painted.size.height - bounds.size.height).abs() < px(1.)
                        })
                        .map_or(0., |(_, color)| color.a);
                    assert!(
                        (opacity - expected).abs() < 0.001,
                        "paint {frame}: {opacity}"
                    );
                    let actual_y =
                        f32::from(bounds.bottom_right().y - root.bottom_right().y) - rem * 0.5;
                    assert!(
                        (actual_y - expected_y).abs() < 1.,
                        "position {frame}: {actual_y}"
                    );
                }
            });
        }
    }
    // A fading panel still paints, but even its enabled child controls must let clicks through.
    cx.simulate_modifiers_change(Modifiers {
        shift: true,
        ..Default::default()
    });
    cx.update(|_, cx| {
        volume.update(cx, |view, cx| {
            view.hovered = true;
            view.visibility =
                CssTransition::new(1., Duration::from_millis(180), [0.25, 0.1, 0.25, 1.]);
            cx.notify();
        })
    });
    cx.run_until_parked();
    let member = cx.debug_bounds("network-member-mute-0").unwrap().center();
    cx.update(|_, cx| {
        volume.update(cx, |view, cx| {
            view.hovered = false;
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(45));
    cx.refresh().unwrap();
    cx.run_until_parked();
    cx.simulate_click(
        member,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    cx.run_until_parked();
    assert_eq!(clicks.get(), 1);
    assert!(speaker.calls.lock().unwrap().is_empty());
    std::fs::remove_dir_all(dir).ok();
}
