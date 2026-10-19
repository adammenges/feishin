#[path = "lyrics_actions.rs"]
pub(crate) mod actions;
#[path = "lyric_search.rs"]
mod search;
#[path = "lyric_settings.rs"]
mod settings;

use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    lyric_sources::{self, Match, Provider, Record, Sources},
    lyrics,
    model::{Item, ItemKind},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, StyledExt,
    button::ButtonVariants,
    input::{InputEvent, NumberInputEvent, StepAction},
    menu::{DropdownMenu, PopupMenuItem},
    slider::{SliderEvent, SliderState},
};

pub const LYRIC_NUMBERS: [(&str, f64, f64, f64); 11] = [
    (
        "setting.lyricOffset",
        -9_007_199_254_740_991.,
        9_007_199_254_740_991.,
        10.,
    ),
    (
        "page.fullscreenPlayer.config.lyricFollowScrollAlignment",
        -50.,
        50.,
        1.,
    ),
    (
        "page.fullscreenPlayer.config.lyricLineLeadTime",
        0.,
        3000.,
        50.,
    ),
    ("page.fullscreenPlayer.config.lyricSize", 8., 72., 1.),
    ("page.fullscreenPlayer.config.lyricSize", 8., 72., 1.),
    ("page.fullscreenPlayer.config.lyricGap", 0., 50., 1.),
    ("page.fullscreenPlayer.config.lyricGap", 0., 50., 1.),
    (
        "page.fullscreenPlayer.config.lyricOpacityNonActive",
        0.,
        1.,
        0.01,
    ),
    (
        "page.fullscreenPlayer.config.lyricScaleNonActive",
        0.5,
        1.,
        0.01,
    ),
    ("page.fullscreenPlayer.config.lyricPaddingLeft", 0., 50., 1.),
    (
        "page.fullscreenPlayer.config.lyricPaddingRight",
        0.,
        50.,
        1.,
    ),
];

#[derive(Default)]
pub struct Selection {
    override_generation: Option<uuid::Uuid>,
    index: Option<usize>,
    offset: Option<f64>,
    variant_offsets: std::collections::HashMap<usize, f64>,
}

impl Feishin {
    fn lyric_number_values(&self) -> [f64; 11] {
        let settings = &self.state.settings;
        let display = settings.lyric_display_for(&self.lyric_settings_key);
        [
            settings.lyric_offset * 1000.,
            settings.lyric_follow_alignment as f64,
            settings.lyric_lead_time * 1000.,
            display.font_size as f64,
            display.font_size_unsync as f64,
            display.gap as f64,
            display.gap_unsync as f64,
            display.opacity_non_active as f64,
            display.scale_non_active as f64,
            display.padding_left as f64,
            display.padding_right as f64,
        ]
    }
    pub fn set_lyric_settings_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.lyric_settings_key = key.into();
        let values = self.lyric_number_values();
        for (input, value) in self.lyric_numbers.iter().zip(values) {
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
        }
        for (index, slider) in &self.lyric_sliders {
            slider.update(cx, |slider, cx| {
                slider.cancel_pointer_change();
                slider.set_value(values[*index] as f32, window, cx);
            });
        }
        cx.notify();
    }
    pub fn init_lyric_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for offset_input in [
            self.lyric_song_offset.clone(),
            self.lyric_sidebar_offset.clone(),
        ] {
            self.subscriptions.push(cx.subscribe_in(
                &offset_input,
                window,
                |this, input, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let value = input.read(cx).value().parse::<f64>().unwrap_or(0.)
                        + if *action == StepAction::Increment {
                            1.
                        } else {
                            -1.
                        };
                    this.set_song_lyric_offset(value, cx);
                    input.update(cx, |input, cx| {
                        input.set_value((this.lyric_offset() * 1000.).to_string(), window, cx)
                    });
                },
            ));
            self.subscriptions.push(cx.subscribe_in(
                &offset_input,
                window,
                |this, input, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        let text = input.read(cx).value();
                        let value = if text.trim().is_empty() {
                            Some(0.)
                        } else {
                            text.parse::<f64>().ok()
                        };
                        if let Some(value) = value
                            && (value / 1000. - this.lyric_offset()).abs() > f64::EPSILON
                        {
                            this.set_song_lyric_offset(value, cx);
                        }
                    }
                },
            ));
        }
        let values = self.lyric_number_values();
        for (index, value) in values.into_iter().enumerate() {
            let input = cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .aria_label(LYRIC_NUMBERS[index].0)
                    .label_transform(move |_, cx| settings::number_label(index, cx).into())
            });
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.apply_lyric_number(index, cx);
                    }
                },
            ));
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let (_, min, max, step) = LYRIC_NUMBERS[index];
                    let input = this.lyric_numbers[index].clone();
                    let value = input.read(cx).value().parse::<f64>().unwrap_or(min);
                    let value = (value
                        + if *action == StepAction::Increment {
                            step
                        } else {
                            -step
                        })
                    .clamp(min, max);
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                    this.apply_lyric_number(index, cx);
                },
            ));
            self.lyric_numbers.push(input);
            if !matches!(index, 0 | 2) {
                let (_, min, max, step) = LYRIC_NUMBERS[index];
                let slider = cx.new(|_| {
                    SliderState::new()
                        .min(min as f32)
                        .max(max as f32)
                        .step(step as f32)
                        .default_value(value as f32)
                });
                self.subscriptions.push(cx.subscribe_in(
                    &slider,
                    window,
                    move |this, _, event: &SliderEvent, window, cx| {
                        if let SliderEvent::ChangeEnd(value) = event {
                            this.lyric_numbers[index].update(cx, |input, cx| {
                                input.set_value(value.end().to_string(), window, cx);
                            });
                            this.apply_lyric_number(index, cx);
                        }
                        cx.notify();
                    },
                ));
                self.lyric_sliders.push((index, slider));
            }
        }
    }
    pub fn apply_lyric_number(&mut self, index: usize, cx: &mut Context<Self>) {
        let (_, min, max, _) = LYRIC_NUMBERS[index];
        let raw = self.lyric_numbers[index].read(cx).value();
        let Ok(value) = (if raw.trim().is_empty() {
            Ok(0.)
        } else {
            raw.parse::<f64>()
        }) else {
            self.message = "Enter a valid lyric setting".into();
            cx.notify();
            return;
        };
        if !(min..=max).contains(&value) {
            self.message = format!("Value must be between {min} and {max}");
            cx.notify();
            return;
        }
        let settings = &mut self.state.settings;
        if index < 3 {
            match index {
                0 => settings.lyric_offset = value / 1000.,
                1 => settings.lyric_follow_alignment = value as f32,
                2 => settings.lyric_lead_time = value / 1000.,
                _ => unreachable!(),
            }
        } else {
            let display = if self.lyric_settings_key == "default" {
                &mut settings.lyric_display
            } else {
                settings
                    .lyric_displays
                    .entry(self.lyric_settings_key.clone())
                    .or_insert_with(|| settings.lyric_display.clone())
            };
            match index {
                3 => display.font_size = value as f32,
                4 => display.font_size_unsync = value as f32,
                5 => display.gap = value as f32,
                6 => display.gap_unsync = value as f32,
                7 => display.opacity_non_active = value as f32,
                8 => display.scale_non_active = value as f32,
                9 => display.padding_left = value as f32,
                10 => display.padding_right = value as f32,
                _ => unreachable!(),
            }
        }
        self.lyric_scroll_paused_until = None;
        self.sidebar_lyric_scroll_paused_until = None;
        self.save();
        cx.notify();
    }

    pub(crate) fn pause_lyric_follow(&mut self, sidebar: bool, cx: &App) {
        let scroll = if sidebar {
            &self.sidebar_lyric_scroll
        } else {
            &self.lyric_scroll
        };
        self.lyric_scroll_motion[usize::from(sidebar)].cancel(f32::from(scroll.offset().y));
        let until = Some(cx.background_executor().now() + crate::lyric_scroll::MANUAL_PAUSE);
        if sidebar {
            self.sidebar_lyric_scroll_paused_until = until;
        } else {
            self.lyric_scroll_paused_until = until;
        }
    }

    pub fn annotate_lyrics(&mut self, cx: &mut Context<Self>) {
        self.annotation_generation += 1;
        self.lyric_annotations.clear();
        if !(self.state.settings.lyric_furigana || self.state.settings.lyric_romaji)
            || self.lyrics.is_empty()
        {
            return;
        }
        let generation = self.annotation_generation;
        let lines = self.lyrics.clone();
        let task = cx
            .background_executor()
            .spawn(async move { feishin_core::japanese::annotate(&lines) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.annotation_generation {
                    return;
                }
                match result {
                    Ok(annotations) => this.lyric_annotations = annotations,
                    Err(error) => {
                        log::warn!("Japanese lyric conversion failed: {error}");
                        this.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn lyric_line(
        &self,
        line: &lyrics::Line,
        index: usize,
        key: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let annotation = self.lyric_annotations.get(index);
        let line_end = self
            .lyrics
            .get(index + 1)
            .and_then(|line| line.time)
            .or_else(|| {
                self.current_entry()
                    .map(|entry| entry.item.duration + self.lyric_offset())
                    .filter(|end| *end > 0.)
            });
        let agents = self
            .lyric_record
            .as_ref()
            .and_then(|record| record.variants.get(record.selected))
            .map(|variant| variant.agents.as_slice())
            .unwrap_or_default();
        let ruby = self.state.settings.lyric_furigana
            && annotation.is_some_and(|line| !line.spans.is_empty());
        let karaoke = self.state.settings.lyric_words
            && self.lyrics.iter().any(|line| {
                !line.words.is_empty() || line.cue_lines.iter().any(|cue| !cue.words.is_empty())
            });
        let main = if !line.cue_lines.is_empty() {
            self.lyric_cues(
                line,
                (index, false, key),
                agents,
                (line_end, self.lyric_position()),
                annotation,
                cx,
            )
        } else if ruby {
            self.lyric_ruby(annotation.unwrap(), &line.words, line_end, key, cx)
        } else if self.state.settings.lyric_words && !line.words.is_empty() {
            self.lyric_words(
                &line.words,
                line_end,
                self.lyric_position(),
                (
                    self.state
                        .settings
                        .lyric_display_for(key)
                        .opacity_non_active,
                    false,
                ),
                cx,
            )
        } else {
            gpui::div()
                .v_flex()
                .gap_0()
                .children(line.text.split("_BREAK_").enumerate().map(|(index, text)| {
                    gpui::div()
                        .id(("lyric-main-part", index))
                        .child(crate::lyrics_markup::Text(text.into()))
                }))
                .into_any_element()
        };
        let mut result = gpui::div()
            .debug_selector(move || format!("lyric-annotation-{index}"))
            .min_w_0()
            .w_full()
            .v_flex()
            .gap_0()
            .child(gpui::div().id("lyric-main").min_w_0().w_full().child(main));
        if let Some(record) = &self.lyric_record {
            for (layer, variant) in record.variants.iter().enumerate().filter(|(_, variant)| {
                variant.synchronized
                    && variant.kind != "main"
                    && record
                        .variants
                        .get(record.selected)
                        .is_none_or(|selected| !std::ptr::eq(*variant, selected))
                    && self
                        .state
                        .settings
                        .lyric_overlay_layers
                        .contains(&format!("{}:{}", variant.kind, variant.language))
            }) {
                let shift = variant.offset.unwrap_or_default()
                    - record
                        .variants
                        .get(record.selected)
                        .and_then(|variant| variant.offset)
                        .unwrap_or_default();
                if let Some(overlay) =
                    lyrics::overlay_line(&variant.lines, line.time.map(|time| time + shift), index)
                {
                    let text = self.lyric_cues(
                        overlay,
                        (index, true, key),
                        &variant.agents,
                        (
                            line_end.map(|end| end + shift),
                            self.lyric_position() + shift,
                        ),
                        None,
                        cx,
                    );
                    result = result.child(
                        gpui::div()
                            .id(("lyric-overlay", layer))
                            .debug_selector(move || format!("lyric-overlay-{index}"))
                            .text_size(px(self.lyric_font_size(key)
                                * if variant.kind == "translation" {
                                    0.85
                                } else {
                                    0.8
                                }))
                            .font_weight(
                                if matches!(variant.kind.as_str(), "pronunciation" | "romaji") {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::MEDIUM
                                },
                            )
                            .text_wrap(TextWrap::BreakWord)
                            .child(text),
                    );
                }
            }
        }
        if self.state.settings.lyric_romaji
            && line.cue_lines.is_empty()
            && let Some(annotation) = annotation.filter(|line| !line.romaji.is_empty())
        {
            result = result.child(
                gpui::div()
                    .text_size(px(self.lyric_font_size(key) * 0.8))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.lyric_romaji(annotation, &line.words, line_end, key, cx)),
            );
        }
        if self.lyric_translation_visible
            && let Some(text) = self
                .lyric_translated
                .get(index)
                .filter(|text| !text.is_empty() && **text != line.text)
        {
            result = result.child(
                gpui::div()
                    .text_size(px(
                        self.lyric_font_size(key) * if karaoke { 0.85 } else { 1. }
                    ))
                    .font_weight(if karaoke {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::SEMIBOLD
                    })
                    .text_wrap(TextWrap::BreakWord)
                    .child(crate::lyrics_markup::Text(text.clone())),
            );
        }
        result.into_any_element()
    }
    fn lyric_font_size(&self, key: &str) -> f32 {
        if key == "preview" {
            return 20. * self.state.settings.zoom;
        }
        let display = self.state.settings.lyric_display_for(key);
        (if self.lyrics.iter().any(|line| line.time.is_some()) {
            display.font_size
        } else {
            display.font_size_unsync
        }) * self.state.settings.zoom
    }
    pub(crate) fn lyric_information(&self) -> Vec<String> {
        let matched = self
            .lyric_record
            .as_ref()
            .and_then(|record| record.matched.as_ref());
        let mut rows = vec![];
        if self.state.settings.lyric_show_provider {
            rows.push(matched.map_or_else(
                || "Server or local lyrics".into(),
                |matched| matched.provider.name().into(),
            ));
        }
        if self.state.settings.lyric_show_match {
            if let Some(matched) = matched {
                rows.push(format!("{} \u{2014} {}", matched.name, matched.artist));
            } else if let Some(item) = self.current_metadata() {
                rows.push(format!("{} \u{2014} {}", item.name, item.artist));
            }
        }
        rows
    }
    pub(crate) fn seek_lyric(&mut self, time: f64, cx: &mut Context<Self>) {
        if !time.is_finite() || time < 0. {
            return;
        }
        self.lyric_scroll_paused_until = None;
        self.sidebar_lyric_scroll_paused_until = None;
        for (motion, scroll) in self
            .lyric_scroll_motion
            .iter_mut()
            .zip([&self.lyric_scroll, &self.sidebar_lyric_scroll])
        {
            motion.cancel(f32::from(scroll.offset().y));
        }
        self.commit_seek(time, cx);
    }
    pub fn lyric_position(&self) -> f64 {
        self.current_position()
            + self.lyric_offset()
            + if self.playing && self.loaded && !self.buffering {
                let (interval, speed) = if self.state.settings.jukebox {
                    (1., 1.)
                } else {
                    (0.25, self.state.settings.speed as f64)
                };
                self.lyric_position_at.elapsed().as_secs_f64().min(interval) * speed
            } else {
                0.
            }
    }
    fn lyric_words(
        &self,
        words: &[lyrics::Word],
        end: Option<f64>,
        position: f64,
        display: (f32, bool),
        cx: &Context<Self>,
    ) -> AnyElement {
        let (opacity, overlay) = display;
        let words = if overlay {
            words.to_vec()
        } else {
            lyrics::split_word_cues(words, end)
        };
        let mut base = cx.theme().foreground;
        base.a = opacity;
        gpui::div()
            .h_flex()
            .min_w_0()
            .w_full()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .flex_wrap()
            .when(self.state.settings.lyric_alignment == "center", |div| {
                div.justify_center()
            })
            .when(self.state.settings.lyric_alignment == "right", |div| {
                div.justify_end()
            })
            .children(words.iter().enumerate().map(|(index, word)| {
                let end = word
                    .end
                    .or_else(|| words.get(index + 1).map(|word| word.time))
                    .or(end)
                    .unwrap_or(word.time);
                crate::lyrics_highlight::Word {
                    key: format!("{index}-{:.3}-{}", word.time, word.text).into(),
                    text: word.text.clone(),
                    position,
                    start: word.time,
                    duration: (end - word.time).max(0.),
                    playing: self.playing,
                    base,
                    foreground: cx.theme().foreground,
                    zoom: self.state.settings.zoom,
                }
            }))
            .into_any_element()
    }
    fn lyric_cues(
        &self,
        line: &lyrics::Line,
        selection: (usize, bool, &str),
        agents: &[lyrics::Agent],
        clock: (Option<f64>, f64),
        annotation: Option<&feishin_core::japanese::Annotation>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (index, overlay, key) = selection;
        let (end, position) = clock;
        if line.cue_lines.is_empty() {
            if self.state.settings.lyric_words && !line.words.is_empty() {
                return self.lyric_words(
                    &line.words,
                    end,
                    position,
                    (
                        self.state
                            .settings
                            .lyric_display_for(key)
                            .opacity_non_active,
                        overlay,
                    ),
                    cx,
                );
            }
            return gpui::div()
                .child(crate::lyrics_markup::Text(line.text.clone()))
                .into_any_element();
        }
        gpui::div()
            .v_flex()
            .text_wrap(TextWrap::BreakWord)
            .gap_0()
            .children(line.cue_lines.iter().enumerate().map(|(cue_index, cue)| {
                let agent = agents
                    .iter()
                    .find(|agent| Some(agent.id.as_str()) == cue.agent_id.as_deref());
                let background =
                    agent.is_some_and(|agent| matches!(agent.role.as_str(), "bg" | "group"));
                let annotation = annotation.and_then(|annotation| annotation.cues.get(cue_index));
                let cue_start = cue.start;
                gpui::div()
                    .id(("lyric-cue", cue_index))
                    .debug_selector(move || format!("lyric-cue-{index}-{cue_index}"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.seek_lyric(cue_start, cx);
                        cx.stop_propagation();
                    }))
                    .when(background && !overlay, |div| {
                        div.min_h(px(self.lyric_font_size(key) * 1.2))
                            .text_size(px(self.lyric_font_size(key) * 0.675))
                            .font_weight(FontWeight::NORMAL)
                    })
                    .child(
                        if self.state.settings.lyric_furigana
                            && annotation.is_some_and(|annotation| !annotation.spans.is_empty())
                        {
                            self.lyric_ruby(
                                annotation.unwrap(),
                                &cue.words,
                                cue.end.or(end),
                                key,
                                cx,
                            )
                        } else if cue.words.is_empty() || !self.state.settings.lyric_words {
                            gpui::div()
                                .child(crate::lyrics_markup::Text(cue.text.clone()))
                                .into_any_element()
                        } else {
                            self.lyric_words(
                                &cue.words,
                                cue.end.or(end),
                                position,
                                (
                                    self.state
                                        .settings
                                        .lyric_display_for(key)
                                        .opacity_non_active,
                                    overlay,
                                ),
                                cx,
                            )
                        },
                    )
                    .when(self.state.settings.lyric_romaji, |div| {
                        div.when_some(
                            annotation.filter(|annotation| !annotation.romaji.is_empty()),
                            |div, annotation| {
                                div.child(
                                    gpui::div()
                                        .debug_selector(move || {
                                            format!("lyric-cue-romaji-{index}-{cue_index}")
                                        })
                                        .text_size(px(self.lyric_font_size(key) * 0.8))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(self.lyric_romaji(
                                            annotation,
                                            &cue.words,
                                            cue.end.or(end),
                                            key,
                                            cx,
                                        )),
                                )
                            },
                        )
                    })
            }))
            .into_any_element()
    }
    fn lyric_ruby(
        &self,
        annotation: &feishin_core::japanese::Annotation,
        source: &[lyrics::Word],
        end: Option<f64>,
        key: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        let words = feishin_core::japanese::furigana_words(annotation, source);
        if self.state.settings.lyric_words && !source.is_empty() {
            self.lyric_words(
                &words,
                end,
                self.lyric_position(),
                (
                    self.state
                        .settings
                        .lyric_display_for(key)
                        .opacity_non_active,
                    false,
                ),
                cx,
            )
        } else {
            gpui::div()
                .child(crate::lyrics_markup::Text(
                    words.iter().map(|word| word.text.as_str()).collect(),
                ))
                .into_any_element()
        }
    }
    fn lyric_romaji(
        &self,
        annotation: &feishin_core::japanese::Annotation,
        source: &[lyrics::Word],
        end: Option<f64>,
        key: &str,
        cx: &Context<Self>,
    ) -> AnyElement {
        if self.state.settings.lyric_words
            && let Some(words) = feishin_core::japanese::romaji_words(annotation, source)
        {
            gpui::div()
                .id("lyric-romaji-words")
                .child(
                    self.lyric_words(
                        &words,
                        end,
                        self.lyric_position(),
                        (
                            self.state
                                .settings
                                .lyric_display_for(key)
                                .opacity_non_active,
                            true,
                        ),
                        cx,
                    ),
                )
                .into_any_element()
        } else {
            gpui::div()
                .child(crate::lyrics_markup::Text(
                    annotation
                        .romaji
                        .iter()
                        .map(|span| span.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                ))
                .into_any_element()
        }
    }
    pub fn reset_lyric_translation(&mut self, cx: &mut Context<Self>) {
        self.lyric_translation_generation += 1;
        self.lyric_translation_busy = false;
        self.lyric_translated.clear();
        self.lyric_translation_visible = false;
        if self.state.settings.lyric_cloud_translation.automatic
            && !self
                .state
                .settings
                .lyric_cloud_translation
                .provider
                .is_empty()
            && !self.state.settings.private_mode
        {
            self.translate_lyrics(cx);
        }
    }
    pub fn translate_lyrics(&mut self, cx: &mut Context<Self>) {
        if self.state.settings.private_mode || self.lyrics.is_empty() {
            return;
        }
        let config = self.state.settings.lyric_cloud_translation.clone();
        if let Err(error) = config.validate() {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        if config.provider.is_empty() {
            self.message = "Choose a cloud translation provider in Settings".into();
            cx.notify();
            return;
        }
        let entered_key = self.lyric_translation_key.read(cx).value().to_string();
        let lines = self
            .lyrics
            .iter()
            .map(|line| line.text.clone())
            .collect::<Vec<_>>();
        self.lyric_translation_generation += 1;
        let generation = self.lyric_translation_generation;
        self.lyric_translation_busy = true;
        let task = cx.background_executor().spawn(async move {
            let key = if entered_key.is_empty() {
                feishin_core::storage::load_translation_key(&config.provider)?
            } else {
                entered_key
            };
            feishin_core::translation::translate(&config, &key, &lines)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.lyric_translation_generation {
                    return;
                }
                this.lyric_translation_busy = false;
                if this.state.settings.private_mode {
                    this.lyric_translated.clear();
                    this.lyric_translation_visible = false;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(lines) => {
                        this.lyric_translated = lines;
                        this.lyric_translation_visible = true;
                    }
                    Err(error) => {
                        log::debug!("Lyric translation failed: {error}");
                        this.message = format!("Translation: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn save_translation_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut config = self.state.settings.lyric_cloud_translation.clone();
        config.language = self
            .lyric_translation_language
            .read(cx)
            .value()
            .trim()
            .to_owned();
        config.azure_region = self
            .lyric_translation_region
            .read(cx)
            .value()
            .trim()
            .to_owned();
        if let Err(error) = config.validate() {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        let key = self.lyric_translation_key.read(cx).value().to_string();
        self.state.settings.lyric_cloud_translation = config.clone();
        self.save();
        if key.is_empty() {
            self.reset_lyric_translation(cx);
            cx.notify();
            return;
        }
        let task = cx.background_executor().spawn(async move {
            feishin_core::storage::store_translation_key(&config.provider, &key)
        });
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            handle
                .update(cx, |_, window, cx| {
                    this.update(cx, |this, cx| {
                        match result {
                            Ok(()) => {
                                this.lyric_translation_key
                                    .update(cx, |input, cx| input.set_value("", window, cx));
                                this.message = "Translation credentials saved".into();
                                this.reset_lyric_translation(cx);
                            }
                            Err(error) => this.message = error.to_string(),
                        }
                        cx.notify();
                    })
                })
                .ok();
        })
        .detach();
    }
    pub fn lyric_offset(&self) -> f64 {
        self.lyric_record
            .as_ref()
            .and_then(|record| record.variants.get(record.selected))
            .and_then(|variant| variant.offset)
            .or_else(|| {
                self.lyric_key()
                    .and_then(|key| self.lyric_selections.get(&key)?.offset)
            })
            .unwrap_or(self.state.settings.lyric_offset)
    }
    fn lyric_key(&self) -> Option<(uuid::Uuid, String)> {
        self.current_entry()
            .map(|entry| (entry.item.server_id, entry.item.id.clone()))
    }
    fn set_song_lyric_offset(&mut self, milliseconds: f64, cx: &mut Context<Self>) {
        if !milliseconds.is_finite() || milliseconds.abs() > 9_007_199_254_740_991. {
            return;
        }
        let Some(key) = self.lyric_key() else {
            return;
        };
        let offset = milliseconds / 1000.;
        let selection = self.lyric_selections.entry(key).or_default();
        selection.offset = Some(offset);
        if let Some(record) = &mut self.lyric_record
            && let Some(variant) = record.variants.get_mut(record.selected)
        {
            variant.offset = Some(offset);
            selection.variant_offsets.insert(record.selected, offset);
        }
        self.lyric_scroll_paused_until = None;
        self.sidebar_lyric_scroll_paused_until = None;
        self.update_lyric_follow(self.current_position(), cx);
        cx.notify();
    }
    pub(crate) fn sync_song_lyric_offset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.lyric_key();
        let changed = self.lyric_offset_target != key;
        self.lyric_offset_target = key;
        let value = self.lyric_offset() * 1000.;
        for state in [&self.lyric_song_offset, &self.lyric_sidebar_offset] {
            let input = state.read(cx);
            let parsed = input.value().parse::<f64>().ok();
            // Keep an unfinished sign while typing, but refresh valid text when an
            // asynchronous lyric load or language change replaces its offset.
            if (changed || parsed.is_some() || !input.focus_handle(cx).is_focused(window))
                && parsed != Some(value)
            {
                state.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
            }
        }
    }
    fn restore_lyric_selection(&mut self, item: &Item, record: &mut Record, refresh: bool) {
        let Some(selection) = self
            .lyric_selections
            .get_mut(&(item.server_id, item.id.clone()))
        else {
            return;
        };
        if refresh {
            selection.variant_offsets.clear();
        }
        if let Some(index) = selection.index {
            record.selected = index.min(record.variants.len().saturating_sub(1));
        }
        for (index, offset) in &selection.variant_offsets {
            if let Some(variant) = record.variants.get_mut(*index) {
                variant.offset = Some(*offset);
            }
        }
        if !record.variants.is_empty() {
            record.raw = lyrics::render_lrc(&record.lines());
        }
    }
    pub fn update_lyric_follow(&mut self, position: f64, cx: &mut Context<Self>) {
        self.lyric_active = lyrics::active_line(&self.lyrics, position + self.lyric_offset());
        if !self.state.settings.lyric_scroll {
            return;
        }
        let Some(index) = lyrics::follow_line(
            &self.lyrics,
            position + self.lyric_offset(),
            self.state.settings.lyric_lead_time,
        ) else {
            return;
        };
        let information_count = self.lyric_information().len();
        let ratio = lyrics::follow_ratio(self.state.settings.lyric_follow_alignment);
        let now = cx.background_executor().now();
        for (site, (scroll, paused)) in [
            (&self.lyric_scroll, &mut self.lyric_scroll_paused_until),
            (
                &self.sidebar_lyric_scroll,
                &mut self.sidebar_lyric_scroll_paused_until,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            if paused.is_some_and(|until| until >= now) {
                continue;
            }
            if paused.take().is_some() {
                self.lyric_scroll_motion[site].cancel(f32::from(scroll.offset().y));
            }
            let viewport = scroll.bounds();
            let Some(line) = scroll.bounds_for_item(index + 1 + information_count) else {
                continue;
            };
            if viewport.size.height <= px(0.) {
                continue;
            }
            let line_top = line.top() - viewport.top();
            let target = (line_top + line.size.height / 2. - viewport.size.height * ratio)
                .min(line_top)
                .max(line_top - viewport.size.height + line.size.height)
                .max(px(0.))
                .min(scroll.max_offset().height);
            let old = scroll.offset();
            let motion = &mut self.lyric_scroll_motion[site];
            motion.request(index, -f32::from(target), f32::from(old.y), now);
            scroll.set_offset(point(old.x, px(motion.tick(now).0)));
        }
    }
    pub fn animate_lyric_scroll(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = cx.background_executor().now();
        for (motion, scroll) in self
            .lyric_scroll_motion
            .iter_mut()
            .zip([&self.lyric_scroll, &self.sidebar_lyric_scroll])
        {
            let offset = scroll.offset();
            if !self.state.settings.lyric_scroll {
                motion.cancel(f32::from(offset.y));
            }
            if !motion.active() {
                continue;
            }
            let (value, animating) = motion.tick(now);
            scroll.set_offset(point(offset.x, px(value)));
            if animating {
                window.request_animation_frame();
            }
        }
    }
    pub fn select_lyric_variant(&mut self, index: usize, cx: &mut Context<Self>) {
        let key = self.lyric_key();
        if let Some(record) = &mut self.lyric_record
            && index < record.variants.len()
        {
            record.selected = index;
            if let Some(key) = key {
                let selection = self.lyric_selections.entry(key).or_default();
                selection.index = Some(index);
                selection.offset = Some(
                    record.variants[index]
                        .offset
                        .or(selection.offset)
                        .unwrap_or(self.state.settings.lyric_offset),
                );
            }
            self.lyrics = record.lines();
            self.lyric_scroll_motion = Default::default();
            record.raw = lyrics::render_lrc(&self.lyrics);
            self.lyric_scroll_paused_until = None;
            self.sidebar_lyric_scroll_paused_until = None;
            self.annotate_lyrics(cx);
            self.reset_lyric_translation(cx);
            cx.notify();
        }
    }
    fn clear_lyrics(&mut self, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        let Some(item) = self.current_entry().map(|entry| entry.item.clone()) else {
            return;
        };
        let path = lyric_sources::cache_path(&self.data_dir, &item, true);
        let result = lyric_sources::read_record(&path).and_then(|record| {
            // The original Clear action discards remote choices, retaining local lyrics.
            if record.is_some_and(|record| record.matched.is_some()) {
                std::fs::remove_file(path)?;
            }
            Ok(())
        });
        if let Err(error) = result {
            log::warn!("Could not clear lyric selection: {error}");
            self.message = format!("Lyrics: {error}");
            cx.notify();
            return;
        }
        self.lyric_selections
            .entry((item.server_id, item.id.clone()))
            .or_default()
            .override_generation = None;
        self.lyric_remote_suppressed
            .insert((item.server_id, item.id));
        self.load_lyrics(true, cx);
        cx.notify();
    }

    pub fn load_lyrics(&mut self, refresh: bool, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        self.lyric_generation += 1;
        let generation = self.lyric_generation;
        let Some(entry) = self
            .state
            .queue
            .current()
            .cloned()
            .filter(|entry| entry.item.kind == ItemKind::Song)
        else {
            self.lyric_busy = false;
            return;
        };
        let Some(api) = self.sessions.get(&entry.item.server_id).cloned() else {
            self.lyric_busy = false;
            return;
        };
        let root = self.data_dir.clone();
        let mut providers = self.state.settings.lyric_sources.clone();
        if self.state.settings.private_mode {
            providers.clear();
        }
        let options = lyric_sources::LoadOptions {
            providers,
            translation: self.state.settings.lyric_translation,
            fetch: self.state.settings.lyric_fetch
                && !self
                    .lyric_remote_suppressed
                    .contains(&(entry.item.server_id, entry.item.id.clone())),
            prefer_local: self.state.settings.lyric_prefer_local,
        };
        self.lyric_busy = true;
        let revision = self.state.cache_revision;
        let stale_path = lyric_sources::cache_path_revision(&root, &entry.item, revision);
        let item = entry.item.clone();
        let task = cx.background_executor().spawn(async move {
            lyric_sources::load_with_cache_revision(
                &root,
                &entry.item,
                &options,
                refresh,
                revision,
                || api.server_lyric_variants(&entry.item),
            )
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if this.lyric_generation != generation {
                    return;
                }
                this.lyric_busy = false;
                match result {
                    Ok(mut record) => {
                        if let Some(record) = &mut record {
                            this.restore_lyric_selection(&item, record, refresh);
                        }
                        this.lyrics = record.as_ref().map(Record::lines).unwrap_or_default();
                        this.lyric_record = record;
                        this.lyric_scroll = ScrollHandle::default();
                        this.sidebar_lyric_scroll = ScrollHandle::default();
                        this.lyric_active = None;
                        this.lyric_scroll_motion = Default::default();
                        this.lyric_scroll_paused_until = None;
                        this.sidebar_lyric_scroll_paused_until = None;
                        this.annotate_lyrics(cx);
                        this.reset_lyric_translation(cx);
                    }
                    Err(error) => {
                        log::debug!("Lyrics unavailable: {error}");
                        this.message = format!("Lyrics: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn apply_lyric_match(&mut self, item: Item, matched: Match, cx: &mut Context<Self>) {
        if self.state.settings.private_mode || self.cache_busy {
            self.message =
                "External lyrics are unavailable during private mode or cache maintenance".into();
            cx.notify();
            return;
        }
        let translation = self.state.settings.lyric_translation;
        let generation = uuid::Uuid::new_v4();
        let key = (item.server_id, item.id.clone());
        self.lyric_selections
            .entry(key.clone())
            .or_default()
            .override_generation = Some(generation);
        if self.lyric_key().as_ref() == Some(&key) {
            self.lyric_generation += 1;
            self.lyric_busy = true;
        }
        let task = cx
            .background_executor()
            .spawn(async move { Sources::new()?.get(&matched, translation) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.finish_lyric_match(&item, generation, result, cx)
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn finish_lyric_match(
        &mut self,
        item: &Item,
        generation: uuid::Uuid,
        result: anyhow::Result<Record>,
        cx: &mut Context<Self>,
    ) {
        let key = (item.server_id, item.id.clone());
        if self
            .lyric_selections
            .get(&key)
            .is_none_or(|value| value.override_generation != Some(generation))
        {
            return;
        }
        if self.lyric_key().as_ref() == Some(&key) {
            self.lyric_busy = false;
        }
        match result {
            Ok(record) => self.install_lyric_match(item, record, cx),
            Err(error) => {
                log::warn!("Could not fetch chosen lyrics: {error}");
                self.message = error.to_string();
            }
        }
        cx.notify();
    }
    fn install_lyric_match(&mut self, item: &Item, record: Record, cx: &mut Context<Self>) {
        if self.cache_busy {
            self.message = "Wait for cache maintenance before choosing lyrics".into();
            cx.notify();
            return;
        }
        match lyric_sources::save_record(
            &lyric_sources::cache_path(&self.data_dir, item, true),
            &record,
        ) {
            Ok(()) => {
                self.show_lyric_record(item, record, cx);
                self.message = "Lyric override saved".into();
            }
            Err(error) => {
                log::warn!("Could not save chosen lyrics: {error}");
                self.message = error.to_string();
            }
        }
        cx.notify();
    }
    fn show_lyric_record(&mut self, item: &Item, mut record: Record, cx: &mut Context<Self>) {
        self.lyric_selections
            .entry((item.server_id, item.id.clone()))
            .or_default()
            .override_generation = None;
        if self
            .state
            .queue
            .current()
            .is_some_and(|entry| entry.item.id == item.id && entry.item.server_id == item.server_id)
        {
            self.lyric_busy = false;
            self.restore_lyric_selection(item, &mut record, true);
            self.lyric_generation += 1;
            self.lyrics = record.lines();
            self.lyric_record = Some(record);
            self.lyric_scroll = ScrollHandle::default();
            self.sidebar_lyric_scroll = ScrollHandle::default();
            self.lyric_scroll_motion = Default::default();
            self.lyric_active = None;
            self.lyric_scroll_paused_until = None;
            self.sidebar_lyric_scroll_paused_until = None;
            self.annotate_lyrics(cx);
            self.reset_lyric_translation(cx);
        }
    }
    pub fn edit_lyrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.current_entry().map(|entry| entry.item.clone()) else {
            return;
        };
        let raw = self
            .lyric_record
            .as_ref()
            .map(|record| record.raw.clone())
            .unwrap_or_else(|| lyrics::render_lrc(&self.lyrics));
        self.lyric_text
            .update(cx, |input, cx| input.set_value(raw, window, cx));
        self.lyric_target = Some(item);
        self.navigate(Route::LyricEditor, cx);
    }
    fn save_lyric_edit(&mut self, cx: &mut Context<Self>) {
        let Some(item) = self.lyric_target.clone() else {
            return;
        };
        let raw = self.lyric_text.read(cx).value().to_string();
        if !raw.trim().is_empty() && lyrics::parse_lrc(&raw).is_empty() {
            self.message = "Enter lyric text or valid LRC timestamps".into();
            cx.notify();
            return;
        }
        let record = Record {
            raw,
            matched: None,
            variants: vec![],
            selected: 0,
        };
        match lyric_sources::save_record(
            &lyric_sources::cache_path(&self.data_dir, &item, true),
            &record,
        ) {
            Ok(()) => {
                self.show_lyric_record(&item, record, cx);
                self.navigate(Route::Lyrics, cx);
                self.message = "Lyric override saved".into();
            }
            Err(error) => self.message = error.to_string(),
        };
        cx.notify();
    }
    fn import_lyrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_lyrics(window, cx);
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import plain text or LRC lyrics".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await
                && let Some(path) = paths.first()
            {
                let path = path.clone();
                let task = cx.background_executor().spawn(async move {
                    use std::io::Read;
                    let mut bytes = vec![];
                    std::fs::File::open(path)?
                        .take(1_000_001)
                        .read_to_end(&mut bytes)?;
                    anyhow::ensure!(bytes.len() <= 1_000_000, "Lyrics are too large");
                    Ok::<_, anyhow::Error>(String::from_utf8(bytes)?)
                });
                let result = task.await;
                this.update_in(cx, |this, window, cx| {
                    match result {
                        Ok(raw) => this
                            .lyric_text
                            .update(cx, |input, cx| input.set_value(raw, window, cx)),
                        Err(error) => this.message = error.to_string(),
                    };
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }
    pub(crate) fn lyric_export_metadata(&self) -> feishin_core::lyric_export::Metadata {
        let item = self.current_metadata();
        let matched = self
            .lyric_record
            .as_ref()
            .and_then(|record| record.matched.as_ref());
        let variant = self
            .lyric_record
            .as_ref()
            .and_then(|record| record.variants.get(record.selected));
        let structured = variant.is_some() || self.lyrics.iter().any(|line| line.time.is_some());
        feishin_core::lyric_export::Metadata {
            artist: matched
                .map(|matched| matched.artist.clone())
                .or_else(|| item.as_ref().map(|item| item.artist.clone()))
                .unwrap_or_default(),
            name: matched
                .map(|matched| matched.name.clone())
                .or_else(|| item.as_ref().map(|item| item.name.clone()))
                .unwrap_or_else(|| "lyrics".into()),
            offset_ms: variant
                .and_then(|variant| variant.offset)
                .map(|offset| offset * 1000.),
            lyrics: if structured {
                serde_json::Value::Array(self.lyrics.iter().map(|line| {
                    serde_json::json!({"startMs": (line.time.unwrap_or_default() * 1_000_000.).round() / 1000., "text": line.text})
                }).collect())
            } else {
                serde_json::Value::String(
                    self.lyric_record
                        .as_ref()
                        .map(|record| record.raw.clone())
                        .unwrap_or_else(|| {
                            self.lyrics
                                .iter()
                                .map(|line| line.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n")
                        }),
                )
            },
        }
    }
    fn export_lyrics(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::lyric_export::open(
            self.lyric_export_metadata(),
            self.lyric_offset() * 1000.,
            self.lyrics.iter().any(|line| line.time.is_some()),
            self.data_dir.clone(),
            window,
            cx,
        );
    }
    pub fn lyric_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        gpui::div()
            .v_flex()
            .gap_2()
            .child(
                crate::styles::button("lyric-resume-follow", cx)
                    .label(crate::appearance::text("Follow current lyric", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.lyric_scroll = true;
                        this.lyric_scroll_paused_until = None;
                        this.sidebar_lyric_scroll_paused_until = None;
                        this.update_lyric_follow(this.current_position(), cx);
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                gpui::div()
                    .h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        crate::styles::button("lyric-edit", cx)
                            .label(self.tr("Edit lyrics"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.edit_lyrics(window, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("lyric-import", cx)
                            .label(self.tr("Import"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.import_lyrics(window, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("lyric-copy", cx)
                            .label(self.tr("Copy"))
                            .disabled(self.lyrics.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    lyrics::render_lrc(&this.lyrics),
                                ))
                            })),
                    )
                    .child(
                        crate::styles::button("lyric-reset", cx)
                            .label(self.tr("Remove override"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(entry) = this.current_entry() {
                                    let path = lyric_sources::cache_path(
                                        &this.data_dir,
                                        &entry.item,
                                        true,
                                    );
                                    match std::fs::remove_file(path) {
                                        Ok(()) => this.load_lyrics(true, cx),
                                        Err(error)
                                            if error.kind() == std::io::ErrorKind::NotFound =>
                                        {
                                            this.load_lyrics(true, cx)
                                        }
                                        Err(error) => this.message = error.to_string(),
                                    };
                                    cx.notify();
                                }
                            })),
                    ),
            )
            .into_any_element()
    }
    pub fn lyric_editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        gpui::div()
            .v_flex()
            .gap_4()
            .size_full()
            .child(
                self.lyric_target
                    .as_ref()
                    .map(|item| format!("{} - {}", item.name, item.artist))
                    .unwrap_or_default(),
            )
            .child(crate::styles::input(&self.lyric_text, cx))
            .child(
                crate::styles::button("save-lyric", cx)
                    .primary()
                    .label(self.tr("Save override"))
                    .on_click(cx.listener(|this, _, _, cx| this.save_lyric_edit(cx))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "lyric_clear_tests.rs"]
mod clear_tests;
