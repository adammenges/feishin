use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{StyledExt, input::InputEvent, slider::SliderEvent};

pub(crate) type PresetSelect = gpui_component::select::SelectState<
    gpui_component::select::SearchableVec<gpui_component::select::SelectGroup<String>>,
>;

pub(crate) fn preset_options(
    settings: &feishin_core::storage::Settings,
    compressor: bool,
) -> gpui_component::select::SearchableVec<gpui_component::select::SelectGroup<String>> {
    use gpui_component::select::{SearchableVec, SelectGroup};
    let builtin = if compressor {
        feishin_core::presets::compressors()
            .into_iter()
            .map(|(name, _)| name.to_owned())
            .collect::<Vec<_>>()
    } else {
        feishin_core::presets::EQ
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect()
    };
    let custom = if compressor {
        settings
            .compressor_presets
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    } else {
        settings.equalizer_presets.keys().cloned().collect()
    };
    let mut groups = vec![SelectGroup {
        title: feishin_core::i18n::text(&settings.language, "setting.equalizerPresetGroupBuiltIn")
            .into(),
        items: builtin,
    }];
    if !custom.is_empty() {
        groups.push(SelectGroup {
            title: feishin_core::i18n::text(
                &settings.language,
                "setting.equalizerPresetGroupCustom",
            )
            .into(),
            items: custom,
        });
    }
    SearchableVec::new(groups)
}

impl Feishin {
    pub(crate) fn refresh_audio_preset_options(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for (index, select) in self.audio_preset_selects.iter().enumerate() {
            let options = preset_options(&self.state.settings, index == 1);
            select.update(cx, |select, cx| select.set_items(options, window, cx));
        }
    }
    pub(crate) fn select_audio_preset(
        &mut self,
        compressor: bool,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if compressor {
            if let Some(preset) = self
                .state
                .settings
                .compressor_presets
                .get(name)
                .copied()
                .or_else(|| {
                    feishin_core::presets::compressors()
                        .into_iter()
                        .find(|(label, _)| *label == name)
                        .map(|(_, preset)| preset)
                })
            {
                self.apply_compressor_preset(preset, window, cx);
            }
        } else if let Some(gains) = self
            .state
            .settings
            .equalizer_presets
            .get(name)
            .copied()
            .or_else(|| {
                feishin_core::presets::EQ
                    .iter()
                    .find(|(label, _)| *label == name)
                    .map(|(_, gains)| *gains)
            })
        {
            self.apply_eq_preset(gains, window, cx);
        }
    }

    pub(crate) fn delete_audio_preset(
        &mut self,
        compressor: bool,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if compressor {
            self.state.settings.compressor_presets.remove(name);
        } else {
            self.state.settings.equalizer_presets.remove(name);
        }
        self.save();
        self.refresh_audio_preset_options(window, cx);
        cx.notify();
    }
    pub(crate) fn init_equalizer_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (index, select) in self.audio_preset_selects.clone().iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                select,
                window,
                move |this,
                      select,
                      event: &gpui_component::select::SelectEvent<
                    gpui_component::select::SearchableVec<
                        gpui_component::select::SelectGroup<String>,
                    >,
                >,
                      window,
                      cx| {
                    if let gpui_component::select::SelectEvent::Confirm(Some(name)) = event {
                        this.select_audio_preset(index == 1, name, window, cx);
                        select.update(cx, |select, cx| select.set_selected_index(None, window, cx));
                    }
                },
            ));
        }
        for (index, slider) in self.equalizer.clone().iter().enumerate() {
            self.subscriptions.push(cx.subscribe_in(
                slider,
                window,
                move |this, _, event, window, cx| {
                    if let SliderEvent::ChangeEnd(value) = event {
                        this.set_equalizer_band(index, value.end(), window, cx);
                    }
                },
            ));
        }
        for (index, input) in self.equalizer_inputs.clone().iter().enumerate() {
            self.subscriptions
                .extend(Self::bind_number_step(input, -12., 12., 0.5, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        if let Ok(value) = input.read(cx).value().parse::<f32>()
                            && value.is_finite()
                        {
                            this.set_equalizer_band(index, value.clamp(-12., 12.), window, cx);
                        }
                        if matches!(event, InputEvent::PressEnter { .. }) {
                            window.focus(&this.focus);
                        }
                    }
                },
            ));
        }
        self.subscriptions.push(cx.subscribe_in(
            &self.preamp_slider,
            window,
            |this, _, event, window, cx| {
                let (value, commit) = match event {
                    SliderEvent::Change(value) => (value.end(), false),
                    SliderEvent::ChangeEnd(value) => (value.end(), true),
                };
                this.set_preamp(value, commit, window, cx);
            },
        ));
        for (compressor, input) in [
            (false, self.preset_name.clone()),
            (true, self.compressor_preset_name.clone()),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.save_audio_preset(compressor, window, cx);
                    } else if matches!(event, InputEvent::Change) {
                        cx.notify();
                    }
                },
            ));
        }
    }

    pub(crate) fn set_equalizer_band(
        &mut self,
        index: usize,
        value: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(-12., 12.);
        self.state.settings.equalizer[index] = value;
        self.equalizer_inputs[index].update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
        self.equalizer[index].update(cx, |slider, cx| slider.set_value(value, window, cx));
        self.apply_audio_settings(cx);
    }

    pub(crate) fn set_preamp(
        &mut self,
        value: f32,
        commit: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !value.is_finite() {
            return;
        }
        let value = value.clamp(-12., 12.);
        if self.state.settings.equalizer_preamp != value {
            self.state.settings.equalizer_preamp = value;
            self.audio_numbers[0].update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        self.preamp_slider
            .update(cx, |slider, cx| slider.set_value(value, window, cx));
        if commit {
            self.apply_audio_settings(cx);
        } else {
            self.save();
            cx.notify();
        }
    }

    pub(crate) fn equalizer_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_4()
            .when(self.state.settings.equalizer_enabled, |view| {
                view.child(self.audio_presets(false, cx))
                    .child(self.tr("setting.equalizerPreamp"))
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                            .gap_3()
                            .child(
                                div()
                                    .w(px(200.))
                                    .debug_selector(|| "equalizer-preamp-slider".into())
                                    .child(
                                        crate::styles::slider(&self.preamp_slider, cx)
                                            .aria_label(self.tr("setting.equalizerPreamp")),
                                    ),
                            )
                            .child(
                                div().w(px(70.)).child(
                                    crate::styles::number_input(&self.audio_numbers[0], cx)
                                        .hide_controls(true),
                                ),
                            )
                            .child("dB")
                            .child(
                                crate::styles::button("eq-flat", cx)
                                    .debug_selector(|| "equalizer-reset".into())
                                    .label(self.tr("common.reset"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.apply_eq_preset([0.; 12], window, cx)
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |row| row.flex_row_reverse())
                            .gap(px(2.))
                            .children(self.equalizer.iter().enumerate().map(|(index, slider)| {
                                let label = [
                                    "31.5", "63", "125", "250", "500", "1k", "2k", "3k", "4k",
                                    "6.3k", "10k", "16k",
                                ][index];
                                div()
                                    .v_flex()
                                    .items_center()
                                    .gap_1()
                                    .w(px(52.))
                                    .child(
                                        div()
                                            .w_full()
                                            .h(rems(2.25))
                                            .debug_selector(move || {
                                                format!("equalizer-number-{index}")
                                            })
                                            .child(
                                                crate::styles::number_input(
                                                    &self.equalizer_inputs[index],
                                                    cx,
                                                )
                                                .hide_controls(true),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .h(px(120.))
                                            .w(px(20.))
                                            .debug_selector(move || {
                                                format!("equalizer-band-{index}")
                                            })
                                            .child(
                                                crate::styles::slider(slider, cx)
                                                    .aria_label(format!(
                                                        "{} Hz",
                                                        feishin_core::dsp::FREQUENCIES[index]
                                                    ))
                                                    .vertical()
                                                    .h_full(),
                                            ),
                                    )
                                    .child(div().text_xs().child(label))
                            })),
                    )
            })
            .into_any_element()
    }
}
