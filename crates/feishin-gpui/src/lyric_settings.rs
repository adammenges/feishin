use super::*;
use gpui_component::switch::Switch;

pub(super) fn number_label(index: usize, cx: &App) -> String {
    let label = crate::appearance::text(LYRIC_NUMBERS[index].0, cx);
    if matches!(index, 3..=6) {
        format!(
            "{label} ({})",
            crate::appearance::text(
                if matches!(index, 3 | 5) {
                    "page.fullscreenPlayer.config.synchronized"
                } else {
                    "page.fullscreenPlayer.config.unsynchronized"
                },
                cx
            )
        )
    } else {
        label
    }
}

fn row(
    id: &'static str,
    label: String,
    description: String,
    control: AnyElement,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || format!("{id}-row"))
        .h_flex()
        .items_center()
        .gap_4()
        .py_2()
        .w_full()
        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
        .child(
            div()
                .v_flex()
                .gap_2()
                .flex_1()
                .min_w_0()
                .text_size(rems(13. / 16.))
                .child(label)
                .when(!description.is_empty(), |div| {
                    div.child(
                        gpui::div()
                            .text_size(rems(11. / 16.))
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    )
                }),
        )
        .child(div().flex_1().min_w_0().child(control))
}

fn group(key: &str, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    div()
        .v_flex()
        .w_full()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .p_3()
        .child(
            div()
                .text_sm()
                .mb_2()
                .child(crate::appearance::text(key, cx)),
        )
        .children(rows)
        .into_any_element()
}

impl Feishin {
    fn lyric_setting_row(
        &self,
        id: &'static str,
        key: &str,
        control: AnyElement,
        cx: &App,
    ) -> AnyElement {
        let description = feishin_core::i18n::translate(
            &self.state.settings.language,
            &format!("{key}_description"),
        )
        .unwrap_or_default();
        row(id, self.tr(key), description, control, cx).into_any_element()
    }

    fn lyric_boolean(
        &self,
        id: &'static str,
        key: &str,
        value: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let control = div()
            .h_flex()
            .justify_end()
            .child(
                Switch::new(id)
                    .checked(value)
                    .tooltip(self.tr(key))
                    .settings(self.state.settings.zoom, crate::appearance::rtl(cx), id)
                    .on_click(cx.listener(move |this, value, _, cx| {
                        this.set_lyric_boolean(id, *value, cx);
                    })),
            )
            .into_any_element();
        self.lyric_setting_row(id, key, control, cx)
    }

    fn set_lyric_boolean(&mut self, id: &str, value: bool, cx: &mut Context<Self>) {
        let settings = &mut self.state.settings;
        match id {
            "lyric-fetch" => settings.lyric_fetch = value,
            "lyric-prefer-local" => settings.lyric_prefer_local = value,
            "lyric-furigana" => settings.lyric_furigana = value,
            "lyric-romaji" => settings.lyric_romaji = value,
            "lyric-translation" => settings.lyric_translation = value,
            "lyric-scroll" => settings.lyric_scroll = value,
            "lyric-match" => settings.lyric_show_match = value,
            "lyric-provider" => settings.lyric_show_provider = value,
            "lyric-cloud-auto" => settings.lyric_cloud_translation.automatic = value,
            "lyric-words" => settings.lyric_words = value,
            _ => return,
        }
        match id {
            "lyric-fetch" | "lyric-prefer-local" | "lyric-translation" => {
                self.load_lyrics(true, cx)
            }
            "lyric-furigana" | "lyric-romaji" => self.annotate_lyrics(cx),
            "lyric-cloud-auto" => self.reset_lyric_translation(cx),
            "lyric-scroll" => {
                self.lyric_scroll_paused_until = None;
                self.sidebar_lyric_scroll_paused_until = None;
            }
            _ => {}
        }
        self.save();
        cx.notify();
    }

    fn lyric_numeric_control(&self, index: usize, cx: &Context<Self>) -> AnyElement {
        let label = number_label(index, cx);
        let key = match index {
            3 => "page.fullscreenPlayer.config.lyricSizeSynchronized",
            4 => "page.fullscreenPlayer.config.lyricSizeUnsynchronized",
            5 => "page.fullscreenPlayer.config.lyricGapSynchronized",
            6 => "page.fullscreenPlayer.config.lyricGapUnsynchronized",
            _ => LYRIC_NUMBERS[index].0,
        };
        let description = feishin_core::i18n::translate(
            &self.state.settings.language,
            &format!("{key}_description"),
        )
        .unwrap_or_default();
        let control = if let Some((_, state)) = self.lyric_sliders.iter().find(|(i, _)| *i == index)
        {
            let value = state.read(cx).value().end();
            let formatted = match index {
                7 | 8 => format!("{:.0}%", value * 100.),
                9 | 10 => format!("{value:.0}%"),
                3..=6 => format!("{value:.0}px"),
                _ => format!("{value:.0}"),
            };
            div()
                .w_full()
                .child(
                    crate::styles::slider(state, cx)
                        .settings(self.state.settings.zoom, "lyric-setting-slider")
                        .aria_label(label.clone())
                        .value_label(formatted),
                )
                .into_any_element()
        } else {
            crate::styles::number_input(&self.lyric_numbers[index], cx).into_any_element()
        };
        div()
            .id(("lyric-number-setting", index))
            .debug_selector(move || format!("lyric-number-{index}"))
            .child(row("lyric-number-row", label, description, control, cx))
            .into_any_element()
    }

    pub fn lyric_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = &self.state.settings;
        let mut general = vec![self.lyric_numeric_control(0, cx)];
        for (id, key, value) in [
            (
                "lyric-prefer-local",
                "setting.preferLocalLyrics",
                settings.lyric_prefer_local,
            ),
            ("lyric-fetch", "setting.lyricFetch", settings.lyric_fetch),
        ] {
            general.push(self.lyric_boolean(id, key, value, cx));
        }
        let providers = crate::styles::button("lyric-sources", cx)
            .debug_selector(|| "lyric-sources".into())
            .label(if settings.lyric_sources.is_empty() {
                self.tr("common.none")
            } else {
                settings
                    .lyric_sources
                    .iter()
                    .map(|p| p.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .dropdown_menu({
                let entity = cx.entity();
                move |mut menu, _, cx| {
                    for provider in Provider::ALL {
                        let entity = entity.clone();
                        let checked = entity
                            .read(cx)
                            .state
                            .settings
                            .lyric_sources
                            .contains(&provider);
                        menu = menu.item(
                            PopupMenuItem::new(provider.name())
                                .checked(checked)
                                .on_click(move |_, _, cx| {
                                    entity.update(cx, |this, cx| {
                                        if checked {
                                            this.state
                                                .settings
                                                .lyric_sources
                                                .retain(|p| *p != provider);
                                        } else {
                                            this.state.settings.lyric_sources.push(provider);
                                        }
                                        this.save();
                                        this.load_lyrics(true, cx);
                                        cx.notify();
                                    })
                                }),
                        );
                    }
                    let entity = entity.clone();
                    menu.item(
                        PopupMenuItem::new(crate::appearance::text("common.clear", cx)).on_click(
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.state.settings.lyric_sources.clear();
                                    this.save();
                                    this.load_lyrics(true, cx);
                                    cx.notify();
                                })
                            },
                        ),
                    )
                }
            });
        general.push(self.lyric_setting_row(
            "lyric-provider-options",
            "setting.lyricFetchProvider",
            providers.into_any_element(),
            cx,
        ));
        for (id, key, value) in [
            (
                "lyric-furigana",
                "setting.enableFurigana",
                settings.lyric_furigana,
            ),
            (
                "lyric-romaji",
                "setting.enableRomaji",
                settings.lyric_romaji,
            ),
            (
                "lyric-translation",
                "setting.neteaseTranslation",
                settings.lyric_translation,
            ),
        ] {
            general.push(self.lyric_boolean(id, key, value, cx));
        }

        let mut display = [3, 4, 5, 6, 9, 10]
            .into_iter()
            .map(|index| self.lyric_numeric_control(index, cx))
            .collect::<Vec<_>>();
        let alignment = div()
            .h_flex()
            .gap_1()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .children(["left", "center", "right"].into_iter().map(|value| {
                crate::styles::button(SharedString::from(format!("lyric-alignment-{value}")), cx)
                    .label(self.tr(&format!("common.{value}")))
                    .selected(settings.lyric_alignment == value)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.state.settings.lyric_alignment = value.into();
                        this.save();
                        cx.notify();
                    }))
            }));
        display.push(self.lyric_setting_row(
            "lyric-alignment",
            "page.fullscreenPlayer.config.lyricAlignment",
            alignment.into_any_element(),
            cx,
        ));
        display.push(self.lyric_boolean(
            "lyric-scroll",
            "page.fullscreenPlayer.config.followCurrentLyric",
            settings.lyric_scroll,
            cx,
        ));
        for index in [1, 2, 7, 8] {
            display.push(self.lyric_numeric_control(index, cx));
        }
        for (id, key, value) in [
            (
                "lyric-match",
                "page.fullscreenPlayer.config.showLyricMatch",
                settings.lyric_show_match,
            ),
            (
                "lyric-provider",
                "page.fullscreenPlayer.config.showLyricProvider",
                settings.lyric_show_provider,
            ),
            ("lyric-words", "Highlight timed words", settings.lyric_words),
        ] {
            display.push(self.lyric_boolean(id, key, value, cx));
        }

        let translation = &settings.lyric_cloud_translation;
        let mut cloud = vec![self.lyric_boolean(
            "lyric-cloud-auto",
            "setting.enableAutoTranslation",
            translation.automatic,
            cx,
        )];
        let choices: Vec<(String, String)> =
            serde_json::from_str(include_str!("../assets/lyric-language-choices.json")).unwrap();
        let language = crate::styles::button("lyric-cloud-language", cx)
            .debug_selector(|| "lyric-cloud-language".into())
            .label(
                choices
                    .iter()
                    .find(|(_, value)| *value == translation.language)
                    .map(|(label, _)| label.clone())
                    .unwrap_or_else(|| translation.language.clone()),
            )
            .dropdown_menu({
                let entity = cx.entity();
                move |mut menu, _, cx| {
                    for (label, value) in &choices {
                        let entity = entity.clone();
                        let value = value.clone();
                        let checked = entity
                            .read(cx)
                            .state
                            .settings
                            .lyric_cloud_translation
                            .language
                            == value;
                        menu =
                            menu.item(PopupMenuItem::new(label.clone()).checked(checked).on_click(
                                move |_, window, cx| {
                                    entity.update(cx, |this, cx| {
                                        this.state.settings.lyric_cloud_translation.language =
                                            value.clone();
                                        this.lyric_translation_language.update(cx, |input, cx| {
                                            input.set_value(value.clone(), window, cx)
                                        });
                                        this.save();
                                        this.reset_lyric_translation(cx);
                                        cx.notify();
                                    })
                                },
                            ));
                    }
                    menu
                }
            });
        cloud.push(self.lyric_setting_row(
            "lyric-translation-language",
            "setting.translationTargetLanguage",
            language.into_any_element(),
            cx,
        ));
        let provider = crate::styles::button("lyric-cloud-provider", cx)
            .debug_selector(|| "lyric-cloud-provider".into())
            .label(if translation.provider.is_empty() {
                self.tr("common.none")
            } else {
                translation.provider.clone()
            })
            .dropdown_menu({
                let entity = cx.entity();
                move |mut menu, _, cx| {
                    for value in ["", "Microsoft Azure", "Google Cloud"] {
                        let entity = entity.clone();
                        let checked = entity
                            .read(cx)
                            .state
                            .settings
                            .lyric_cloud_translation
                            .provider
                            == value;
                        menu = menu.item(
                            PopupMenuItem::new(if value.is_empty() {
                                crate::appearance::text("common.none", cx)
                            } else {
                                value.into()
                            })
                            .checked(checked)
                            .on_click(move |_, _, cx| {
                                entity.update(cx, |this, cx| {
                                    this.state.settings.lyric_cloud_translation.provider =
                                        value.into();
                                    this.save();
                                    this.reset_lyric_translation(cx);
                                    cx.notify();
                                })
                            }),
                        );
                    }
                    menu
                }
            });
        cloud.push(self.lyric_setting_row(
            "lyric-translation-provider",
            "setting.translationApiProvider",
            provider.into_any_element(),
            cx,
        ));
        cloud.push(
            self.lyric_setting_row(
                "lyric-translation-key",
                "setting.translationApiKey",
                crate::styles::input(&self.lyric_translation_key, cx)
                    .mask_toggle()
                    .into_any_element(),
                cx,
            ),
        );
        if translation.provider == "Microsoft Azure" {
            cloud.push(self.lyric_setting_row(
                "lyric-translation-region",
                "Azure region (optional)",
                crate::styles::input(&self.lyric_translation_region, cx).into_any_element(),
                cx,
            ));
        }
        cloud.push(
            crate::styles::button("lyric-cloud-save", cx)
                .label(self.tr("common.save"))
                .on_click(
                    cx.listener(|this, _, window, cx| this.save_translation_settings(window, cx)),
                )
                .into_any_element(),
        );
        div()
            .v_flex()
            .gap_3()
            .w_full()
            .child(
                crate::styles::button("lyrics-display-surface", cx)
                    .debug_selector(|| "lyrics-display-surface".into())
                    .label(format!(
                        "{}: {}",
                        self.tr("page.setting.lyricsDisplay"),
                        self.tr(if self.lyric_settings_key == "sidebar" {
                            "Sidebar"
                        } else {
                            "Player"
                        })
                    ))
                    .on_click(cx.listener(|this, _, window, cx| {
                        let key = if this.lyric_settings_key == "default" {
                            "sidebar"
                        } else {
                            "default"
                        };
                        this.set_lyric_settings_key(key, window, cx);
                    })),
            )
            .child(group("page.setting.lyrics", general, cx))
            .child(group("page.setting.lyricsDisplay", display, cx))
            .child(group("page.setting.lyricsTranslation", cloud, cx))
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "lyric_settings_tests.rs"]
mod tests;
