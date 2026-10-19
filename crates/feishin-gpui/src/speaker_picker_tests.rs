use super::*;
use crate::test_speaker::Speaker;
use ::core::prelude::v1::test;
use feishin_core::storage::State;
use gpui_component::{Root, Theme};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

struct Probe {
    app: Entity<Feishin>,
    _subscription: Subscription,
}
impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .h_flex()
            .items_start()
            .font(crate::fonts::app_font(cx))
            .child(
                self.app
                    .update(cx, |app, cx| app.speaker_picker_view(window, cx)),
            )
    }
}
fn member(device: &Device) -> Member {
    Member {
        id: device.id.clone(),
        name: device.name.clone(),
        location: device.location.clone(),
        satellite: false,
    }
}
fn fixture<'a>(
    cx: &'a mut TestAppContext,
    name: &str,
) -> (
    Entity<Feishin>,
    &'a mut VisualTestContext,
    std::path::PathBuf,
) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let dir = std::env::temp_dir().join(format!("feishin-{name}-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let mut state = State::default();
        state.settings.scrobble = false;
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        captured.replace(Some(app.clone()));
        let probe = cx.new(|cx| Probe {
            _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
            app,
        });
        Root::new(probe, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    (app, cx, dir)
}
#[gpui::test]
fn network_picker_matches_original_content_layout(cx: &mut TestAppContext) {
    cx.use_platform_text_system();
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    let (app, cx, dir) = fixture(cx, "picker-layout");
    cx.simulate_scale_factor(if cfg!(target_os = "macos") { 2. } else { 1. });
    let speaker = Speaker::new();
    #[cfg(target_os = "linux")]
    let reference = include_str!("../assets/network-picker-layout-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let reference = include_str!("../assets/network-picker-layout-darwin-reference.json");
    let cases: Vec<Value> = serde_json::from_str(reference).unwrap();
    for case in cases {
        let zoom = case["zoom"].as_f64().unwrap() as f32;
        let rem = case["rem"].as_f64().unwrap() as f32;
        let screen = case["screen"].as_str().unwrap();
        cx.simulate_resize(size(px(1200. * zoom), px(800. * zoom)));
        cx.simulate_modifiers_change(Modifiers {
            shift: case["shift"] == true,
            ..Default::default()
        });
        cx.update(|window, cx| {
            window.set_rem_size(px(rem * zoom));
            window.text_system().set_optical_size_scale(zoom);
            Theme::global_mut(cx).font_size = px(rem * zoom);
            cx.set_global(crate::appearance::Language(
                if case["rtl"] == true { "ar" } else { "en" }.into(),
            ));
            app.update(cx, |app, cx| {
                app.state.settings.zoom = zoom;
                app.last_theme_scan = std::time::Instant::now();
                app.cast_devices = ["Living room", "Kitchen", "Bedroom"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let mut device = speaker.device(["a", "b", "c"][i]);
                        device.name = name.into();
                        device.sonos = i != 0 || case["sonos"] == true;
                        device
                    })
                    .collect();
                let group = Group {
                    id: "group".into(),
                    coordinator: app.cast_devices[0].id.clone(),
                    stereo_pair: false,
                    members: app.cast_devices[..2].iter().map(member).collect(),
                };
                app.cast_members = app
                    .cast_devices
                    .iter()
                    .map(|d| (d.id.clone(), d.clone()))
                    .collect();
                app.cast_selected = matches!(screen, "connected" | "group" | "expand-group")
                    .then(|| app.cast_devices[0].clone());
                app.cast_connecting = (screen == "connecting").then(|| (1, "uuid:a".into()));
                app.cast_groups = if matches!(screen, "group" | "expand-group") {
                    vec![group.clone()]
                } else {
                    vec![]
                };
                app.cast_discovery_groups = if case["grouped"] == true {
                    vec![group]
                } else {
                    vec![]
                };
                app.cast_builder = match screen {
                    "group-build" => Some(GroupSelection::default()),
                    "expand-group" => Some(GroupSelection::new(app.cast_selected.clone())),
                    _ => None,
                };
                app.cast_scanning = case["loading"] == true;
                if case["empty"] == true {
                    app.cast_devices.clear();
                }
                cx.notify();
            });
        });
        cx.run_until_parked();
        let root = cx.debug_bounds("player-output-panel").unwrap();
        let mut check = |selector: &str, bounds: &Value| {
            let actual = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}: {case}"));
            let values = [
                f32::from(actual.origin.x - root.origin.x),
                f32::from(actual.origin.y - root.origin.y),
                f32::from(actual.size.width),
                f32::from(actual.size.height),
            ];
            for (i, value) in values.into_iter().enumerate() {
                let expected = bounds[i].as_f64().unwrap() as f32 * zoom;
                // Nested origins accumulate native physical-pixel rounding.
                let tolerance = if i < 2 { 2.0 } else { 1.1 };
                assert!(
                    (value - expected).abs() <= tolerance,
                    "{selector} axis {i}: {value} != {expected}; {case}"
                );
            }
        };
        check("player-output-panel", &case["bounds"]);
        check("speaker-picker-paper", &case["paper"]);
        for text in case["text"].as_array().unwrap() {
            let label = text["label"].as_str().unwrap();
            let label = if label == "Living room + Kitchen" {
                "Group (2)"
            } else {
                label
            };
            check(&format!("picker-text-{label}"), &text["bounds"]);
        }
        let mut properties = 0;
        for button in case["buttons"].as_array().unwrap() {
            let label = button["label"].as_str().unwrap();
            let selector = match label {
                "Refresh" => "picker-refresh",
                "Create Group" => "picker-create",
                "Cancel" => "picker-cancel",
                "Connect" | "Add" => "picker-confirm",
                "Add to Group" | "Add Speaker" => "picker-expand",
                "Disconnect" => "speaker-disconnect",
                "Remove" => "picker-remove-uuid:b",
                "Living room" => "picker-connect-uuid:a",
                "Kitchen" => "picker-connect-uuid:b",
                "Bedroom" => "picker-connect-uuid:c",
                "" => {
                    properties += 1;
                    if properties == 1 {
                        "speaker-properties-uuid:a"
                    } else {
                        "speaker-properties-uuid:b"
                    }
                }
                _ => panic!("Unexpected label {label}"),
            };
            check(selector, &button["bounds"]);
        }
    }
    std::fs::remove_dir_all(dir).ok();
}

#[track_caller]
fn wait(cx: &mut VisualTestContext, app: &Entity<Feishin>, condition: impl Fn(&Feishin) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(6);
    loop {
        cx.run_until_parked();
        if app.read_with(cx, |app, _| condition(app)) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "speaker request timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
}
#[gpui::test]
fn network_picker_group_creation_partial_failure_removal_and_cancel(cx: &mut TestAppContext) {
    let (app, cx, dir) = fixture(cx, "picker-group");
    let speaker = Speaker::new();
    let failure = Speaker::new();
    failure.fail.store(true, Ordering::SeqCst);
    speaker.snapshot.lock().unwrap().transport = "STOPPED".into();
    let devices = vec![
        speaker.device("a"),
        speaker.device("b"),
        failure.device("c"),
    ];
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.cast_devices = devices.clone();
            app.cast_scanning = false;
            app.cast_builder = Some(GroupSelection::default());
            cx.notify();
        })
    });
    cx.run_until_parked();
    for id in ["b", "a", "c"] {
        let bounds = cx.debug_bounds(&format!("picker-check-uuid:{id}")).unwrap();
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    }
    app.read_with(cx, |app, _| {
        assert_eq!(
            app.cast_builder.as_ref().unwrap().coordinator().unwrap().id,
            "uuid:b"
        )
    });
    let position = cx.debug_bounds("picker-confirm").unwrap().center();
    cx.simulate_click(position, Modifiers::none());
    wait(cx, &app, |app| {
        app.cast_connecting.is_none() && app.cast_group_pending.is_none()
    });
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_selected.as_ref().unwrap().id, "uuid:b");
        let group = app.speaker_volume_group().unwrap();
        assert_eq!(
            group
                .members
                .iter()
                .map(|member| member.id.as_str())
                .collect::<Vec<_>>(),
            ["uuid:b", "uuid:a"]
        );
        assert!(app.message.contains("Speaker c"));
        assert!(app.cast_builder.is_none());
    });
    assert!(
        speaker
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call[0] == "SetAVTransportURI" && call[1] == "a")
    );
    let position = cx.debug_bounds("picker-remove-uuid:a").unwrap().center();
    cx.simulate_click(position, Modifiers::none());
    wait(cx, &app, |app| {
        app.cast_group_pending.is_none() && app.cast_property_loading.is_empty()
    });
    app.read_with(cx, |app, _| {
        assert!(app.speaker_volume_group().is_none());
        assert!(!app.cast_members.contains_key("uuid:a"));
    });
    // A held first join cannot enqueue later joins after the user switches outputs.
    speaker.hold.store(true, Ordering::SeqCst);
    speaker.calls.lock().unwrap().clear();
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.add_speaker_group(
                devices[1].clone(),
                vec![devices[0].clone(), speaker.device("d")],
                cx,
            )
        })
    });
    wait(cx, &app, |_| !speaker.calls.lock().unwrap().is_empty());
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.select_speaker(Some(failure.device("other")), cx)
        })
    });
    speaker.hold.store(false, Ordering::SeqCst);
    wait(cx, &app, |app| app.cast_group_pending.is_none());
    std::thread::sleep(std::time::Duration::from_millis(50));
    cx.run_until_parked();
    assert!(
        !speaker
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call[0] == "SetAVTransportURI" && call[1] == "d")
    );
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_selected.as_ref().unwrap().id, "uuid:other")
    });
    std::fs::remove_dir_all(dir).ok();
}
#[gpui::test]
fn network_speaker_properties_coalesce_and_reject_stale_reads(cx: &mut TestAppContext) {
    use feishin_core::dlna::Property;
    let (app, cx, dir) = fixture(cx, "speaker-properties");
    let speaker = Speaker::new();
    let device = speaker.device("a");
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.cast_devices = vec![device.clone()];
            app.load_speaker_properties(device.clone(), cx);
        })
    });
    wait(cx, &app, |app| app.cast_property_loading.is_empty());
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_properties[&device.id].bass, Some(0))
    });
    speaker.calls.lock().unwrap().clear();
    speaker.hold.store(true, Ordering::SeqCst);
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.load_speaker_properties(device.clone(), cx)
        })
    });
    wait(cx, &app, |_| !speaker.calls.lock().unwrap().is_empty());
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.speaker_property(device.clone(), Property::Bass(1), cx);
            for value in 2..=10 {
                app.speaker_property(device.clone(), Property::Bass(value), cx);
            }
            app.speaker_property(device.clone(), Property::Treble(-3), cx);
            // A refresh started during queued writes must not replace their optimistic values.
            app.load_speaker_properties(device.clone(), cx);
            assert!(!app.cast_property_loading.contains(&device.id));
        })
    });
    speaker.hold.store(false, Ordering::SeqCst);
    wait(cx, &app, |app| app.cast_property_pending.is_empty());
    wait(cx, &app, |_| {
        speaker
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call[0] == "GetZoneGroupState")
    });
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_properties[&device.id].bass, Some(10));
        assert_eq!(app.cast_properties[&device.id].treble, Some(-3));
    });
    assert_eq!(speaker.properties.lock().unwrap().bass, Some(10));
    let bass = speaker
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|call| call[0] == "bass")
        .map(|call| call[2].clone())
        .collect::<Vec<_>>();
    assert_eq!(bass, vec![serde_json::json!(1), serde_json::json!(10)]);
    assert_eq!(
        speaker
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call[0] == "GetBass")
            .count(),
        1
    );

    // The same UDN can be rediscovered at a new endpoint while an old write is pending.
    // Use a fresh transport so the earlier stale-read requests cannot hold this gate.
    let speaker = Speaker::new();
    let device = speaker.device("a");
    cx.update(|_, cx| app.update(cx, |app, _| app.cast_devices = vec![device.clone()]));
    speaker.calls.lock().unwrap().clear();
    speaker.hold.store(true, Ordering::SeqCst);
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.speaker_property(device.clone(), Property::Bass(-8), cx);
            app.speaker_property(device.clone(), Property::Bass(7), cx);
        })
    });
    wait(cx, &app, |_| {
        speaker
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call[0] == "bass")
    });
    let replacement = Speaker::new();
    let current = replacement.device("a");
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.cast_devices = vec![current.clone()];
            app.speaker_property(current.clone(), Property::Bass(5), cx);
        })
    });
    wait(cx, &app, |app| app.cast_property_pending.is_empty());
    speaker.hold.store(false, Ordering::SeqCst);
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            app.load_speaker_properties(current.clone(), cx)
        })
    });
    wait(cx, &app, |app| app.cast_property_loading.is_empty());
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_properties[&device.id].bass, Some(5))
    });
    assert_eq!(replacement.properties.lock().unwrap().bass, Some(5));
    assert_eq!(speaker.properties.lock().unwrap().bass, Some(-8));
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn network_speaker_properties_popup_keeps_picker_open_during_edits(cx: &mut TestAppContext) {
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    let speaker = Speaker::new();
    let device = speaker.device("a");
    let dir = std::env::temp_dir().join(format!("feishin-nested-speaker-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_window();
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
        app.update(cx, |app, _| {
            app.cast_devices = vec![device.clone()];
            app.cast_selected = Some(device.clone());
            app.cast_scanning = true;
        });
        captured.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1200.), px(800.)));
    cx.run_until_parked();
    let trigger = cx.debug_bounds("player-output").unwrap();
    let target = trigger.center();
    let resting_bottom = f32::from(trigger.top()) - 10.;
    cx.simulate_click(target, Modifiers::none());
    wait(cx, &app, |app| app.cast_property_loading.is_empty());
    let entering = cx.debug_bounds("player-output-panel").unwrap();
    assert!((f32::from(entering.bottom()) - (resting_bottom + 30.)).abs() < 1.1);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(75));
    cx.refresh().unwrap();
    cx.run_until_parked();
    let midpoint = cx.debug_bounds("player-output-panel").unwrap();
    // Original fade-up: 30 CSS pixels, 150 ms ease. ease(0.5) = 0.802403.
    assert!((f32::from(midpoint.bottom()) - (resting_bottom + 30. * (1. - 0.802403))).abs() < 1.1);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.refresh().unwrap();
    cx.run_until_parked();
    let bounds = cx.debug_bounds("player-output-panel").unwrap();
    assert!((f32::from(bounds.center().x - target.x)).abs() < 1.1);
    assert!((f32::from(bounds.bottom()) - resting_bottom).abs() < 1.1);
    let settings = cx.debug_bounds("speaker-properties-uuid:a").unwrap();
    let target = settings.center();
    cx.simulate_click(target, Modifiers::none());
    wait(cx, &app, |app| app.cast_property_loading.is_empty());
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(200));
    cx.refresh().unwrap();
    cx.run_until_parked();
    let properties = cx.debug_bounds("speaker-property-panel").unwrap();
    // Mantine reserves half of its seven-pixel arrow in addition to the ten-pixel gap.
    assert!((f32::from(settings.left() - properties.right()) - 13.5).abs() < 1.1);
    // The player bar sits at the bottom, so the popup must stay inside the viewport.
    let expected_top = (settings.center().y - properties.size.height / 2.)
        .clamp(px(5.), px(795.) - properties.size.height);
    assert!((f32::from(properties.top() - expected_top)).abs() < 1.1);
    app.read_with(cx, |app, _| {
        assert!(app.cast_output_open, "Opening properties closed the picker")
    });
    for name in ["loudness", "crossfade", "ledState", "touchControls"] {
        let target = cx
            .debug_bounds(&format!("speaker-property-{name}"))
            .unwrap()
            .center();
        cx.simulate_click(target, Modifiers::none());
        wait(cx, &app, |app| app.cast_property_pending.is_empty());
        app.read_with(cx, |app, _| {
            assert!(
                app.cast_output_open,
                "Property {name} click dismissed the speaker picker"
            )
        });
    }
    app.read_with(cx, |app, _| {
        let p = &app.cast_properties[&device.id];
        assert_eq!(
            (p.loudness, p.crossfade, p.led, p.touch),
            (Some(false), Some(true), Some(false), Some(false))
        );
    });
    {
        let p = speaker.properties.lock().unwrap();
        assert_eq!(
            (p.loudness, p.crossfade, p.led, p.touch),
            (Some(false), Some(true), Some(false), Some(false))
        );
    }
    // The last switch retains keyboard focus and uses the same property write path.
    cx.simulate_keystrokes("space");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").unwrap(),
    });
    wait(cx, &app, |app| app.cast_property_pending.is_empty());
    assert_eq!(speaker.properties.lock().unwrap().touch, Some(true));
    app.read_with(cx, |app, _| assert!(app.cast_output_open));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    app.read_with(cx, |app, _| assert!(app.cast_output_open));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    app.read_with(cx, |app, _| assert!(!app.cast_output_open));
    std::fs::remove_dir_all(dir).ok();
}

#[gpui::test]
fn network_speaker_properties_match_original_content_layout(cx: &mut TestAppContext) {
    use crate::speaker_properties::PropertiesPanel;
    use feishin_core::dlna::Properties;
    cx.set_assets(crate::icons::Assets);
    cx.update(gpui_component::init);
    cx.use_platform_text_system();
    cx.update(|cx| crate::fonts::register_builtins(cx).unwrap());
    let speaker = Speaker::new();
    let mut device = speaker.device("properties");
    device.name = "Living room".into();
    let dir =
        std::env::temp_dir().join(format!("feishin-property-layout-{}", uuid::Uuid::new_v4()));
    let captured = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(State::default()), window, cx));
        app.update(cx, |app, _| {
            // This layout fixture supplies its own theme. A wall-clock reload must
            // not replace it with the saved 16px theme when the suite is busy.
            app.last_theme_scan = std::time::Instant::now() + std::time::Duration::from_secs(3600);
        });
        let panel = cx.new(|cx| PropertiesPanel::new(app.downgrade(), device.clone(), window, cx));
        struct Content {
            panel: Entity<PropertiesPanel>,
            case: Value,
        }

        impl Render for Content {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                let content = if let Some(position) = self.case["position"].as_str() {
                    let zoom = self.case["zoom"].as_f64().unwrap() as f32;
                    let left = position == "left";
                    let panel = self.panel.clone();
                    div()
                        .absolute()
                        .left(px(self.case["x"].as_f64().unwrap() as f32 * zoom))
                        .top(px(self.case["y"].as_f64().unwrap() as f32 * zoom))
                        .child(
                            gpui_component::popover::Popover::new("placement-probe")
                                .default_open(true)
                                .appearance(false)
                                .anchor(if left {
                                    if self.case["rtl"] == true {
                                        Corner::TopLeft
                                    } else {
                                        Corner::TopRight
                                    }
                                } else {
                                    Corner::BottomRight
                                })
                                .centered(if left {
                                    Axis::Vertical
                                } else {
                                    Axis::Horizontal
                                })
                                .offset(px(if left { 13.5 } else { 10. } * zoom))
                                .viewport_margin(px(5. * zoom))
                                .trigger(Button::new("target").size(px(24. * zoom)).p_0())
                                .content(move |_, _, _| panel.clone()),
                        )
                        .into_any_element()
                } else {
                    self.panel.clone().into_any_element()
                };
                div()
                    .size_full()
                    .h_flex()
                    .items_start()
                    .font(crate::fonts::app_font(cx))
                    .child(content)
            }
        }
        let content = cx.new(|_| Content {
            panel,
            case: Value::Null,
        });
        captured.replace(Some((app.clone(), content.clone())));
        Root::new(content, window, cx)
    });
    let (app, content) = captured.borrow_mut().take().unwrap();
    cx.simulate_scale_factor(if cfg!(target_os = "macos") { 2. } else { 1. });
    #[cfg(target_os = "linux")]
    let reference = include_str!("../assets/network-properties-layout-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let reference = include_str!("../assets/network-properties-layout-darwin-reference.json");
    let mut cases: Vec<Value> = serde_json::from_str(reference).unwrap();
    #[cfg(target_os = "linux")]
    let popup_reference = include_str!("../assets/network-popup-layout-linux-reference.json");
    #[cfg(not(target_os = "linux"))]
    let popup_reference = include_str!("../assets/network-popup-layout-darwin-reference.json");
    cases.extend(serde_json::from_str::<Vec<Value>>(popup_reference).unwrap());
    let names = [
        "bass",
        "treble",
        "loudness",
        "crossfade",
        "ledState",
        "touchControls",
    ];
    for case in cases {
        let zoom = case["zoom"].as_f64().unwrap() as f32;
        let rem = case["rem"].as_f64().unwrap() as f32;
        cx.simulate_resize(size(px(1200. * zoom), px(800. * zoom)));
        cx.update(|window, cx| {
            content.update(cx, |content, cx| {
                content.case = case.clone();
                cx.notify();
            });
            window.set_rem_size(px(rem * zoom));
            window.text_system().set_optical_size_scale(zoom);
            Theme::global_mut(cx).font_size = px(rem * zoom);
            cx.set_global(crate::appearance::Language(
                if case["rtl"] == true { "ar" } else { "en" }.into(),
            ));
            app.update(cx, |app, cx| {
                app.state.settings.zoom = zoom;
                app.cast_properties.clear();
                app.cast_property_loading.clear();
                if case["state"] == "loading" {
                    app.cast_property_loading.insert(device.id.clone());
                }
                if case["state"] == "loaded" {
                    let value = case["value"].as_i64().unwrap() as i32;
                    let checked = case["checked"].as_bool().unwrap();
                    app.cast_properties.insert(
                        device.id.clone(),
                        Properties {
                            bass: Some(value),
                            treble: Some(-value),
                            loudness: Some(checked),
                            crossfade: Some(!checked),
                            led: Some(checked),
                            touch: Some(!checked),
                        },
                    );
                }
                cx.notify();
            });
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(200));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let root = cx.debug_bounds("speaker-property-panel").unwrap();
        if let Some(bounds) = case["viewport"].as_array() {
            assert_eq!(case["viewportSize"], serde_json::json!([1200, 800]));
            for (index, actual) in [
                root.origin.x,
                root.origin.y,
                root.size.width,
                root.size.height,
            ]
            .into_iter()
            .enumerate()
            {
                let expected = bounds[index].as_f64().unwrap() as f32 * zoom;
                assert!(
                    (f32::from(actual) - expected).abs() <= 1.1,
                    "Popup placement axis {index}: {actual:?} != {expected}; {case}"
                );
            }
        }
        let mut check = |selector: &str, bounds: &Value| {
            let actual = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}: {case}"));
            let values = [
                f32::from(actual.origin.x - root.origin.x),
                f32::from(actual.origin.y - root.origin.y),
                f32::from(actual.size.width),
                f32::from(actual.size.height),
            ];
            for (i, value) in values.into_iter().enumerate() {
                let expected = bounds[i].as_f64().unwrap() as f32 * zoom;
                let tolerance = if i < 2 { 2. } else { 1.1 };
                assert!(
                    (value - expected).abs() <= tolerance,
                    "{selector} axis {i}: {value} != {expected}; {case}"
                );
            }
        };
        check("speaker-property-panel", &case["bounds"]);
        for (index, paper) in case["papers"].as_array().unwrap().iter().enumerate() {
            check(&format!("property-paper-{index}"), paper);
        }
        for text in case["text"].as_array().unwrap() {
            check(
                &format!("property-text-{}", text["label"].as_str().unwrap()),
                &text["bounds"],
            );
        }
        for (index, row) in case["rows"].as_array().unwrap().iter().enumerate() {
            check(&format!("property-row-{}", names[index]), row);
        }
        for (index, slider) in case["sliders"].as_array().unwrap().iter().enumerate() {
            for (field, suffix) in [("bounds", "slider"), ("track", "track"), ("thumb", "thumb")] {
                check(
                    &format!("speaker-property-{}-{suffix}", names[index]),
                    &slider[field],
                );
            }
        }
        for (index, switch) in case["switches"].as_array().unwrap().iter().enumerate() {
            check(
                &format!("speaker-property-{}", names[index + 2]),
                &switch["bounds"],
            );
            check(
                &format!("speaker-property-{}-thumb", names[index + 2]),
                &switch["thumb"],
            );
        }
    }
    // Large text in a short window retains access to the last setting.
    speaker.properties.lock().unwrap().touch = Some(false);
    cx.simulate_resize(size(px(1200.), px(400.)));
    cx.update(|window, cx| {
        window.set_rem_size(px(25.));
        content.update(cx, |content, cx| {
            content.case = Value::Null;
            cx.notify();
        });
        app.update(cx, |app, cx| {
            app.cast_devices = vec![device.clone()];
            app.cast_properties.get_mut(&device.id).unwrap().touch = Some(false);
            cx.notify();
        });
    });
    cx.run_until_parked();
    let viewport = cx.debug_bounds("speaker-property-panel").unwrap();
    assert_eq!(viewport.size.height, px(320.));
    let last = cx.debug_bounds("speaker-property-touchControls").unwrap();
    assert!(last.bottom() > viewport.bottom());
    cx.simulate_event(ScrollWheelEvent {
        position: viewport.center(),
        delta: ScrollDelta::Pixels(point(px(0.), px(-1000.))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    cx.run_until_parked();
    let last = cx.debug_bounds("speaker-property-touchControls").unwrap();
    assert!(viewport.contains(&last.center()));
    cx.simulate_click(last.center(), Modifiers::none());
    wait(cx, &app, |app| app.cast_property_pending.is_empty());
    assert_eq!(speaker.properties.lock().unwrap().touch, Some(true));
    app.read_with(cx, |app, _| {
        assert_eq!(app.cast_properties[&device.id].touch, Some(true))
    });
    std::fs::remove_dir_all(dir).ok();
}
