use super::*;
use ::core::prelude::v1::test;
use feishin_core::storage;
use gpui_component::Root;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

struct Form {
    app: Entity<Feishin>,
    scroll: ScrollHandle,
    _subscription: Subscription,
}
impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("lyric-form")
            .debug_selector(|| "lyric-form".into())
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(self.app.update(cx, |app, cx| app.lyric_settings(cx)))
    }
}

fn reveal(form: &Entity<Form>, selector: &str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    let row = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("Missing {selector}"));
    let viewport = cx.debug_bounds("lyric-form").unwrap();
    form.update(cx, |form, cx| {
        let offset = form.scroll.offset();
        form.scroll.set_offset(point(
            px(0.),
            offset.y + viewport.top() - row.top() + px(24.),
        ));
        cx.notify();
    });
    cx.run_until_parked();
    cx.debug_bounds(selector).unwrap()
}

fn choose(form: &Entity<Form>, selector: &str, index: usize, cx: &mut VisualTestContext) {
    let button = reveal(form, selector, cx);
    cx.simulate_click(button.center(), Modifiers::none());
    cx.run_until_parked();
    let item = cx
        .debug_bounds(&format!("popup-menu-item-{index}"))
        .unwrap();
    cx.simulate_click(item.center(), Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn lyric_settings_match_original_limits_units_surfaces_labels_and_switches(
    cx: &mut TestAppContext,
) {
    cx.set_assets(crate::icons::Assets);
    cx.use_platform_text_system();
    cx.update(gpui_component::init);
    let dir = std::env::temp_dir().join(format!("feishin-lyric-settings-{}", uuid::Uuid::new_v4()));
    let saved = Rc::new(RefCell::new(None));
    let (_, cx) = cx.add_window_view(|window, cx| {
        window.activate_accessibility_for_test();
        let mut state = storage::State::default();
        state.settings.private_mode = true;
        let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
        let form = cx.new(|cx| Form {
            _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
            app: app.clone(),
            scroll: ScrollHandle::new(),
        });
        saved.replace(Some((app, form.clone())));
        Root::new(form, window, cx)
    });
    let (app, form) = saved.borrow_mut().take().unwrap();
    let reference: Vec<Value> =
        serde_json::from_str(include_str!("../assets/lyric-settings-reference.json")).unwrap();
    let number_ids = [
        "delayMs",
        "followScrollAlignment",
        "lineLeadTimeMs",
        "fontSize",
        "fontSizeUnsync",
        "gap",
        "gapUnsync",
        "opacityNonActive",
        "scaleNonActive",
        "paddingLeft",
        "paddingRight",
    ];
    let mut comparisons = 0;
    for case in &reference {
        let surface = case["settingsKey"].as_str().unwrap();
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.language = case["language"].as_str().unwrap().into();
                app.apply_theme(window, cx);
                app.set_lyric_settings_key(surface, window, cx);
                for option in case["groups"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .flat_map(|group| group["options"].as_array().unwrap())
                {
                    let id = option["id"].as_str().unwrap();
                    if let Some(index) = number_ids.iter().position(|candidate| *candidate == id) {
                        assert_eq!(number_label(index, cx), option["label"]);
                        let (_, min, max, step) = LYRIC_NUMBERS[index];
                        for (key, actual) in [("min", min), ("max", max), ("step", step)] {
                            if let Some(expected) = option["control"][key].as_f64() {
                                assert_eq!(actual, expected, "{id}/{key}");
                            }
                        }
                        for change in option["changes"].as_array().unwrap() {
                            let value = change["value"].as_f64().unwrap();
                            app.lyric_numbers[index].update(cx, |input, cx| {
                                input.set_value(value.to_string(), window, cx)
                            });
                            app.apply_lyric_number(index, cx);
                            let actual = app.lyric_number_values()[index];
                            let expected = if index < 3 {
                                &change["updates"][0]["lyrics"][id]
                            } else {
                                &change["updates"][0]["lyricsDisplay"][surface][id]
                            };
                            assert!(
                                (actual - expected.as_f64().unwrap()).abs() < 0.000001,
                                "{id}: {actual} != {expected}"
                            );
                            comparisons += 1;
                        }
                    } else if option["control"]["type"] == "ListConfigBooleanControl" {
                        let native_id = match id {
                            "preferLocalLyrics" => "lyric-prefer-local",
                            "fetch" => "lyric-fetch",
                            "enableFurigana" => "lyric-furigana",
                            "enableRomaji" => "lyric-romaji",
                            "enableNeteaseTranslation" => "lyric-translation",
                            "follow" => "lyric-scroll",
                            "showMatch" => "lyric-match",
                            "showProvider" => "lyric-provider",
                            "enableAutoTranslation" => "lyric-cloud-auto",
                            _ => panic!("{id}"),
                        };
                        for change in option["changes"].as_array().unwrap() {
                            app.set_lyric_boolean(
                                native_id,
                                change["value"].as_bool().unwrap(),
                                cx,
                            );
                            let state = &app.state.settings;
                            let actual = match id {
                                "preferLocalLyrics" => state.lyric_prefer_local,
                                "fetch" => state.lyric_fetch,
                                "enableFurigana" => state.lyric_furigana,
                                "enableRomaji" => state.lyric_romaji,
                                "enableNeteaseTranslation" => state.lyric_translation,
                                "follow" => state.lyric_scroll,
                                "showMatch" => state.lyric_show_match,
                                "showProvider" => state.lyric_show_provider,
                                "enableAutoTranslation" => state.lyric_cloud_translation.automatic,
                                _ => unreachable!(),
                            };
                            assert_eq!(
                                actual,
                                change["updates"][0]["lyrics"][id].as_bool().unwrap(),
                                "{id}"
                            );
                            comparisons += 1;
                        }
                    }
                }
            })
        });
        cx.run_until_parked();
        let bounds = reveal(&form, "lyric-fetch", cx);
        let before = app.read_with(cx, |app, _| app.state.settings.lyric_fetch);
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
        assert_ne!(
            app.read_with(cx, |app, _| app.state.settings.lyric_fetch),
            before,
            "{} / {surface}, bounds: {bounds:?}",
            case["language"]
        );
        let tree = cx.update(|window, _| window.accessibility_tree_for_test().unwrap().clone());
        let label = app.read_with(cx, |app, _| app.tr("setting.lyricFetch"));
        assert!(
            tree.nodes.iter().any(
                |(_, node)| node.label() == Some(label.as_str()) && node.role() == Role::Switch
            )
        );
    }
    assert_eq!(comparisons, 204);
    // A real slider edit commits through its event subscription and stays on the
    // chosen display surface. Changing surfaces synchronizes the retained thumb.
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.state.settings.language = "en".into();
            app.apply_theme(window, cx);
            app.state.settings.lyric_display.font_size = 24.;
            app.set_lyric_settings_key("sidebar", window, cx);
            let slider = app
                .lyric_sliders
                .iter()
                .find(|(index, _)| *index == 3)
                .unwrap()
                .1
                .clone();
            slider.update(cx, |slider, cx| {
                slider.set_value(36., window, cx);
                cx.emit(SliderEvent::ChangeEnd(36_f32.into()));
            });
        })
    });
    app.read_with(cx, |app, _| {
        assert_eq!(app.state.settings.lyric_display.font_size, 24.);
        assert_eq!(
            app.state.settings.lyric_display_for("sidebar").font_size,
            36.
        );
    });
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.set_lyric_settings_key("default", window, cx)
        })
    });
    app.read_with(cx, |app, cx| {
        assert_eq!(
            app.lyric_sliders
                .iter()
                .find(|(index, _)| *index == 3)
                .unwrap()
                .1
                .read(cx)
                .value()
                .end(),
            24.
        )
    });
    cx.run_until_parked();
    choose(&form, "lyric-sources", 4, cx);
    assert!(app.read_with(cx, |app, _| app.state.settings.lyric_sources.is_empty()));
    choose(&form, "lyric-sources", 0, cx);
    choose(&form, "lyric-sources", 2, cx);
    app.read_with(cx, |app, _| {
        assert_eq!(
            app.state.settings.lyric_sources,
            [Provider::Lrclib, Provider::Genius]
        )
    });
    choose(&form, "lyric-cloud-provider", 2, cx);
    app.read_with(cx, |app, _| {
        assert_eq!(
            app.state.settings.lyric_cloud_translation.provider,
            "Google Cloud"
        )
    });
    choose(&form, "lyric-cloud-provider", 0, cx);
    assert!(app.read_with(cx, |app, _| {
        app.state
            .settings
            .lyric_cloud_translation
            .provider
            .is_empty()
    }));
    choose(&form, "lyric-cloud-language", 1, cx);
    app.read_with(cx, |app, _| {
        assert_eq!(app.state.settings.lyric_cloud_translation.language, "ar")
    });
    reveal(&form, "lyric-number-0", cx);
    cx.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.lyric_numbers[0].update(cx, |input, cx| input.focus(window, cx))
        })
    });
    cx.simulate_keystrokes("up");
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert_eq!(app.state.settings.lyric_offset, 0.26)
    });
    cx.simulate_keystrokes("down");
    cx.run_until_parked();
    reveal(&form, "lyric-number-3", cx);
    cx.update(|window, cx| {
        let label = number_label(3, cx);
        let node = window
            .accessibility_tree_for_test()
            .unwrap()
            .nodes
            .iter()
            .find(|(_, node)| node.role() == Role::Slider && node.label() == Some(label.as_str()))
            .unwrap()
            .0;
        window.dispatch_accessibility_action_for_test(
            gpui::accesskit::ActionRequest {
                action: gpui::accesskit::Action::Focus,
                target_tree: gpui::accesskit::TreeId::ROOT,
                target_node: node,
                data: None,
            },
            cx,
        );
    });
    cx.simulate_keystrokes("right");
    cx.run_until_parked();
    app.read_with(cx, |app, _| {
        assert_eq!(app.state.settings.lyric_display.font_size, 25.)
    });
    let saved = storage::load(&dir.join("state.json")).unwrap();
    assert_eq!(saved.settings.lyric_display_for("sidebar").font_size, 36.);
    assert_eq!(saved.settings.lyric_offset, 0.25);
    assert_eq!(saved.settings.lyric_lead_time, 3.);
    cx.update(|window, _| window.remove_window());
    std::fs::remove_dir_all(dir).ok();
}
