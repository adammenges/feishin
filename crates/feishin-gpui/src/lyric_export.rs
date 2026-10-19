use feishin_core::lyric_export::{self, Metadata};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, StyledExt, WindowExt,
    checkbox::Checkbox,
    input::{InputEvent, InputState, NumberInput, NumberInputEvent, StepAction},
    text::TextView,
};
use std::path::PathBuf;

pub(crate) struct Editor {
    metadata: Metadata,
    offset: Entity<InputState>,
    synchronized: bool,
    can_synchronize: bool,
    destination: PathBuf,
    busy: bool,
    message: String,
    _subscriptions: Vec<Subscription>,
}
impl Editor {
    pub(crate) fn new(
        metadata: Metadata,
        offset_ms: f64,
        synchronized: bool,
        destination: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let offset = cx.new(|cx| {
            crate::appearance::input_state(window, cx).aria_label(crate::appearance::text(
                "form.lyricsExport.input_offset",
                cx,
            ))
        });
        offset.update(cx, |input, cx| {
            input.set_value(offset_ms.to_string(), window, cx)
        });
        let subscription =
            cx.subscribe_in(&offset, window, |_, _, _: &InputEvent, _, cx| cx.notify());
        let step = cx.subscribe_in(
            &offset,
            window,
            |this, _, event: &NumberInputEvent, window, cx| {
                let NumberInputEvent::Step(action) = event;
                let value = this
                    .offset
                    .read(cx)
                    .value()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .unwrap_or_default();
                this.offset.update(cx, |input, cx| {
                    input.set_value(
                        (value
                            + if *action == StepAction::Increment {
                                1.
                            } else {
                                -1.
                            })
                        .to_string(),
                        window,
                        cx,
                    )
                });
                cx.notify();
            },
        );
        Self {
            metadata,
            offset,
            synchronized,
            can_synchronize: synchronized,
            destination,
            busy: false,
            message: String::new(),
            _subscriptions: vec![subscription, step],
        }
    }
    fn contents(&self, cx: &App) -> anyhow::Result<String> {
        let offset = if self.synchronized {
            let value = self.offset.read(cx).value().parse::<f64>()?;
            anyhow::ensure!(value.is_finite(), "Enter a finite lyric export offset");
            value
        } else {
            0.
        };
        lyric_export::render(&self.metadata, offset, self.synchronized)
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let raw = match self.contents(cx) {
            Ok(raw) => raw,
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
                return;
            }
        };
        let name = self
            .metadata
            .name
            .chars()
            .map(|c| {
                if c.is_control() || "<>:\"/\\|?*".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect::<String>();
        let name = name.trim_matches(['.', ' ']);
        let name = if name.is_empty() { "lyrics" } else { name };
        let filename = format!("{name}.{}", if self.synchronized { "lrc" } else { "txt" });
        let picker = cx.prompt_for_new_path(&self.destination, Some(&filename));
        self.busy = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = match picker.await {
                Ok(Ok(Some(path))) => {
                    Some(feishin_core::storage::atomic_write(&path, raw.as_bytes()))
                }
                Ok(Ok(None)) => None,
                Ok(Err(error)) => Some(Err(error)),
                Err(error) => Some(Err(anyhow::anyhow!("{error}"))),
            };
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                if let Some(result) = result {
                    match result {
                        Ok(()) => {
                            window.close_dialog(cx);
                            this.message = "Lyrics exported".into()
                        }
                        Err(error) => {
                            log::warn!("Lyric export failed: {error:#}");
                            this.message = error.to_string()
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let contents = self.contents(cx);
        let valid = contents.is_ok();
        let raw = contents.unwrap_or_else(|error| error.to_string());
        let html = format!(
            "<pre>{}</pre>",
            raw.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        );
        gpui::div()
            .v_flex()
            .w_full()
            .gap_3()
            .h_full()
            .when(self.can_synchronize, |div| {
                div.child(
                    gpui::div()
                        .h_flex()
                        .gap_3()
                        .child(
                            Checkbox::new("lyric-export-synchronized")
                                .label(crate::appearance::text(
                                    "form.lyricsExport.input_synced",
                                    cx,
                                ))
                                .checked(self.synchronized)
                                .disabled(self.busy)
                                .on_click(cx.listener(|this, value: &bool, _, cx| {
                                    this.synchronized = *value;
                                    cx.notify()
                                })),
                        )
                        .child(
                            gpui::div()
                                .v_flex()
                                .gap_1()
                                .flex_1()
                                .child(crate::appearance::text(
                                    "form.lyricsExport.input_offset",
                                    cx,
                                ))
                                .child(
                                    gpui::div()
                                        .debug_selector(|| "lyric-export-offset".into())
                                        .child(NumberInput::new(&self.offset).disabled(self.busy)),
                                ),
                        ),
                )
            })
            .child(
                gpui::div()
                    .id("lyric-export-preview")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(cx.theme().mono_font_size)
                    .child(TextView::html("lyric-export-text", html, window, cx).selectable(true)),
            )
            .when(!self.message.is_empty(), |div| {
                div.child(self.message.clone())
            })
            .child(
                gpui::div()
                    .h_flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        crate::styles::button("lyric-export-close", cx)
                            .label(crate::appearance::text("Close", cx))
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        crate::styles::button("lyric-export-copy", cx)
                            .debug_selector(|| "lyric-export-copy".into())
                            .label(crate::appearance::text("Copy", cx))
                            .disabled(!valid || self.busy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(raw) = this.contents(cx) {
                                    cx.write_to_clipboard(ClipboardItem::new_string(raw))
                                }
                            })),
                    )
                    .child(
                        crate::styles::button("lyric-export-save", cx)
                            .label(crate::appearance::text("Export", cx))
                            .debug_selector(|| "lyric-export-save".into())
                            .disabled(!valid || self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

pub(crate) fn open(
    metadata: Metadata,
    offset: f64,
    synchronized: bool,
    destination: PathBuf,
    window: &mut Window,
    cx: &mut App,
) {
    let editor = cx.new(|cx| Editor::new(metadata, offset, synchronized, destination, window, cx));
    let title = crate::appearance::text("form.lyricSearch.title", cx);
    let zoom = cx
        .try_global::<crate::styles::NativeStyles>()
        .map_or(1., |styles| styles.zoom());
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(title.clone())
            .aria_label(title.clone())
            .width(rems(48.75))
            .body_height(px(600. * zoom))
            .child(editor.clone())
    });
}

#[cfg(test)]
mod tests {
    use super::{Editor, Metadata};
    use gpui::{AppContext, TestAppContext, point, px};
    use gpui_component::{Root, WindowExt};
    use std::{cell::RefCell, rc::Rc};

    #[gpui::test]
    fn export_uses_selected_language_without_mutating_cached_lyrics_or_queue(
        cx: &mut TestAppContext,
    ) {
        use crate::app::Feishin;
        use feishin_core::{
            lyric_sources::{Match, Provider, Record},
            lyrics::{Line, Variant},
            storage::State,
        };
        cx.set_assets(crate::icons::Assets);
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-export-selection-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let app =
                cx.new(|cx| Feishin::new(directory.clone(), Ok(State::default()), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|_, cx| {
            app.update(cx, |app, _| {
                app.state.settings.lyric_offset = 0.5;
                let raw = "[00:01.00]English".to_string();
                app.lyric_record = Some(Record {
                    raw: raw.clone(),
                    selected: 1,
                    matched: Some(Match {
                        provider: Provider::Lrclib,
                        id: "fixture".into(),
                        name: "Source title".into(),
                        artist: "Source artist".into(),
                        synchronized: true,
                        score: 0.,
                    }),
                    variants: vec![
                        Variant {
                            language: "en".into(),
                            synchronized: true,
                            lines: feishin_core::lyrics::parse_lrc(&raw),
                            ..Default::default()
                        },
                        Variant {
                            language: "fr".into(),
                            synchronized: true,
                            offset: Some(0.125),
                            lines: vec![Line {
                                time: Some(1.001),
                                text: "Français".into(),
                                words: vec![],
                                cue_lines: vec![],
                            }],
                            ..Default::default()
                        },
                    ],
                });
                app.lyrics = app.lyric_record.as_ref().unwrap().lines();
                let queue = serde_json::to_value(&app.state.queue).unwrap();
                let metadata = app.lyric_export_metadata();
                assert_eq!(metadata.lyrics[0]["text"], "Français");
                assert_eq!(metadata.lyrics[0]["startMs"], 1001.);
                assert_eq!(metadata.offset_ms, Some(125.));
                assert_eq!(
                    feishin_core::lyric_export::render(&metadata, app.lyric_offset() * 1000., true)
                        .unwrap(),
                    "[ar:Source artist]\n[ti:Source title]\n[offset:250]\n[00:01.001]Français\n"
                );
                assert_eq!(app.lyric_record.as_ref().unwrap().raw, raw);
                assert_eq!(app.lyric_record.as_ref().unwrap().selected, 1);
                assert_eq!(serde_json::to_value(&app.state.queue).unwrap(), queue);
            })
        });
        cx.simulate_resize(gpui::size(px(900.), px(500.)));
        cx.update(|window, cx| {
            let metadata = app.read(cx).lyric_export_metadata();
            super::open(metadata, 250., true, directory.clone(), window, cx);
        });
        cx.run_until_parked();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(310));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let frame = cx.debug_bounds("native-modal-content-0").unwrap();
        let body = cx.debug_bounds("native-modal-body-0").unwrap();
        let save = cx.debug_bounds("lyric-export-save").unwrap();
        assert!(
            save.bottom() > frame.bottom(),
            "The original 600px body must overflow the small viewport"
        );
        cx.simulate_event(gpui::ScrollWheelEvent {
            position: body.center(),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-500.))),
            modifiers: gpui::Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        cx.run_until_parked();
        let save = cx.debug_bounds("lyric-export-save").unwrap();
        assert!(
            save.bottom() <= frame.bottom(),
            "Export must remain reachable by scrolling: {save:?} / {frame:?}"
        );
        cx.simulate_click(save.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
        std::fs::remove_dir_all(directory).ok();
    }

    #[gpui::test]
    fn export_dialog_validates_offsets_cancels_and_saves_the_preview(cx: &mut TestAppContext) {
        cx.set_assets(crate::icons::Assets);
        cx.use_platform_text_system();
        cx.update(gpui_component::init);
        let root =
            std::env::temp_dir().join(format!("feishin-lyric-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let editor = cx.new(|cx| {
                Editor::new(
                    Metadata {
                        artist: "Artist".into(),
                        name: "Selected title".into(),
                        offset_ms: Some(125.),
                        lyrics: serde_json::json!([[1001, "Français"], [1234.5, "Second"]]),
                    },
                    250.,
                    true,
                    root.clone(),
                    window,
                    cx,
                )
            });
            captured.replace(Some(editor.clone()));
            Root::new(editor, window, cx)
        });
        let editor = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        let expected = "[ar:Artist]\n[ti:Selected title]\n[offset:375]\n[00:01.001]Français\n[00:01.234]Second\n";
        let rendered = cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .collect::<Vec<_>>()
                .join("\n")
        });
        assert!(
            rendered.contains("[offset:375]") && rendered.contains("Français"),
            "{rendered}"
        );
        editor.read_with(cx, |editor, cx| {
            assert_eq!(editor.contents(cx).unwrap(), expected)
        });
        let copy = cx.debug_bounds("lyric-export-copy").unwrap().center();
        cx.simulate_click(copy, gpui::Modifiers::none());
        assert_eq!(
            cx.read_from_clipboard().unwrap().text().as_deref(),
            Some(expected)
        );
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .offset
                    .update(cx, |input, cx| input.set_value("500", window, cx));
            })
        });
        cx.run_until_parked();
        // Rich text uses the platform timer for its 200 ms debounce.
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.run_until_parked();
        let rendered = cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .filter_map(|(_, node)| node.value())
                .collect::<Vec<_>>()
                .join("\n")
        });
        assert!(rendered.contains("[offset:625]"), "{rendered}");
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .offset
                    .update(cx, |input, cx| input.set_value("invalid", window, cx));
            })
        });
        cx.run_until_parked();
        editor.read_with(cx, |editor, cx| assert!(editor.contents(cx).is_err()));
        let save = cx.debug_bounds("lyric-export-save").unwrap().center();
        cx.simulate_click(save, gpui::Modifiers::none());
        assert!(!cx.did_prompt_for_new_path());
        cx.update(|window, cx| {
            editor.update(cx, |editor, cx| {
                editor
                    .offset
                    .update(cx, |input, cx| input.set_value("250", window, cx));
            })
        });
        cx.run_until_parked();
        let save = cx.debug_bounds("lyric-export-save").unwrap().center();
        cx.simulate_click(save, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.did_prompt_for_new_path());
        cx.simulate_new_path_selection(|_| None);
        cx.run_until_parked();
        editor.read_with(cx, |editor, _| assert!(!editor.busy));
        let checkbox = cx.update(|window, _| {
            let node = window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .find(|(_, node)| node.role() == gpui::accesskit::Role::CheckBox)
                .unwrap()
                .1
                .clone();
            let bounds = node.bounds().unwrap();
            let scale = window.scale_factor() as f64;
            point(
                px(((bounds.x0 + bounds.x1) / 2. / scale) as f32),
                px(((bounds.y0 + bounds.y1) / 2. / scale) as f32),
            )
        });
        cx.simulate_click(checkbox, gpui::Modifiers::none());
        cx.run_until_parked();
        editor.read_with(cx, |editor, cx| {
            assert_eq!(editor.contents(cx).unwrap(), "Français\nSecond\n")
        });
        let save = cx.debug_bounds("lyric-export-save").unwrap().center();
        cx.simulate_click(save, gpui::Modifiers::none());
        cx.run_until_parked();
        let destination = root.join("selected.txt");
        cx.simulate_new_path_selection(|_| Some(destination.clone()));
        cx.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(&destination).unwrap(),
            "Français\nSecond\n"
        );
        editor.read_with(cx, |editor, _| assert!(!editor.busy));
        cx.update(|window, cx| window.close_all_dialogs(cx));
        std::fs::remove_dir_all(root).unwrap();
    }
}
