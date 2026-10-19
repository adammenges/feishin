use crate::app::Feishin;
use feishin_core::{
    model::ItemKind,
    playback::Control,
    sleep_timer::{Mode, Timer},
};
use gpui::{prelude::*, *};
use gpui_component::{
    Disableable, StyledExt,
    menu::{DropdownMenu, PopupMenuItem},
};
use std::time::{Duration, Instant};
impl Feishin {
    pub fn stop_playback(&mut self, cx: &mut Context<Self>) {
        let radio = self.radio_item.is_some();
        self.stop_transport(cx);
        if !radio {
            self.state.queue.position = 0.0;
        }
        self.seek_preview.clear();
        self.progress
            .update(cx, |slider, _| slider.cancel_pointer_change());
    }
    pub(crate) fn stop_transport(&mut self, cx: &mut Context<Self>) {
        self.report("stop", cx);
        self.radio_item = None;
        self.radio_position = 0.0;
        self.radio_metadata = Default::default();
        self.play_request_generation += 1;
        self.player_generation = self.player.send(Control::Stop);
        self.playing = false;
        self.loaded = false;
        self.buffering = false;
        cx.notify();
    }
    pub fn sleep_stops_at_end(&self) -> bool {
        self.sleep_timer.as_ref().is_some_and(|timer| {
            self.state.queue.current().is_some_and(|entry| {
                timer.stop_at_end(
                    entry.key,
                    &entry.item,
                    self.state.queue.next_entry(true).map(|entry| &entry.item),
                )
            })
        })
    }
    pub fn set_sleep_minutes(&mut self, minutes: u64, cx: &mut Context<Self>) {
        if !(1..=1440).contains(&minutes) {
            self.message = "Choose 1 to 1440 minutes".into();
        } else {
            self.sleep_timer = Some(Timer::timed(
                Duration::from_secs(minutes * 60),
                Instant::now(),
            ));
            self.arm_next();
        }
        cx.notify();
    }
    fn custom_sleep_duration(
        inputs: &[Entity<gpui_component::input::InputState>],
        cx: &App,
    ) -> Option<Duration> {
        let mut seconds = 0;
        for (index, input) in inputs.iter().enumerate() {
            let value = input.read(cx).value().parse::<u64>().ok()?;
            if value > if index == 0 { 23 } else { 59 } {
                return None;
            }
            seconds += value * [3600, 60, 1][index];
        }
        (seconds > 0).then(|| Duration::from_secs(seconds))
    }

    pub fn sleep_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        self.sleep_controls_for(&self.sleep_inputs, None, cx)
    }
    pub(crate) fn sleep_controls_for(
        &self,
        inputs: &[Entity<gpui_component::input::InputState>],
        popover: Option<WeakEntity<gpui_component::popover::PopoverState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let custom_inputs = inputs.to_vec();
        let custom_popover = popover.clone();
        let presets_popover = popover.clone();
        let view = cx.weak_entity();
        let language = self.state.settings.language.clone();
        div()
            .v_flex()
            .gap_2()
            .child(
                crate::styles::button("sleep-presets", cx)
                    .debug_selector(|| "sleep-presets".into())
                    .label(match self.sleep_timer.as_ref().map(|timer| &timer.mode) {
                        Some(Mode::Timed(_)) => self.sleep_timer.as_ref().unwrap().label(),
                        Some(Mode::Song(_)) => self.tr("player.sleepTimer_endOfSong"),
                        Some(Mode::Album(_, _)) => self.tr("player.sleepTimer_endOfAlbum"),
                        None => format!(
                            "{}: {}",
                            self.tr("player.sleepTimer"),
                            self.tr("player.sleepTimer_off")
                        ),
                    })
                    .dropdown_menu(move |mut menu, _, _| {
                        for minutes in [5, 10, 15, 30, 45, 60, 120, 180, 240] {
                            let view = view.clone();
                            let popover = presets_popover.clone();
                            menu = menu.item(
                                PopupMenuItem::new(
                                    feishin_core::i18n::translate_with(
                                        &language,
                                        "player.sleepTimer_minutes",
                                        &serde_json::json!({"count":minutes}),
                                    )
                                    .unwrap_or_else(|| format!("{minutes} min")),
                                )
                                .on_click(move |_, window, cx| {
                                    view.update(cx, |this, cx| this.set_sleep_minutes(minutes, cx))
                                        .ok();
                                    if let Some(popover) = &popover {
                                        popover
                                            .update(cx, |popover, cx| popover.dismiss(window, cx))
                                            .ok();
                                    }
                                }),
                            );
                        }
                        menu
                    }),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .children(inputs.iter().enumerate().map(|(index, input)| {
                        div()
                            .flex_1()
                            .min_w_0()
                            .max_w(px(140.))
                            .v_flex()
                            .gap_1()
                            .child(self.tr([
                                "datetime.hourLong",
                                "datetime.minuteLong",
                                "datetime.secondLong",
                            ][index]))
                            .child(crate::styles::number_input(input, cx))
                    })),
            )
            .child(
                crate::styles::button("sleep-custom", cx)
                    .debug_selector(|| "sleep-custom".into())
                    .label(crate::appearance::text("Set custom timer", cx))
                    .disabled(Self::custom_sleep_duration(inputs, cx).is_none())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(duration) = Self::custom_sleep_duration(&custom_inputs, cx) {
                            this.sleep_timer = Some(Timer::timed(duration, Instant::now()));
                            this.arm_next();
                            if let Some(popover) = &custom_popover {
                                popover
                                    .update(cx, |popover, cx| popover.dismiss(window, cx))
                                    .ok();
                            }
                            cx.notify();
                        }
                    })),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .children([false, true].into_iter().map(|album| {
                        let popover = popover.clone();
                        crate::styles::button(if album { "sleep-album" } else { "sleep-song" }, cx)
                            .debug_selector(move || {
                                if album { "sleep-album" } else { "sleep-song" }.into()
                            })
                            .label(self.tr(if album {
                                "player.sleepTimer_endOfAlbum"
                            } else {
                                "player.sleepTimer_endOfSong"
                            }))
                            .disabled(
                                (album && self.state.queue.shuffle)
                                    || self
                                        .current_entry()
                                        .is_none_or(|entry| entry.item.kind != ItemKind::Song),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(entry) = this
                                    .current_entry()
                                    .filter(|entry| entry.item.kind == ItemKind::Song)
                                {
                                    this.sleep_timer = Some(if album {
                                        Timer::album(&entry.item, Instant::now())
                                    } else {
                                        Timer::song(entry.key, Instant::now())
                                    });
                                    this.arm_next();
                                    if let Some(popover) = &popover {
                                        popover
                                            .update(cx, |popover, cx| popover.dismiss(window, cx))
                                            .ok();
                                    }
                                    cx.notify();
                                }
                            }))
                    }))
                    .child(
                        crate::styles::button("cancel-sleep", cx)
                            .debug_selector(|| "cancel-sleep".into())
                            .label(crate::appearance::text("Cancel sleep timer", cx))
                            .disabled(self.sleep_timer.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.sleep_timer = None;
                                this.arm_next();
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
