use crate::app::Feishin;
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Selectable, Sizable, StyledExt,
    button::{ButtonCustomVariant, ButtonVariants},
    input::{InputEvent, InputState},
    popover::{Popover, PopoverState},
};

impl Feishin {
    pub fn auto_dj_button(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        self.playback_controls_button(false, window, cx)
    }
    pub fn sleep_button(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        self.playback_controls_button(true, window, cx)
    }
    fn playback_controls_button(
        &self,
        sleep: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app = cx.weak_entity();
        let key = if sleep {
            "player.sleepTimer"
        } else {
            "setting.autoDJ"
        };
        let label = self.tr(key);
        let trigger = if sleep {
            crate::playerbar::button("player-sleep", false, window, cx)
                .icon(
                    gpui_component::Icon::default()
                        .path(if self.sleep_timer.is_some() {
                            "native-icons/timer.svg"
                        } else {
                            "native-icons/timer-off.svg"
                        })
                        .with_size(window.rem_size()),
                )
                .when(self.sleep_timer.is_some(), |button| {
                    button.text_color(cx.theme().primary)
                })
        } else {
            crate::styles::button("player-auto-dj", cx)
                .custom(
                    ButtonCustomVariant::new(cx)
                        .color(transparent_black())
                        .foreground(if self.state.settings.auto_dj {
                            cx.theme().primary
                        } else {
                            cx.theme().foreground
                        })
                        .hover(transparent_black())
                        .active(transparent_black()),
                )
                .with_size(window.rem_size() * 1.375)
                .h(rems(1.375))
                .px(rems(7. / 16.))
                .border(px(self.state.settings.zoom))
                .border_color(transparent_black())
                .child(
                    div()
                        .text_size(rems(11. / 16.))
                        .line_height(rems(11. / 16.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(label.to_uppercase()),
                )
        };
        Popover::new(if sleep {
            "sleep-player-popover"
        } else {
            "auto-dj-player-popover"
        })
        .anchor(Corner::BottomRight)
        .trigger(
            trigger
                .debug_selector(move || {
                    if sleep {
                        "player-sleep"
                    } else {
                        "player-auto-dj"
                    }
                    .into()
                })
                .tooltip(label),
        )
        .content(move |_, window, cx| {
            let popover = cx.weak_entity();
            window.use_keyed_state(
                if sleep {
                    "sleep-player-fields"
                } else {
                    "auto-dj-player-fields"
                },
                cx,
                |window, cx| QuickControls::new(app.clone(), sleep, popover, window, cx),
            )
        })
        .into_any_element()
    }
}

// Each popover owns its input entities so an open Settings page keeps independent focus.
struct QuickControls {
    app: WeakEntity<Feishin>,
    sleep: bool,
    popover: WeakEntity<PopoverState>,
    numbers: Vec<Entity<InputState>>,
    last_values: Vec<String>,
    _subscriptions: Vec<Subscription>,
}
impl QuickControls {
    fn values(app: &Feishin, sleep: bool, cx: &App) -> Vec<String> {
        if sleep {
            app.sleep_inputs
                .iter()
                .map(|input| input.read(cx).value().to_string())
                .collect()
        } else {
            let config = &app.state.settings.auto_dj_config;
            [config.item_count, config.timing]
                .map(|value| value.to_string())
                .to_vec()
        }
    }
    fn new(
        app: WeakEntity<Feishin>,
        sleep: bool,
        popover: WeakEntity<PopoverState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let values = app
            .read_with(cx, |app, cx| Self::values(app, sleep, cx))
            .unwrap_or_default();
        let labels: &[&str] = if sleep {
            &[
                "datetime.hourLong",
                "datetime.minuteLong",
                "datetime.secondLong",
            ]
        } else {
            &["setting.autoDJ_itemCount", "setting.autoDJ_timing"]
        };
        let numbers = labels
            .iter()
            .zip(&values)
            .map(|(label, value)| {
                cx.new(|cx| {
                    crate::appearance::input_state(window, cx)
                        .aria_label(*label)
                        .default_value(value.clone())
                })
            })
            .collect::<Vec<_>>();
        let subscriptions = app
            .update(cx, |_, cx| {
                if !sleep {
                    return Feishin::bind_auto_dj_numbers(&numbers, window, cx);
                }
                let mut subscriptions = vec![];
                for (index, input) in numbers.iter().enumerate() {
                    subscriptions.extend(Feishin::bind_number_step(
                        input,
                        0.,
                        if index == 0 { 23. } else { 59. },
                        1.,
                        window,
                        cx,
                    ));
                    subscriptions.push(cx.subscribe_in(
                        input,
                        window,
                        move |app, input, event: &InputEvent, window, cx| {
                            if matches!(event, InputEvent::Change) {
                                let value = input.read(cx).value();
                                app.sleep_inputs[index]
                                    .update(cx, |input, cx| input.set_value(value, window, cx));
                            }
                        },
                    ));
                }
                subscriptions
            })
            .unwrap_or_default();
        Self {
            app,
            sleep,
            popover,
            numbers,
            last_values: values,
            _subscriptions: subscriptions,
        }
    }
}
impl Render for QuickControls {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(values) = self
            .app
            .read_with(cx, |app, cx| Self::values(app, self.sleep, cx))
        {
            for (index, value) in values.iter().enumerate() {
                if *value != self.last_values[index]
                    && self.numbers[index].read(cx).value().as_str() != value
                {
                    self.numbers[index]
                        .update(cx, |input, cx| input.set_value(value.clone(), window, cx));
                }
            }
            self.last_values = values;
        }
        let height = (window.viewport_size().height - px(150.)).max(px(180.));
        let sleep = self.sleep;
        self.app
            .update(cx, |app, cx| {
                div()
                    .id("playback-quick-controls")
                    .role(Role::Dialog)
                    .aria_label(app.tr(if sleep {
                        "player.sleepTimer"
                    } else {
                        "setting.autoDJ"
                    }))
                    .debug_selector(move || {
                        if sleep {
                            "sleep-player-controls"
                        } else {
                            "auto-dj-player-controls"
                        }
                        .into()
                    })
                    .w(px(420.))
                    .max_w_full()
                    .max_h(height)
                    .v_flex()
                    .gap_3()
                    .overflow_y_scroll()
                    .when(!sleep, |div| {
                        div.child(
                            crate::styles::button("auto-dj-player-enabled", cx)
                                .debug_selector(|| "auto-dj-player-enabled".into())
                                .label(app.tr("setting.autoDJ_enabled"))
                                .selected(app.state.settings.auto_dj)
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.state.settings.auto_dj = !app.state.settings.auto_dj;
                                    app.reset_auto_dj();
                                    app.save();
                                    cx.notify();
                                })),
                        )
                    })
                    .child(if sleep {
                        app.sleep_controls_for(&self.numbers, Some(self.popover.clone()), cx)
                    } else {
                        app.auto_dj_controls_for(&self.numbers, true, cx)
                    })
                    .into_any_element()
            })
            .unwrap_or_else(|_| div().into_any_element())
    }
}
