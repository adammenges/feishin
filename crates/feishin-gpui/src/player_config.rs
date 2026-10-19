use crate::app::Feishin;
use feishin_core::{
    playback::Control,
    transition::{Curve, Mode},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Selectable, Sizable, StyledExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    menu::{DropdownMenu, PopupMenuItem},
    popover::Popover,
    slider::{SliderEvent, SliderState},
};

#[derive(Clone)]
enum Choice {
    Engine(&'static str),
    Device(String),
    Curve(Curve),
}
impl Feishin {
    pub fn player_config_button(
        &self,
        window: Option<&Window>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let settings_entry = window.is_none();
        let app = cx.weak_entity();
        let id = if settings_entry {
            "settings-player-configure"
        } else {
            "player-configure"
        };
        Popover::new(if settings_entry {
            "settings-player-config-popover"
        } else {
            "player-config-popover"
        })
        .anchor(if settings_entry {
            Corner::TopLeft
        } else {
            Corner::BottomRight
        })
        .trigger(
            (if let Some(window) = window {
                crate::playerbar::button(id, false, window, cx)
            } else {
                crate::styles::button(id, cx).ghost()
            })
            .icon(
                Icon::default()
                    .path("native-icons/sliders.svg")
                    .with_size(window.map_or(cx.theme().font_size, Window::rem_size)),
            )
            .when(settings_entry, |button| {
                button.label(self.tr("common.configure"))
            })
            .debug_selector(move || id.into())
            .tooltip(self.tr("common.setting_other")),
        )
        .content(move |_, window, cx| {
            window.use_keyed_state(
                if settings_entry {
                    "settings-player-config-fields"
                } else {
                    "player-config-fields"
                },
                cx,
                |window, cx| PlayerConfig::new(app.clone(), window, cx),
            )
        })
        .into_any_element()
    }

    fn native_transitions_enabled(&self) -> bool {
        !self.playing
            && self.cast_selected.is_none()
            && !self.state.settings.jukebox
            && self.state.settings.mpv_path.is_empty()
    }

    fn choose_player_config(
        &mut self,
        choice: Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match choice {
            Choice::Engine(engine) => {
                if self.playing || self.cast_selected.is_some() {
                    return;
                }
                if !self.state.settings.jukebox
                    && (engine == "MPV") != self.state.settings.mpv_path.is_empty()
                    && engine != "Jukebox"
                {
                    return;
                }
                if engine == "Jukebox" {
                    self.select_jukebox(cx);
                    return;
                }
                self.stop_playback(cx);
                self.select_speaker(None, cx);
                self.state.settings.select_mpv(engine == "MPV");
                self.mpv_path.update(cx, |input, cx| {
                    input.set_value(self.state.settings.mpv_path.clone(), window, cx)
                });
                self.apply_audio_settings(cx);
            }
            Choice::Device(device) => {
                if self.playing || self.cast_selected.is_some() || self.state.settings.jukebox {
                    return;
                }
                self.select_playback_device(device, window, cx);
            }
            Choice::Curve(curve) => {
                if !self.native_transitions_enabled() {
                    return;
                }
                self.state.settings.crossfade_style = curve;
                self.apply_audio_settings(cx);
            }
        }
    }

    fn player_config_choice(
        &self,
        id: &'static str,
        label: String,
        choices: Vec<(String, Choice, bool)>,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app = cx.weak_entity();
        crate::styles::button(id, cx)
            .debug_selector(move || id.into())
            .label(label)
            .icon(IconName::ChevronDown)
            .disabled(disabled)
            .dropdown_menu(move |mut menu, _, _| {
                for (label, choice, checked) in &choices {
                    let app = app.clone();
                    let choice = choice.clone();
                    menu = menu.item(
                        PopupMenuItem::new(label.clone())
                            .checked(*checked)
                            .on_click(move |_, window, cx| {
                                let app = app.clone();
                                let choice = choice.clone();
                                window.defer(cx, move |window, cx| {
                                    app.update(cx, |app, cx| {
                                        app.choose_player_config(choice, window, cx)
                                    })
                                    .ok();
                                });
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }
}

// Own sliders independently from Settings, as with the sleep and Auto DJ popovers.
struct PlayerConfig {
    app: WeakEntity<Feishin>,
    speed: Entity<SliderState>,
    crossfade: Entity<SliderState>,
    last_crossfade: f64,
    device_source: Option<String>,
    devices: Vec<(String, String)>,
    device_error: Option<String>,
    device_generation: u64,
    device_loading: bool,
    _subscriptions: Vec<Subscription>,
}
impl PlayerConfig {
    fn new(app: WeakEntity<Feishin>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = app
            .read_with(cx, |app, _| app.state.settings.clone())
            .unwrap_or_default();
        let speed = cx.new(|_| {
            SliderState::new()
                .min(0.5)
                .max(2.)
                .step(0.01)
                .default_value(settings.speed)
        });
        let crossfade = cx.new(|_| {
            SliderState::new()
                .min(3.)
                .max(21.)
                .step(1.)
                .default_value(settings.crossfade_seconds as f32)
        });
        let subscriptions = app
            .update(cx, |_, cx| {
                vec![
                    cx.subscribe_in(&speed, window, |app, _, event: &SliderEvent, window, cx| {
                        let SliderEvent::Change(value) = event else {
                            return;
                        };
                        app.set_playback_speed(value.end(), window, cx);
                    }),
                    cx.subscribe_in(&crossfade, window, |app, _, event: &SliderEvent, _, cx| {
                        let SliderEvent::ChangeEnd(value) = event else {
                            return;
                        };
                        if app.native_transitions_enabled() {
                            app.state.settings.crossfade_seconds =
                                value.end().clamp(3., 21.) as f64;
                            app.apply_audio_settings(cx);
                        }
                    }),
                ]
            })
            .unwrap_or_default();
        Self {
            app,
            speed,
            crossfade,
            last_crossfade: settings.crossfade_seconds,
            devices: vec![],
            device_source: None,
            device_error: None,
            device_generation: 0,
            device_loading: false,
            _subscriptions: subscriptions,
        }
    }

    fn refresh_devices(&mut self, cx: &mut Context<Self>) {
        let Ok(settings) = self.app.read_with(cx, |app, _| app.state.settings.clone()) else {
            return;
        };
        self.device_source = Some(settings.mpv_path.clone());
        self.device_generation += 1;
        let generation = self.device_generation;
        self.device_loading = true;
        self.device_error = None;
        self.devices.clear();
        cx.spawn(async move |this, cx| {
            let result =
                smol::unblock(move || feishin_core::playback::output_devices(&settings)).await;
            this.update(cx, |this, cx| {
                if generation != this.device_generation {
                    return;
                }
                this.device_loading = false;
                match result {
                    Ok(devices) => this.devices = devices,
                    Err(error) => {
                        log::warn!("Audio device query failed: {error}");
                        this.device_error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

fn row(rtl: bool, label: String, control: impl IntoElement) -> Div {
    gpui::div()
        .h_flex()
        .when(rtl, |div| div.flex_row_reverse())
        .gap_3()
        .items_center()
        .child(gpui::div().w(px(175.)).flex_shrink_0().child(label))
        .child(gpui::div().flex_1().min_w_0().child(control))
}

impl Render for PlayerConfig {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Ok(settings) = self.app.read_with(cx, |app, _| app.state.settings.clone()) else {
            return gpui::div().into_any_element();
        };
        if self.device_source.as_ref() != Some(&settings.mpv_path) {
            // Headless UI tests supply devices explicitly; protocol tests cover discovery.
            if cfg!(test) {
                self.device_source = Some(settings.mpv_path.clone());
                self.devices.clear();
            } else {
                self.refresh_devices(cx);
            }
        }
        if self.speed.read(cx).value().end() != settings.speed {
            self.speed.update(cx, |slider, cx| {
                slider.set_value(settings.speed, window, cx)
            });
        }
        if self.last_crossfade != settings.crossfade_seconds {
            self.last_crossfade = settings.crossfade_seconds;
            self.crossfade.update(cx, |slider, cx| {
                slider.set_value(settings.crossfade_seconds as f32, window, cx)
            });
        }
        let crossfade_seconds = self.crossfade.read(cx).value().end();
        let refresh = crate::styles::button("player-config-devices-refresh", cx)
            .icon(IconName::Redo)
            .loading(self.device_loading)
            .tooltip(crate::appearance::text("common.reload", cx))
            .on_click(cx.listener(|this, _, _, cx| this.refresh_devices(cx)));
        let reload = crate::styles::button("player-config-reload", cx)
            .debug_selector(|| "player-config-reload".into())
            .icon(IconName::Redo)
            .tooltip(crate::appearance::text("common.reload", cx))
            .on_click(cx.listener(|this, _, _, cx| {
                this.app
                    .update(cx, |app, cx| {
                        app.stop_playback(cx);
                        app.player
                            .send(Control::ResetMpv(Box::new(app.state.settings.clone())));
                    })
                    .ok();
                this.refresh_devices(cx);
            }));
        let height = (window.viewport_size().height - px(150.)).max(px(180.));
        self.app
            .update(cx, |app, cx| {
                let engine = if app.cast_selected.is_some() {
                    "DLNA"
                } else if settings.jukebox {
                    "Jukebox"
                } else if settings.mpv_path.is_empty() {
                    "Native"
                } else {
                    "MPV"
                };
                let engine_control = app.player_config_choice(
                    "player-config-engine",
                    engine.into(),
                    ["Native", "MPV", "Jukebox"]
                        .into_iter()
                        .map(|value| (value.into(), Choice::Engine(value), engine == value))
                        .collect(),
                    app.playing || app.cast_selected.is_some(),
                    cx,
                );
                let local = !settings.jukebox && app.cast_selected.is_none();
                let device = if settings.mpv_path.is_empty() {
                    &settings.audio_device
                } else {
                    &settings.mpv_audio_device
                };
                let default = if settings.mpv_path.is_empty() {
                    ""
                } else {
                    "auto"
                };
                let mut devices = vec![(default.to_owned(), app.tr("setting.audioDeviceDefault"))];
                devices.extend(self.devices.iter().filter(|(id, _)| id != default).cloned());
                let label = devices
                    .iter()
                    .find(|(id, _)| id == device)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| device.clone());
                let device_control = app.player_config_choice(
                    "player-config-device",
                    label,
                    devices
                        .into_iter()
                        .map(|(id, name)| {
                            let selected = &id == device;
                            (name, Choice::Device(id), selected)
                        })
                        .collect(),
                    app.playing || !local,
                    cx,
                );
                let curves = [
                    (Curve::Linear, "Linear"),
                    (Curve::EqualPower, "Equal Power"),
                    (Curve::SCurve, "S-Curve"),
                    (Curve::Exponential, "Exponential"),
                ];
                let curve_control = app.player_config_choice(
                    "player-config-curve",
                    curves
                        .iter()
                        .find(|(curve, _)| *curve == settings.crossfade_style)
                        .unwrap()
                        .1
                        .into(),
                    curves
                        .into_iter()
                        .map(|(curve, label)| {
                            (
                                label.into(),
                                Choice::Curve(curve),
                                curve == settings.crossfade_style,
                            )
                        })
                        .collect(),
                    !app.native_transitions_enabled(),
                    cx,
                );
                let mut speed_label = format!("{:.2} x", settings.speed);
                if let Some(bpm) = app
                    .current_entry()
                    .and_then(|entry| entry.item.column_data.get("bpm"))
                    .and_then(|bpm| bpm.parse::<f32>().ok())
                    .filter(|bpm| bpm.is_finite() && *bpm > 0.)
                {
                    speed_label.push_str(&format!(" / {:.1} BPM", bpm * settings.speed));
                }
                let panel = &settings.panels;
                let rtl = crate::appearance::rtl(cx);
                gpui::div()
                    .id("player-config-controls")
                    .debug_selector(|| "player-config-controls".into())
                    .role(Role::Dialog)
                    .aria_label(app.tr("common.setting_other"))
                    .w(px(600.))
                    .max_w_full()
                    .max_h(height)
                    .v_flex()
                    .gap_3()
                    .overflow_y_scroll()
                    .child(row(
                        rtl,
                        app.tr("setting.audioPlayer"),
                        gpui::div()
                            .h_flex()
                            .gap_2()
                            .child(engine_control)
                            .when(engine == "MPV", |div| div.child(reload)),
                    ))
                    .child(row(
                        rtl,
                        app.tr("setting.audioDevice"),
                        gpui::div()
                            .h_flex()
                            .gap_2()
                            .child(device_control)
                            .child(refresh),
                    ))
                    .when_some(self.device_error.clone(), |div, error| {
                        div.child(gpui::div().text_color(cx.theme().danger).child(error))
                    })
                    .child(row(
                        rtl,
                        app.tr("setting.playbackStyle"),
                        gpui::div().h_flex().gap_2().children(
                            [
                                (
                                    Mode::Gapless,
                                    "setting.playbackStyle_optionNormal",
                                    "player-config-gapless",
                                ),
                                (
                                    Mode::Crossfade,
                                    "setting.playbackStyle_optionCrossFade",
                                    "player-config-crossfade",
                                ),
                            ]
                            .into_iter()
                            .map(|(mode, key, id)| {
                                crate::styles::button(id, cx)
                                    .debug_selector(move || id.into())
                                    .label(app.tr(key))
                                    .selected(settings.transition == mode)
                                    .disabled(!app.native_transitions_enabled())
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if app.native_transitions_enabled() {
                                            app.state.settings.transition = mode;
                                            app.apply_audio_settings(cx);
                                        }
                                    }))
                            }),
                        ),
                    ))
                    .when(settings.transition == Mode::Crossfade, |div| {
                        div.child(row(rtl, app.tr("setting.crossfadeStyle"), curve_control))
                            .child(row(
                                rtl,
                                app.tr("setting.crossfadeDuration"),
                                gpui::div()
                                    .v_flex()
                                    .gap_2()
                                    .child(format!("{crossfade_seconds:.0} s"))
                                    .child(
                                        crate::styles::slider(&self.crossfade, cx)
                                            .aria_label(app.tr("setting.crossfadeDuration"))
                                            .disabled(!app.native_transitions_enabled()),
                                    )
                                    .child(
                                        gpui::div().h_flex().justify_between().children(
                                            [3, 6, 9, 12, 15, 18, 21].map(|n| {
                                                gpui::div().text_xs().child(n.to_string())
                                            }),
                                        ),
                                    ),
                            ))
                    })
                    .child(row(
                        rtl,
                        app.tr("player.playbackSpeed"),
                        gpui::div()
                            .id("player-config-speed")
                            .debug_selector(|| "player-config-speed".into())
                            .v_flex()
                            .gap_2()
                            .on_click(cx.listener(|app, event: &ClickEvent, window, cx| {
                                if event.click_count() == 2 {
                                    app.set_playback_speed(1., window, cx);
                                }
                            }))
                            .child(speed_label)
                            .child(
                                crate::styles::slider(&self.speed, cx)
                                    .aria_label(app.tr("player.playbackSpeed")),
                            )
                            .child(
                                gpui::div().h_flex().justify_between().children(
                                    ["0.5", "0.75", "1", "1.25", "1.5", "1.75", "2"]
                                        .map(|n| gpui::div().text_xs().child(n)),
                                ),
                            ),
                    ))
                    .when(!settings.preserve_pitch, |div| {
                        let pitch = 12. * settings.speed.log2();
                        let pitch_label = format!(
                            "{:.2}x {}{pitch:.2}st",
                            settings.speed,
                            if pitch > 0. {
                                "+"
                            } else if pitch == 0. {
                                "±"
                            } else {
                                ""
                            }
                        );
                        let mut controls = vec![];
                        for (index, (step, label, aria)) in [
                            (-1., "-1st", "-1 semitone"),
                            (-0.1, "-10ct", "-10 cents"),
                            (0.1, "+10ct", "+10 cents"),
                            (1., "+1st", "+1 semitone"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            if index == 2 {
                                controls.push(
                                    gpui::div()
                                        .text_sm()
                                        .child(pitch_label.clone())
                                        .into_any_element(),
                                );
                            }
                            if !settings.microtonal_pitch_controls && (index == 1 || index == 2) {
                                continue;
                            }
                            controls.push(
                                crate::styles::button(("player-config-pitch", index), cx)
                                    .label(label)
                                    .tooltip(aria)
                                    .flex_1()
                                    .on_click(cx.listener(move |app, _, window, cx| {
                                        app.adjust_musical_speed(step, window, cx)
                                    }))
                                    .into_any_element(),
                            );
                        }
                        div.child(
                            gpui::div()
                                .h_flex()
                                .gap_2()
                                .items_center()
                                .children(controls),
                        )
                    })
                    .child(
                        Checkbox::new("player-config-preserve-pitch")
                            .label(app.tr("setting.preservePitch"))
                            .checked(settings.preserve_pitch)
                            .on_click(cx.listener(|app, checked, _, cx| {
                                app.state.settings.preserve_pitch = *checked;
                                app.apply_audio_settings(cx);
                            })),
                    )
                    .children(
                        [
                            (0, "setting.showQueueInSidebar", panel.queue),
                            (1, "setting.showLyricsInSidebar", panel.lyrics),
                            (2, "setting.showVisualizerInSidebar", panel.visualizer),
                            (3, "setting.combinedLyricsAndVisualizer", panel.combined),
                        ]
                        .into_iter()
                        .map(|(index, key, checked)| {
                            gpui::div()
                                .debug_selector(move || format!("player-config-panel-{index}"))
                                .child(
                                    Checkbox::new(("player-config-panel", index as usize))
                                        .label(app.tr(key))
                                        .checked(checked)
                                        .on_click(cx.listener(move |app, checked, _, cx| {
                                            let panel = &mut app.state.settings.panels;
                                            match index {
                                                0 => panel.queue = *checked,
                                                1 => panel.lyrics = *checked,
                                                2 => panel.visualizer = *checked,
                                                _ => panel.combined = *checked,
                                            }
                                            app.save();
                                            cx.notify();
                                        })),
                                )
                        }),
                    )
                    .into_any_element()
            })
            .unwrap_or_else(|_| gpui::div().into_any_element())
    }
}
