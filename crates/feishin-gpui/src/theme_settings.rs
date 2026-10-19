use crate::app::Feishin;
use crate::settings::option_row as row;
use feishin_core::{appearance, settings_search};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Selectable, Sizable, StyledExt,
    button::ButtonVariants,
    color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState},
    input::{InputEvent, InputState},
    popover::{Popover, PopoverState},
    slider::{SliderEvent, SliderState},
    switch::Switch,
};

const KEYS: [&str; 10] = [
    "setting.useSystemTheme",
    "setting.theme",
    "setting.themeDark",
    "setting.themeLight",
    "setting.useThemeAccentColor",
    "setting.accentColor",
    "setting.useThemePrimaryShade",
    "setting.primaryShade",
    "setting.customThemes",
    "setting.customCssEnable",
];
const SOLARIZED_KEY: &str = "setting.solarizedAppearance";

fn color(value: &str) -> Hsla {
    let [r, g, b, a] = appearance::color(value).unwrap_or([0.208, 0.455, 0.988, 1.]);
    Rgba { r, g, b, a }.into()
}

#[derive(Clone, PartialEq, serde::Deserialize)]
struct Choice {
    label: String,
    value: String,
    #[serde(rename = "type")]
    mode: String,
    #[serde(skip)]
    swatches: Vec<Hsla>,
}

fn custom_label(id: &str) -> String {
    let mut label = String::new();
    let mut separator = false;
    let mut word_start = true;
    for character in id.chars() {
        if matches!(character, '-' | '_') {
            if !separator {
                label.push(' ');
            }
            separator = true;
            word_start = true;
        } else {
            separator = false;
            let word = character.is_ascii_alphanumeric();
            label.push(if word && word_start {
                character.to_ascii_uppercase()
            } else {
                character
            });
            word_start = !word;
        }
    }
    let label = label.trim();
    if label.is_empty() {
        id.into()
    } else {
        label.into()
    }
}

fn choices(directory: &std::path::Path) -> (Vec<Choice>, Vec<String>, Vec<String>) {
    let names: Vec<Choice> = serde_json::from_str(include_str!("../assets/theme-choices.json"))
        .expect("Invalid original theme names");
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut choices = appearance::available(directory)
        .into_iter()
        .filter_map(|id| {
            let palette = match appearance::resolve(&id, directory) {
                Ok(palette) => palette,
                Err(error) => {
                    errors.push(format!("{id}: {error:#}"));
                    return None;
                }
            };
            if let Some(warning) = palette.color_warning() {
                warnings.push(format!("{id}: {warning}"));
            }
            let label = names
                .iter()
                .find(|choice| choice.value == id)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| custom_label(&id));
            Some(Choice {
                label,
                value: id,
                mode: palette.mode,
                swatches: [
                    ("background", "#000000"),
                    ("surface", "#000000"),
                    ("foreground", "#ffffff"),
                    ("primary", "#3574fc"),
                ]
                .into_iter()
                .map(|(key, fallback)| {
                    color(palette.colors.get(key).map_or(fallback, String::as_str))
                })
                .collect(),
            })
        })
        .collect::<Vec<_>>();
    choices.sort_by_key(|choice| choice.label.to_lowercase());
    (choices, errors, warnings)
}

fn files_signature(directory: &std::path::Path) -> Vec<String> {
    let mut files = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| crate::fonts::file_signature(entry.path().to_str()))
        .collect::<Vec<_>>();
    files.sort();
    files
}

#[derive(IntoElement)]
struct ThemeSection {
    app: WeakEntity<Feishin>,
}
impl RenderOnce for ThemeSection {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        window.use_keyed_state("theme-settings-fields", cx, |window, cx| {
            ThemeFields::new(self.app, window, cx)
        })
    }
}

struct ThemeFields {
    app: WeakEntity<Feishin>,
    accent: Entity<ColorPickerState>,
    shade: Entity<SliderState>,
    synced_shade: u8,
    css_open: bool,
    css_mounted: bool,
    css_saved: String,
    css_preview: String,
    choices: Vec<Choice>,
    errors: Vec<String>,
    warnings: Vec<String>,
    files: Vec<String>,
    last_scan: std::time::Instant,
    _subscriptions: Vec<Subscription>,
}
impl ThemeFields {
    fn finish_css_edit(
        &mut self,
        input: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Accessibility activation may skip pointer focus, so capture an active draft explicitly.
        if input.read(cx).focus_handle(cx).is_focused(window) {
            self.css_preview = input.read(cx).value().to_string();
            window.blur();
        }
    }

    fn new(app: WeakEntity<Feishin>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (settings, directory) = app
            .read_with(cx, |view, _| {
                (view.state.settings.clone(), view.data_dir.join("Themes"))
            })
            .expect("Theme settings app was released");
        let accent =
            cx.new(|cx| ColorPickerState::new(window, cx).default_value(color(&settings.accent)));
        let shade = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(9.)
                .step(1.)
                .default_value(settings.primary_shade as f32)
        });
        let mut subscriptions = app
            .update(cx, |view, cx| {
                let input_picker = accent.clone();
                vec![
                    cx.subscribe_in(
                        &view.accent_input,
                        window,
                        move |view, input, event: &InputEvent, window, cx| {
                            if view.state.settings.use_theme_accent_color
                                // A pointer preview can arrive before the old input's Blur event.
                                || input_picker.read(cx).is_dragging()
                                || !matches!(
                                    event,
                                    InputEvent::Blur | InputEvent::PressEnter { .. }
                                )
                            {
                                return;
                            }
                            let value = input.read(cx).value().to_string();
                            if value == view.state.settings.accent {
                                return;
                            }
                            match appearance::color(&value) {
                                Ok(_) => {
                                    view.state.settings.accent = value;
                                    view.apply_theme(window, cx);
                                    view.save();
                                }
                                Err(error) => view.message = error.to_string(),
                            }
                            cx.notify();
                        },
                    ),
                    cx.subscribe_in(
                        &accent,
                        window,
                        |view, _, event: &ColorPickerEvent, window, cx| {
                            let (ColorPickerEvent::Change(Some(value))
                            | ColorPickerEvent::Preview(Some(value))) = event
                            else {
                                return;
                            };
                            if view.state.settings.use_theme_accent_color {
                                return;
                            }
                            let rgba: Rgba = (*value).into();
                            let accent = format!(
                                "rgb({}, {}, {})",
                                (rgba.r * 255.).round() as u8,
                                (rgba.g * 255.).round() as u8,
                                (rgba.b * 255.).round() as u8
                            );
                            view.accent_input.update(cx, |input, cx| {
                                input.set_value(accent.clone(), window, cx)
                            });
                            if matches!(event, ColorPickerEvent::Change(_)) {
                                view.state.settings.accent = accent;
                                view.apply_theme(window, cx);
                                view.save();
                            }
                            cx.notify();
                        },
                    ),
                    cx.subscribe_in(
                        &shade,
                        window,
                        |view, _, event: &SliderEvent, window, cx| {
                            let SliderEvent::ChangeEnd(value) = event else {
                                return;
                            };
                            if view.state.settings.use_theme_primary_shade {
                                return;
                            }
                            view.state.settings.primary_shade =
                                value.end().round().clamp(0., 9.) as u8;
                            view.apply_theme(window, cx);
                            view.save();
                            cx.notify();
                        },
                    ),
                ]
            })
            .unwrap_or_default();
        let css_input = app.read_with(cx, |view, _| view.css_input.clone()).unwrap();
        subscriptions.push(
            cx.subscribe_in(&css_input, window, |fields, input, event, _, cx| {
                if matches!(event, InputEvent::Blur) {
                    fields.css_preview = input.read(cx).value().to_string();
                    cx.notify();
                }
            }),
        );
        let (choices, errors, warnings) = choices(&directory);
        Self {
            app,
            accent,
            shade,
            synced_shade: settings.primary_shade,
            css_open: false,
            css_mounted: false,
            css_saved: settings.css.content.clone(),
            css_preview: settings.css.content,
            choices,
            errors,
            warnings,
            files: files_signature(&directory),
            last_scan: cx.background_executor().now(),
            _subscriptions: subscriptions,
        }
    }
}

impl Feishin {
    pub(crate) fn theme_settings_rows(&self, query: &str, cx: &Context<Self>) -> Vec<AnyElement> {
        if KEYS
            .iter()
            .chain(
                [
                    SOLARIZED_KEY,
                    "setting.customCss",
                    "setting.customThemeErrors",
                    "setting.customThemeWarnings",
                ]
                .iter(),
            )
            .any(|key| settings_search::matches(&self.state.settings.language, query, &[key]))
        {
            vec![
                ThemeSection {
                    app: cx.weak_entity(),
                }
                .into_any_element(),
            ]
        } else {
            vec![]
        }
    }
}

impl Render for ThemeFields {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let fields = cx.weak_entity();
        self.app
            .update(cx, |view, cx| {
                let now = cx.background_executor().now();
                if now.saturating_duration_since(self.last_scan)
                    >= std::time::Duration::from_secs(2)
                {
                    self.last_scan = now;
                    let directory = view.data_dir.join("Themes");
                    let files = files_signature(&directory);
                    if files != self.files {
                        self.files = files;
                        (self.choices, self.errors, self.warnings) = choices(&directory);
                    }
                }
                let query = view.settings_search.read(cx).value().to_string();
                let matches = |key: &str| {
                    settings_search::matches(&view.state.settings.language, &query, &[key])
                };
                let mut rows = Vec::new();
                if matches(SOLARIZED_KEY) {
                    let settings = &view.state.settings;
                    let controls = div().h_flex().gap_1().children(
                        [
                            ("light", "setting.solarizedLight", "solarizedLight"),
                            ("dark", "setting.solarizedDark", "solarizedDark"),
                            ("system", "setting.solarizedSystem", "solarizedDark"),
                        ]
                        .into_iter()
                        .map(|(mode, label, theme)| {
                            let selected = if mode == "system" {
                                settings.follow_system_theme
                                    && settings.theme_dark == "solarizedDark"
                                    && settings.theme_light == "solarizedLight"
                            } else {
                                !settings.follow_system_theme && settings.theme == theme
                            };
                            let label = view.tr(label);
                            crate::styles::button(
                                SharedString::from(format!("solarized-{mode}")),
                                cx,
                            )
                            .debug_selector(move || format!("solarized-{mode}"))
                            .label(label.clone())
                            .aria_label(format!("Solarized {label}"))
                            .selected(selected)
                            .when(selected, |button| button.primary())
                            .on_click(cx.listener(
                                move |view, _, window, cx| {
                                    view.state.settings.theme = theme.into();
                                    view.state.settings.theme_dark = "solarizedDark".into();
                                    view.state.settings.theme_light = "solarizedLight".into();
                                    view.state.settings.follow_system_theme = mode == "system";
                                    view.state.settings.use_theme_accent_color = true;
                                    view.state.settings.use_theme_primary_shade = true;
                                    view.apply_theme(window, cx);
                                    view.save();
                                    cx.notify();
                                },
                            ))
                        }),
                    );
                    rows.push((
                        0,
                        row(
                            "theme-solarized",
                            SOLARIZED_KEY,
                            controls.into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                for (index, id, value) in [
                    (0, "theme-system", view.state.settings.follow_system_theme),
                    (
                        4,
                        "theme-use-accent",
                        view.state.settings.use_theme_accent_color,
                    ),
                    (
                        6,
                        "theme-use-shade",
                        view.state.settings.use_theme_primary_shade,
                    ),
                    (9, "theme-css-enabled", view.state.settings.css.enabled),
                ] {
                    if !matches(KEYS[index]) {
                        continue;
                    }
                    let control = Switch::new(id)
                        .checked(value)
                        .tooltip(view.tr(KEYS[index]))
                        .settings(view.state.settings.zoom, crate::appearance::rtl(cx), id)
                        .on_click(cx.listener(move |view, checked, window, cx| {
                            match index {
                                0 => view.state.settings.follow_system_theme = *checked,
                                4 => view.state.settings.use_theme_accent_color = *checked,
                                6 => view.state.settings.use_theme_primary_shade = *checked,
                                _ => view.state.settings.css.enabled = *checked,
                            }
                            view.apply_theme(window, cx);
                            view.save();
                            cx.notify();
                        }))
                        .into_any_element();
                    rows.push((index, row(id, KEYS[index], control, view, cx)));
                }
                for (index, id, value, visible) in [
                    (
                        1,
                        "theme-select",
                        &view.state.settings.theme,
                        !view.state.settings.follow_system_theme,
                    ),
                    (
                        2,
                        "theme-dark-select",
                        &view.state.settings.theme_dark,
                        view.state.settings.follow_system_theme,
                    ),
                    (
                        3,
                        "theme-light-select",
                        &view.state.settings.theme_light,
                        view.state.settings.follow_system_theme,
                    ),
                ] {
                    if !visible || !matches(KEYS[index]) {
                        continue;
                    }
                    let app = cx.weak_entity();
                    let choices = self.choices.clone();
                    let label = choices
                        .iter()
                        .find(|choice| {
                            choice.value
                                == match value.as_str() {
                                    "dark" => "defaultDark",
                                    "light" => "defaultLight",
                                    value => value,
                                }
                        })
                        .map_or_else(|| value.clone(), |choice| choice.label.clone());
                    let control = Popover::new(id)
                        .anchor(Corner::TopRight)
                        .trigger(
                            crate::styles::button(id, cx)
                                .label(label)
                                .aria_label(view.tr(KEYS[index]))
                                .debug_selector(move || id.into())
                                .w(rems(15.))
                                .justify_between(),
                        )
                        .content(move |_, window, cx| {
                            let popover = cx.weak_entity();
                            let picker =
                                window.use_keyed_state("theme-choice-picker", cx, |window, cx| {
                                    ChoicePicker::new(
                                        app.clone(),
                                        popover,
                                        index,
                                        choices.clone(),
                                        window,
                                        cx,
                                    )
                                });
                            if picker.read(cx).choices != choices {
                                picker.update(cx, |picker, cx| {
                                    picker.choices = choices.clone();
                                    picker.highlight = 0;
                                    cx.notify();
                                });
                            }
                            picker
                        })
                        .into_any_element();
                    rows.push((index, row(id, KEYS[index], control, view, cx)));
                }
                if matches(KEYS[5]) {
                    let value = color(&view.state.settings.accent);
                    if self.accent.read(cx).value() != Some(value) {
                        self.accent
                            .update(cx, |picker, cx| picker.set_value(value, window, cx));
                    }
                    let picker = ColorPicker::new(&self.accent)
                        .spectrum()
                        .with_size(gpui_component::Size::Size(cx.theme().font_size))
                        .aria_label(view.tr(KEYS[5]))
                        .disabled(view.state.settings.use_theme_accent_color)
                        .featured_colors(
                            ["#3574fc", "#f0aa16", "#1db954", "#d6513f", "#aa6ed8"]
                                .map(color)
                                .to_vec(),
                        );
                    // Chromium's un-sized color input uses platform input metrics.
                    let width = if cfg!(target_os = "macos") {
                        196.
                    } else {
                        250.
                    };
                    let control = div()
                        .h_flex()
                        .w(rems(width / 16.))
                        .gap_1()
                        .child(picker)
                        .child(
                            div().flex_1().min_w_0().child(
                                crate::styles::input(&view.accent_input, cx)
                                    .disabled(view.state.settings.use_theme_accent_color),
                            ),
                        );
                    rows.push((
                        5,
                        row(
                            "theme-accent",
                            KEYS[5],
                            control.into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                if !view.state.settings.use_theme_primary_shade && matches(KEYS[7]) {
                    // Parent refreshes must preserve a drag until ChangeEnd saves it.
                    if self.synced_shade != view.state.settings.primary_shade {
                        self.synced_shade = view.state.settings.primary_shade;
                        self.shade.update(cx, |slider, cx| {
                            slider.set_value(view.state.settings.primary_shade as f32, window, cx)
                        });
                    }
                    rows.push((
                        7,
                        row(
                            "theme-shade",
                            KEYS[7],
                            div()
                                .w(rems(7.5))
                                .child(
                                    crate::styles::slider(&self.shade, cx)
                                        .aria_label(view.tr(KEYS[7]))
                                        .settings(view.state.settings.zoom, "theme-shade")
                                        .value_label(
                                            (self.shade.read(cx).value().end() as u8).to_string(),
                                        ),
                                )
                                .into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                if matches(KEYS[8]) {
                    let fields = fields.clone();
                    let controls = div()
                        .h_flex()
                        .gap_2()
                        .child(
                            crate::styles::button("theme-folder", cx)
                                .ghost()
                                .h(rems(1.875))
                                .label(view.tr("common.openFolder"))
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let path = view.data_dir.join("Themes");
                                    if let Err(error) = std::fs::create_dir_all(&path) {
                                        view.message = error.to_string();
                                        cx.notify();
                                    } else {
                                        cx.reveal_path(&path);
                                    }
                                })),
                        )
                        .child(
                            crate::styles::button("theme-reload", cx)
                                .ghost()
                                .h(rems(1.875))
                                .label(view.tr("common.reload"))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    let directory = view.data_dir.join("Themes");
                                    _ = fields.update(cx, |fields, cx| {
                                        (fields.choices, fields.errors, fields.warnings) =
                                            choices(&directory);
                                        cx.notify();
                                    });
                                    view.theme_signature.clear();
                                    view.apply_theme(window, cx);
                                    cx.notify();
                                })),
                        );
                    rows.push((
                        8,
                        row(
                            "theme-custom",
                            KEYS[8],
                            controls.into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                if !self.errors.is_empty() && matches("setting.customThemeErrors") {
                    rows.push((
                        8,
                        row(
                            "theme-errors",
                            "setting.customThemeErrors",
                            div()
                                .v_flex()
                                .text_color(cx.theme().danger)
                                .text_sm()
                                .children(self.errors.clone())
                                .into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                if !self.warnings.is_empty() && matches("setting.customThemeWarnings") {
                    rows.push((
                        8,
                        row(
                            "theme-warnings",
                            "setting.customThemeWarnings",
                            div()
                                .v_flex()
                                .text_color(cx.theme().warning)
                                .text_sm()
                                .children(self.warnings.clone())
                                .into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                if self.css_saved != view.state.settings.css.content {
                    self.css_saved = view.state.settings.css.content.clone();
                    self.css_preview = self.css_saved.clone();
                }
                let css_visible = view.state.settings.css.enabled && matches("setting.customCss");
                if css_visible {
                    let save_fields = fields.clone();
                    let controls = div()
                        .h_flex()
                        .gap_4()
                        .child(
                            crate::styles::button("theme-css-folder", cx)
                                .ghost()
                                .h(rems(1.875))
                                .label(view.tr("common.openFolder"))
                                .on_click(
                                    cx.listener(|view, _, _, cx| cx.reveal_path(&view.data_dir)),
                                ),
                        )
                        .when(self.css_open, |controls| {
                            controls.child(
                                crate::styles::button("theme-css-save", cx)
                                    .debug_selector(|| "theme-css-save".into())
                                    .primary()
                                    .h(rems(1.875))
                                    .label(view.tr("common.save"))
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        if let Ok(content) = save_fields.update(cx, |fields, cx| {
                                            fields.finish_css_edit(&view.css_input, window, cx);
                                            fields.css_preview.clone()
                                        }) {
                                            view.save_custom_css(content, window, cx);
                                        }
                                    })),
                            )
                        })
                        .child(
                            crate::styles::button("custom-css-settings", cx)
                                .debug_selector(|| "theme-css-toggle".into())
                                .primary()
                                .h(rems(1.875))
                                .label(view.tr(if self.css_open {
                                    "common.close"
                                } else {
                                    "common.edit"
                                }))
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    _ = fields.update(cx, |fields, cx| {
                                        fields.finish_css_edit(&view.css_input, window, cx);
                                        fields.css_open = !fields.css_open;
                                        cx.notify();
                                    });
                                })),
                        );
                    rows.push((
                        10,
                        row(
                            "theme-css",
                            "setting.customCss",
                            controls.into_any_element(),
                            view,
                            cx,
                        ),
                    ));
                }
                let mounted = css_visible && self.css_open;
                if mounted {
                    if !self.css_mounted {
                        view.css_input.update(cx, |input, cx| {
                            input.set_value(self.css_preview.clone(), window, cx)
                        });
                    }
                    rows.push((
                        11,
                        div()
                            .w_full()
                            .child(crate::styles::input(&view.css_input, cx))
                            .into_any_element(),
                    ));
                    rows.push((
                        12,
                        div()
                            .child(format!("{}: ", view.tr("common.preview")))
                            .into_any_element(),
                    ));
                    rows.push((
                        13,
                        div()
                            .id("theme-css-preview")
                            .debug_selector(|| "theme-css-preview".into())
                            .w_full()
                            .rounded_md()
                            .p_3()
                            .bg(cx.theme().muted)
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(rems(0.8125))
                            .line_height(rems(1.25))
                            .child(
                                gpui_component::text::TextView::html(
                                    "theme-css-preview-text",
                                    format!(
                                        "<pre>{}</pre>",
                                        self.css_preview
                                            .replace('&', "&amp;")
                                            .replace('<', "&lt;")
                                            .replace('>', "&gt;")
                                    ),
                                    window,
                                    cx,
                                )
                                .selectable(true),
                            )
                            .into_any_element(),
                    ));
                    if let Some(styles) = cx.try_global::<crate::styles::NativeStyles>()
                        && !styles.warnings.is_empty()
                    {
                        rows.push((
                            14,
                            div()
                                .v_flex()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .children(styles.warnings.clone())
                                .into_any_element(),
                        ));
                    }
                }
                self.css_mounted = mounted;
                rows.sort_by_key(|(index, _)| *index);
                div()
                    .v_flex()
                    .w_full()
                    .gap(rems(1.5))
                    .children(rows.into_iter().map(|(_, row)| row))
            })
            .unwrap_or_else(|_| div())
    }
}

struct ChoicePicker {
    app: WeakEntity<Feishin>,
    index: usize,
    choices: Vec<Choice>,
    popover: WeakEntity<PopoverState>,
    highlight: usize,
    scroll: ScrollHandle,
    search: Entity<InputState>,
    _subscription: Subscription,
}
impl ChoicePicker {
    fn new(
        app: WeakEntity<Feishin>,
        popover: WeakEntity<PopoverState>,
        index: usize,
        choices: Vec<Choice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label("Theme")
                .placeholder("Search")
        });
        let subscription = cx.subscribe_in(
            &search,
            window,
            |picker, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    picker.highlight = 0;
                    picker.scroll.scroll_to_top_of_item(0);
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    if let Some(choice) = picker.filtered(cx).get(picker.highlight) {
                        picker.choose(&choice.value, window, cx);
                    }
                }
                _ => {}
            },
        );
        let focus = search.clone();
        window.defer(cx, move |window, cx| {
            focus.update(cx, |input, cx| input.focus(window, cx))
        });
        Self {
            app,
            popover,
            index,
            choices,
            search,
            highlight: 0,
            scroll: ScrollHandle::new(),
            _subscription: subscription,
        }
    }
    fn filtered(&self, cx: &App) -> Vec<Choice> {
        let query = self.search.read(cx).value().to_lowercase();
        let query = query.as_str();
        ["dark", "light"]
            .into_iter()
            .flat_map(|mode| {
                self.choices
                    .iter()
                    .filter(move |choice| {
                        choice.mode == mode && choice.label.to_lowercase().contains(query)
                    })
                    .cloned()
            })
            .collect()
    }
    fn choose(&self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        _ = self.app.update(cx, |view, cx| {
            match self.index {
                2 => view.state.settings.theme_dark = id.into(),
                3 => view.state.settings.theme_light = id.into(),
                _ => view.state.settings.theme = id.into(),
            }
            view.apply_theme(window, cx);
            view.save();
            cx.notify();
        });
        _ = self
            .popover
            .update(cx, |popover, cx| popover.dismiss(window, cx));
    }
    fn step(&mut self, direction: i32, cx: &mut Context<Self>) {
        let filtered = self.filtered(cx);
        self.highlight =
            (self.highlight as i32 + direction).rem_euclid(filtered.len().max(1) as i32) as usize;
        let mut last = "";
        let headers = filtered
            .iter()
            .take(self.highlight + 1)
            .filter(|choice| {
                let changed = last != choice.mode;
                last = &choice.mode;
                changed
            })
            .count();
        self.scroll.scroll_to_item(self.highlight + headers);
        cx.stop_propagation();
        cx.notify();
    }
}
impl Render for ChoicePicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered(cx);
        let mut mode = String::new();
        let mut rows = Vec::new();
        for (index, choice) in filtered.iter().enumerate() {
            if mode != choice.mode {
                mode.clone_from(&choice.mode);
                rows.push(
                    div()
                        .p_2()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::appearance::text(
                            if mode == "dark" {
                                "setting.themeDark"
                            } else {
                                "setting.themeLight"
                            },
                            cx,
                        ))
                        .into_any_element(),
                );
            }
            let id = choice.value.clone();
            rows.push(
                crate::styles::button(SharedString::from(format!("theme-choice-{id}")), cx)
                    .ghost()
                    .aria_label(choice.label.clone())
                    .w_full()
                    .justify_start()
                    .selected(index == self.highlight)
                    .child(
                        div().h_flex().gap_1().children(
                            choice
                                .swatches
                                .iter()
                                .map(|color| div().size(rems(0.875)).rounded(px(3.)).bg(*color)),
                        ),
                    )
                    .child(choice.label.clone())
                    .on_click(
                        cx.listener(move |picker, _, window, cx| picker.choose(&id, window, cx)),
                    )
                    .into_any_element(),
            );
        }
        div()
            .v_flex()
            .w(rems(20.))
            .max_w_full()
            .gap_2()
            .capture_action(
                cx.listener(|picker, _: &gpui_component::input::MoveDown, _, cx| {
                    picker.step(1, cx)
                }),
            )
            .capture_action(
                cx.listener(|picker, _: &gpui_component::input::MoveUp, _, cx| picker.step(-1, cx)),
            )
            .capture_key_down(cx.listener(|picker, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers != Modifiers::none() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "down" => picker.step(1, cx),
                    "up" => picker.step(-1, cx),
                    "enter" => {
                        if let Some(choice) = picker.filtered(cx).get(picker.highlight) {
                            picker.choose(&choice.value, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(crate::styles::input(&self.search, cx))
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .id("theme-choices")
                            .v_flex()
                            .max_h(rems(20.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .children(rows),
                    )
                    .child(crate::scroll::Scrollbar::vertical(
                        "theme-choice-scrollbar",
                        &self.scroll,
                    )),
            )
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests {
    use super::KEYS;
    use crate::app::{Feishin, Route};
    use feishin_core::{i18n, storage::State};
    use gpui::accesskit::{Action, ActionData, ActionRequest, TreeId};
    use gpui::{AppContext, Role, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{cell::RefCell, rc::Rc};

    fn action(
        cx: &mut VisualTestContext,
        role: Role,
        label: &str,
        action: Action,
        data: Option<ActionData>,
    ) {
        cx.update(|window, cx| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let (id, _) = tree
                .nodes
                .iter()
                .find(|(_, node)| node.role() == role && node.label() == Some(label))
                .unwrap_or_else(|| {
                    panic!(
                        "Missing {role:?}: {label}; present: {:?}",
                        tree.nodes
                            .iter()
                            .filter(|(_, node)| node.role() == role)
                            .map(|(_, node)| node.label())
                            .collect::<Vec<_>>()
                    )
                });
            window.dispatch_accessibility_action_for_test(
                ActionRequest {
                    action,
                    target_tree: TreeId::ROOT,
                    target_node: *id,
                    data,
                },
                cx,
            );
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn inline_css_editor_matches_original_draft_save_and_reload_states(cx: &mut TestAppContext) {
        fn declarations(css: &str) -> std::collections::BTreeMap<String, String> {
            feishin_core::styles::Stylesheet::parse([css])
                .unwrap()
                .declarations(&[feishin_core::styles::Target::new("div", "", &["test"])])
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-inline-css-{}", uuid::Uuid::new_v4()));
        let capture = Rc::new(RefCell::new(None));
        let captured = capture.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            window.activate_window();
            let mut state = State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.css.content = ".test { color: red; }".into();
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.settings_search
                    .update(cx, |input, cx| input.set_value("Custom CSS", window, cx));
            });
            let fields = cx.new(|cx| super::ThemeFields::new(app.downgrade(), window, cx));
            captured.replace(Some((app, fields.clone())));
            Root::new(fields, window, cx)
        });
        let (app, fields) = capture.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(900.), px(700.)));
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/css-editor-reference.json")).unwrap();
        for case in reference.iter().take(11) {
            let name = case["name"].as_str().unwrap();
            match name {
                "enabled" | "disable" | "reenable" => {
                    action(
                        cx,
                        Role::Switch,
                        &i18n::text("en", "setting.customCssEnable"),
                        Action::Click,
                        None,
                    );
                }
                "open" | "reopen" => action(
                    cx,
                    Role::Button,
                    &i18n::text("en", "common.edit"),
                    Action::Click,
                    None,
                ),
                "close" => action(
                    cx,
                    Role::Button,
                    &i18n::text("en", "common.close"),
                    Action::Click,
                    None,
                ),
                "draft" => {
                    action(
                        cx,
                        Role::MultilineTextInput,
                        &i18n::text("en", "setting.customCss"),
                        Action::Focus,
                        None,
                    );
                    action(
                        cx,
                        Role::MultilineTextInput,
                        &i18n::text("en", "setting.customCss"),
                        Action::SetValue,
                        Some(ActionData::Value(".test { color: blue; }".into())),
                    );
                }
                "blur" => cx.update(|window, _| window.blur()),
                "save" => action(
                    cx,
                    Role::Button,
                    &i18n::text("en", "common.save"),
                    Action::Click,
                    None,
                ),
                "external" => {
                    std::fs::write(
                        directory.join("custom.css"),
                        case["stored"]["content"].as_str().unwrap(),
                    )
                    .unwrap();
                    app.update(cx, |view, cx| {
                        view.sync_css_file();
                        cx.notify();
                    });
                }
                _ => {}
            }
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let mounted = case["editor"].as_bool().unwrap();
            app.read_with(cx, |view, cx| {
                assert_eq!(
                    view.route,
                    Route::Settings,
                    "CSS editing left Settings: {name}"
                );
                assert_eq!(
                    view.state.settings.css.enabled,
                    case["stored"]["enabled"].as_bool().unwrap(),
                    "{name}"
                );
                // The native editor preserves source formatting; compare applied declarations.
                assert_eq!(
                    declarations(&view.state.settings.css.content),
                    declarations(case["stored"]["content"].as_str().unwrap()),
                    "Saved CSS: {name}"
                );
                if mounted {
                    assert_eq!(
                        declarations(view.css_input.read(cx).value().as_ref()),
                        declarations(case["textarea"].as_str().unwrap()),
                        "Draft: {name}"
                    );
                    assert_eq!(
                        declarations(&fields.read(cx).css_preview),
                        declarations(case["preview"].as_str().unwrap()),
                        "Preview: {name}"
                    );
                }
                assert_eq!(fields.read(cx).css_mounted, mounted, "{name}");
            });
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert_eq!(
                    tree.nodes
                        .iter()
                        .any(|(_, node)| node.role() == Role::MultilineTextInput),
                    mounted,
                    "{name}"
                );
                let mut buttons = tree
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.role() == Role::Button)
                    .filter_map(|(_, node)| node.label().map(str::to_owned))
                    .collect::<Vec<_>>();
                let mut expected = case["buttons"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|key| i18n::text("en", key.as_str().unwrap()))
                    .collect::<Vec<_>>();
                buttons.sort();
                expected.sort();
                assert_eq!(buttons, expected, "{name}");
            });
        }
        // Save must also collect an active edit when a button retains input focus.
        action(
            cx,
            Role::MultilineTextInput,
            &i18n::text("en", "setting.customCss"),
            Action::Focus,
            None,
        );
        action(
            cx,
            Role::MultilineTextInput,
            &i18n::text("en", "setting.customCss"),
            Action::SetValue,
            Some(ActionData::Value(".test { color: purple; }".into())),
        );
        action(
            cx,
            Role::Button,
            &i18n::text("en", "common.save"),
            Action::Click,
            None,
        );
        let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(saved.settings.css.content, ".test { color: purple; }");
        assert_eq!(
            std::fs::read_to_string(directory.join("custom.css")).unwrap(),
            saved.settings.css.content
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn custom_theme_labels_and_invalid_files_match_original_choices() {
        let labels: serde_json::Value =
            serde_json::from_str(include_str!("../assets/custom-theme-labels-reference.json"))
                .unwrap();
        for case in labels.as_array().unwrap() {
            assert_eq!(
                super::custom_label(case["id"].as_str().unwrap()),
                case["label"].as_str().unwrap()
            );
        }
        let directory =
            std::env::temp_dir().join(format!("feishin-theme-choices-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("night_blue.json"),
            r#"{"mode":"light","extends":"defaultLight"}"#,
        )
        .unwrap();
        std::fs::write(directory.join("broken.json"), b"invalid").unwrap();
        std::fs::write(
            directory.join("partly_valid.json"),
            r##"{"colors":{"primary":"#abcdef","background":42}}"##,
        )
        .unwrap();
        let (choices, errors, warnings) = super::choices(&directory);
        assert!(choices.iter().any(|choice| choice.label == "Night Blue"
            && choice.value == "night_blue"
            && choice.mode == "light"));
        assert!(!choices.iter().any(|choice| choice.value == "broken"));
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("broken:"));
        assert!(choices.iter().any(|choice| choice.value == "partly_valid"));
        assert_eq!(
            warnings,
            ["partly_valid: Ignored invalid color value(s) for: background"]
        );
        let original: Vec<super::Choice> =
            serde_json::from_str(include_str!("../assets/theme-choices.json")).unwrap();
        for expected in original {
            assert!(choices.iter().any(|choice| choice.value == expected.value
                && choice.label == expected.label
                && choice.mode == expected.mode));
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn original_accent_swatches_keep_exact_color_bytes() {
        use gpui_component::Colorize;
        for value in ["#3574FC", "#F0AA16", "#1DB954", "#D6513F", "#AA6ED8"] {
            assert_eq!(super::color(value).to_hex(), value);
        }
    }

    #[gpui::test]
    fn accent_spectrum_matches_original_callbacks_layout_and_drag_commit(cx: &mut TestAppContext) {
        use gpui::{Context, Entity, IntoElement, Render, Window, point, prelude::*};
        use gpui_component::color_picker::{ColorPicker, ColorPickerEvent, ColorPickerState};
        struct Probe(Entity<ColorPickerState>);
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                gpui::div().size_full().child(
                    ColorPicker::new(&self.0)
                        .spectrum()
                        .aria_label("Accent color")
                        .featured_colors(
                            ["#3574fc", "#f0aa16", "#1db954", "#d6513f", "#aa6ed8"]
                                .map(super::color)
                                .to_vec(),
                        ),
                )
            }
        }
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let capture = Rc::new(RefCell::new(None));
        let captured = capture.clone();
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let picker = cx
                .new(|cx| ColorPickerState::new(window, cx).default_value(super::color("#3574fc")));
            captured.replace(Some(picker.clone()));
            Root::new(cx.new(|_| Probe(picker)), window, cx)
        });
        let picker = capture.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(700.), px(700.)));
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/accent-spectrum-reference.json")).unwrap();
        let rgb = |value: gpui::Hsla| {
            let value: gpui::Rgba = value.into();
            format!(
                "rgb({}, {}, {})",
                (value.r * 255.).round() as u8,
                (value.g * 255.).round() as u8,
                (value.b * 255.).round() as u8
            )
        };
        cx.update(|window, cx| {
            for case in reference["conversion"].as_array().unwrap() {
                let hsv = [&case["hsv"]["h"], &case["hsv"]["s"], &case["hsv"]["v"]]
                    .map(|value| value.as_f64().unwrap());
                picker.update(cx, |picker, cx| picker.set_hsv(hsv, window, cx));
                assert_eq!(
                    rgb(picker.read(cx).value().unwrap()),
                    case["rgb"].as_str().unwrap(),
                    "{case}"
                );
            }
            picker.update(cx, |picker, cx| {
                picker.set_value(super::color("#3574fc"), window, cx)
            });
        });
        cx.run_until_parked();
        action(cx, Role::Button, "Accent color", Action::Click, None);
        let layouts: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/accent-spectrum-layout.json")).unwrap();
        for layout in layouts {
            cx.update(|window, cx| {
                gpui_component::Theme::global_mut(cx).font_size =
                    px(layout["fontSize"].as_f64().unwrap() as f32);
                window.set_rem_size(px(layout["fontSize"].as_f64().unwrap() as f32));
                cx.refresh_windows();
            });
            cx.run_until_parked();
            let root = cx.debug_bounds("color-spectrum").unwrap();
            let check = |actual: gpui::Bounds<gpui::Pixels>, expected: &serde_json::Value| {
                for (actual, expected) in [
                    f32::from(actual.left() - root.left()),
                    f32::from(actual.top() - root.top()),
                    f32::from(actual.size.width),
                    f32::from(actual.size.height),
                ]
                .into_iter()
                .zip(expected.as_array().unwrap())
                {
                    assert!(
                        (actual - expected.as_f64().unwrap() as f32).abs() <= 1.,
                        "{actual} != {expected}; {layout}"
                    );
                }
            };
            check(root, &layout["wrapper"]);
            check(
                cx.debug_bounds("color-saturation").unwrap(),
                &layout["saturation"],
            );
            check(cx.debug_bounds("color-hue").unwrap(), &layout["hue"]);
            for (i, selector) in ["color-saturation-thumb", "color-hue-thumb"]
                .iter()
                .enumerate()
            {
                check(cx.debug_bounds(selector).unwrap(), &layout["thumbs"][i]);
            }
            for (i, color) in ["#3574FC", "#F0AA16", "#1DB954", "#D6513F", "#AA6ED8"]
                .iter()
                .enumerate()
            {
                check(
                    cx.debug_bounds(&format!("color-swatch-{color}")).unwrap(),
                    &layout["swatches"][i],
                );
            }
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            cx.update(|window, _| {
                let scale = window.scale_factor();
                let pixels = window
                    .rasterized_scene_for_test(size(px(700.), px(700.)))
                    .unwrap();
                let width = (700. * scale).round() as usize;
                for sample in layout["samples"].as_array().unwrap() {
                    let x = ((f32::from(root.left()) + sample["x"].as_f64().unwrap() as f32)
                        * scale)
                        .round() as usize;
                    let y = ((f32::from(root.top()) + sample["y"].as_f64().unwrap() as f32) * scale)
                        .round() as usize;
                    let actual = &pixels[(y * width + x) * 4..(y * width + x) * 4 + 3];
                    for (actual, expected) in actual.iter().zip(sample["rgb"].as_array().unwrap()) {
                        assert!(
                            actual.abs_diff(expected.as_u64().unwrap() as u8) <= 3,
                            "Spectrum gradient: {actual} != {expected}; {sample}; font {}",
                            layout["fontSize"]
                        );
                    }
                }
            });
        }
        cx.update(|window, cx| {
            gpui_component::Theme::global_mut(cx).font_size = px(16.);
            window.set_rem_size(px(16.));
            cx.refresh_windows();
        });
        cx.run_until_parked();
        let events = Rc::new(RefCell::new(Vec::new()));
        let observed = events.clone();
        let _subscription = cx.update(|_, cx| {
            cx.subscribe(&picker, move |_, event: &ColorPickerEvent, _| {
                observed
                    .borrow_mut()
                    .push(matches!(event, ColorPickerEvent::Change(_)));
            })
        });
        for case in reference["cases"].as_array().unwrap() {
            cx.update(|window, cx| {
                picker.update(cx, |picker, cx| {
                    picker.set_value(super::color(case["color"].as_str().unwrap()), window, cx)
                })
            });
            cx.run_until_parked();
            events.borrow_mut().clear();
            let hue = case["field"] == "hue";
            let selector = if hue { "color-hue" } else { "color-saturation" };
            let command = case["action"].as_str().unwrap();
            if command.starts_with("pointer") {
                let bounds = cx.debug_bounds(selector).unwrap();
                let position = match command {
                    "pointer-start" => 0.,
                    "pointer-end" => 1.,
                    _ => 0.5,
                };
                let target = point(
                    bounds.left() + bounds.size.width * position,
                    bounds.top() + bounds.size.height * position,
                );
                let before = picker.read_with(cx, |picker, _| picker.value());
                cx.simulate_mouse_down(
                    bounds.center(),
                    gpui::MouseButton::Left,
                    gpui::Modifiers::none(),
                );
                cx.simulate_mouse_move(
                    target,
                    Some(gpui::MouseButton::Left),
                    gpui::Modifiers::none(),
                );
                cx.run_until_parked();
                assert_eq!(
                    picker.read_with(cx, |picker, _| picker.value()),
                    before,
                    "Drag saved early: {case}"
                );
                assert!(!events.borrow().iter().any(|commit| *commit));
                cx.simulate_mouse_up(target, gpui::MouseButton::Left, gpui::Modifiers::none());
            } else {
                action(
                    cx,
                    Role::Slider,
                    if hue { "Hue" } else { "Color saturation" },
                    Action::Focus,
                    None,
                );
                cx.simulate_keystrokes(&command.trim_start_matches("Arrow").to_lowercase());
            }
            cx.run_until_parked();
            assert_eq!(
                rgb(picker.read_with(cx, |picker, _| picker.value().unwrap())),
                case["rgb"].as_str().unwrap(),
                "{case}"
            );
            assert_eq!(
                events.borrow().iter().filter(|commit| **commit).count(),
                usize::from(!case["changes"].as_array().unwrap().is_empty()),
                "{case}"
            );
        }
        cx.update(|window, cx| {
            picker.update(cx, |picker, cx| {
                picker.set_value(super::color("#ffffff"), window, cx)
            })
        });
        cx.run_until_parked();
        action(
            cx,
            Role::Slider,
            "Hue",
            Action::SetValue,
            Some(ActionData::NumericValue(180.)),
        );
        action(
            cx,
            Role::Slider,
            "Color saturation",
            Action::Increment,
            None,
        );
        assert_eq!(
            rgb(picker.read_with(cx, |picker, _| picker.value().unwrap())),
            "rgb(242, 255, 255)"
        );
        action(
            cx,
            Role::Slider,
            "Color saturation",
            Action::Decrement,
            None,
        );
        let saved = picker.read_with(cx, |picker, _| picker.value());
        events.borrow_mut().clear();
        let point = cx.debug_bounds("color-saturation").unwrap().center();
        cx.simulate_mouse_down(point, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_keystrokes("escape");
        cx.simulate_mouse_up(point, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(picker.read_with(cx, |picker, _| picker.value()), saved);
        assert!(!events.borrow().iter().any(|commit| *commit));
        cx.update(|window, _| {
            assert!(
                !window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some("Hue"))
            );
        });
    }

    #[gpui::test]
    fn solarized_switcher_updates_native_controls_and_persists_modes(cx: &mut TestAppContext) {
        use gpui_component::{ActiveTheme, Colorize};
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("feishin-solarized-ui-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            window.activate_window();
            let mut state = State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.route = Route::Settings;
                view.settings_search
                    .update(cx, |input, cx| input.set_value("Solarized", window, cx));
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1100.), px(760.)));
        cx.run_until_parked();
        for (label, id, background, surface, foreground) in [
            (
                "Solarized Light",
                "solarizedLight",
                "#FDF6E3",
                "#EEE8D5",
                "#657B83",
            ),
            (
                "Solarized Dark",
                "solarizedDark",
                "#002B36",
                "#073642",
                "#839496",
            ),
        ] {
            action(cx, Role::Button, label, Action::Click, None);
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(300));
            cx.refresh().unwrap();
            cx.run_until_parked();
            app.read_with(cx, |view, cx| {
                assert_eq!(view.state.settings.theme, id);
                assert!(!view.state.settings.follow_system_theme);
                let theme = cx.theme();
                for color in [
                    theme.background,
                    theme.sidebar,
                    theme.table,
                    theme.list,
                    theme.title_bar,
                ] {
                    assert_eq!(color.to_hex(), background);
                }
                for color in [
                    theme.popover,
                    theme.secondary,
                    theme.muted,
                    theme.list_hover,
                    theme.tab_active,
                ] {
                    assert_eq!(color.to_hex(), surface);
                }
                let track = cx.global::<crate::styles::NativeStyles>().style_in(
                    feishin_core::styles::Target::pseudo("before"),
                    [
                        feishin_core::styles::Target::new("div", "", &["mantine-Slider-root"]),
                        feishin_core::styles::Target::new("div", "", &["mantine-Slider-track"]),
                    ],
                    16.,
                );
                assert_eq!(track.background, Some(super::color(surface).into()));
                assert_eq!(theme.foreground.to_hex(), foreground);
                for color in [
                    theme.primary,
                    theme.ring,
                    theme.caret,
                    theme.slider_bar,
                    theme.link,
                ] {
                    assert_eq!(color.to_hex(), "#268BD2");
                }
                assert_eq!(theme.danger.to_hex(), "#DC322F");
                assert_eq!(theme.success.to_hex(), "#859900");
                assert_eq!(theme.warning.to_hex(), "#B58900");
                let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
                assert_eq!(saved.settings.theme, id);
            });
            let button = cx.debug_bounds("solarized-system").unwrap();
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                assert!(tree.nodes.iter().any(
                    |(_, node)| node.label() == Some(label) && node.is_selected() == Some(true)
                ));
                let scale = window.scale_factor();
                let width = (1100. * scale).round() as u32;
                let height = (760. * scale).round() as u32;
                let pixels = window
                    .rasterized_scene_for_test(size(px(1100.), px(760.)))
                    .unwrap();
                let offset =
                    (((600. * scale) as u32 * width + (1080. * scale) as u32) * 4) as usize;
                let expected = feishin_core::appearance::color(background)
                    .unwrap()
                    .map(|value| (value * 255.).round() as u8);
                assert_eq!(&pixels[offset..offset + 4], &expected);
                let button_x = ((f32::from(button.left()) + 8.) * scale).round() as u32;
                let button_y = ((f32::from(button.top()) + 8.) * scale).round() as u32;
                let button_offset = ((button_y * width + button_x) * 4) as usize;
                let expected_surface = feishin_core::appearance::color(surface)
                    .unwrap()
                    .map(|value| (value * 255.).round() as u8);
                assert_eq!(&pixels[button_offset..button_offset + 4], &expected_surface);
                if let Some(directory) = std::env::var_os("FEISHIN_THEME_PREVIEWS") {
                    let directory = std::path::PathBuf::from(directory);
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        directory.join(format!("{id}.png")),
                        &pixels,
                        width,
                        height,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            });
        }
        action(cx, Role::Button, "Solarized System", Action::Click, None);
        app.read_with(cx, |view, _| {
            assert!(view.state.settings.follow_system_theme);
            assert_eq!(view.selected_theme(true), "solarizedDark");
            assert_eq!(view.selected_theme(false), "solarizedLight");
        });
        let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert!(saved.settings.follow_system_theme);
        assert_eq!(saved.settings.theme_dark, "solarizedDark");
        assert_eq!(saved.settings.theme_light, "solarizedLight");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[gpui::test]
    fn inline_theme_settings_layout_matches_original_page(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::fonts::register_builtins(cx).unwrap();
        });
        let directory =
            std::env::temp_dir().join(format!("feishin-theme-layout-{}", uuid::Uuid::new_v4()));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.window_bar_style = "web".into();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| view.route = Route::Settings);
            Root::new(app, window, cx)
        });
        cx.simulate_resize(size(px(1320.), px(840.)));
        cx.run_until_parked();
        let reference: serde_json::Value = serde_json::from_str(if cfg!(target_os = "linux") {
            include_str!("../assets/theme-settings-layout-reference-linux.json")
        } else {
            include_str!("../assets/theme-settings-layout-reference.json")
        })
        .unwrap();
        let solarized = cx.debug_bounds("theme-solarized-row").unwrap();
        let new_row_offset = f32::from(solarized.size.height) + 24.;
        for (index, id) in [
            "theme-system",
            "theme-select",
            "theme-use-accent",
            "theme-accent",
            "theme-use-shade",
            "theme-custom",
            "theme-css-enabled",
        ]
        .into_iter()
        .enumerate()
        {
            for (selector, property) in [("row", "bounds"), ("caption", "caption")] {
                let actual = cx.debug_bounds(&format!("{id}-{selector}")).unwrap();
                let actual = [
                    actual.origin.x,
                    actual.origin.y,
                    actual.size.width,
                    actual.size.height,
                ]
                .map(f32::from);
                for (axis, value) in actual.into_iter().enumerate() {
                    let expected = reference[index][property][axis].as_f64().unwrap() as f32
                        + if axis == 1 { new_row_offset } else { 0. };
                    assert!(
                        (value - expected).abs() <= 1.,
                        "{id}/{selector}[{axis}]: {value} != {expected}"
                    );
                }
            }
        }
        let _ = std::fs::remove_dir_all(directory);
    }

    #[gpui::test]
    fn inline_theme_settings_match_original_visibility_and_save_interactions(
        cx: &mut TestAppContext,
    ) {
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-inline-theme-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            window.activate_window();
            let mut state = State::default();
            state.settings.home.items.clear();
            state.settings.home.feature = false;
            state.settings.lyric_fetch = false;
            state.settings.scrobble = false;
            state.settings.mpv_path = directory.join("missing-mpv").to_string_lossy().into();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            app.update(cx, |view, _| view.route = Route::Settings);
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1400.), px(1800.)));
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/theme-settings-reference.json")).unwrap();
        for case in reference.as_array().unwrap() {
            app.update(cx, |view, cx| {
                view.state.settings.follow_system_theme = case["system"].as_bool().unwrap();
                view.state.settings.use_theme_accent_color = case["accent"].as_bool().unwrap();
                view.state.settings.use_theme_primary_shade = case["shade"].as_bool().unwrap();
                cx.notify();
            });
            cx.run_until_parked();
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                for control in case["controls"].as_array().unwrap() {
                    let label = i18n::text("en", control["title"]["key"].as_str().unwrap());
                    let role = match control["inputs"][0]["type"].as_str().unwrap() {
                        "switch" => Role::Switch,
                        "slider" => Role::Slider,
                        _ => Role::Button,
                    };
                    let node = tree
                        .nodes
                        .iter()
                        .find(|(_, node)| node.role() == role && node.label() == Some(&label));
                    assert_eq!(
                        node.is_some(),
                        !control["hidden"].as_bool().unwrap(),
                        "{label}: {case}"
                    );
                    if let Some((_, node)) = node {
                        assert_eq!(
                            node.is_disabled(),
                            control["inputs"][0]["disabled"].as_bool().unwrap(),
                            "{label}"
                        );
                    }
                }
            });
        }
        // The original system switch changes which saved theme selectors are exposed.
        let system = i18n::text("en", KEYS[0]);
        action(cx, Role::Switch, &system, Action::Click, None);
        assert!(!app.read_with(cx, |view, _| view.state.settings.follow_system_theme));
        action(
            cx,
            Role::Button,
            &i18n::text("en", KEYS[1]),
            Action::Click,
            None,
        );
        action(
            cx,
            Role::TextInput,
            "Theme",
            Action::SetValue,
            Some(ActionData::Value("Dracula".into())),
        );
        assert!(cx.update(|window, _| {
            !window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| node.role() == Role::Button && node.label() == Some("Nord"))
        }));
        assert!(
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                tree.nodes.iter().any(|(id, node)| {
                    node.role() == Role::TextInput
                        && node.label() == Some("Theme")
                        && *id == tree.focus
                })
            }),
            "Theme search must receive focus when the picker opens"
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.theme.clone()),
            "dracula"
        );
        action(cx, Role::Switch, &system, Action::Click, None);
        for (key, search, expected, dark) in [
            (KEYS[2], "Nord", "nord", true),
            (KEYS[3], "GitHub Light", "githubLight", false),
        ] {
            action(
                cx,
                Role::Button,
                &i18n::text("en", key),
                Action::Click,
                None,
            );
            action(
                cx,
                Role::TextInput,
                "Theme",
                Action::SetValue,
                Some(ActionData::Value(
                    if dark { search } else { "GitHub" }.into(),
                )),
            );
            if dark {
                action(cx, Role::Button, search, Action::Click, None);
            } else {
                cx.simulate_keystrokes("down enter");
                cx.run_until_parked();
            }
            assert_eq!(
                app.read_with(cx, |view, _| if dark {
                    view.state.settings.theme_dark.clone()
                } else {
                    view.state.settings.theme_light.clone()
                }),
                expected
            );
        }
        action(
            cx,
            Role::Switch,
            &i18n::text("en", KEYS[4]),
            Action::Click,
            None,
        );
        action(cx, Role::TextInput, "Accent color", Action::Focus, None);
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            let input = tree
                .nodes
                .iter()
                .find(|(_, node)| {
                    node.role() == Role::TextInput && node.label() == Some("Accent color")
                })
                .unwrap();
            assert!(!input.1.is_disabled());
            assert_eq!(tree.focus, input.0);
        });
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.simulate_input("rgb(240, 170, 22)");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.accent.clone()),
            "rgb(240, 170, 22)"
        );
        action(
            cx,
            Role::Button,
            &i18n::text("en", KEYS[5]),
            Action::Focus,
            None,
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        action(cx, Role::Button, "#1DB954", Action::Click, None);
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.accent.clone()),
            "rgb(29, 185, 84)"
        );
        // Accessibility activation can open the picker while the RGB field retains focus.
        action(cx, Role::TextInput, "Accent color", Action::Focus, None);
        let spectrum = cx.debug_bounds("color-saturation").unwrap();
        cx.simulate_mouse_down(
            spectrum.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, cx| view
                .accent_input
                .read(cx)
                .value()
                .to_string()),
            "rgb(64, 128, 86)"
        );
        assert_eq!(
            feishin_core::storage::load(&directory.join("state.json"))
                .unwrap()
                .settings
                .accent,
            "rgb(29, 185, 84)"
        );
        cx.simulate_mouse_up(
            spectrum.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(
            feishin_core::storage::load(&directory.join("state.json"))
                .unwrap()
                .settings
                .accent,
            "rgb(64, 128, 86)"
        );
        action(cx, Role::Button, "#1DB954", Action::Click, None);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        action(
            cx,
            Role::Switch,
            &i18n::text("en", KEYS[6]),
            Action::Click,
            None,
        );
        let slider = cx.debug_bounds("theme-shade-slider").unwrap();
        let target = gpui::point(slider.left() + slider.size.width / 3., slider.center().y);
        cx.simulate_mouse_down(
            slider.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            target,
            Some(gpui::MouseButton::Left),
            gpui::Modifiers::none(),
        );
        // Playback and async settings updates can refresh the parent during a drag.
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.simulate_mouse_up(target, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.settings.primary_shade),
            3
        );
        action(
            cx,
            Role::Slider,
            &i18n::text("en", KEYS[7]),
            Action::SetValue,
            Some(ActionData::NumericValue(9.)),
        );
        let saved = feishin_core::storage::load(&directory.join("state.json")).unwrap();
        assert_eq!(saved.settings.primary_shade, 9);
        assert_eq!(saved.settings.accent, "rgb(29, 185, 84)");
        assert_eq!(saved.settings.theme, "dracula");
        assert_eq!(saved.settings.theme_dark, "nord");
        assert_eq!(saved.settings.theme_light, "githubLight");
        let themes = directory.join("Themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(
            themes.join("night_blue.json"),
            r##"{"mode":"dark","extends":"defaultDark","colors":{"primary":17,"background":"#123456"}}"##,
        )
        .unwrap();
        std::fs::write(themes.join("broken.json"), b"invalid").unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2100));
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        assert!(cx.debug_bounds("theme-errors-row").is_some());
        assert!(cx.debug_bounds("theme-warnings-row").is_some());
        action(
            cx,
            Role::Button,
            &i18n::text("en", KEYS[2]),
            Action::Click,
            None,
        );
        action(
            cx,
            Role::TextInput,
            "Theme",
            Action::SetValue,
            Some(ActionData::Value("Night Blue".into())),
        );
        action(cx, Role::Button, "Night Blue", Action::Click, None);
        assert_eq!(
            feishin_core::storage::load(&directory.join("state.json"))
                .unwrap()
                .settings
                .theme_dark,
            "night_blue"
        );
        app.update_in(cx, |view, window, cx| {
            view.settings_search.update(cx, |input, cx| {
                input.set_value(i18n::text("en", "setting.customThemeWarnings"), window, cx)
            });
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            assert!(
                tree.nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some("Warnings"))
            );
            assert!(
                !tree
                    .nodes
                    .iter()
                    .any(|(_, node)| node.value() == Some("Errors"))
            );
        });
        std::fs::write(
            themes.join("night_blue.json"),
            r##"{"mode":"dark","colors":{"primary":"#abcdef"}}"##,
        )
        .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2100));
        app.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
        cx.update(|window, _| {
            let tree = window.accessibility_tree_for_test().unwrap();
            assert!(!tree.nodes.iter().any(|(_, node)| {
                node.value()
                    .is_some_and(|text| text.contains("Ignored invalid color value(s)"))
            }));
        });
        let _ = std::fs::remove_dir_all(directory);
    }
}
