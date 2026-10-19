use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::Item,
    player_display::{Config, FIELDS},
    styles::Target,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, IconName, Sizable, StyledExt, WindowExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    input::{InputEvent, NumberInputEvent, StepAction},
    menu::{DropdownMenu, PopupMenuItem},
};
use std::path::{Path, PathBuf};

fn control_button(
    id: impl Into<ElementId>,
    parent: &Target,
    cx: &App,
) -> gpui_component::button::Button {
    let id = id.into();
    crate::styles::button(id.clone(), cx)
        .state_style(crate::styles::state_style_path(
            Target::new(
                "button",
                &id.to_string(),
                &[
                    "fs-button-module-root",
                    "mantine-Button-root",
                    "fs-action-icon-module-root",
                    "mantine-ActionIcon-root",
                ],
            ),
            vec![parent.clone()],
        ))
        .ghost()
        .size(cx.theme().font_size * 2.25)
        .rounded(cx.theme().font_size * (3. / 16.))
        .p_0()
}

pub(crate) fn background_album_item(item: &Item) -> Option<Item> {
    if item.id.is_empty() || item.album_id.is_empty() || item.album_id.starts_with("dummy/") {
        return None;
    }
    let cover = item.cover_id.replacen(&item.id, &item.album_id, 1);
    if cover == item.cover_id {
        return None;
    }
    Some(Item {
        cover_id: cover,
        ..item.clone()
    })
}

pub struct Background {
    pub image: PathBuf,
    pub color: Hsla,
    pub aspect: f32,
}

#[derive(IntoElement)]
pub(crate) struct LibraryBackground {
    pub image_mode: bool,
    pub image: Option<ImageSource>,
    pub color: Hsla,
    pub overlay: AnyElement,
    pub blur: Pixels,
    pub zoom: f32,
}

impl RenderOnce for LibraryBackground {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        canvas(
            move |bounds, window, cx| {
                // The original measures offsetHeight, an integer in unzoomed CSS pixels.
                let height = (f32::from(bounds.size.height) / self.zoom).round() * self.zoom;
                let overlay_height = if self.image_mode {
                    if height > 0. {
                        height + 64. * self.zoom
                    } else {
                        0.
                    }
                } else {
                    (height + 64. * self.zoom).max(200. * self.zoom)
                };
                let mut background = div()
                    .relative()
                    .w(bounds.size.width)
                    .h(px(overlay_height))
                    .when(self.image_mode, |background| {
                        background.child(
                            div()
                                .debug_selector(|| "detail-background-image".into())
                                .absolute()
                                .top_0()
                                .w_full()
                                .h(px((height - 64. * self.zoom).max(0.)))
                                .opacity(0.7)
                                .foreground_filters(vec![BackdropFilterOperation::Blur(
                                    self.blur.into(),
                                )])
                                .children(self.image.map(|source| {
                                    img(source)
                                        .absolute()
                                        .inset_0()
                                        .w(bounds.size.width)
                                        .h(px((height - 64. * self.zoom).max(0.)))
                                        .object_fit(ObjectFit::Cover)
                                })),
                        )
                    })
                    .child(
                        div()
                            .debug_selector(|| "detail-background-overlay".into())
                            .absolute()
                            .inset_0()
                            .when(!self.image_mode, |overlay| overlay.bg(self.color))
                            .opacity(if self.image_mode { 0.9 } else { 0.7 })
                            .child(self.overlay),
                    )
                    .into_any_element();
                background.layout_as_root(
                    size(
                        AvailableSpace::Definite(bounds.size.width),
                        AvailableSpace::Definite(px(overlay_height)),
                    ),
                    window,
                    cx,
                );
                background.prepaint_at(bounds.origin, window, cx);
                background
            },
            |_, mut background, window, cx| background.paint(window, cx),
        )
        .absolute()
        .inset_0()
    }
}

struct PlayerSettingsDialog {
    parent: Entity<Feishin>,
    layout: bool,
    _subscription: Subscription,
}
impl Render for PlayerSettingsDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self.parent.update(cx, |parent, cx| {
            if self.layout {
                parent.list_settings_view(cx)
            } else {
                parent.player_settings_view(cx)
            }
        });
        div()
            .id("expanded-player-settings-dialog")
            .debug_selector(|| "expanded-player-settings-dialog".into())
            .h(window.viewport_size().height * 0.75)
            .w_full()
            .child(content)
    }
}
pub const NUMBERS: &[(&str, f32, f32, f32)] = &[
    ("Artwork size (%)", 50., 100., 5.),
    ("Background blur (rem)", 0., 6., 0.5),
    ("Background shading (%)", 0., 100., 5.),
    ("Title lines", 1., 10., 1.),
    ("setting.waveformLoadingDelay", 0., 30., 1.),
    ("common.backward", 0., f32::INFINITY, 1.),
    ("common.forward", 0., f32::INFINITY, 1.),
];
fn values(config: &Config) -> [f32; 4] {
    [
        config.cover_art_size,
        config.dynamic_image_blur,
        config.opacity,
        config.title_line_count as f32,
    ]
}
impl Feishin {
    pub fn record_activity(&mut self) {
        if self.last_user_activity.elapsed() >= std::time::Duration::from_secs(1) {
            self.last_user_activity = std::time::Instant::now();
        }
    }
    pub fn poll_fullscreen_idle(&mut self, cx: &mut Context<Self>) {
        let minutes = self
            .state
            .settings
            .fullscreen_auto_open_minutes
            .clamp(0., 120.);
        let active = self.playing
            && !self.buffering
            && !self.full_player
            && !self.visualizer_display.mounted
            && self.current_entry().is_some()
            && minutes > 0.;
        if self.fullscreen_idle_state != (active, minutes.to_bits()) {
            self.fullscreen_idle_state = (active, minutes.to_bits());
            self.last_user_activity = std::time::Instant::now();
        }
        if active
            && self.last_user_activity.elapsed()
                >= std::time::Duration::from_secs_f64(minutes * 60.)
        {
            self.toggle_full_player(cx);
            self.fullscreen_idle_state = (false, minutes.to_bits());
            self.last_user_activity = std::time::Instant::now();
        }
    }

    pub fn sync_detail_background(&mut self, cx: &mut Context<Self>) {
        if self.cache_busy {
            return;
        }
        let candidate = if let Route::Detail(item) = &self.route {
            match item.kind {
                feishin_core::model::ItemKind::Album | feishin_core::model::ItemKind::Song => {
                    Some((item, feishin_core::images::Placement::ItemCard))
                }
                feishin_core::model::ItemKind::Artist
                | feishin_core::model::ItemKind::AlbumArtist => {
                    Some((item, feishin_core::images::Placement::Header))
                }
                _ => None,
            }
        } else {
            None
        };
        let Some((item, placement)) = candidate else {
            self.detail_background = None;
            self.detail_background_key.clear();
            return;
        };
        let key = self.cover_cache_key(item, placement);
        let Some(source) = self.covers.get(&key).filter(|path| path.exists()).cloned() else {
            self.detail_background = None;
            self.detail_background_key.clear();
            return;
        };
        let signature = format!(
            "{key}-{:x}-detail-color",
            feishin_core::api::cover_hash(&source.to_string_lossy())
        );
        if signature == self.detail_background_key {
            return;
        }
        self.detail_background_key = signature.clone();
        self.detail_background = None;
        let destination = self
            .data_dir
            .join("detail-backgrounds")
            .join(format!("{signature}.png"));
        let stale_path = destination.clone();
        let revision = self.state.cache_revision;
        let task = cx
            .background_executor()
            .spawn(async move { prepare_background(&source, &destination, 0.) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if this.detail_background_key != signature {
                    return;
                }
                match result {
                    Ok(background) => this.detail_background = Some(background),
                    Err(error) => {
                        log::debug!("Detail background preparation failed: {error}");
                        this.detail_background = Some(Background {
                            image: stale_path,
                            color: transparent_black(),
                            aspect: 1.,
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn detail_background_view(&self, cx: &App) -> AnyElement {
        let Route::Detail(item) = &self.route else {
            return Empty.into_any_element();
        };
        let settings = &self.state.settings;
        let (image_mode, blur) = match item.kind {
            feishin_core::model::ItemKind::Album | feishin_core::model::ItemKind::Song => {
                (settings.album_background, settings.album_background_blur)
            }
            feishin_core::model::ItemKind::Artist | feishin_core::model::ItemKind::AlbumArtist => {
                (settings.artist_background, settings.artist_background_blur)
            }
            _ => return Empty.into_any_element(),
        };
        LibraryBackground {
            image_mode,
            image: self.player_card_path(item).map(|path| path.clone().into()),
            color: self
                .detail_background
                .as_ref()
                .map_or(cx.theme().muted_foreground, |background| background.color),
            overlay: self.theme_overlay("overlay-subheader"),
            blur: cx.theme().font_size * blur,
            zoom: settings.zoom,
        }
        .into_any_element()
    }

    pub fn prepare_censored_cover_at(
        &mut self,
        item: &Item,
        placement: feishin_core::images::Placement,
        cx: &mut Context<Self>,
    ) {
        if self.cache_busy {
            return;
        }
        if !self.state.settings.blur_explicit_images || !item.explicit() {
            return;
        }
        let key = self.cover_cache_key(item, placement);
        if self.censored_covers.contains_key(&key) {
            return;
        }
        let Some(source) = self.covers.get(&key).filter(|path| path.exists()).cloned() else {
            return;
        };
        let path = self
            .data_dir
            .join("censored-covers")
            .join(format!("{key}-animated-color-managed.png"));
        self.censored_covers.insert(key.clone(), path.clone());
        if path.exists() {
            return;
        }
        let stale_path = path.clone();
        let revision = self.state.cache_revision;
        let task = cx
            .background_executor()
            .spawn(async move { prepare_background(&source, &path, 1.25) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if let Err(error) = result {
                    log::warn!("Explicit artwork preparation failed: {error}");
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn toggle_player_tab(&mut self, key: &str, cx: &mut Context<Self>) {
        self.state.settings.player_display.active_tab =
            if self.state.settings.player_display.active_tab == key {
                String::new()
            } else {
                key.into()
            };
        self.restore_player_tab(cx);
        self.save();
        cx.notify();
    }
    pub fn toggle_player_lyrics(&mut self, cx: &mut Context<Self>) {
        if self.full_player {
            self.toggle_player_tab("lyrics", cx);
        } else {
            self.state.settings.player_display.active_tab = "lyrics".into();
            self.full_player = true;
            self.visualizer_display
                .dismiss(cx.background_executor().now());
            self.persist_player_modes();
            self.restore_player_tab(cx);
            self.fetch_covers(cx);
            cx.notify();
        }
    }
    pub(crate) fn player_header_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let font = cx.theme().font_size;
        let controls = Target::new(
            "div",
            "expanded-player-header-controls",
            &[
                "full-screen-player-controls-container",
                "mantine-Group-root",
            ],
        );
        div()
            .debug_selector(|| "expanded-player-header-controls".into())
            .absolute()
            .top_0()
            .left_0()
            .p(font * 0.5)
            .h_flex()
            .gap(font * 0.5)
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .map(|div| crate::styles::apply_in(div, controls.clone(), [], cx))
            .child(
                control_button("minimize-player", &controls, cx)
                    .debug_selector(|| "minimize-player".into())
                    .icon(Icon::new(IconName::ChevronDown).with_size(font))
                    .tooltip(self.tr("common.minimize"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_full_player(cx))),
            )
            .child(
                control_button("configure-player", &controls, cx)
                    .debug_selector(|| "configure-player".into())
                    .icon(Icon::new(IconName::Settings).with_size(font))
                    .tooltip(self.tr("common.configure"))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_player_settings(window, cx)),
                    ),
            )
            .child(
                control_button("configure-player-list", &controls, cx)
                    .debug_selector(|| "configure-player-list".into())
                    .icon(Icon::new(IconName::Settings2).with_size(font))
                    .tooltip(self.tr("common.configure"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_player_settings_dialog(true, window, cx)
                    })),
            )
            .into_any_element()
    }

    pub fn player_tabs(&self, cx: &mut Context<Self>) -> AnyElement {
        let font = cx.theme().font_size;
        let controls = feishin_core::styles::Target::new(
            "div",
            "",
            &[
                "fs-full-screen-player-queue-module-controls-container",
                "full-screen-player-controls-container",
                "mantine-Group-root",
            ],
        );
        div()
            .absolute()
            .bottom_0()
            .right_0()
            .h_flex()
            .gap(font * 0.25)
            .p(font * 0.5)
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .map(|div| crate::styles::apply_in(div, controls.clone(), [], cx))
            .children(
                [
                    (
                        "queue",
                        "page.fullscreenPlayer.upNext",
                        "native-icons/queue.svg",
                    ),
                    (
                        "related",
                        "page.fullscreenPlayer.related",
                        "native-icons/related.svg",
                    ),
                    (
                        "lyrics",
                        "page.fullscreenPlayer.lyrics",
                        "native-icons/microphone.svg",
                    ),
                    (
                        "visualizer",
                        "page.fullscreenPlayer.visualizer",
                        "native-icons/visualizer.svg",
                    ),
                ]
                .into_iter()
                .filter(|(key, _, _)| {
                    *key != "visualizer" || self.state.settings.mpv_path.is_empty()
                })
                .enumerate()
                .map(|(index, (key, label, icon))| {
                    control_button(("full-player-tab", index), &controls, cx)
                        .debug_selector(move || format!("full-player-tab-{key}"))
                        .aria_label(self.tr(label))
                        .tooltip(self.tr(label))
                        .icon(Icon::default().path(icon).with_size(font).text_color(
                            if self.state.settings.player_display.active_tab == key {
                                cx.theme().primary
                            } else {
                                cx.theme().foreground
                            },
                        ))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.toggle_player_tab(key, cx)),
                        )
                }),
            )
            .into_any_element()
    }
    pub fn restore_player_tab(&mut self, cx: &mut Context<Self>) {
        self.sync_player_related(cx);
    }
    pub fn init_player_numbers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.init_waveform_controls(window, cx);
        for (index, value) in values(&self.state.settings.player_display)
            .into_iter()
            .map(f64::from)
            .chain([
                f64::from(self.state.settings.playerbar.loading_delay),
                self.state.settings.playerbar.skip_backward_seconds,
                self.state.settings.playerbar.skip_forward_seconds,
            ])
            .enumerate()
        {
            if let Some(input) = self.player_numbers.get(index) {
                input.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx)
                });
                continue;
            }
            let input = cx
                .new(|cx| crate::appearance::input_state(window, cx).aria_label(NUMBERS[index].0));
            input.update(cx, |input, cx| {
                input.set_value(value.to_string(), window, cx)
            });
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        this.apply_player_number(index, cx);
                    }
                },
            ));
            self.subscriptions.push(cx.subscribe_in(
                &input,
                window,
                move |this, _, event: &NumberInputEvent, window, cx| {
                    let NumberInputEvent::Step(action) = event;
                    let (_, min, max, step) = NUMBERS[index];
                    let input = this.player_numbers[index].clone();
                    let value = (input
                        .read(cx)
                        .value()
                        .parse::<f64>()
                        .ok()
                        .filter(|value| value.is_finite())
                        .unwrap_or(f64::from(min))
                        + if *action == StepAction::Increment {
                            f64::from(step)
                        } else {
                            -f64::from(step)
                        })
                    .clamp(f64::from(min), f64::from(max));
                    input.update(cx, |input, cx| {
                        input.set_value(value.to_string(), window, cx)
                    });
                    this.apply_player_number(index, cx);
                },
            ));
            self.player_numbers.push(input);
        }
    }
    pub fn apply_player_number(&mut self, index: usize, cx: &mut Context<Self>) {
        let (_, min, max, _) = NUMBERS[index];
        let raw = self.player_numbers[index].read(cx).value();
        let value = if index >= 4 && raw.trim().is_empty() {
            Some(if index == 4 { 2. } else { 0. })
        } else {
            raw.parse::<f64>().ok().filter(|value| {
                value.is_finite() && (f64::from(min)..=f64::from(max)).contains(value)
            })
        };
        let Some(value) = value else {
            self.message = format!("Value must be between {min} and {max}");
            cx.notify();
            return;
        };
        let config = &mut self.state.settings.player_display;
        match index {
            0 => config.cover_art_size = value as f32,
            1 => config.dynamic_image_blur = value as f32,
            2 => config.opacity = value as f32,
            3 => config.title_line_count = value.round() as usize,
            4 => self.state.settings.playerbar.loading_delay = value as f32,
            5 => self.state.settings.playerbar.skip_backward_seconds = value,
            6 => self.state.settings.playerbar.skip_forward_seconds = value,
            _ => unreachable!(),
        }
        self.save();
        cx.notify();
    }
    pub fn open_player_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.init_player_numbers(window, cx);
        if self.full_player {
            self.open_player_settings_dialog(false, window, cx);
            return;
        }
        self.full_player = false;
        self.persist_player_modes();
        self.navigate(Route::PlayerSettings, cx);
    }
    pub fn open_player_settings_dialog(
        &mut self,
        layout: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let previous = if layout {
            let previous = Some(std::rc::Rc::new(std::cell::RefCell::new(
                self.list_editor.take(),
            )));
            let config = self.list_config();
            self.prepare_list_editor("fullScreenSong".into(), config, window, cx);
            previous
        } else {
            None
        };
        let parent = cx.entity();
        let content = cx.new(|cx| PlayerSettingsDialog {
            parent: parent.clone(),
            layout,
            _subscription: cx.observe(&parent, |_, _, cx| cx.notify()),
        });
        let parent = parent.downgrade();
        let zoom = self.state.settings.zoom;
        window.open_dialog(cx, move |dialog, _, _| {
            let parent = parent.clone();
            let previous = previous.clone();
            dialog
                .width(rems(48.75))
                .overlay(layout)
                .when(!layout, |dialog| dialog.pop_transition(px(10. * zoom)))
                .close_button(false)
                .child(content.clone())
                .on_close(move |_, _, cx| {
                    if let Some(previous) = &previous {
                        parent
                            .update(cx, |parent, cx| {
                                parent.list_editor = previous.borrow_mut().take();
                                cx.notify();
                            })
                            .ok();
                    }
                })
        });
    }
    pub fn player_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.player_display;
        let mut page = div()
            .id("player-settings")
            .debug_selector(|| "player-settings".into())
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll();
        for (index, (label, _, _, _)) in NUMBERS.iter().take(4).enumerate() {
            page = page.child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(*label)
                    .child(
                        crate::styles::number_input(&self.player_numbers[index], cx).w(px(160.)),
                    ),
            );
        }
        for (index, label, value) in [
            (0u32, "Dynamic background", config.dynamic_background),
            (1, "Use artwork for background", config.dynamic_is_image),
            (
                2,
                "Preserve artwork aspect ratio",
                config.use_image_aspect_ratio,
            ),
        ] {
            page = page.child(
                Checkbox::new(("player-display-flag", index))
                    .label(label)
                    .checked(value)
                    .on_click(cx.listener(move |this, value: &bool, _, cx| {
                        let config = &mut this.state.settings.player_display;
                        match index {
                            0 => config.dynamic_background = *value,
                            1 => config.dynamic_is_image = *value,
                            _ => config.use_image_aspect_ratio = *value,
                        }
                        this.save();
                        cx.notify();
                    })),
            );
        }
        for (title, key, choices, selected) in [
            (
                "Metadata alignment",
                "alignment",
                vec!["left", "center", "right"],
                config.player_item_alignment.clone(),
            ),
            (
                "Title display",
                "title",
                vec!["scroll", "multiLine"],
                config.title_display_type.clone(),
            ),
        ] {
            let view = cx.entity().clone();
            page = page.child(
                crate::styles::button(key, cx)
                    .label(format!("{title}: {selected}"))
                    .dropdown_menu(move |mut menu, _, cx| {
                        for choice in &choices {
                            let view = view.clone();
                            let choice = (*choice).to_owned();
                            menu = menu.item(
                                PopupMenuItem::new(crate::appearance::text(&choice.clone(), cx))
                                    .on_click(move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            let config = &mut this.state.settings.player_display;
                                            if key == "alignment" {
                                                config.player_item_alignment = choice.clone();
                                            } else {
                                                config.title_display_type = choice.clone();
                                            }
                                            this.save();
                                            cx.notify();
                                        })
                                    }),
                            );
                        }
                        menu
                    }),
            );
        }
        page.children(self.waveform_controls("", cx))
            .child(crate::appearance::text("Metadata fields", cx))
            .children(config.fields.iter().enumerate().map(|(index, field)| {
                let label = FIELDS
                    .iter()
                    .find(|(id, _, _)| *id == field.id)
                    .map_or(field.id.as_str(), |(_, label, _)| *label)
                    .to_owned();
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(
                        Checkbox::new(("player-field", index))
                            .label(label)
                            .checked(!field.disabled)
                            .on_click(cx.listener(move |this, value: &bool, _, cx| {
                                this.state.settings.player_display.fields[index].disabled = !*value;
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button(("player-field-up", index), cx)
                            .label(crate::appearance::text("↑", cx))
                            .disabled(index <= 3)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.player_display.move_field(index, -1);
                                this.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::styles::button(("player-field-down", index), cx)
                            .label(crate::appearance::text("↓", cx))
                            .disabled(index < 3 || index + 1 == config.fields.len())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.state.settings.player_display.move_field(index, 1);
                                this.save();
                                cx.notify();
                            })),
                    )
            }))
            .child(
                crate::styles::button("return-full-player", cx)
                    .label(crate::appearance::text("Show fullscreen player", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.full_player = true;
                        this.persist_player_modes();
                        this.restore_player_tab(cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    pub fn is_full_player_view(&self) -> bool {
        self.rendering_full_player.unwrap_or(self.full_player)
    }
    pub fn dynamic_player_background(&self) -> bool {
        self.state.settings.player_display.dynamic_background
            && !(self.radio_item.is_some() && self.playing)
    }
    fn player_card_path(&self, item: &Item) -> Option<&PathBuf> {
        self.covers
            .get(&self.cover_cache_key(item, feishin_core::images::Placement::ItemCard))
            .filter(|path| path.exists())
    }
    pub fn sync_player_background(&mut self, cx: &mut Context<Self>) {
        if self.cache_busy || !self.full_player {
            return;
        }
        let Some(item) = self.current_entry().map(|entry| &entry.item) else {
            self.player_background = None;
            self.player_background_key.clear();
            return;
        };
        let key = self.cover_cache_key(item, feishin_core::images::Placement::ItemCard);
        let Some(source) = self.player_card_path(item).cloned() else {
            // Keep the previous dominant color while the next image is loading.
            if item.cover_id.is_empty() {
                self.player_background = None;
            }
            self.player_background_key = format!("pending-{key}");
            return;
        };
        let signature = format!(
            "{key}-{:x}-player-card-color",
            feishin_core::api::cover_hash(&source.to_string_lossy())
        );
        if signature == self.player_background_key {
            return;
        }
        self.player_background_key = signature.clone();
        let destination = self
            .data_dir
            .join("player-backgrounds")
            .join(format!("{signature}.png"));
        let stale_path = destination.clone();
        let revision = self.state.cache_revision;
        let task = cx
            .background_executor()
            .spawn(async move { prepare_background(&source, &destination, 0.) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.cache_revision != revision {
                    feishin_core::cache::discard_stale(&stale_path);
                    return;
                }
                if this.player_background_key != signature {
                    return;
                }
                match result {
                    Ok(background) => this.player_background = Some(background),
                    Err(error) => {
                        log::debug!("Player background preparation failed: {error}");
                        this.player_background = Some(Background {
                            image: stale_path,
                            color: transparent_black(),
                            aspect: 1.,
                        });
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn player_background_color(&self, cx: &App) -> Rgba {
        if self.dynamic_player_background() {
            self.player_background
                .as_ref()
                .map_or(cx.theme().muted_foreground, |background| background.color)
                .into()
        } else {
            cx.theme().background.into()
        }
    }
    pub fn player_background_view(
        &self,
        color: Rgba,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = &self.state.settings.player_display;
        let dynamic = self.dynamic_player_background();
        let current = self.state.queue.current();
        let item = current.map(|entry| entry.item.clone()).unwrap_or_default();
        let key = current
            .map(|entry| entry.key.to_string())
            .unwrap_or_default();
        let image_enabled = dynamic && config.dynamic_is_image;
        let key = if item.cover_id.is_empty() {
            ""
        } else {
            key.as_str()
        };
        let motion = window.use_keyed_state("expanded-player-background-images", cx, |_, _| {
            (
                image_enabled,
                crate::player_layout::ArtworkMotion::new(
                    if image_enabled { key } else { "" },
                    &item,
                    true,
                ),
            )
        });
        let (layers, active) = motion.update(cx, |(enabled, motion), cx| {
            if *enabled != image_enabled {
                *enabled = image_enabled;
                *motion = crate::player_layout::ArtworkMotion::new(
                    if image_enabled { key } else { "" },
                    &item,
                    true,
                );
            }
            motion.frame(
                if image_enabled { key } else { "" },
                &item,
                true,
                cx.background_executor().now(),
            )
        });
        if active {
            window.request_animation_frame();
        }
        div()
            .absolute()
            .inset_0()
            .bg(color)
            .debug_selector(|| "expanded-player-background".into())
            .children(layers.into_iter().filter_map(|(key, item, opacity)| {
                let track = self.player_card_path(&item).cloned()?;
                let album = background_album_item(&item)
                    .and_then(|album| self.player_card_path(&album).cloned());
                Some(
                    div()
                        .id(SharedString::from(format!("player-background-{key}")))
                        .absolute()
                        .inset_0()
                        .opacity(opacity)
                        // CSS background images paint the track fallback below album art.
                        .child(img(track).size_full().object_fit(ObjectFit::Cover))
                        .when_some(album, |div, album| {
                            div.child(
                                img(album)
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            )
                        }),
                )
            }))
            .when(dynamic, |div| {
                div.child(
                    gpui::div()
                        .absolute()
                        .inset_0()
                        .bg(black().opacity((config.opacity / 120.).clamp(0., 1.)))
                        .debug_selector(|| "expanded-player-background-shading".into()),
                )
                .child(
                    gpui::div()
                        .absolute()
                        .inset_0()
                        .debug_selector(|| "expanded-player-background-overlay".into())
                        .backdrop_blur(cx.theme().font_size * config.dynamic_image_blur)
                        .child(self.theme_overlay("overlay-header")),
                )
            })
            .into_any_element()
    }
    pub fn player_artwork(&self, item: &Item, cx: &App) -> AnyElement {
        let config = &self.state.settings.player_display;
        let width = config.cover_art_size * 4.;
        let aspect = if config.use_image_aspect_ratio {
            self.player_background
                .as_ref()
                .map_or(1., |background| background.aspect)
                .clamp(0.3, 3.)
        } else {
            1.
        };
        div()
            .debug_selector(|| "player-artwork".into())
            .w(px(width))
            .h(px(width / aspect))
            .rounded_md()
            .overflow_hidden()
            .bg(cx.theme().secondary)
            .child(self.image_content(item, feishin_core::images::Placement::FullScreenPlayer, cx))
            .into_any_element()
    }
    fn full_player_image_path(&self, item: &Item) -> Option<&PathBuf> {
        self.covers
            .get(&self.cover_cache_key(item, feishin_core::images::Placement::FullScreenPlayer))
            .or_else(|| self.covers.get(&Self::cover_key(item)))
            .filter(|path| path.exists())
    }
    pub fn full_player_image(
        &self,
        entry: &feishin_core::queue::Entry,
        metadata: &Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = if self.radio_item.is_some() {
            "radio".into()
        } else {
            entry.key.to_string()
        };
        let ready = self.full_player_image_path(&entry.item).is_some();
        let motion = window.use_keyed_state("expanded-player-artwork-motion", cx, |_, _| {
            crate::player_layout::ArtworkMotion::new(&key, &entry.item, ready)
        });
        let (layers, active) = motion.update(cx, |motion, cx| {
            motion.frame(&key, &entry.item, ready, cx.background_executor().now())
        });
        if active {
            window.request_animation_frame();
        }
        let config = &self.state.settings.player_display;
        let show_metadata = ["title", "artist", "album"]
            .into_iter()
            .any(|field| config.visible(field))
            || !config
                .metadata(metadata, &self.state.settings.language)
                .is_empty();
        // The original measures on mount, resize and title/artwork settings changes.
        // Opening another panel keeps the previously measured metadata width.
        let viewport = window.viewport_size();
        let measured_width = window.use_keyed_state(
            SharedString::from(format!(
                "expanded-artwork-width-{viewport:?}-{}-{}-{}-{}",
                config.cover_art_size,
                config.title_display_type,
                config.title_line_count,
                self.state.settings.zoom
            )),
            cx,
            |_, _| None::<Pixels>,
        );
        let metadata_width: Length = if viewport.height >= viewport.width {
            relative(0.9).into()
        } else if let Some(width) = *measured_width.read(cx)
            && width >= px(300. * self.state.settings.zoom)
        {
            width.into()
        } else {
            relative(0.8).into()
        };
        let zoom = self.state.settings.zoom;
        let native = config.use_image_aspect_ratio || self.state.settings.native_image_aspect_ratio;
        let layers = layers
            .into_iter()
            .map(|(key, item, opacity)| {
                let cache_key =
                    self.cover_cache_key(&item, feishin_core::images::Placement::FullScreenPlayer);
                let path = self.full_player_image_path(&item);
                let content = if let Some(path) = path {
                    let aspect = window.use_keyed_state(
                        SharedString::from(format!(
                            "player-image-aspect-{cache_key}-{}",
                            path.display()
                        )),
                        cx,
                        |_, _| {
                            artwork_aspect(path).unwrap_or_else(|error| {
                                log::debug!("Expanded artwork dimensions unavailable: {error}");
                                1.
                            })
                        },
                    );
                    crate::player_layout::artwork_image(
                        img(path.clone())
                            .size_full()
                            .rounded(px(5. * zoom))
                            .object_fit(if native {
                                ObjectFit::Contain
                            } else {
                                ObjectFit::Cover
                            }),
                        native.then(|| *aspect.read(cx)),
                        zoom,
                        self.state.settings.blur_explicit_images && item.explicit(),
                    )
                } else {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(cx.theme().secondary)
                        .rounded(cx.theme().radius)
                        .child(
                            Icon::default()
                                .path(if item.kind == feishin_core::model::ItemKind::Radio {
                                    "native-icons/radio.svg"
                                } else {
                                    "native-icons/album.svg"
                                })
                                .w(relative(0.25))
                                .h(relative(0.25))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .into_any_element()
                };
                div()
                    .debug_selector(move || format!("expanded-player-artwork-{key}"))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .opacity(if path.is_some() { opacity } else { 1. })
                    .child(content)
            })
            .collect::<Vec<_>>();
        let image = div()
            .absolute()
            .inset_0()
            .size_full()
            .children(layers)
            .into_any_element();
        crate::player_layout::ArtworkLayout {
            font: cx.theme().font_size,
            cover: config.cover_art_size,
            show_metadata,
            image,
            measured_width: Some(measured_width),
            metadata: if show_metadata {
                div()
                    .max_w(metadata_width)
                    .mx_auto()
                    .child(self.player_metadata(metadata, cx))
                    .into_any_element()
            } else {
                Empty.into_any_element()
            },
        }
        .into_any_element()
    }
    pub fn player_title(&self, title: String, cx: &Context<Self>) -> AnyElement {
        if title.is_empty() {
            return div().w_full().into_any_element();
        }
        let config = &self.state.settings.player_display;
        let font_size = cx.theme().font_size
            * if self.is_full_player_view() {
                1.75
            } else {
                1.5
            };
        if config.title_display_type == "multiLine" {
            return div()
                .w_full()
                .text_size(font_size)
                .line_height(relative(1.2))
                .font_weight(FontWeight::BLACK)
                .line_clamp(config.title_line_count)
                .child(title)
                .into_any_element();
        }
        crate::player_layout::ScrollingTitle {
            title,
            font_size,
            zoom: self.state.settings.zoom,
        }
        .into_any_element()
    }
    pub fn player_metadata(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.player_display;
        let fields = config
            .metadata(item, &self.state.settings.language)
            .into_iter()
            .filter(|_| self.radio_item.is_none())
            .filter(|(label, _)| !matches!(*label, "Title" | "Artist" | "Album"))
            .enumerate()
            .map(|(index, (_, value))| {
                div()
                    .debug_selector(move || format!("player-metadata-field-{index}"))
                    .text_sm()
                    .when(self.is_full_player_view(), |div| {
                        let font = cx.theme().font_size;
                        div.h(font * 1.25)
                            .px(font * 0.5)
                            .py(font * 0.25)
                            .rounded(font * 5. / 16.)
                            .bg(cx.theme().secondary)
                            .text_size(font * 11. / 16.)
                            .line_height(font * 18. / 16.)
                            .font_weight(FontWeight::MEDIUM)
                            .letter_spacing(font / 64.)
                    })
                    .child(if self.is_full_player_view() {
                        value.to_uppercase()
                    } else {
                        value
                    })
            })
            .collect::<Vec<_>>();
        div()
            .debug_selector(|| "player-metadata".into())
            .v_flex()
            .gap(
                cx.theme().font_size
                    * if self.is_full_player_view() {
                        0.75
                    } else {
                        0.5
                    },
            )
            .when(self.is_full_player_view(), |div| {
                div.px(cx.theme().font_size * 0.5)
                    .text_size(cx.theme().font_size * 18. / 16.)
            })
            .w_full()
            .when(config.player_item_alignment == "center", |div| {
                div.items_center().text_center()
            })
            .when(config.player_item_alignment == "right", |div| {
                div.items_end().text_right()
            })
            .when(config.visible("title"), |div| {
                div.child(self.player_title(item.name.clone(), cx))
            })
            .when(config.visible("artist"), |div| {
                if self.radio_item.is_some() {
                    div.child(item.artist.clone())
                } else {
                    div.child(self.artist_names(item, "player-artists", cx))
                }
            })
            .when(config.visible("album"), |div| {
                let album = Item {
                    id: item.album_id.clone(),
                    name: item.album.clone(),
                    server_id: item.server_id,
                    kind: feishin_core::model::ItemKind::Album,
                    ..Default::default()
                };
                div.child(
                    gpui::div()
                        .id("player-album")
                        .debug_selector(|| "player-album".into())
                        .child(item.album.clone())
                        .when(self.radio_item.is_none() && !album.id.is_empty(), |div| {
                            div.role(Role::Link)
                                .focusable()
                                .tab_stop(true)
                                .cursor_pointer()
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.navigate(
                                        crate::app::Route::Detail(Box::new(album.clone())),
                                        cx,
                                    );
                                }))
                        }),
                )
            })
            .when(!fields.is_empty(), |div| {
                div.child(
                    gpui::div()
                        .w_full()
                        .when(self.is_full_player_view(), |div| {
                            div.h_flex()
                                .flex_wrap()
                                .gap(cx.theme().font_size * 0.75)
                                .mt(cx.theme().font_size * 0.5)
                                .when(config.player_item_alignment == "center", |div| {
                                    div.justify_center()
                                })
                                .when(config.player_item_alignment == "right", |div| {
                                    div.justify_end()
                                })
                        })
                        .when(!self.is_full_player_view(), |div| div.v_flex().gap_2())
                        .children(fields),
                )
            })
            .when(!self.is_full_player_view(), |div| {
                div.child(gpui::div().text_sm().child(format!(
                    "{} - {}{}",
                    if self.buffering {
                        "Buffering"
                    } else if self.playing {
                        "Playing"
                    } else if self.loaded {
                        "Paused"
                    } else {
                        "Stopped"
                    },
                    self.cast_selected.as_ref().map_or_else(
                        || if self.state.settings.jukebox {
                            "Jukebox".into()
                        } else if self.state.settings.mpv_path.is_empty() {
                            "Native audio".into()
                        } else {
                            "MPV".into()
                        },
                        |device| device.name.clone()
                    ),
                    if self.listening.submitted {
                        " - Scrobbled"
                    } else {
                        ""
                    }
                )))
                .child(
                    crate::styles::button("configure-player", cx)
                        .label(crate::appearance::text("Configure player", cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.open_player_settings(window, cx)
                            }),
                        ),
                )
            })
            .into_any_element()
    }
}
pub(crate) fn artwork_aspect(path: &Path) -> anyhow::Result<f32> {
    use image::ImageDecoder;
    let mut decoder = image::ImageReader::open(path)?
        .with_guessed_format()?
        .into_decoder()?;
    let (width, height) = decoder.dimensions();
    anyhow::ensure!(width > 0 && height > 0, "Artwork has no dimensions");
    let rotated = matches!(decoder.orientation()?.to_exif(), 5..=8);
    Ok(if rotated {
        height as f32 / width as f32
    } else {
        width as f32 / height as f32
    })
}

pub fn prepare_background(
    source: &Path,
    destination: &Path,
    blur: f32,
) -> anyhow::Result<Background> {
    prepare_background_with_color(source, destination, blur, false)
}

pub fn prepare_expanded_background(
    source: &Path,
    destination: &Path,
) -> anyhow::Result<Background> {
    prepare_background_with_color(source, destination, 0., true)
}

fn ignored_average_pixel(pixel: &image::Rgba<u8>) -> bool {
    let [r, g, b, a] = pixel.0;
    a <= 40
        || (a >= 165 && r >= 165 && g >= 165 && b >= 165)
        || (a >= 225 && r <= 30 && g <= 30 && b <= 30)
}

// Port of the original Fast Average Color 9.5.0 algorithms (MIT).
fn artwork_average(pixels: &image::RgbaImage, square_root: bool) -> Hsla {
    if square_root {
        return expanded_average(pixels);
    }
    let mut buckets = std::collections::HashMap::<[u8; 3], (u64, u64, [u64; 3])>::new();
    let mut maximum = (0, 0, [0; 3]);
    for pixel in pixels
        .pixels()
        .filter(|pixel| !ignored_average_pixel(pixel))
    {
        let key = [pixel[0], pixel[1], pixel[2]].map(|value| ((u16::from(value) + 12) / 24) as u8);
        let bucket = buckets.entry(key).or_default();
        bucket.0 += 1;
        bucket.1 += u64::from(pixel[3]);
        for (sum, value) in bucket.2.iter_mut().zip(pixel.0) {
            *sum += u64::from(value) * u64::from(pixel[3]);
        }
        // Match the original's first bucket to reach the highest count, including ties.
        if bucket.0 > maximum.0 {
            maximum = *bucket;
        }
    }
    if maximum.1 == 0 {
        return black();
    }
    Rgba {
        r: (maximum.2[0] as f64 / maximum.1 as f64).round() as f32 / 255.,
        g: (maximum.2[1] as f64 / maximum.1 as f64).round() as f32 / 255.,
        b: (maximum.2[2] as f64 / maximum.1 as f64).round() as f32 / 255.,
        a: 1.,
    }
    .into()
}

fn expanded_average(pixels: &image::RgbaImage) -> Hsla {
    let mut sums = [0_u64; 3];
    let mut alpha = 0_u64;
    for pixel in pixels.pixels() {
        let [r, g, b, a] = pixel.0;
        if ignored_average_pixel(pixel) {
            continue;
        }
        alpha += u64::from(a);
        for (sum, value) in sums.iter_mut().zip([r, g, b]) {
            *sum += u64::from(value).pow(2) * u64::from(a);
        }
    }
    if alpha == 0 {
        return black();
    }
    Rgba {
        r: (sums[0] as f64 / alpha as f64).sqrt().round() as f32 / 255.,
        g: (sums[1] as f64 / alpha as f64).sqrt().round() as f32 / 255.,
        b: (sums[2] as f64 / alpha as f64).sqrt().round() as f32 / 255.,
        // The original uses color.rgb, which discards the calculated alpha.
        a: 1.,
    }
    .into()
}

fn prepare_background_with_color(
    source: &Path,
    destination: &Path,
    blur: f32,
    square_root: bool,
) -> anyhow::Result<Background> {
    use image::ImageReader;
    let reader = ImageReader::open(source)?.with_guessed_format()?;
    let format = reader
        .format()
        .ok_or_else(|| anyhow::anyhow!("Unknown artwork format"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    anyhow::ensure!(
        std::fs::metadata(source)?.len() <= 128 * 1024 * 1024,
        "Artwork file exceeds allocation limit"
    );
    let mut frames =
        gpui::decode_raster_image_with_limits(&std::fs::read(source)?, format, limits)?;
    anyhow::ensure!(!frames.is_empty(), "Empty artwork");
    let (width, height) = frames[0].buffer().dimensions();
    anyhow::ensure!(width > 0 && height > 0, "Empty artwork");
    for frame in &mut frames {
        for pixel in frame.buffer_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    let color = {
        let factor = 100. / width.max(height) as f64;
        let resized_width = (width as f64 * factor).round() as u32;
        let resized_height = (height as f64 * factor).round() as u32;
        if resized_width <= width
            && resized_height <= height
            && resized_width >= 10
            && resized_height >= 10
        {
            let source = frames[0].buffer();
            // Chromium's software canvas uses bilinear samples with four-bit
            // fractional weights and floors the premultiplied channel sums.
            let pixels = image::RgbaImage::from_fn(resized_width, resized_height, |x, y| {
                let inverse_x = width as f32 / resized_width as f32;
                let inverse_y = height as f32 / resized_height as f32;
                let sx = (f64::from(0.5 * inverse_x) - 0.5 + x as f64 * f64::from(inverse_x))
                    .clamp(0., width as f64 - 1.);
                let sy =
                    (f64::from((y as f32 + 0.5) * inverse_y) - 0.5).clamp(0., height as f64 - 1.);
                let ix = sx.floor() as u32;
                let iy = sy.floor() as u32;
                let fx = (sx.fract() * 16.).floor() / 16.;
                let fy = (sy.fract() * 16.).floor() / 16.;
                let mut rgba = [0_f64; 4];
                for (dx, dy, weight) in [
                    (0, 0, (1. - fx) * (1. - fy)),
                    (1, 0, fx * (1. - fy)),
                    (0, 1, (1. - fx) * fy),
                    (1, 1, fx * fy),
                ] {
                    let pixel =
                        source.get_pixel((ix + dx).min(width - 1), (iy + dy).min(height - 1));
                    rgba[3] += f64::from(pixel[3]) * weight;
                    for channel in 0..3 {
                        rgba[channel] += ((u32::from(pixel[channel]) * u32::from(pixel[3]) + 127)
                            / 255) as f64
                            * weight;
                    }
                }
                image::Rgba(std::array::from_fn(|channel| {
                    if channel == 3 {
                        rgba[3].floor() as u8
                    } else if rgba[3] > 0. {
                        (rgba[channel].floor() * 255. / rgba[3].floor().max(1.)).round() as u8
                    } else {
                        0
                    }
                }))
            });
            artwork_average(&pixels, square_root)
        } else {
            let pixels = image::RgbaImage::from_fn(width, height, |x, y| {
                let pixel = frames[0].buffer().get_pixel(x, y);
                image::Rgba(std::array::from_fn(|channel| {
                    if channel == 3 {
                        pixel[3]
                    } else if pixel[3] > 0 {
                        let premultiplied =
                            (u32::from(pixel[channel]) * u32::from(pixel[3]) + 127) / 255;
                        (premultiplied as f64 * 255. / f64::from(pixel[3])).round() as u8
                    } else {
                        0
                    }
                }))
            });
            artwork_average(&pixels, square_root)
        }
    };
    for frame in &mut frames {
        let image = image::DynamicImage::ImageRgba8(std::mem::take(frame.buffer_mut()));
        *frame.buffer_mut() = image.thumbnail(width.min(512), height.min(512)).to_rgba8();
    }
    let preview = frames[0].buffer();
    if !destination.exists() {
        let blur = blur.clamp(0., 6.) * 8.;
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, preview.width(), preview.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            if frames.len() > 1 {
                encoder.set_animated(frames.len() as u32, 0)?;
            }
            let mut writer = encoder.write_header()?;
            let animated = frames.len() > 1;
            for frame in frames {
                if animated {
                    let delay = png_frame_delay(frame.delay());
                    writer.set_frame_delay(delay.0, delay.1)?;
                }
                let pixels = if blur > 0. {
                    image::imageops::blur(frame.buffer(), blur)
                } else {
                    frame.into_buffer()
                };
                writer.write_image_data(&pixels)?;
            }
            writer.finish()?;
        }
        feishin_core::storage::atomic_write(destination, &bytes)?;
    }
    Ok(Background {
        image: destination.into(),
        color,
        aspect: width as f32 / height as f32,
    })
}
// APNG stores seconds as a pair of u16 values; preserve exact ratios when representable.
fn png_frame_delay(delay: image::Delay) -> (u16, u16) {
    let (numerator, denominator) = delay.numer_denom_ms();
    let (numerator, denominator) = (u64::from(numerator), u64::from(denominator) * 1000);
    let (mut a, mut b) = (numerator, denominator);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let (numerator, denominator) = (numerator / a, denominator / a);
    if let (Ok(n), Ok(d)) = (u16::try_from(numerator), u16::try_from(denominator)) {
        return (n, d);
    }
    let seconds = numerator as f64 / denominator as f64;
    let denominator = (u16::MAX as f64 / seconds.max(1.)).floor().clamp(1., 1000.) as u16;
    (
        (seconds * f64::from(denominator))
            .round()
            .clamp(0., u16::MAX as f64) as u16,
        denominator,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use ::core::prelude::v1::test;

    struct BackgroundProbe {
        image: bool,
        header: f32,
        font: f32,
        blur: f32,
        zoom: f32,
    }
    impl Render for BackgroundProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let pixel = || {
                std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(
                    image::RgbaImage::from_pixel(1, 1, image::Rgba([150, 90, 30, 255])),
                )]))
            };
            div()
                .relative()
                .w_full()
                .h(px(self.header * self.zoom))
                .child(LibraryBackground {
                    image_mode: self.image,
                    image: Some(pixel().into()),
                    color: rgb(0x1e5a96).into(),
                    overlay: {
                        let pixel = pixel();
                        canvas(
                            move |_, _, _| pixel,
                            |bounds, pixel, window, _| {
                                window
                                    .paint_image(bounds, Corners::default(), pixel, 0, false)
                                    .unwrap();
                            },
                        )
                        .size_full()
                        .into_any_element()
                    },
                    blur: px(self.blur * self.font * self.zoom),
                    zoom: self.zoom,
                })
        }
    }

    #[gpui::test]
    fn library_background_layers_match_original_header_measurements(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/library-background-reference.json"))
                .unwrap();
        for zoom in [1., 1.25] {
            for case in cases.as_array().unwrap() {
                let probe = cx.new(|_| BackgroundProbe {
                    image: case["image"].as_bool().unwrap(),
                    header: case["header"].as_f64().unwrap() as f32,
                    font: case["font"].as_f64().unwrap() as f32,
                    blur: case["blur"].as_f64().unwrap() as f32,
                    zoom,
                });
                let (_, cx) =
                    cx.add_window_view(|window, cx| gpui_component::Root::new(probe, window, cx));
                cx.simulate_resize(size(px(960. * zoom), px(900. * zoom)));
                cx.run_until_parked();
                let images = cx.update(|window, _| window.painted_images_for_test());
                for layer in case["layers"].as_array().unwrap() {
                    let image = layer["kind"] == "background-image";
                    let bounds = cx
                        .debug_bounds(if image {
                            "detail-background-image"
                        } else {
                            "detail-background-overlay"
                        })
                        .unwrap();
                    for (label, value) in
                        [("width", bounds.size.width), ("height", bounds.size.height)]
                    {
                        assert!(
                            (f32::from(value) - layer[label].as_f64().unwrap() as f32 * zoom).abs()
                                <= 1.,
                            "{case}: {zoom} {label} {bounds:?}"
                        );
                    }
                    if bounds.size.height <= px(0.) {
                        continue;
                    }
                    assert!(
                        images.iter().any(|(actual, _, opacity)| actual == &bounds
                            && (*opacity - layer["opacity"].as_f64().unwrap() as f32).abs()
                                < 0.0001),
                        "opacity: {case}, {images:?}"
                    );
                    if image && case["blur"].as_f64().unwrap() > 0. {
                        let sigma = case["blur"].as_f64().unwrap() as f32
                            * case["font"].as_f64().unwrap() as f32
                            * zoom;
                        assert!(
                            cx.update(|window, _| window.painted_foreground_filters_for_test())
                                .iter()
                                .any(|(_, value)| (f32::from(*value) - sigma).abs() < 0.001),
                            "filter: {case}"
                        );
                    }
                }
            }
        }
    }

    #[::core::prelude::v1::test]
    fn artwork_average_colors_match_original_algorithms_and_speed_sampling() {
        #[derive(serde::Deserialize)]
        struct Case {
            width: u32,
            height: u32,
            palette: Vec<[u8; 4]>,
            expected: [u8; 3],
            is_dark: bool,
        }
        let references = [
            (
                "sqrt",
                include_str!("../assets/expanded-album-colors-reference.json"),
            ),
            (
                "dominant",
                include_str!("../assets/artwork-dominant-colors-reference.json"),
            ),
        ];
        let dir =
            std::env::temp_dir().join(format!("feishin-expanded-colors-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut failures = vec![];
        for (algorithm, reference) in references {
            let cases: Vec<Case> = serde_json::from_str(reference).unwrap();
            for (index, case) in cases.iter().enumerate() {
                let image = image::RgbaImage::from_fn(case.width, case.height, |x, y| {
                    image::Rgba(case.palette[((x / 7 + y / 5) % 4) as usize])
                });
                let source = dir.join(format!("{index}.png"));
                image.save(&source).unwrap();
                let destination = dir.join(format!("{algorithm}-{index}-blur.png"));
                let prepared = if algorithm == "sqrt" {
                    prepare_expanded_background(&source, &destination)
                } else {
                    prepare_background(&source, &destination, 0.)
                }
                .unwrap();
                let color: Rgba = prepared.color.into();
                let actual = [color.r, color.g, color.b].map(|value| (value * 255.).round() as u8);
                if actual
                    .iter()
                    .zip(case.expected)
                    .any(|(actual, expected)| actual.abs_diff(expected) > 1)
                    || ((0.299 * f32::from(actual[0])
                        + 0.587 * f32::from(actual[1])
                        + 0.114 * f32::from(actual[2]))
                        < 128.)
                        != case.is_dark
                {
                    failures.push(format!(
                        "{algorithm} {index} {}x{}: {actual:?} expected {:?}",
                        case.width, case.height, case.expected
                    ));
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[::core::prelude::v1::test]
    fn artwork_background_decodes_unknown_extensions_preserves_aspect_and_blurs_edges() {
        let dir = std::env::temp_dir().join(format!(
            "feishin-player-background-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("cover.img");
        let image = image::RgbaImage::from_fn(80, 40, |x, _| {
            if x < 60 {
                image::Rgba([0, 0, 255, 255])
            } else {
                image::Rgba([255, 0, 0, 255])
            }
        });
        image
            .save_with_format(&source, image::ImageFormat::Png)
            .unwrap();
        let original = prepare_background(&source, &dir.join("plain.png"), 0.).unwrap();
        assert_eq!(original.aspect, 2.);
        let color = original.color.to_rgb();
        assert!(color.b > 0.99 && color.r < 0.01);
        let blurred = prepare_background(&source, &dir.join("blur.png"), 1.).unwrap();
        let pixels = image::open(blurred.image).unwrap().to_rgba8();
        assert!(pixels.get_pixel(59, 20)[0] > 0 && pixels.get_pixel(60, 20)[2] > 0);
        std::fs::write(&source, b"invalid image").unwrap();
        assert!(prepare_background(&source, &dir.join("invalid.png"), 0.).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
