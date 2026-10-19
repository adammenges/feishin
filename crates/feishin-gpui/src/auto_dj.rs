use crate::app::Feishin;
use feishin_core::{
    auto_dj::{Mode, Strategy},
    model::ItemKind,
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Selectable, StyledExt,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
};
use std::collections::HashSet;

impl Feishin {
    pub fn init_auto_dj_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.extend(Self::bind_auto_dj_numbers(
            &self.auto_dj_numbers,
            window,
            cx,
        ));
    }
    pub(crate) fn bind_auto_dj_numbers(
        numbers: &[Entity<InputState>],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        let mut subscriptions = vec![];
        for (index, input) in numbers.iter().enumerate() {
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event: &InputEvent, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let Ok(value) = input.read(cx).value().parse::<usize>() else {
                        return;
                    };
                    let mut config = this.state.settings.auto_dj_config.clone();
                    let target = if index == 0 {
                        &mut config.item_count
                    } else {
                        &mut config.timing
                    };
                    if *target == value {
                        return;
                    }
                    *target = value;
                    if config.validate().is_ok() {
                        this.state.settings.auto_dj_config = config;
                        if input != &this.auto_dj_numbers[index] {
                            this.auto_dj_numbers[index].update(cx, |input, cx| {
                                input.set_value(value.to_string(), window, cx)
                            });
                        }
                        this.reset_auto_dj();
                        this.save();
                        cx.notify();
                    }
                },
            ));
            subscriptions.push(cx.subscribe_in(
                input,
                window,
                move |this, input, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let config = &this.state.settings.auto_dj_config;
                    let current = if index == 0 {
                        config.item_count
                    } else {
                        config.timing
                    };
                    let value = input
                        .read(cx)
                        .value()
                        .parse::<i64>()
                        .unwrap_or(current as i64);
                    let value = value
                        .saturating_add(if *action == StepAction::Increment {
                            1
                        } else {
                            -1
                        })
                        .clamp(1, if index == 0 { 50 } else { 5 });
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                },
            ));
        }
        subscriptions
    }

    pub fn reset_auto_dj(&mut self) {
        self.auto_dj_generation += 1;
        self.auto_dj_busy = false;
        self.auto_dj_last = None;
    }
    pub fn poll_auto_dj(&mut self, cx: &mut Context<Self>) {
        if !self.state.settings.auto_dj || self.auto_dj_busy || self.radio_item.is_some() {
            return;
        }
        let Some(entry) = self
            .state
            .queue
            .current()
            .cloned()
            .filter(|entry| entry.item.kind == ItemKind::Song)
        else {
            return;
        };
        let remaining = self
            .state
            .queue
            .order
            .iter()
            .position(|key| *key == entry.key)
            .map_or(0, |index| {
                self.state.queue.order.len().saturating_sub(index + 1)
            });
        let config = self.state.settings.auto_dj_config.clone();
        if remaining >= config.timing || self.auto_dj_last == Some((entry.key, remaining)) {
            return;
        }
        let Some(api) = self.sessions.get(&entry.item.server_id).cloned() else {
            return;
        };
        self.auto_dj_busy = true;
        self.auto_dj_last = Some((entry.key, remaining));
        let generation = self.auto_dj_generation;
        let queue = self
            .state
            .queue
            .entries
            .iter()
            .map(|entry| entry.item.clone())
            .collect::<Vec<_>>();
        let policy = config.clone();
        log::info!(
            "Auto DJ preparing {:?} with {:?} strategy",
            config.mode,
            if config.mode == Mode::Songs {
                config.song_strategy
            } else {
                config.album_strategy
            }
        );
        let current = entry.item.clone();
        let task = cx
            .background_executor()
            .spawn(async move { api.auto_dj(&current, &config, &queue) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.auto_dj_generation {
                    return;
                }
                this.auto_dj_busy = false;
                if !this.state.settings.auto_dj || !this.state.queue.order.contains(&entry.key) {
                    return;
                }
                match result {
                    Ok(mut songs) => {
                        if !policy.allow_duplicates {
                            let mut excluded = this
                                .state
                                .queue
                                .entries
                                .iter()
                                .filter(|queued| queued.item.server_id == entry.item.server_id)
                                .map(|queued| {
                                    if policy.mode == Mode::Albums {
                                        queued.item.album_id.clone()
                                    } else {
                                        queued.item.id.clone()
                                    }
                                })
                                .collect::<HashSet<_>>();
                            songs.retain(|song| {
                                if policy.mode == Mode::Albums {
                                    !excluded.contains(&song.album_id)
                                } else {
                                    excluded.insert(song.id.clone())
                                }
                            });
                        }
                        this.filter_playback_tracks(&mut songs);
                        if !songs.is_empty() {
                            let resume = this.state.queue.current.is_none();
                            let count = songs.len();
                            this.state.queue.add(songs, Enqueue::End);
                            this.save();
                            this.message = format!("Auto DJ added {count} tracks");
                            if resume {
                                this.play_current(cx);
                            }
                        } else {
                            this.message = "Auto DJ found no additional tracks".into();
                        }
                    }
                    Err(error) => {
                        log::warn!("Auto DJ failed: {error}");
                        this.message = format!("Auto DJ: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    pub fn auto_dj_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        self.auto_dj_controls_for(&self.auto_dj_numbers, false, cx)
    }
    pub(crate) fn auto_dj_controls_for(
        &self,
        numbers: &[Entity<InputState>],
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = &self.state.settings.auto_dj_config;
        div()
            .v_flex()
            .max_w(px(900.))
            .min_w_0()
            .gap_2()
            .child(
                crate::styles::button("auto-dj-mode", cx)
                    .debug_selector(|| "auto-dj-mode".into())
                    .label(format!(
                        "{}: {}",
                        self.tr("setting.autoDJ_mode"),
                        self.tr(match config.mode {
                            Mode::Songs => "setting.autoDJ_mode_songs",
                            Mode::Albums => "setting.autoDJ_mode_albums",
                        })
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.auto_dj_config.mode =
                            match this.state.settings.auto_dj_config.mode {
                                Mode::Songs => Mode::Albums,
                                Mode::Albums => Mode::Songs,
                            };
                        this.reset_auto_dj();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("setting.autoDJ_mode_description")),
            )
            .children(
                [false, true]
                    .into_iter()
                    .filter(|albums| !compact || *albums == (config.mode == Mode::Albums))
                    .map(|albums| {
                        let strategy = if albums {
                            config.album_strategy
                        } else {
                            config.song_strategy
                        };
                        crate::styles::button(
                            if albums {
                                "auto-dj-album-strategy"
                            } else {
                                "auto-dj-song-strategy"
                            },
                            cx,
                        )
                        .debug_selector(move || {
                            if albums {
                                "auto-dj-album-strategy"
                            } else {
                                "auto-dj-song-strategy"
                            }
                            .into()
                        })
                        .label(format!(
                            "{}: {}",
                            self.tr(if albums {
                                "setting.autoDJ_albumStrategy"
                            } else {
                                "setting.autoDJ_songStrategy"
                            }),
                            self.tr(match strategy {
                                Strategy::Similar => "setting.autoDJ_strategy_option_similar",
                                Strategy::LibraryRandom =>
                                    "setting.autoDJ_strategy_option_library_random",
                            })
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let config = &mut this.state.settings.auto_dj_config;
                            let strategy = if albums {
                                &mut config.album_strategy
                            } else {
                                &mut config.song_strategy
                            };
                            *strategy = match strategy {
                                Strategy::Similar => Strategy::LibraryRandom,
                                Strategy::LibraryRandom => Strategy::Similar,
                            };
                            this.reset_auto_dj();
                            this.save();
                            cx.notify();
                        }))
                    }),
            )
            .child(
                crate::styles::button("auto-dj-duplicates", cx)
                    .debug_selector(|| "auto-dj-duplicates".into())
                    .label(self.tr("setting.autoDJ_allowDuplicates"))
                    .selected(config.allow_duplicates)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.auto_dj_config.allow_duplicates =
                            !this.state.settings.auto_dj_config.allow_duplicates;
                        this.reset_auto_dj();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("setting.autoDJ_allowDuplicates_description")),
            )
            .child(
                crate::styles::button("auto-dj-only-similar", cx)
                    .debug_selector(|| "auto-dj-only-similar".into())
                    .label(self.tr("setting.autoDJ_onlySimilar"))
                    .selected(config.only_similar)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.auto_dj_config.only_similar =
                            !this.state.settings.auto_dj_config.only_similar;
                        this.reset_auto_dj();
                        this.save();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.tr("setting.autoDJ_onlySimilar_description")),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .children(numbers.iter().enumerate().map(|(index, input)| {
                        div()
                            .v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(self.tr(if index == 0 {
                                "setting.autoDJ_itemCount"
                            } else {
                                "setting.autoDJ_timing"
                            }))
                            .child(crate::styles::number_input(input, cx))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.tr(if index == 0 {
                                        "setting.autoDJ_itemCount_description"
                                    } else {
                                        "setting.autoDJ_timing_description"
                                    })),
                            )
                    })),
            )
            .when(!compact, |controls| {
                controls
                    .child(crate::styles::input(&self.auto_dj_folder, cx))
                    .child(
                        crate::styles::button("auto-dj-save", cx)
                            .debug_selector(|| "auto-dj-save".into())
                            .label(self.tr("common.save"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let values = this
                                    .auto_dj_numbers
                                    .iter()
                                    .map(|input| input.read(cx).value().parse::<usize>())
                                    .collect::<Result<Vec<_>, _>>();
                                let result =
                                    values.map_err(anyhow::Error::from).and_then(|values| {
                                        let mut config = this.state.settings.auto_dj_config.clone();
                                        config.item_count = values[0];
                                        config.timing = values[1];
                                        let folder =
                                            this.auto_dj_folder.read(cx).value().trim().to_owned();
                                        config.music_folder =
                                            (!folder.is_empty()).then_some(folder);
                                        config.validate()?;
                                        Ok(config)
                                    });
                                match result {
                                    Ok(config) => {
                                        this.state.settings.auto_dj_config = config;
                                        this.reset_auto_dj();
                                        this.save();
                                    }
                                    Err(error) => this.message = error.to_string(),
                                }
                                cx.notify();
                            })),
                    )
            })
            .into_any_element()
    }
}
