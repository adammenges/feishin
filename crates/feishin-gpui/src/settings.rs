use crate::{app::Feishin, scroll::VerticalScroll};
use feishin_core::{settings_search, storage};
use gpui::{
    AnyElement, App, Context, ElementId, Entity, PromptLevel, Role, Subscription, Window, div,
    prelude::*, relative, rems,
};
use gpui_component::input::{InputEvent, InputState, NumberInputEvent, StepAction};
use gpui_component::{ActiveTheme, StyledExt, checkbox::Checkbox};

pub(crate) fn option_row(
    id: &'static str,
    key: &str,
    control: AnyElement,
    view: &Feishin,
    cx: &App,
) -> AnyElement {
    div()
        .id(id)
        .debug_selector(move || format!("{id}-row"))
        .h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .gap(rems(0.75))
        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
        .child(
            div()
                .v_flex()
                .debug_selector(move || format!("{id}-caption"))
                .gap(rems(0.25))
                .max_w(relative(0.5))
                .min_w_0()
                .text_size(rems(0.875))
                .line_height(rems(1.125))
                .child(
                    div()
                        .h_flex()
                        .gap(rems(1.))
                        .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                        .child(view.tr(key))
                        .when(key == "setting.customCssEnable", |div| {
                            let note = view.tr("setting.customCssNotice");
                            div.child(
                                gpui::div()
                                    .id("theme-css-notice")
                                    .role(Role::Image)
                                    .aria_label(note.clone())
                                    .tooltip(move |window, cx| {
                                        gpui_component::tooltip::Tooltip::new(note.clone())
                                            .max_w(rems(24.))
                                            .build(window, cx)
                                    })
                                    .child(
                                        gpui_component::Icon::default()
                                            .path("native-icons/info.svg")
                                            .size(rems(0.875)),
                                    ),
                            )
                        }),
                )
                .when_some(
                    feishin_core::i18n::translate(
                        &view.state.settings.language,
                        &format!("{key}_description"),
                    ),
                    |div, description| {
                        div.child(
                            gpui::div()
                                .text_size(rems(0.8125))
                                .line_height(rems(1.))
                                .text_color(cx.theme().muted_foreground)
                                .child(description),
                        )
                    },
                ),
        )
        .child(
            div()
                .debug_selector(move || format!("{id}-control"))
                .h_flex()
                .min_w_0()
                .justify_end()
                .child(control),
        )
        .into_any_element()
}

pub(crate) struct List {
    language: String,
    query: String,
    tab: settings_search::Tab,
    children: Vec<(usize, AnyElement)>,
}

impl FluentBuilder for List {}
impl List {
    pub(crate) fn new(language: &str, query: &str, tab: settings_search::Tab) -> Self {
        Self {
            language: language.into(),
            query: query.into(),
            tab,
            children: vec![],
        }
    }
    pub(crate) fn setting(mut self, titles: &[&str], child: impl IntoElement) -> Self {
        let section = settings_search::section(titles);
        debug_assert!(
            section.is_some(),
            "Unclassified settings control: {titles:?}"
        );
        if let Some(section) = section
            && settings_search::SECTIONS[section].tab == self.tab
            && settings_search::matches(&self.language, &self.query, titles)
        {
            self.children.push((section, child.into_any_element()));
        }
        self
    }
    /// These controls already apply their individual search and visibility predicates.
    pub(crate) fn settings_children(
        mut self,
        group: &str,
        children: impl IntoIterator<Item = impl IntoElement>,
    ) -> Self {
        let section = settings_search::section(&[group]).expect("Known settings section");
        if settings_search::SECTIONS[section].tab == self.tab {
            self.children.extend(
                children
                    .into_iter()
                    .map(|child| (section, child.into_any_element())),
            );
        }
        self
    }
    pub(crate) fn render(mut self, cx: &gpui::App) -> AnyElement {
        self.children.sort_by_key(|(section, _)| *section);
        let mut groups: Vec<(usize, Vec<AnyElement>)> = Vec::new();
        for (section, child) in self.children {
            if groups
                .last()
                .is_none_or(|(previous, _)| *previous != section)
            {
                groups.push((section, vec![]));
            }
            groups.last_mut().unwrap().1.push(child);
        }
        div()
            .id(gpui::SharedString::from(format!(
                "settings-options-{}",
                self.tab.id()
            )))
            .debug_selector(|| "settings-options".into())
            .v_flex()
            .gap_4()
            .flex_1()
            .min_h_0()
            .w_full()
            .px(cx.theme().font_size * 0.5)
            .py(cx.theme().font_size)
            .native_vertical_scroll()
            .when(groups.is_empty(), |div| {
                div.child(feishin_core::i18n::text(&self.language, "No results"))
            })
            .children(
                groups
                    .into_iter()
                    .enumerate()
                    .map(|(index, (section, children))| {
                        let section = &settings_search::SECTIONS[section];
                        div()
                            .id(section.group)
                            .v_flex()
                            .gap_4()
                            .when(section.group == "theme-settings", |div| {
                                div.gap(gpui::rems(0.75))
                            })
                            .when(index > 0, |div| {
                                div.border_t_1().border_color(cx.theme().border).pt_4()
                            })
                            .when(!section.title.is_empty(), |div| {
                                div.child(
                                    gpui::div()
                                        .debug_selector(move || {
                                            format!("settings-section-{}", section.group)
                                        })
                                        .text_size(cx.theme().font_size * 1.25)
                                        .line_height(cx.theme().font_size * 1.875)
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(feishin_core::i18n::text(
                                            &self.language,
                                            section.title,
                                        )),
                                )
                            })
                            .child(
                                div()
                                    .v_flex()
                                    .gap_5()
                                    .px(cx.theme().font_size * 1.5)
                                    .children(children),
                            )
                    }),
            )
            .into_any_element()
    }
}

impl Feishin {
    pub(crate) fn settings_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        use gpui_component::{Selectable, button::ButtonVariants};
        use settings_search::Tab;
        let rtl = crate::appearance::rtl(cx);
        div()
            .id("settings-tabs")
            .debug_selector(|| "settings-tabs".into())
            .role(gpui::Role::TabList)
            .aria_label(self.tr("Settings"))
            .relative()
            .h_flex()
            .flex_wrap()
            .flex_shrink_0()
            .when(rtl, |div| div.flex_row_reverse())
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(gpui::px(self.state.settings.zoom))
                    .bg(cx.theme().border),
            )
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    let current = Tab::ALL
                        .iter()
                        .position(|tab| *tab == this.state.settings.settings_tab)
                        .unwrap();
                    let next = match event.keystroke.key.as_str() {
                        "left" => (current + if rtl { 1 } else { 4 }) % 5,
                        "right" => (current + if rtl { 4 } else { 1 }) % 5,
                        "home" => 0,
                        "end" => 4,
                        _ => return,
                    };
                    this.state.settings.settings_tab = Tab::ALL[next];
                    this.settings_tab_focus[next].focus(window);
                    this.save();
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .children(Tab::ALL.into_iter().enumerate().map(|(index, tab)| {
                let selected = tab == self.state.settings.settings_tab;
                gpui_component::button::Button::new(gpui::SharedString::from(format!(
                    "settings-tab-{}",
                    tab.id()
                )))
                .debug_selector(move || format!("settings-tab-{}", tab.id()))
                .role(gpui::Role::Tab)
                .label(self.tr(tab.title()))
                .ghost()
                .selected(selected)
                .tab_stop(selected)
                .focus_handle(&self.settings_tab_focus[index])
                .rounded_none()
                .h(cx.theme().font_size * 2.4375)
                .px(cx.theme().font_size * 0.75)
                .text_size(cx.theme().font_size * 0.8125)
                .line_height(cx.theme().font_size * 0.8125)
                .font_weight(gpui::FontWeight::MEDIUM)
                .bg(gpui::transparent_black())
                .text_color(cx.theme().foreground)
                .border_0()
                .border_b_2()
                .border_color(if selected {
                    cx.theme().primary
                } else {
                    gpui::transparent_black()
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.state.settings.settings_tab = tab;
                    this.settings_tab_focus[index].focus(window);
                    this.save();
                    cx.notify();
                }))
            }))
            .into_any_element()
    }

    pub(crate) fn apply_control_number(&mut self, index: usize, cx: &mut Context<Self>) {
        let raw = self.volume_settings_inputs[index].read(cx).value();
        let value = if raw.trim().is_empty() {
            if index != 0 {
                return;
            }
            Some(0.)
        } else {
            raw.parse::<f32>().ok().filter(|value| value.is_finite())
        };
        match value {
            Some(value) if index == 0 && (0. ..=180.).contains(&value) => {
                self.state.settings.volume_width = value
            }
            Some(value) if index == 1 && value.fract() == 0. => {
                self.state.settings.artist_radio_count = value.clamp(10., 100.) as usize
            }
            Some(value) if index == 2 => self.state.settings.button_size = value.clamp(15., 30.),
            _ => {
                self.message =
                    "Enter a finite control size or a whole song count within the displayed limits"
                        .into();
                cx.notify();
                return;
            }
        }
        self.message.clear();
        self.save();
        cx.notify();
    }

    pub(crate) fn scrobble_controls(&self, content: List, cx: &mut Context<Self>) -> List {
        use gpui_component::menu::{DropdownMenu, PopupMenuItem};
        use storage::ScrobbleMode;
        let row = |key: &str, control: AnyElement| {
            div()
                .v_flex()
                .gap_2()
                .child(self.tr(key))
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.tr(&format!("{key}_description"))),
                )
                .child(control)
        };
        let mode = self.state.settings.scrobble_mode;
        let mode_key = |mode| match mode {
            ScrobbleMode::Both => "setting.scrobbleMinimumMode_optionBoth",
            ScrobbleMode::Seconds => "setting.scrobbleMinimumMode_optionSeconds",
            ScrobbleMode::Percentage => "setting.scrobbleMinimumMode_optionPercentage",
        };
        content
            .setting(
                &[
                    "Scrobble to server",
                    "setting.scrobble",
                    "scrobble-settings",
                ],
                self.setting_checkbox(
                    "scrobble",
                    "setting.scrobble",
                    self.state.settings.scrobble,
                    cx,
                )
                .debug_selector(|| "scrobble".into())
                .on_click(cx.listener(|this, value: &bool, _, cx| {
                    this.state.settings.scrobble = *value;
                    this.save();
                    cx.notify();
                })),
            )
            .setting(
                &[
                    "Scrobble threshold mode",
                    "setting.scrobbleMinimumMode",
                    "scrobble-settings",
                ],
                row(
                    "setting.scrobbleMinimumMode",
                    crate::styles::button("scrobble-mode", cx)
                        .debug_selector(|| "scrobble-mode".into())
                        .label(self.tr(mode_key(mode)))
                        .aria_label(self.tr("setting.scrobbleMinimumMode"))
                        .dropdown_menu({
                            let view = cx.weak_entity();
                            move |mut menu, _, cx| {
                                for choice in [
                                    ScrobbleMode::Both,
                                    ScrobbleMode::Seconds,
                                    ScrobbleMode::Percentage,
                                ] {
                                    let view = view.clone();
                                    menu =
                                        menu.item(
                                            PopupMenuItem::new(crate::appearance::text(
                                                mode_key(choice),
                                                cx,
                                            ))
                                            .checked(mode == choice)
                                            .on_click(move |_, _, cx| {
                                                view.update(cx, |this, cx| {
                                                    this.state.settings.scrobble_mode = choice;
                                                    this.save();
                                                    cx.notify();
                                                })
                                                .ok();
                                            }),
                                        );
                                }
                                menu
                            }
                        })
                        .into_any_element(),
                ),
            )
            .when(mode != ScrobbleMode::Seconds, |content| {
                content.setting(
                    &[
                        "Scrobble percentage",
                        "setting.minimumScrobblePercentage",
                        "scrobble-settings",
                    ],
                    row(
                        "setting.minimumScrobblePercentage",
                        div()
                            .debug_selector(|| "scrobble-percentage".into())
                            .w(gpui::px(180.))
                            .child(
                                crate::styles::slider(&self.scrobble_percentage_slider, cx)
                                    .aria_label(self.tr("setting.minimumScrobblePercentage")),
                            )
                            .child(format!("{}%", self.state.settings.scrobble_percentage))
                            .into_any_element(),
                    ),
                )
            })
            .when(mode != ScrobbleMode::Percentage, |content| {
                content.setting(
                    &[
                        "Scrobble after seconds",
                        "setting.minimumScrobbleSeconds",
                        "scrobble-settings",
                    ],
                    row(
                        "setting.minimumScrobbleSeconds",
                        div()
                            .debug_selector(|| "scrobble-seconds".into())
                            .w(gpui::px(180.))
                            .child(crate::styles::number_input(&self.audio_numbers[8], cx))
                            .into_any_element(),
                    ),
                )
            })
            .setting(
                &[
                    "Enable song notifications",
                    "setting.notify",
                    "scrobble-settings",
                ],
                self.setting_checkbox(
                    "track-notifications",
                    "setting.notify",
                    self.state.settings.notify_tracks,
                    cx,
                )
                .debug_selector(|| "track-notifications".into())
                .on_click(cx.listener(|this, value: &bool, _, cx| {
                    this.state.settings.notify_tracks = *value;
                    this.save();
                    cx.notify();
                })),
            )
    }

    pub(crate) fn setting_checkbox(
        &self,
        id: impl Into<ElementId>,
        key: &str,
        checked: bool,
        cx: &Context<Self>,
    ) -> Checkbox {
        Checkbox::new(id)
            .label(self.tr(key))
            .checked(checked)
            .when(crate::appearance::rtl(cx), |checkbox| {
                checkbox.flex_row_reverse()
            })
            .when_some(
                feishin_core::i18n::translate(
                    &self.state.settings.language,
                    &format!("{key}_description"),
                ),
                |checkbox, description| {
                    checkbox.child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(description),
                    )
                },
            )
    }

    pub fn init_number_steps(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let inputs = [
            (&self.speed_input, 0.5, 2., 0.05),
            (&self.fullscreen_idle_input, 0., 120., 1.),
            (&self.queue_autosave_input, 1., f64::from(u32::MAX), 1.),
            (&self.playlist_folder_indent_input, 0., 64., 1.),
            (&self.remote_port, 1., f64::from(u16::MAX), 1.),
            (&self.sample_rate_input, 0., f64::from(u32::MAX), 1.),
            (&self.bitrate_input, 0., f64::from(u32::MAX), 1.),
            (&self.volume_settings_inputs[0], 30., 180., 1.),
            (&self.volume_settings_inputs[1], 10., 200., 1.),
            (&self.volume_settings_inputs[2], 15., 30., 1.),
            (&self.album_group_size, 0., 2000., 1.),
            (&self.share_amount, 1., f64::from(u32::MAX), 1.),
            (&self.sleep_inputs[0], 0., 23., 1.),
            (&self.sleep_inputs[1], 0., 59., 1.),
            (&self.sleep_inputs[2], 0., 59., 1.),
        ]
        .map(|(input, min, max, step)| (input.clone(), min, max, step));
        for (input, min, max, step) in inputs {
            self.subscriptions
                .extend(Self::bind_number_step(&input, min, max, step, window, cx));
        }
    }

    pub(crate) fn bind_number_step(
        input: &Entity<InputState>,
        min: f64,
        max: f64,
        step: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        vec![
            cx.subscribe_in(input, window, |_, _, _: &InputEvent, _, cx| cx.notify()),
            cx.subscribe_in(
                input,
                window,
                move |_, input, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let current = input
                        .read(cx)
                        .value()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .unwrap_or(min);
                    let mut value = ((current
                        + if *action == StepAction::Increment {
                            step
                        } else {
                            -step
                        })
                        * 1_000_000.)
                        .round()
                        / 1_000_000.;
                    value = value.clamp(min, max);
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                    cx.notify();
                },
            ),
        ]
    }

    pub fn apply_transcoding_settings(&mut self, cx: &mut Context<Self>) {
        let result = (|| -> anyhow::Result<_> {
            let number = |input: &gpui::Entity<gpui_component::input::InputState>| -> anyhow::Result<Option<u32>> {
                let value = input.read(cx).value();
                let value = value.trim();
                Ok(if value.is_empty() { None } else { Some(value.parse()?) })
            };
            let mut settings = self.state.settings.clone();
            settings.bitrate = number(&self.bitrate_input)?;
            settings.max_sample_rate = number(&self.sample_rate_input)?;
            settings.format = self
                .transcode_format_input
                .read(cx)
                .value()
                .trim()
                .to_owned();
            settings.validate_transcoding()?;
            Ok(settings)
        })();
        match result {
            Ok(settings) => {
                self.state.settings = settings;
                self.message.clear();
                self.save();
            }
            Err(error) => self.message = error.to_string(),
        }
        cx.notify();
    }

    pub(crate) fn settings_header(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .debug_selector(|| "settings-page-header".into())
            .h_flex()
            .flex_wrap()
            .flex_basis(gpui::px(65. * self.state.settings.zoom))
            .w_full()
            .px(cx.theme().font_size * 0.75)
            .justify_between()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .child(
                div()
                    .id("settings-page-title")
                    .debug_selector(|| "settings-page-title".into())
                    .role(gpui::Role::Heading)
                    .min_w_0()
                    .text_size(cx.theme().font_size * 2.25)
                    .line_height(cx.theme().font_size * 2.75)
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(self.tr("Settings")),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .max_w_full()
                    .gap(cx.theme().font_size * 0.75)
                    .pr(cx.theme().font_size * 2.)
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .child(self.settings_search_control(cx))
                    .child(
                        crate::styles::button("reset-settings", cx)
                            .debug_selector(|| "reset-settings".into())
                            .label(self.tr("common.resetToDefault"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.reset_settings(window, cx)),
                            ),
                    ),
            )
            .into_any_element()
    }
    pub(crate) fn save_for_restart(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.save_enabled,
            "Cannot restart with unsaved preferences; the original settings file was preserved"
        );
        storage::save(&self.data_dir.join("state.json"), &self.state)
    }
    pub(crate) fn restart_native(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.remember_window(window);
        if let Err(error) = self.save_for_restart() {
            self.message = format!("Cannot save preferences before restarting: {error}");
            log::warn!("{}", self.message);
            cx.notify();
            return;
        }
        if self.automatic_updates_enabled() && self.updates.ready.is_some() {
            self.install_update(true, cx);
            return;
        }
        let result = std::env::current_exe()
            .map_err(anyhow::Error::from)
            .and_then(|executable| {
                feishin_core::instance::restart_command(&executable, &self.data_dir)
                    .spawn()
                    .map_err(anyhow::Error::from)
            });
        match result {
            Ok(_) => cx.quit(),
            Err(error) => {
                log::warn!("Cannot restart Feishin: {error}");
                self.message = format!("Cannot restart Feishin: {error}");
                cx.notify();
            }
        }
    }
    pub(crate) fn reset_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let confirmation = window.prompt(
            PromptLevel::Warning,
            &self.tr("common.resetToDefault"),
            Some(&self.tr("common.areYouSure")),
            &[self.tr("Cancel").as_str(), self.tr("common.reset").as_str()],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if confirmation.await != Ok(1) {
                return;
            }
            this.update_in(cx, |this, window, cx| {
                let state_path = this.data_dir.join("state.json");
                let backup = this
                    .data_dir
                    .join(format!("state-before-reset-{}.json", uuid::Uuid::new_v4()));
                let mut next = this.state.clone();
                // These belong to the original player store, rather than store_settings.
                next.settings = storage::Settings {
                    volume: next.settings.volume,
                    muted: next.settings.muted,
                    speed: next.settings.speed,
                    ..Default::default()
                };
                next.settings.discord_config.last_fm_key_enabled = Some(false);
                if let Err(error) = storage::save(&backup, &this.state)
                    .and_then(|()| storage::save(&state_path, &next))
                {
                    log::warn!("Could not reset preferences: {error}");
                    this.message = format!("Could not reset preferences: {error}");
                    cx.notify();
                    return;
                }
                this.state = next;
                this.restore_player_modes();
                this.sync_tls_settings(cx);
                this.refresh_setting_inputs(window, cx);
                this.reset_playlists();
                this.reset_auto_dj();
                this.playback_filter_inputs.clear();
                this.playlist_folder_path.clear();
                this.smart_editor = None;
                this.remote_generation += 1;
                this.remote = None;
                this.remote_password
                    .update(cx, |input, cx| input.set_value("", window, cx));
                this.settings_search
                    .update(cx, |input, cx| input.set_value("", window, cx));
                this.install_shortcuts(cx);
                this.update_presence(cx);
                log::set_max_level(if this.state.settings.log_level == "debug" {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                });
                this.media_attempted = false;
                this.sync_media_controls(window);
                this.sync_jukebox(cx);
                this.reload(false, cx);
                this.message = format!(
                    "{}. {}: {}",
                    this.tr("common.resetToDefault"),
                    this.tr("Backup"),
                    backup.file_name().unwrap().to_string_lossy()
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Route;
    use feishin_core::{
        model::{Item, ItemKind, Server, ServerType},
        queue::Enqueue,
        storage::State,
    };
    use gpui::{Entity, Modifiers, Render, Subscription, TestAppContext, VisualTestContext};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn complete_settings_header_matches_original_page(cx: &mut TestAppContext) {
        use gpui::{AppContext, px, size};
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("feishin-settings-page-{}", uuid::Uuid::new_v4()));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.window_bar_style = "web".into();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| {
                let mut view = Feishin::new(directory.clone(), Ok(state), window, cx);
                view.route = Route::Settings;
                view
            });
            Root::new(app, window, cx)
        });
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/settings-page-header-reference.json"
        ))
        .unwrap();
        let mut checks = vec![
            ("settings-page-header", &reference["header"]),
            ("settings-page-title", &reference["title"]),
            ("settings-tabs", &reference["tabs"]),
            ("reset-settings", &reference["buttons"][1]["bounds"]),
            (
                "settings-section-theme-settings",
                &reference["sectionTitle"],
            ),
        ];
        // The original web build omits the desktop-only Window tab.
        for (index, id) in [
            "settings-tab-general",
            "settings-tab-playback",
            "settings-tab-hotkeys",
        ]
        .into_iter()
        .enumerate()
        {
            checks.push((id, &reference["tabItems"][index]["bounds"]));
        }
        for (selector, expected) in checks {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"));
            for (actual, expected) in [
                bounds.origin.x,
                bounds.origin.y,
                bounds.size.width,
                bounds.size.height,
            ]
            .into_iter()
            .zip(expected.as_array().unwrap())
            {
                assert!(
                    (f32::from(actual) - expected.as_f64().unwrap() as f32).abs() <= 1.,
                    "{selector}: {bounds:?} differs from {expected}"
                );
            }
        }
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn control_sizes_and_volume_wheel_match_original_callbacks_and_persist_independently(
        cx: &mut TestAppContext,
    ) {
        use gpui::{
            MouseButton, Role,
            accesskit::{Action, ActionData, ActionRequest, TreeId},
            point, px,
        };
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/control-settings-reference.json"))
                .unwrap();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-control-settings-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.queue.add(
                vec![
                    Item {
                        id: "same".into(),
                        ..Default::default()
                    };
                    2
                ],
                Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(px(1500.), px(1200.)));
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        for language in ["en", "fr", "ar"] {
            for (index, key, field) in [
                (0, "volumeWidth", "volume_width"),
                (1, "artistRadioCount", "artist_radio_count"),
                (2, "buttonSize", "button_size"),
            ] {
                let key = format!("setting.{key}");
                let control = reference[0]["controls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|control| control["title"]["key"] == key)
                    .unwrap();
                let original = &control["inputs"][0];
                let label = feishin_core::i18n::translate(language, &key).unwrap();
                let input = app.read_with(cx, |view, _| view.volume_settings_inputs[index].clone());
                for change in original["changes"].as_array().unwrap() {
                    let raw = change["value"].as_str().unwrap();
                    cx.update(|window, cx| {
                        app.update(cx, |view, cx| {
                            view.state.settings.volume_width = 70.;
                            view.state.settings.artist_radio_count = 20;
                            view.state.settings.button_size = 15.;
                            view.state.settings.language = language.into();
                            view.route = Route::Settings;
                            view.settings_search
                                .update(cx, |input, cx| input.set_value(label.clone(), window, cx));
                            view.refresh_setting_inputs(window, cx);
                            view.apply_theme(window, cx);
                            view.save();
                            cx.notify();
                        })
                    });
                    cx.run_until_parked();
                    assert!(cx.update(|window, _| {
                        window
                            .accessibility_tree_for_test()
                            .unwrap()
                            .nodes
                            .iter()
                            .any(|(_, node)| {
                                node.role() == Role::TextInput
                                    && node.label() == Some(label.as_str())
                            })
                    }));
                    let before = app.read_with(cx, |view, _| {
                        serde_json::to_value(&view.state.settings).unwrap()
                    });
                    cx.update(|window, cx| {
                        input.update(cx, |input, cx| input.set_value(raw.to_owned(), window, cx))
                    });
                    assert_eq!(
                        app.read_with(cx, |view, _| serde_json::to_value(&view.state.settings)
                            .unwrap()[field]
                            .clone()),
                        before[field]
                    );
                    input.update(cx, |_, cx| cx.emit(InputEvent::Blur));
                    cx.run_until_parked();
                    let valid = raw.is_empty()
                        || raw.parse::<f32>().is_ok_and(|value| {
                            value.is_finite() && (index != 0 || (0. ..=180.).contains(&value))
                        });
                    let expected = if valid {
                        change["settings"]["general"][key.trim_start_matches("setting.")]
                            .as_f64()
                            .unwrap_or(original["value"].as_f64().unwrap())
                    } else {
                        original["value"].as_f64().unwrap()
                    };
                    let mut saved = storage::load(&dir.join("state.json")).unwrap();
                    assert_eq!(
                        serde_json::to_value(&saved.settings).unwrap()[field].as_f64(),
                        Some(expected),
                        "{language} {key} {raw}"
                    );
                    feishin_core::migration::validate(&mut saved.settings).unwrap();
                    assert_eq!(serde_json::to_value(saved.queue).unwrap(), queue);
                }
            }
            let label = feishin_core::i18n::translate(language, "setting.volumeWheelStep").unwrap();
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.settings_search
                        .update(cx, |input, cx| input.set_value(label.clone(), window, cx));
                    cx.notify();
                })
            });
            cx.run_until_parked();
            let (id, node) = cx.update(|window, _| {
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .find(|(_, node)| {
                        node.role() == Role::Slider && node.label() == Some(label.as_str())
                    })
                    .cloned()
                    .unwrap()
            });
            assert_eq!(node.min_numeric_value(), Some(1.));
            assert_eq!(node.max_numeric_value(), Some(20.));
            let control = reference[0]["controls"]
                .as_array()
                .unwrap()
                .iter()
                .find(|control| control["title"]["key"] == "setting.volumeWheelStep")
                .unwrap();
            for change in control["inputs"][0]["changes"].as_array().unwrap() {
                let value = change["value"].as_f64().unwrap();
                cx.update(|window, cx| {
                    window.dispatch_accessibility_action_for_test(
                        ActionRequest {
                            action: Action::SetValue,
                            target_tree: TreeId::ROOT,
                            target_node: id,
                            data: Some(ActionData::NumericValue(value)),
                        },
                        cx,
                    )
                });
                cx.run_until_parked();
                assert_eq!(
                    f64::from(
                        storage::load(&dir.join("state.json"))
                            .unwrap()
                            .settings
                            .volume_wheel_step
                    ),
                    value
                );
            }
            let bounds = cx.debug_bounds("volume-wheel-step").unwrap();
            let position = point(bounds.left() + bounds.size.width * 0.25, bounds.center().y);
            cx.simulate_mouse_down(position, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            assert_eq!(
                app.read_with(cx, |view, _| view.state.settings.volume_wheel_step),
                20.,
                "Wheel step saves on release, not while dragging"
            );
            cx.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            assert!(
                app.read_with(cx, |view, _| view.state.settings.volume_wheel_step < 20.
                    && !view.playing)
            );
        }
        cx.update(|window, cx| {
            window.activate_window();
            app.update(cx, |view, cx| {
                let label = view.tr("setting.buttonSize");
                view.settings_search
                    .update(cx, |input, cx| input.set_value(label, window, cx));
                view.volume_settings_inputs[0]
                    .update(cx, |input, cx| input.set_value("Infinity", window, cx));
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.volume_settings_inputs[2].update(cx, |input, cx| {
                    input.focus(window, cx);
                    input.set_value("28", window, cx);
                });
            })
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.settings_search
                    .update(cx, |input, cx| input.focus(window, cx));
            })
        });
        cx.run_until_parked();
        let saved = storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(saved.settings.button_size, 28.);
        assert_eq!(
            saved.settings.volume_width, 70.,
            "An invalid unrelated draft must not block or alter a valid blur save"
        );
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn scrobble_controls_match_original_modes_limits_and_localized_changes(
        cx: &mut TestAppContext,
    ) {
        use gpui::{
            Role,
            accesskit::{Action, ActionData, ActionRequest, TreeId},
            point, px,
        };
        fn node(
            cx: &mut VisualTestContext,
            role: Role,
            label: &str,
        ) -> Option<(gpui::accesskit::NodeId, gpui::accesskit::Node)> {
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
                tree.nodes
                    .iter()
                    .find(|(id, node)| {
                        node.role() == role
                            && consumer
                                .state()
                                .node_by_tree_local_id(*id, TreeId::ROOT)
                                .unwrap()
                                .label()
                                .as_deref()
                                == Some(label)
                    })
                    .cloned()
            })
        }
        fn click(cx: &mut VisualTestContext, role: Role, label: &str) {
            let bounds = node(cx, role, label)
                .unwrap_or_else(|| panic!("Missing {role:?}: {label}"))
                .1
                .bounds()
                .unwrap();
            let position = cx.update(|window, _| {
                let scale = f64::from(window.scale_factor());
                point(
                    px(((bounds.x0 + bounds.x1) / (2. * scale)) as f32),
                    px(((bounds.y0 + bounds.y1) / (2. * scale)) as f32),
                )
            });
            cx.simulate_click(position, Modifiers::none());
            cx.run_until_parked();
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/scrobble-settings-reference.json"))
                .unwrap();
        let dir = std::env::temp_dir().join(format!(
            "feishin-scrobble-settings-{}",
            uuid::Uuid::new_v4()
        ));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    };
                    2
                ],
                Enqueue::End,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(gpui::size(px(1400.), px(1600.)));
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        for language in ["en", "fr", "ar"] {
            let tr = |key: &str| feishin_core::i18n::translate(language, key).unwrap();
            for case in &reference {
                cx.update(|window, cx| {
                    app.update(cx, |view, cx| {
                        view.state.settings.language = language.into();
                        view.state.settings.scrobble_mode =
                            serde_json::from_value(case["mode"].clone()).unwrap();
                        view.state.settings.scrobble = case["enabled"].as_bool().unwrap();
                        view.state.settings.notify_tracks = case["enabled"].as_bool().unwrap();
                        view.state.settings.scrobble_percentage = 75.;
                        view.state.settings.scrobble_seconds = 240.;
                        view.refresh_setting_inputs(window, cx);
                        view.settings_search.update(cx, |input, cx| {
                            input.set_value("scrobble-settings", window, cx)
                        });
                        view.route = Route::Settings;
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                for (key, role) in [
                    ("setting.minimumScrobblePercentage", Role::Slider),
                    ("setting.minimumScrobbleSeconds", Role::TextInput),
                ] {
                    let original = case["controls"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|control| control["title"] == key)
                        .unwrap();
                    assert_eq!(
                        node(cx, role, &tr(key)).is_none(),
                        original["hidden"].as_bool().unwrap(),
                        "{language}: {case}"
                    );
                }
                for (key, field) in [
                    ("setting.scrobble", "scrobble"),
                    ("setting.notify", "notify_tracks"),
                ] {
                    let label = tr(key);
                    let enabled = case["enabled"].as_bool().unwrap();
                    assert_eq!(
                        node(cx, Role::CheckBox, &label).unwrap().1.toggled(),
                        Some(if enabled {
                            gpui::Toggled::True
                        } else {
                            gpui::Toggled::False
                        })
                    );
                    click(cx, Role::CheckBox, &label);
                    let original = case["controls"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|control| control["title"] == key)
                        .unwrap();
                    let expected = original["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|change| change["value"] == !enabled)
                        .unwrap();
                    app.read_with(cx, |view, _| {
                        let value = serde_json::to_value(&view.state.settings).unwrap();
                        assert_eq!(
                            value[field],
                            expected["settings"]["playback"]["scrobble"][if field == "scrobble" {
                                "enabled"
                            } else {
                                "notify"
                            }]
                        );
                    });
                }
                let percentage = tr("setting.minimumScrobblePercentage");
                if let Some((id, slider)) = node(cx, Role::Slider, &percentage) {
                    assert_eq!(slider.min_numeric_value(), Some(25.));
                    assert_eq!(slider.max_numeric_value(), Some(90.));
                    for value in [25., 50., 75., 90.] {
                        cx.update(|window, cx| {
                            window.dispatch_accessibility_action_for_test(
                                ActionRequest {
                                    action: Action::SetValue,
                                    target_tree: TreeId::ROOT,
                                    target_node: id,
                                    data: Some(ActionData::NumericValue(value)),
                                },
                                cx,
                            )
                        });
                        cx.run_until_parked();
                        assert_eq!(
                            app.read_with(cx, |view, _| f64::from(
                                view.state.settings.scrobble_percentage
                            )),
                            value
                        );
                    }
                }
                if node(cx, Role::TextInput, &tr("setting.minimumScrobbleSeconds")).is_some() {
                    let control = case["controls"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|control| control["title"] == "setting.minimumScrobbleSeconds")
                        .unwrap();
                    for change in control["changes"].as_array().unwrap() {
                        let before =
                            app.read_with(cx, |view, _| view.state.settings.scrobble_seconds);
                        let value = change["value"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| change["value"].to_string());
                        cx.update(|window, cx| {
                            let input = app.read(cx).audio_numbers[8].clone();
                            input.update(cx, |input, cx| {
                                input.set_value(value, window, cx);
                            });
                        });
                        cx.run_until_parked();
                        let expected =
                            change["settings"]["playback"]["scrobble"]["scrobbleAtDuration"]
                                .as_f64()
                                .unwrap_or(f64::from(before));
                        assert_eq!(
                            app.read_with(cx, |view, _| f64::from(
                                view.state.settings.scrobble_seconds
                            )),
                            expected
                        );
                    }
                }
                if case["mode"] == "both" && case["enabled"] == true {
                    let thresholds = app.read_with(cx, |view, _| {
                        (
                            view.state.settings.scrobble_percentage,
                            view.state.settings.scrobble_seconds,
                        )
                    });
                    let choices = &case["controls"][1]["choices"];
                    for choice in choices.as_array().unwrap() {
                        click(cx, Role::Button, &tr("setting.scrobbleMinimumMode"));
                        let context = choice["label"]["options"]["context"].as_str().unwrap();
                        click(
                            cx,
                            Role::MenuItem,
                            &tr(&format!("setting.scrobbleMinimumMode_{context}")),
                        );
                        app.read_with(cx, |view, _| {
                            assert_eq!(
                                serde_json::to_value(view.state.settings.scrobble_mode).unwrap(),
                                choice["value"]
                            );
                            assert_eq!(
                                (
                                    view.state.settings.scrobble_percentage,
                                    view.state.settings.scrobble_seconds
                                ),
                                thresholds
                            );
                        });
                    }
                }
                let saved = storage::load(&dir.join("state.json")).unwrap();
                assert_eq!(serde_json::to_value(saved.queue).unwrap(), queue);
                app.read_with(cx, |view, _| {
                    assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
                    assert!(!view.playing);
                });
            }
        }
        for invalid in ["-1", "1201", "NaN", "Infinity"] {
            let before = app.read_with(cx, |view, _| view.state.settings.scrobble_seconds);
            cx.update(|window, cx| {
                let input = app.read(cx).audio_numbers[8].clone();
                input.update(cx, |input, cx| {
                    input.set_value(invalid, window, cx);
                    cx.emit(InputEvent::Blur);
                });
            });
            cx.run_until_parked();
            assert_eq!(
                app.read_with(cx, |view, _| view.state.settings.scrobble_seconds),
                before
            );
        }
        for (value, action, expected) in [
            (0., StepAction::Decrement, 0.),
            (1200., StepAction::Increment, 1200.),
            (240., StepAction::Increment, 241.),
        ] {
            cx.update(|window, cx| {
                let input = app.read(cx).audio_numbers[8].clone();
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx);
                    cx.emit(NumberInputEvent::Step(action));
                });
            });
            cx.run_until_parked();
            assert_eq!(
                app.read_with(cx, |view, _| view.state.settings.scrobble_seconds),
                expected
            );
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui::test]
    fn localized_settings_search_and_confirmed_reset_preserve_library_transport_backup_and_failed_saves(
        cx: &mut TestAppContext,
    ) {
        use gpui::px;
        struct Probe {
            view: Entity<Feishin>,
            _subscription: Subscription,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                self.view.update(cx, |view, cx| view.settings_view(cx))
            }
        }
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-settings-ui-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let radio_file = dir.join("local-radio.png");
        std::fs::write(&radio_file, b"user artwork").unwrap();
        let server = uuid::Uuid::new_v4();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.language = "fr".into();
            state.settings.volume = 73.;
            state.settings.muted = true;
            state.settings.speed = 1.25;
            state.settings.ignore_ssl = true;
            state.settings.equalizer[3] = 8.;
            state.settings.equalizer_preamp = 4.;
            state.settings.show_ratings = false;
            state.settings.font.built_in = "Poppins".into();
            state.settings.remote_username = "custom-user".into();
            state.settings.discord_config.last_fm_api_key = "fixture-secret".into();
            state.settings.mpv_path = "/saved/player".into();
            state.settings.sidebar.width = 280.;
            state.settings.sidebar.playlist_filter_regex = "saved regex".into();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.radio.push(Item {
                id: "local-radio".into(),
                kind: ItemKind::Radio,
                server_id: server,
                ..Default::default()
            });
            state.queue.add(
                vec![
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    },
                    Item {
                        id: "duplicate".into(),
                        ..Default::default()
                    },
                ],
                Enqueue::Now,
            );
            state.queue.position = 12.;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.state.servers.push(Server {
                    id: server,
                    name: "Saved server".into(),
                    url: "http://127.0.0.1:1".into(),
                    username: "me".into(),
                    kind: ServerType::Subsonic,
                    legacy_auth: false,
                    options: Default::default(),
                });
                view.route = Route::Settings;
                view.settings_search
                    .update(cx, |input, cx| input.set_value("langage", window, cx));
            });
            captured.replace(Some(app.clone()));
            let probe = cx.new(|cx| Probe {
                view: app.clone(),
                _subscription: cx.observe(&app, |_, _, cx| cx.notify()),
            });
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        for width in [280., 420.] {
            cx.simulate_resize(gpui::size(px(width), px(800.)));
            cx.run_until_parked();
            assert!(
                cx.debug_bounds("settings-search-input").unwrap().size.width >= px(160.),
                "Search must remain usable in narrow content"
            );
            for selector in std::iter::once("reset-settings".to_owned()).chain(
                settings_search::Tab::ALL
                    .into_iter()
                    .map(|tab| format!("settings-tab-{}", tab.id())),
            ) {
                let bounds = cx.debug_bounds(&selector).unwrap();
                assert!(
                    bounds.left() >= px(0.) && bounds.right() <= px(width),
                    "{selector}: {bounds:?}"
                );
            }
            assert!(
                cx.debug_bounds("reset-settings").unwrap().bottom()
                    <= cx.debug_bounds("settings-tabs").unwrap().top(),
                "Settings actions overlap tabs at width {width}"
            );
        }
        cx.simulate_resize(gpui::size(px(1200.), px(900.)));
        cx.run_until_parked();
        let click = |selector, cx: &mut VisualTestContext| {
            let point = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing settings control: {selector}"))
                .center();
            cx.simulate_click(point, Modifiers::none());
            cx.run_until_parked();
        };
        click("language", cx);
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::LanguagePicker
        );
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.settings_search
                    .update(cx, |input, cx| input.set_value("", window, cx));
            })
        });
        click("settings-search-input", cx);
        cx.simulate_input("langage");
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, cx| view
                .settings_search
                .read(cx)
                .value()
                .to_string()),
            "langage"
        );
        click("language", cx);
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::LanguagePicker
        );
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        let previous = app.read_with(cx, |view, _| serde_json::to_value(&view.state).unwrap());
        cx.update(|_, cx| app.update(cx, |view, _| view.route = Route::Settings));
        click("reset-settings", cx);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(&feishin_core::i18n::text("fr", "Cancel"));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| serde_json::to_value(&view.state).unwrap()),
            previous
        );
        click("reset-settings", cx);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer(&feishin_core::i18n::text("fr", "common.reset"));
        cx.run_until_parked();
        app.read_with(cx, |view, cx| {
            assert_eq!(
                view.state.settings.language,
                State::default().settings.language
            );
            assert_eq!(view.state.settings.volume, 73.);
            assert!(view.state.settings.muted);
            assert_eq!(view.state.settings.speed, 1.25);
            assert!(!view.state.settings.ignore_ssl);
            assert!(!view.state.servers[0].options.ignore_ssl);
            assert!(!view.form_server_options(cx).ignore_ssl);
            assert_eq!(serde_json::to_value(&view.state.queue).unwrap(), queue);
            assert_eq!(view.state.servers.len(), 1);
            assert_eq!(view.state.servers[0].id, server);
            assert_eq!(view.state.radio.len(), 1);
            assert_eq!(view.state.settings.equalizer, [0.; 12]);
            assert_eq!(view.state.settings.equalizer_preamp, 0.);
            assert!(view.state.settings.mpv_path.is_empty());
            assert!(view.mpv_path.read(cx).value().is_empty());
            assert_eq!(view.audio_numbers[0].read(cx).value().as_ref(), "0");
            assert_eq!(view.sidebar_width.read(cx).value().as_ref(), "400");
            assert!(view.sidebar_regex.read(cx).value().is_empty());
            assert!(view.settings_search.read(cx).value().is_empty());
            assert!(
                view.state
                    .settings
                    .discord_config
                    .last_fm_api_key
                    .is_empty()
            );
            assert_eq!(
                view.state.settings.discord_config.last_fm_key_enabled,
                Some(false)
            );
            assert!(view.message.contains("state-before-reset-"));
        });
        assert_eq!(std::fs::read(&radio_file).unwrap(), b"user artwork");
        let backup = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("state-before-reset-")
            })
            .unwrap();
        assert_eq!(
            serde_json::to_value(storage::load(&backup).unwrap()).unwrap(),
            previous
        );
        let saved = storage::load(&dir.join("state.json")).unwrap();
        assert_eq!(saved.settings.volume, 73.);
        assert_eq!(serde_json::to_value(&saved.queue).unwrap(), queue);
        click("settings-tab-advanced", cx);
        assert!(cx.debug_bounds("restart-native").is_some());
        app.read_with(cx, |view, _| view.save_for_restart().unwrap());
        cx.update(|_, cx| app.update(cx, |view, _| view.state.settings.show_favorites = false));
        std::fs::rename(dir.join("state.json"), dir.join("preserved-state.json")).unwrap();
        std::fs::create_dir(dir.join("state.json")).unwrap();
        assert!(app.read_with(cx, |view, _| view.save_for_restart().is_err()));
        let before_failed_reset =
            app.read_with(cx, |view, _| serde_json::to_value(&view.state).unwrap());
        click("reset-settings", cx);
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Reset");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(!view.state.settings.show_favorites);
            // Pending input autosaves can report the same blocked state file
            // after the reset error. Either message must retain the write failure.
            assert!(
                view.message.starts_with("Could not reset preferences:")
                    || view.message.starts_with("Could not save settings:"),
                "{}",
                view.message
            );
        });
        let failed_backup = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                *path != backup
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("state-before-reset-")
            })
            .expect("The confirmed reset must attempt its backup before the blocked write");
        assert_eq!(
            serde_json::to_value(storage::load(&failed_backup).unwrap()).unwrap(),
            before_failed_reset
        );
        std::fs::remove_dir(dir.join("state.json")).unwrap();
        std::fs::rename(dir.join("preserved-state.json"), dir.join("state.json")).unwrap();
        cx.update(|_, cx| app.update(cx, |view, _| view.save_enabled = false));
        assert!(app.read_with(cx, |view, _| view.save_for_restart().is_err()));
        std::fs::remove_dir_all(dir).ok();
    }
}
