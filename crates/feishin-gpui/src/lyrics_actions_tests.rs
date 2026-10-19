use super::*;
use ::core::prelude::v1::test;
use feishin_core::{queue::Enqueue, storage};
use gpui_component::{Root, Theme};
use std::{cell::RefCell, rc::Rc};

struct Probe {
    app: Entity<Feishin>,
    width: f32,
    dual: bool,
    _subscription: Subscription,
}
impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app
            .update(cx, |app, cx| app.sync_song_lyric_offset(window, cx));
        div()
            .h_flex()
            .gap_4()
            .children((0..if self.dual { 2 } else { 1 }).map(|index| {
                div()
                    .w(px(self.width))
                    .h(px(300.))
                    .flex_none()
                    .child(self.app.update(cx, |app, cx| {
                        app.lyrics_frame(
                            div().size_full().into_any_element(),
                            index == 1,
                            window,
                            cx,
                        )
                    }))
            }))
    }
}
fn initialize(cx: &mut TestAppContext) {
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
}
fn record(config: &serde_json::Value) -> Record {
    Record {
        raw: if config["hasLyrics"] == true {
            "[00:01]Line".into()
        } else {
            String::new()
        },
        matched: None,
        selected: 0,
        variants: config["languages"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, label)| {
                let text = label.as_str().unwrap();
                let (language, kind) = text.split_once(" (").unwrap_or((text, "main"));
                lyrics::Variant {
                    language: language.into(),
                    kind: kind.trim_end_matches(')').into(),
                    synchronized: true,
                    offset: None,
                    lines: if config["hasLyrics"] == true {
                        lyrics::parse_lrc(&format!("[00:01]Line {index}"))
                    } else {
                        vec![]
                    },
                    agents: vec![],
                }
            })
            .collect(),
    }
}

#[gpui::test]
fn lyric_actions_match_original_layout_and_disabled_controls(cx: &mut TestAppContext) {
    initialize(cx);
    #[cfg(target_os = "macos")]
    let reference = include_str!("../assets/lyrics-actions-darwin-reference.json");
    #[cfg(not(target_os = "macos"))]
    let reference = include_str!("../assets/lyrics-actions-linux-reference.json");
    let cases: Vec<serde_json::Value> = serde_json::from_str(reference).unwrap();
    let directory =
        std::env::temp_dir().join(format!("feishin-actions-layout-{}", uuid::Uuid::new_v4()));
    let saved = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let app =
            cx.new(|cx| Feishin::new(directory.clone(), Ok(storage::State::default()), window, cx));
        let probe = cx.new(|cx| Probe {
            app: app.clone(),
            width: 220.,
            dual: false,
            _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
        });
        saved.replace(Some((app, probe.clone())));
        Root::new(probe, window, cx)
    });
    let (app, probe) = saved.borrow_mut().take().unwrap();
    for case in cases {
        let config = &case["config"];
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.state.queue = Default::default();
                if config["song"] == true {
                    app.state.queue.add(
                        vec![Item {
                            name: "Song".into(),
                            artist: "Artist".into(),
                            ..Default::default()
                        }],
                        Enqueue::Now,
                    );
                }
                app.state.settings.lyric_offset = 0.125;
                app.state.settings.lyric_sources = if config["sources"] == true {
                    vec![Provider::Lrclib]
                } else {
                    vec![]
                };
                app.state.settings.lyric_overlay_layers = config["active"]
                    .as_array()
                    .map(|v| v.iter().map(|v| v.as_str().unwrap().to_owned()).collect())
                    .unwrap_or_default();
                let record = record(config);
                app.lyrics = record.lines();
                app.lyric_record = Some(record);
                cx.notify();
            });
            let rem = px(config["rem"].as_f64().unwrap() as f32);
            Theme::global_mut(cx).font_size = rem;
            window.set_rem_size(rem);
            probe.update(cx, |probe, cx| {
                probe.width = config["width"].as_f64().unwrap() as f32;
                cx.notify();
            });
        });
        cx.run_until_parked();
        let frame = cx.debug_bounds("lyrics-frame-false").unwrap();
        let check = |cx: &mut VisualTestContext, selector: &str, expected: &serde_json::Value| {
            let actual = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("missing {selector}: {case}"));
            let values = [
                f32::from(actual.origin.x - frame.origin.x),
                f32::from(actual.origin.y - frame.origin.y),
                f32::from(actual.size.width),
                f32::from(actual.size.height),
            ];
            for (actual, expected) in values.into_iter().zip(expected.as_array().unwrap()) {
                assert!(
                    (actual - expected.as_f64().unwrap() as f32).abs() < 1.,
                    "{selector}: {values:?}, expected {expected}, case {case}"
                );
            }
        };
        check(cx, "lyric-actions-false", &case["bounds"]);
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        for control in case["controls"].as_array().unwrap() {
            let label = control["label"].as_str().unwrap();
            let selector = match label {
                "SEARCH" => "lyric-search",
                "EXPORT LYRICS" => "lyric-export",
                "TRANSLATION" => "lyric-cloud-translate",
                "CLEAR" | "REFRESH" => "lyric-clear",
                "Decrease lyric offset" => "lyric-offset-decrease",
                "Increase lyric offset" => "lyric-offset-increase",
                "Lyric offset" => "lyric-offset-input",
                "Lyric layers" => "lyric-layers",
                "Show pronunciation" => "lyric-overlay-control-0",
                "Hide translation" => "lyric-overlay-control-1",
                value if value.starts_with("Language:") => "lyric-variant",
                _ => panic!("Unknown {label}"),
            };
            check(cx, selector, &control["bounds"]);
            let node = tree
                .nodes
                .iter()
                .find(|(_, n)| n.label() == Some(label))
                .unwrap_or_else(|| panic!("missing accessible {label}"));
            assert_eq!(node.1.is_disabled(), control["disabled"] == true, "{label}");
        }
    }
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(directory).ok();
}

#[gpui::test]
fn lyric_actions_hover_focus_motion_and_two_panes_share_only_song_state(cx: &mut TestAppContext) {
    initialize(cx);
    let directory =
        std::env::temp_dir().join(format!("feishin-actions-input-{}", uuid::Uuid::new_v4()));
    let saved = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = storage::State::default();
        state.settings.private_mode = true;
        state.queue.add(vec![Item::default()], Enqueue::Now);
        let app = cx.new(|cx| {
            let mut app = Feishin::new(directory.clone(), Ok(state), window, cx);
            app.lyrics = lyrics::parse_lrc("[00:01]Line");
            app
        });
        let probe = cx.new(|cx| Probe {
            app: app.clone(),
            width: 360.,
            dual: true,
            _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
        });
        saved.replace(Some(app));
        Root::new(probe, window, cx)
    });
    let app = saved.borrow_mut().take().unwrap();
    cx.run_until_parked();
    let frame = cx.debug_bounds("lyrics-frame-false").unwrap();
    let export = cx.debug_bounds("lyric-export").unwrap();
    let opacity = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| {
            window
                .painted_glyphs_with_opacity_for_test()
                .iter()
                .filter(|(bounds, _, _)| export.contains(&bounds.center()))
                .map(|(_, _, color)| color.a / cx.theme().foreground.a)
                .fold(0_f32, f32::max)
        })
    };
    let reference: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../assets/lyrics-actions-motion-reference.json"
    ))
    .unwrap();
    for case in reference {
        cx.update(|window, _| window.blur());
        cx.simulate_mouse_move(point(px(900.), px(700.)), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.refresh().unwrap();
        cx.run_until_parked();
        match case["phase"].as_str().unwrap() {
            "frame" => cx.simulate_mouse_move(
                frame.origin + point(px(10.), px(30.)),
                None,
                Modifiers::none(),
            ),
            "control" => cx.simulate_mouse_move(export.center(), None, Modifiers::none()),
            "focus" => cx.update(|window, cx| {
                app.read(cx)
                    .lyric_song_offset
                    .clone()
                    .update(cx, |input, cx| input.focus(window, cx))
            }),
            "leave" => {
                cx.simulate_mouse_move(
                    frame.origin + point(px(10.), px(30.)),
                    None,
                    Modifiers::none(),
                );
                cx.run_until_parked();
                cx.executor().advance_clock(Duration::from_millis(200));
                cx.refresh().unwrap();
                cx.run_until_parked();
                cx.simulate_mouse_move(point(px(900.), px(700.)), None, Modifiers::none());
            }
            _ => unreachable!(),
        }
        cx.run_until_parked();
        let mut previous = 0;
        for sample in case["frames"].as_array().unwrap() {
            let elapsed = sample["elapsed"].as_u64().unwrap();
            cx.executor()
                .advance_clock(Duration::from_millis(elapsed - previous));
            cx.refresh().unwrap();
            cx.run_until_parked();
            let actual = opacity(cx);
            assert!(
                (actual - sample["opacity"].as_f64().unwrap() as f32).abs() < 0.002,
                "{} {elapsed}: {actual}, expected {}",
                case["phase"],
                sample["opacity"]
            );
            previous = elapsed;
        }
    }
    let increase = cx.debug_bounds("lyric-offset-increase-sidebar").unwrap();
    cx.simulate_click(increase.center(), Modifiers::none());
    cx.run_until_parked();
    app.read_with(cx, |app, cx| {
        assert_ne!(app.lyric_song_offset, app.lyric_sidebar_offset);
        assert_eq!(app.lyric_song_offset.read(cx).value().as_str(), "50");
        assert_eq!(app.lyric_sidebar_offset.read(cx).value().as_str(), "50");
        assert_eq!(app.state.settings.lyric_offset, 0.);
    });
    cx.update(|window, cx| {
        app.read(cx)
            .lyric_sidebar_offset
            .clone()
            .update(cx, |input, cx| {
                input.focus(window, cx);
                input.set_value("-225", window, cx);
            })
    });
    cx.run_until_parked();
    app.read_with(cx, |app, cx| {
        assert_eq!(app.lyric_song_offset.read(cx).value().as_str(), "-225");
        assert_eq!(app.lyric_sidebar_offset.read(cx).value().as_str(), "-225");
    });
    let config = serde_json::json!({
        "hasLyrics": true,
        "languages": ["English (pronunciation)", "French (translation)", "German (romaji)"]
    });
    cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            let record = record(&config);
            app.lyrics = record.lines();
            app.lyric_record = Some(record);
            app.state.settings.lyric_overlay_layers.clear();
            cx.notify();
        })
    });
    cx.run_until_parked();
    for selector in ["lyric-overlay-control-0-sidebar", "lyric-overlay-control-1"] {
        let bounds = cx.debug_bounds(selector).unwrap();
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    }
    app.read_with(cx, |app, _| {
        assert_eq!(
            app.state.settings.lyric_overlay_layers,
            ["pronunciation:English", "translation:French"]
        )
    });
    let bounds = cx.debug_bounds("lyric-variant-sidebar").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let bounds = cx.debug_bounds("popup-menu-item-1").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert_eq!(app.lyric_record.as_ref().unwrap().selected, 1);
        assert_eq!(app.lyrics[0].text, "Line 1");
    });
    let bounds = cx.debug_bounds("lyric-layers").unwrap();
    cx.simulate_click(bounds.center(), Modifiers::none());
    cx.run_until_parked();
    let position = cx.update(|window, _| {
        let tree = window.accessibility_tree_for_test().unwrap();
        let node = tree
            .nodes
            .iter()
            .find(|(_, node)| {
                node.role() == Role::Switch && node.label() == Some("German (romaji)")
            })
            .unwrap();
        let bounds = node.1.bounds().unwrap();
        let scale = f64::from(window.scale_factor());
        point(
            px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
            px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
        )
    });
    cx.simulate_click(position, Modifiers::none());
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert!(
            app.state
                .settings
                .lyric_overlay_layers
                .contains(&"romaji:German".into())
        )
    });
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(directory).ok();
}
