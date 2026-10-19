use super::*;
use gpui_component::{
    Icon, Sizable, Size,
    animation::CssTransition,
    button::{Button, ButtonCustomVariant},
    popover::Popover,
    switch::Switch,
};
use std::{collections::HashSet, time::Duration};

#[cfg(test)]
#[path = "lyrics_actions_tests.rs"]
mod tests;

pub(crate) struct State {
    frame_hovered: bool,
    controls_hovered: HashSet<String>,
    focus: FocusHandle,
    opacity: CssTransition,
    _subscriptions: [Subscription; 2],
}
impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        let focused = cx.on_focus_in(&focus, window, |_, window, cx| {
            window.refresh();
            cx.notify();
        });
        let blurred = cx.on_focus_out(&focus, window, |_, _, window, cx| {
            window.refresh();
            cx.notify();
        });
        Self {
            frame_hovered: false,
            controls_hovered: HashSet::new(),
            focus,
            opacity: CssTransition::new(0., Duration::from_millis(200), [0.42, 0., 0.58, 1.]),
            _subscriptions: [focused, blurred],
        }
    }
    pub(crate) fn frame_hover(&mut self, hovered: bool, window: &mut Window) {
        self.frame_hovered = hovered;
        window.refresh();
    }
    fn control_hover(&mut self, id: &str, hovered: bool, window: &mut Window) {
        if hovered {
            self.controls_hovered.insert(id.into());
        } else {
            self.controls_hovered.remove(id);
        }
        window.refresh();
    }
    fn frame(&mut self, window: &mut Window, cx: &mut Context<Self>) -> f32 {
        let target = if !self.controls_hovered.is_empty() || self.focus.contains_focused(window, cx)
        {
            1.
        } else if self.frame_hovered {
            0.6
        } else {
            0.
        };
        let now = cx.background_executor().now();
        self.opacity.retarget(target, now);
        let (value, active) = self.opacity.value(now);
        if active {
            window.request_animation_frame();
        }
        value
    }
}

fn id(name: &str, sidebar: bool) -> SharedString {
    if sidebar {
        format!("{name}-sidebar").into()
    } else {
        name.to_owned().into()
    }
}
fn hover(button: Button, state: &Entity<State>, name: &str) -> Button {
    let state = state.clone();
    let name = name.to_owned();
    button.on_hover(move |hovered, window, cx| {
        state.update(cx, |state, _| state.control_hover(&name, *hovered, window));
    })
}
fn icon_button(
    name: &str,
    icon: &'static str,
    large: bool,
    sidebar: bool,
    state: &Entity<State>,
    cx: &App,
) -> Button {
    let id = id(name, sidebar);
    let selector = id.clone();
    let mut target = feishin_core::styles::Target::new(
        "button",
        &id,
        &["fs-action-icon-module-root", "mantine-ActionIcon-root"],
    );
    target
        .attributes
        .insert("data-variant".into(), "subtle".into());
    let background = cx.theme().background.to_rgb();
    let mix = if cx.theme().is_dark() { 0.05 } else { 0. };
    let hover_color = Rgba {
        r: background.r * 0.95 + mix,
        g: background.g * 0.95 + mix,
        b: background.b * 0.95 + mix,
        a: background.a,
    };
    hover(
        Button::new(id)
            .action_icon(
                cx.try_global::<crate::styles::NativeStyles>()
                    .map_or(1., |s| s.zoom()),
            )
            .state_style(crate::styles::state_style_path(target, Vec::new()))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(transparent_black())
                    .foreground(cx.theme().foreground)
                    .hover(hover_color.into())
                    .active(hover_color.into()),
            )
            .debug_selector(move || selector.to_string())
            .with_size(cx.theme().font_size * 2.25)
            .size(rems(2.25))
            .flex_shrink_0()
            .p_0()
            .border_0()
            .rounded(cx.theme().font_size * (3. / 16.))
            .icon(
                Icon::default()
                    .path(format!("native-icons/{icon}.svg"))
                    .with_size(cx.theme().font_size * if large { 1. } else { 13. / 16. }),
            ),
        state,
        name,
    )
}
fn language_label(variant: &lyrics::Variant) -> String {
    if variant.kind == "main" {
        variant.language.clone()
    } else {
        format!("{} ({})", variant.language, variant.kind)
    }
}

impl Feishin {
    fn toggle_lyric_layer(&mut self, key: &str, cx: &mut Context<Self>) {
        let layers = &mut self.state.settings.lyric_overlay_layers;
        if layers.iter().any(|value| value == key) {
            layers.retain(|value| value != key);
        } else {
            layers.push(key.into());
        }
        self.save();
        cx.notify();
    }
    pub(crate) fn lyric_actions(
        &self,
        sidebar: bool,
        state: &Entity<State>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.current_entry().is_none();
        let sources = !self.state.settings.lyric_sources.is_empty();
        let variants = self
            .lyric_record
            .as_ref()
            .map_or(&[][..], |r| r.variants.as_slice());
        let layers: Vec<_> = variants
            .iter()
            .filter(|v| v.synchronized && v.kind != "main")
            .collect();
        let extras: Vec<_> = layers
            .iter()
            .filter(|v| !matches!(v.kind.as_str(), "pronunciation" | "translation"))
            .cloned()
            .cloned()
            .collect();
        let row = || {
            div()
                .h_flex()
                .justify_center()
                .flex_wrap()
                .gap(rems(0.25))
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
        };
        let mut top = row().pb(rems(0.75));
        let make_button = |name, label: String| {
            let name = id(name, sidebar);
            let selector = name.clone();
            hover(
                crate::styles::button(name.clone(), cx)
                    .ghost()
                    .label(label)
                    .debug_selector(move || selector.to_string()),
                state,
                &name,
            )
        };
        if !self.lyrics.is_empty() {
            top = top.child(
                make_button(
                    "lyric-export",
                    self.tr("form.lyricsExport.export").to_uppercase(),
                )
                .with_size(Size::Small)
                .compact()
                .on_click(cx.listener(|this, _, window, cx| this.export_lyrics(window, cx))),
            );
        }
        if variants.len() > 1 {
            let selected = self.lyric_record.as_ref().map_or(0, |r| r.selected);
            let label = format!(
                "{}: {}",
                self.tr("page.fullscreenPlayer.lyricLanguage"),
                language_label(&variants[selected.min(variants.len() - 1)])
            );
            let variants = variants.to_vec();
            let app = cx.entity();
            top = top.child(
                icon_button("lyric-variant", "metadata", true, sidebar, state, cx)
                    .aria_label(label.clone())
                    .tooltip(label)
                    .disabled(disabled)
                    .dropdown_menu_with_anchor(Corner::BottomLeft, move |mut menu, _, _| {
                        for (index, variant) in variants.iter().enumerate() {
                            let app = app.clone();
                            menu = menu.item(
                                PopupMenuItem::new(language_label(variant))
                                    .checked(index == selected)
                                    .on_click(move |_, _, cx| {
                                        app.update(cx, |app, cx| {
                                            app.select_lyric_variant(index, cx)
                                        })
                                    }),
                            );
                        }
                        menu
                    }),
            );
        }
        for (index, layer) in layers
            .iter()
            .enumerate()
            .filter(|(_, v)| matches!(v.kind.as_str(), "pronunciation" | "translation"))
        {
            let key = format!("{}:{}", layer.kind, layer.language);
            let active = self.state.settings.lyric_overlay_layers.contains(&key);
            let (icon, label) = if layer.kind == "pronunciation" {
                (
                    "visualizer",
                    self.tr("page.fullscreenPlayer.showPronunciation"),
                )
            } else {
                (
                    "languages",
                    self.tr("page.fullscreenPlayer.showTranslation"),
                )
            };
            let label = format!(
                "{} {}",
                if active { "Hide" } else { "Show" },
                label.to_lowercase()
            );
            top = top.child(
                icon_button(
                    &format!("lyric-overlay-control-{index}"),
                    icon,
                    true,
                    sidebar,
                    state,
                    cx,
                )
                .aria_label(label.clone())
                .tooltip(label)
                .disabled(disabled)
                .when(active, |button| button.text_color(cx.theme().primary))
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_lyric_layer(&key, cx))),
            );
        }
        if !extras.is_empty() {
            let active = extras.iter().any(|v| {
                self.state
                    .settings
                    .lyric_overlay_layers
                    .contains(&format!("{}:{}", v.kind, v.language))
            });
            let app = cx.entity();
            top = top.child(
                Popover::new(id("lyric-layers-popover", sidebar))
                    .anchor(Corner::BottomLeft)
                    .centered(Axis::Horizontal)
                    .trigger(
                        icon_button("lyric-layers", "queue", true, sidebar, state, cx)
                            .aria_label(self.tr("page.fullscreenPlayer.lyricLayers"))
                            .tooltip(self.tr("page.fullscreenPlayer.lyricLayers"))
                            .disabled(disabled)
                            .when(active, |button| button.text_color(cx.theme().primary)),
                    )
                    .content(move |_, _, cx| {
                        let data = app.read(cx);
                        div()
                            .v_flex()
                            .gap(rems(0.5))
                            .p(rems(0.5))
                            .min_w(px(220. * data.state.settings.zoom))
                            .max_w(px(280. * data.state.settings.zoom))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(data.tr("page.fullscreenPlayer.lyricLayers")),
                            )
                            .children(extras.iter().enumerate().map(|(index, layer)| {
                                let key = format!("{}:{}", layer.kind, layer.language);
                                let app = app.clone();
                                div()
                                    .h_flex()
                                    .gap(rems(0.75))
                                    .justify_between()
                                    .child(div().flex_1().min_w_0().child(language_label(layer)))
                                    .child(
                                        Switch::new(("lyric-extra-layer", index))
                                            .settings(
                                                data.state.settings.zoom,
                                                crate::appearance::rtl(cx),
                                                "lyric-layer",
                                            )
                                            .checked(
                                                data.state
                                                    .settings
                                                    .lyric_overlay_layers
                                                    .contains(&key),
                                            )
                                            .tooltip(language_label(layer))
                                            .on_click(move |_, _, cx| {
                                                app.update(cx, |app, cx| {
                                                    app.toggle_lyric_layer(&key, cx)
                                                })
                                            }),
                                    )
                            }))
                            .into_any_element()
                    }),
            );
        }
        let mut bottom = row();
        if sources {
            bottom = bottom.child(
                make_button("lyric-search", self.tr("common.search").to_uppercase())
                    .disabled(disabled)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_lyric_search(window, cx)),
                    ),
            );
        }
        let input = if sidebar {
            &self.lyric_sidebar_offset
        } else {
            &self.lyric_song_offset
        };
        let input_hover = state.clone();
        bottom = bottom
            .child(
                icon_button("lyric-offset-decrease", "minus", false, sidebar, state, cx)
                    .aria_label("Decrease lyric offset")
                    .tooltip(self.tr("common.slower"))
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_song_lyric_offset(this.lyric_offset() * 1000. - 50., cx)
                    })),
            )
            .child(
                div()
                    .id("offset-input-hover")
                    .debug_selector(move || id("lyric-offset-input", sidebar).to_string())
                    .flex_none()
                    .on_hover(move |hovered, window, cx| {
                        input_hover.update(cx, |state, _| {
                            state.control_hover("offset-input", *hovered, window)
                        });
                    })
                    .child(
                        crate::styles::number_input(input, cx)
                            .hide_controls(true)
                            .flex_none()
                            .w(px(70. * self.state.settings.zoom))
                            .text_align(TextAlign::Center)
                            .disabled(disabled),
                    ),
            )
            .child(
                icon_button("lyric-offset-increase", "plus", false, sidebar, state, cx)
                    .aria_label("Increase lyric offset")
                    .tooltip(self.tr("common.faster"))
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_song_lyric_offset(this.lyric_offset() * 1000. + 50., cx)
                    })),
            );
        if sources {
            bottom = bottom.child(
                make_button(
                    "lyric-clear",
                    self.tr(if self.lyrics.is_empty() {
                        "common.refresh"
                    } else {
                        "common.clear"
                    })
                    .to_uppercase(),
                )
                .disabled(disabled)
                .on_click(cx.listener(|this, _, _, cx| this.clear_lyrics(cx))),
            );
            if !layers.iter().any(|v| v.kind == "translation") {
                bottom = bottom.child(
                    make_button(
                        "lyric-cloud-translate",
                        self.tr("common.translation").to_uppercase(),
                    )
                    .disabled(disabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.lyric_translated.is_empty() {
                            this.translate_lyrics(cx);
                        } else {
                            this.lyric_translation_visible = !this.lyric_translation_visible;
                            cx.notify();
                        }
                    })),
                );
            }
        }
        let opacity = state.update(cx, |state, cx| state.frame(window, cx));
        div()
            .id(id("lyric-actions", sidebar))
            .debug_selector(move || format!("lyric-actions-{sidebar}"))
            .track_focus(&state.read(cx).focus)
            .absolute()
            .bottom_0()
            .left_0()
            .w_full()
            .v_flex()
            .opacity(opacity)
            .when(
                !self.lyrics.is_empty() || variants.len() > 1 || !layers.is_empty(),
                |div| div.child(top),
            )
            .child(bottom)
            .into_any_element()
    }
}
