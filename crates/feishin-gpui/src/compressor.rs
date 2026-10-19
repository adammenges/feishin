use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, StyledExt};

impl Feishin {
    pub(crate) fn set_compressor_value(
        &mut self,
        index: usize,
        value: f32,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (_, min, max, _) = crate::library::AUDIO_NUMBERS[index + 1];
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(min, max);
        let compressor = &mut self.state.settings.compressor;
        let target = match index {
            0 => &mut compressor.threshold,
            1 => &mut compressor.ratio,
            2 => &mut compressor.attack,
            3 => &mut compressor.release,
            4 => &mut compressor.makeup,
            _ => &mut compressor.knee,
        };
        if *target != value {
            *target = value;
            self.audio_numbers[index + 1].update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.compressor_sliders[index].update(cx, |slider, cx| slider.set_value(value, window, cx));
        if commit {
            self.apply_audio_settings(cx);
        } else {
            self.save();
            cx.notify();
        }
    }

    pub(crate) fn compressor_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_4()
            .when(self.state.settings.compressor.enabled, |view| {
                view.child(self.audio_presets(true, cx))
                    .children(
                        self.compressor_sliders
                            .iter()
                            .enumerate()
                            .map(|(index, slider)| {
                                let (key, _, _, _) = crate::library::AUDIO_NUMBERS[index + 1];
                                let title = self.tr(key);
                                let unit = ["dB", ":1", "ms", "ms", "dB", "dB"][index];
                                div()
                                    .v_flex()
                                    .gap_2()
                                    .child(title.clone())
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.tr(&format!("{key}_description"))),
                                    )
                                    .child(
                                        div()
                                            .h_flex()
                                            .when(crate::appearance::rtl(cx), |row| {
                                                row.flex_row_reverse()
                                            })
                                            .gap_3()
                                            .child(
                                                div()
                                                    .w(px(200.))
                                                    .debug_selector(move || {
                                                        format!("compressor-slider-{index}")
                                                    })
                                                    .child(
                                                        crate::styles::slider(slider, cx)
                                                            .aria_label(title.clone()),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .w(px(80.))
                                                    .debug_selector(move || {
                                                        format!("compressor-number-{index}")
                                                    })
                                                    .child(
                                                        crate::styles::number_input(
                                                            &self.audio_numbers[index + 1],
                                                            cx,
                                                        )
                                                        .hide_controls(true),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child(unit),
                                            ),
                                    )
                            }),
                    )
                    .child(
                        crate::styles::button("compressor-reset", cx)
                            .debug_selector(|| "compressor-reset".into())
                            .label(self.tr("common.resetToDefault"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.apply_compressor_preset(Default::default(), window, cx)
                            })),
                    )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Route;
    use ::core::prelude::v1::test;
    use feishin_core::{model::Item, queue::Enqueue, settings_search::Tab, storage::State};
    use gpui_component::{Root, slider::SliderEvent};
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn controls_match_original_ranges_drafts_sliders_reset_and_disabled_states(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-compressor-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let capture = captured.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_window();
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.settings_tab = Tab::Playback;
            state.settings.mpv_path = directory.join("absent-player").to_string_lossy().into();
            state.queue.add(
                vec![
                    Item {
                        id: "keep".into(),
                        ..Default::default()
                    };
                    2
                ],
                Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.settings_search
                    .update(cx, |input, cx| input.set_value("Compressor", window, cx));
            });
            capture.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1400.), px(2200.)));
        cx.run_until_parked();
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let cases: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/compressor-settings-reference.json"))
                .unwrap();
        for case in cases {
            let name = case["name"].as_str().unwrap();
            match name {
                "disabled" => {}
                "enabled" | "disabled-again" | "reset" => {
                    let selector = if name == "reset" {
                        "compressor-reset"
                    } else {
                        "compressor-toggle"
                    };
                    let bounds = cx.debug_bounds(selector).unwrap();
                    cx.simulate_click(bounds.center(), Modifiers::none());
                }
                "makeup-preview" | "makeup-commit" | "knee-preview" | "knee-commit" => {
                    let (index, value) = if name.starts_with("makeup") {
                        (4, 30.)
                    } else {
                        (5, 10.)
                    };
                    let slider =
                        app.read_with(cx, |view, _| view.compressor_sliders[index].clone());
                    slider.update(cx, |_, cx| {
                        cx.emit(if name.ends_with("preview") {
                            SliderEvent::Change(value.into())
                        } else {
                            SliderEvent::ChangeEnd(value.into())
                        })
                    });
                }
                "attack-draft" | "ratio-draft" | "makeup-over" | "threshold-below"
                | "release-empty" => {
                    let (index, value) = match name {
                        "attack-draft" => (3, "0.1"),
                        "ratio-draft" => (2, "2.5"),
                        "makeup-over" => (5, "35"),
                        "threshold-below" => (1, "-70"),
                        _ => (4, ""),
                    };
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.audio_numbers[index].update(cx, |input, cx| {
                                input.focus(window, cx);
                                input.set_value(value, window, cx);
                            })
                        })
                    });
                }
                "ratio-enter" => cx.simulate_keystrokes("enter"),
                "attack-blur" | "makeup-clamp" | "threshold-clamp" | "release-blur" => {
                    cx.update(|window, cx| app.read_with(cx, |view, _| window.focus(&view.focus)));
                }
                _ => unreachable!("{name}"),
            }
            cx.run_until_parked();
            cx.refresh().unwrap();
            app.read_with(cx, |view, cx| {
                let expected: feishin_core::dsp::Compressor =
                    serde_json::from_value(case["stored"].clone()).unwrap();
                assert_eq!(view.state.settings.compressor, expected, "{name}");
                assert_eq!(
                    serde_json::to_value(&view.state.queue).unwrap(),
                    queue,
                    "{name}"
                );
                for (index, parameter) in case["parameters"].as_array().unwrap().iter().enumerate()
                {
                    let (key, min, max, step) = crate::library::AUDIO_NUMBERS[index + 1];
                    assert_eq!(key, parameter["key"].as_str().unwrap());
                    for (actual, key) in [(min, "min"), (max, "max"), (step, "step")] {
                        assert!(
                            (f64::from(actual) - parameter[key].as_f64().unwrap()).abs() < 1e-5
                        );
                    }
                    assert_eq!(
                        view.audio_numbers[index + 1].read(cx).value().as_str(),
                        parameter["input"].as_str().unwrap(),
                        "{name}: {key}"
                    );
                    assert!(
                        (f64::from(view.compressor_sliders[index].read(cx).value().end())
                            - parameter["value"].as_f64().unwrap())
                        .abs()
                            < 1e-5,
                        "{name}: {key}"
                    );
                }
            });
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let parameters = case["parameters"].as_array().unwrap();
                for (key, _, _, _) in crate::library::AUDIO_NUMBERS.iter().skip(1).take(6) {
                    let title = feishin_core::i18n::translate("en", key).unwrap();
                    assert_eq!(
                        tree.nodes
                            .iter()
                            .any(|(_, node)| node.role() == gpui::accesskit::Role::Slider
                                && node.label() == Some(title.as_str())),
                        !parameters.is_empty(),
                        "{name}: {title}"
                    );
                }
            });
        }
        let persisted = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert!(!persisted.settings.compressor.enabled);
        assert_eq!(persisted.settings.compressor.makeup, 6.);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
