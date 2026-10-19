use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use anyhow::Result;
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Selectable, StyledExt, Theme};
use std::{borrow::Cow, path::Path};

pub fn app_font(cx: &App) -> Font {
    Font {
        features: FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)])),
        fallbacks: Some(FontFallbacks::from_fonts(vec![
            "Noto Sans JP".into(),
            "Noto Sans Hebrew".into(),
        ])),
        ..font(cx.theme().font_family.clone())
    }
}

pub fn register_builtins(cx: &App) -> Result<()> {
    let fonts = feishin_core::fonts::BUILTIN_FONT_BYTES
        .iter()
        .map(|bytes| Cow::Borrowed(*bytes))
        .collect();
    cx.text_system().add_fonts(fonts)
}
pub fn file_signature(path: Option<&str>) -> String {
    path.map_or(String::new(), |path| {
        let metadata = std::fs::metadata(path).ok();
        format!(
            "{path}-{:?}-{:?}",
            metadata.as_ref().map(std::fs::Metadata::len),
            metadata.and_then(|metadata| metadata.modified().ok())
        )
    })
}
impl Feishin {
    pub fn apply_font(&mut self, cx: &mut Context<Self>) {
        let config = &self.state.settings.font;
        let family = match config.kind.as_str() {
            "system" => config
                .system
                .clone()
                .unwrap_or_else(|| ".SystemUIFont".into()),
            "custom" => {
                let signature = file_signature(config.custom.as_deref());
                if signature != self.loaded_font_signature {
                    self.loaded_font_signature = signature;
                    self.custom_font_family = "Inter".into();
                    if let Some(path) = &config.custom {
                        match feishin_core::fonts::read(Path::new(path)).and_then(|font| {
                            if !cfg!(test) {
                                cx.text_system().add_fonts(vec![Cow::Owned(font.bytes)])?;
                            }
                            Ok(font.family)
                        }) {
                            Ok(family) => self.custom_font_family = family,
                            Err(error) => {
                                log::warn!("Custom font cannot be loaded: {error}");
                                self.message = format!("Custom font cannot be loaded: {error}");
                            }
                        }
                    }
                }
                if self.custom_font_family.is_empty() {
                    "Inter".into()
                } else {
                    self.custom_font_family.clone()
                }
            }
            _ => config.built_in.clone(),
        };
        Theme::global_mut(cx).font_family = family.into();
    }
    pub fn choose_custom_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose a TrueType, OpenType, WOFF or WOFF2 font".into()),
        });
        let directory = self.data_dir.join("Fonts");
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picker.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let task = cx.background_executor().spawn(async move {
                let font = feishin_core::fonts::read(&path)?;
                let stored = directory.join(format!("{}.ttf", uuid::Uuid::new_v4()));
                feishin_core::storage::atomic_write(&stored, &font.bytes)?;
                Ok::<_, anyhow::Error>(stored)
            });
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(path) => {
                        this.state.settings.font.kind = "custom".into();
                        this.state.settings.font.custom = Some(path.to_string_lossy().into_owned());
                        this.theme_signature.clear();
                        this.apply_theme(window, cx);
                        this.save();
                    }
                    Err(error) => this.message = format!("Cannot import font: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn font_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.state.settings.font;
        let filter = self.local_filter.to_lowercase();
        let choices = if config.kind == "system" {
            cx.text_system().all_font_names()
        } else {
            vec!["Inter".into(), "Poppins".into()]
        };
        div()
            .id("font-picker")
            .v_flex()
            .gap_3()
            .size_full()
            .native_vertical_scroll()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .children(
                        [
                            ("builtIn", "Built-in fonts"),
                            ("system", "System fonts"),
                            ("custom", "Custom font"),
                        ]
                        .into_iter()
                        .enumerate()
                        .map(|(index, (kind, label))| {
                            crate::styles::button(("font-type", index), cx)
                                .label(self.tr(label))
                                .selected(config.kind == kind)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.state.settings.font.kind = kind.into();
                                    this.theme_signature.clear();
                                    this.apply_theme(window, cx);
                                    this.save();
                                    cx.notify();
                                }))
                        }),
                    ),
            )
            .when(config.kind == "custom", |div| {
                div.child(
                    config
                        .custom
                        .clone()
                        .unwrap_or_else(|| "Choose a font file".into()),
                )
                .child(
                    crate::styles::button("custom-font-file", cx)
                        .label(self.tr("Choose file"))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.choose_custom_font(window, cx)),
                        ),
                )
            })
            .when(config.kind == "system", |div| {
                div.child(crate::styles::input(&self.local_search, cx))
            })
            .when(config.kind != "custom", |div| {
                div.children(
                    choices
                        .into_iter()
                        .enumerate()
                        .filter(|(_, family)| family.to_lowercase().contains(&filter))
                        .map(|(index, family)| {
                            let selected = if config.kind == "system" {
                                config.system.as_ref() == Some(&family)
                            } else {
                                config.built_in == family
                            };
                            crate::styles::button(("font-family", index), cx)
                                .label(family.clone())
                                .selected(selected)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if this.state.settings.font.kind == "system" {
                                        this.state.settings.font.system = Some(family.clone());
                                    } else {
                                        this.state.settings.font.built_in = family.clone();
                                    }
                                    this.theme_signature.clear();
                                    this.apply_theme(window, cx);
                                    this.save();
                                    cx.notify();
                                }))
                        }),
                )
            })
            .child(div().text_xl().child(crate::appearance::text(
                "The quick brown fox jumps over the lazy dog. 0123456789",
                cx,
            )))
            .child(
                crate::styles::button("font-back", cx)
                    .label(self.tr("Settings"))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx))),
            )
            .text_color(cx.theme().foreground)
            .into_any_element()
    }
}
