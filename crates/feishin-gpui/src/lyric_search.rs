use super::*;
use gpui_component::{Icon, Sizable, WindowExt, input::InputState, spinner::Spinner};
use std::time::Duration;

struct Search {
    parent: WeakEntity<Feishin>,
    target: Item,
    name: Entity<InputState>,
    artist: Entity<InputState>,
    results: Vec<Match>,
    selected: Option<Match>,
    preview: Option<Record>,
    searching: bool,
    previewing: bool,
    search_generation: u64,
    preview_generation: u64,
    debounced: [String; 2],
    pending: [Option<Task<()>>; 2],
    message: String,
    _subscriptions: Vec<Subscription>,
}
impl Search {
    fn new(
        parent: Entity<Feishin>,
        target: Item,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label(crate::appearance::text("form.lyricSearch.input_name", cx))
        });
        let artist = cx.new(|cx| {
            crate::appearance::input_state(window, cx)
                .aria_label(crate::appearance::text("form.lyricSearch.input_artist", cx))
        });
        name.update(cx, |input, cx| {
            input.set_value(target.name.clone(), window, cx)
        });
        artist.update(cx, |input, cx| {
            input.set_value(target.artist.clone(), window, cx)
        });
        let mut subscriptions = vec![cx.observe(&parent, |_, _, cx| cx.notify())];
        for (index, input) in [&name, &artist].into_iter().enumerate() {
            subscriptions.push(
                cx.subscribe(input, move |this, input, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = input.read(cx).value().to_string();
                        let timer = cx.background_executor().timer(Duration::from_millis(500));
                        this.pending[index] = Some(cx.spawn(async move |this, cx| {
                            timer.await;
                            this.update(cx, |this, cx| {
                                if this.debounced[index] != value {
                                    this.debounced[index] = value;
                                    this.search(cx);
                                }
                            })
                            .ok();
                        }));
                    }
                }),
            );
        }
        let debounced = [target.name.clone(), target.artist.clone()];
        Self {
            parent: parent.downgrade(),
            target,
            name,
            artist,
            results: vec![],
            selected: None,
            preview: None,
            searching: false,
            previewing: false,
            search_generation: 0,
            preview_generation: 0,
            debounced,
            pending: [None, None],
            message: String::new(),
            _subscriptions: subscriptions,
        }
    }
    fn query(&self) -> Item {
        Item {
            name: self.debounced[0].clone(),
            artist: self.debounced[1].clone(),
            ..Default::default()
        }
    }
    fn search(&mut self, cx: &mut Context<Self>) {
        self.search_generation += 1;
        let generation = self.search_generation;
        let item = self.query();
        self.message.clear();
        self.searching = false;
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let parent = parent.read(cx);
        if parent.state.settings.private_mode {
            self.message = "Disable private mode to search external lyric providers".into();
            cx.notify();
            return;
        }
        let providers = parent.state.settings.lyric_sources.clone();
        if providers.is_empty() || item.name.trim().is_empty() && item.artist.trim().is_empty() {
            self.results.clear();
            cx.notify();
            return;
        }
        self.searching = true;
        let task = cx
            .background_executor()
            .spawn(async move { Sources::new()?.search(&item, &providers) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| this.search_result(generation, result, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }
    fn search_result(
        &mut self,
        generation: u64,
        result: anyhow::Result<Vec<Match>>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.search_generation {
            return;
        }
        self.searching = false;
        match result {
            Ok(mut results) => {
                // The manual picker orders the combined provider results by score.
                // Automatic matching retains its preference for synchronized lyrics.
                results.sort_by(|a, b| a.score.total_cmp(&b.score));
                self.results = results;
            }
            Err(error) => {
                log::debug!("Lyric search failed: {error}");
                self.results.clear();
                self.message = error.to_string();
            }
        }
        cx.notify();
    }
    fn select(&mut self, selected: Match, cx: &mut Context<Self>) {
        self.preview_generation += 1;
        let generation = self.preview_generation;
        self.selected = Some(selected.clone());
        self.preview = None;
        self.previewing = false;
        self.message.clear();
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let parent = parent.read(cx);
        if parent.state.settings.private_mode {
            self.message = "Disable private mode to preview external lyrics".into();
            cx.notify();
            return;
        }
        let translation = parent.state.settings.lyric_translation;
        self.previewing = true;
        let task = cx
            .background_executor()
            .spawn(async move { Sources::new()?.get(&selected, translation) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| this.preview_result(generation, result, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }
    fn preview_result(
        &mut self,
        generation: u64,
        result: anyhow::Result<Record>,
        cx: &mut Context<Self>,
    ) {
        if generation != self.preview_generation {
            return;
        }
        self.previewing = false;
        match result {
            Ok(record) => self.preview = Some(record),
            Err(error) => {
                log::debug!("Lyric preview failed: {error}");
                self.message = error.to_string();
            }
        }
        cx.notify();
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selected) = self.selected.clone() else {
            return;
        };
        self.parent
            .update(cx, |parent, cx| {
                if let Some(record) = self.preview.clone() {
                    parent.install_lyric_match(&self.target, record, cx);
                } else {
                    parent.apply_lyric_match(self.target.clone(), selected, cx);
                }
            })
            .ok();
        window.close_all_dialogs(cx);
    }
    fn export_metadata(&self) -> Option<(feishin_core::lyric_export::Metadata, bool)> {
        let selected = self.selected.as_ref()?;
        let record = self.preview.as_ref()?;
        let lines = record.lines();
        let synchronized = lines.iter().any(|line| line.time.is_some());
        Some((
            feishin_core::lyric_export::Metadata {
                artist: selected.artist.clone(),
                name: selected.name.clone(),
                offset_ms: None,
                lyrics: if synchronized {
                    serde_json::Value::Array(lines.iter().map(|line| serde_json::json!({
                    "startMs": (line.time.unwrap_or_default()*1_000_000.).round()/1000., "text": line.text
                })).collect())
                } else {
                    serde_json::Value::String(record.raw.clone())
                },
            },
            synchronized,
        ))
    }
    fn export(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((metadata, synchronized)) = self.export_metadata() else {
            return;
        };
        let Some(parent) = self.parent.upgrade() else {
            return;
        };
        let parent = parent.read(cx);
        crate::lyric_export::open(
            metadata,
            parent.state.settings.lyric_offset * 1000.,
            synchronized,
            parent.data_dir.clone(),
            window,
            cx,
        );
    }
}
fn loading(cx: &App) -> AnyElement {
    div()
        .id("lyric-search-loading")
        .flex()
        .items_center()
        .justify_center()
        .size_full()
        .role(Role::Status)
        .aria_label(crate::appearance::text("Loading lyrics...", cx))
        .child(
            Spinner::new()
                .icon(Icon::default().path("native-icons/spinner.svg"))
                .with_size(cx.theme().font_size * 1.125)
                .animation(Duration::from_secs(1), |value| value),
        )
        .into_any_element()
}
impl Render for Search {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let parent = self.parent.upgrade();
        let zoom = parent
            .as_ref()
            .map_or(1., |parent| parent.read(cx).state.settings.zoom);
        let muted = cx.theme().muted_foreground;
        let fields = div().h_flex().gap(rems(0.5)).children(
            [
                (
                    &self.name,
                    "form.lyricSearch.input_name",
                    "lyric-search-name",
                ),
                (
                    &self.artist,
                    "form.lyricSearch.input_artist",
                    "lyric-search-artist",
                ),
            ]
            .map(|(input, label, selector)| {
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(rems(0.25))
                    .child(crate::appearance::text(label, cx))
                    .child(
                        div()
                            .debug_selector(move || selector.into())
                            .child(crate::styles::input(input, cx).cleanable(true)),
                    )
            }),
        );
        let results = div()
            .id("lyric-search-results")
            .debug_selector(|| "lyric-search-results".into())
            .flex_1()
            .min_w_0()
            .h_full()
            .native_vertical_scroll()
            .pr(rems(1.))
            .child(if self.searching {
                loading(cx)
            } else {
                div()
                    .v_flex()
                    .gap(rems(0.75))
                    .children(self.results.iter().enumerate().map(|(index, matched)| {
                        let selected = self.selected.as_ref().is_some_and(|selected| {
                            selected.id == matched.id && selected.provider == matched.provider
                        });
                        let value = matched.clone();
                        let source_id = if matched.provider == Provider::Genius {
                            gpui::http_client::Url::parse(&matched.id)
                                .ok()
                                .map(|url| url.path().to_owned())
                                .unwrap_or_else(|| matched.id.clone())
                        } else {
                            matched.id.clone()
                        };
                        let status = crate::appearance::text(
                            if matches!(matched.provider, Provider::Genius | Provider::NetEase) {
                                "common.unknown"
                            } else if matched.synchronized {
                                "page.fullscreenPlayer.config.synchronized"
                            } else {
                                "page.fullscreenPlayer.config.unsynchronized"
                            },
                            cx,
                        );
                        let score = if matched.score == 0. {
                            "0%".into()
                        } else {
                            format!("{:.2}%", (1. - matched.score) * 100.)
                        };
                        div()
                            .id(("lyric-search-result", index))
                            .debug_selector(move || format!("lyric-search-result-{index}"))
                            .role(Role::Button)
                            .aria_label(format!(
                                "{} - {} - {} - {source_id} - {status} - {score}",
                                matched.name,
                                matched.artist,
                                matched.provider.name()
                            ))
                            .tab_index(0)
                            .cursor_pointer()
                            .h_flex()
                            .justify_between()
                            .gap_2()
                            .p(rems(0.5))
                            .rounded(px(5. * zoom))
                            .when(selected, |row| row.bg(cx.theme().primary.opacity(0.3)))
                            .hover(|style| {
                                style.bg(if selected {
                                    cx.theme().primary.opacity(0.4)
                                } else {
                                    cx.theme().secondary_hover
                                })
                            })
                            .focus(|style| {
                                style.bg(if selected {
                                    cx.theme().primary.opacity(0.4)
                                } else {
                                    cx.theme().secondary_hover
                                })
                            })
                            .child(
                                div()
                                    .v_flex()
                                    .max_w(relative(0.65))
                                    .min_w_0()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_size(rems(0.875))
                                            .child(matched.name.clone()),
                                    )
                                    .child(div().text_color(muted).child(matched.artist.clone()))
                                    .child(
                                        div().text_size(rems(13. / 16.)).text_color(muted).child(
                                            format!(
                                                "{} - {source_id} - {status}",
                                                matched.provider.name()
                                            ),
                                        ),
                                    ),
                            )
                            .child(score)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select(value.clone(), cx)),
                            )
                    }))
                    .into_any_element()
            });
        let preview = self.selected.as_ref().map(|selected| {
            div()
                .id("lyric-search-preview")
                .debug_selector(|| "lyric-search-preview".into())
                .flex_1()
                .min_w_0()
                .h_full()
                .native_vertical_scroll()
                .pr(rems(1.))
                .child(if self.previewing {
                    loading(cx)
                } else if let Some(record) = &self.preview {
                    parent.as_ref().map_or_else(
                        || div().into_any_element(),
                        |parent| {
                            parent.update(cx, |parent, cx| {
                                parent.lyric_search_preview(record, selected, cx)
                            })
                        },
                    )
                } else {
                    div()
                        .text_color(muted)
                        .child(crate::appearance::text(
                            "page.fullscreenPlayer.noLyrics",
                            cx,
                        ))
                        .into_any_element()
                })
        });
        div()
            .id("lyric-search-dialog")
            .debug_selector(|| "lyric-search-dialog".into())
            .v_flex()
            .size_full()
            .gap(rems(0.75))
            .child(fields)
            .child(div().h(px(zoom)).bg(cx.theme().border).flex_none())
            .child(
                div()
                    .h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap(rems(0.5))
                    .overflow_hidden()
                    .child(results)
                    .children(preview),
            )
            .when(!self.message.is_empty(), |container| {
                container.child(div().text_color(muted).child(self.message.clone()))
            })
            .child(div().h(px(zoom)).bg(cx.theme().border).flex_none())
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap(rems(0.5))
                    .child(
                        crate::styles::button("lyric-search-cancel", cx)
                            .debug_selector(|| "lyric-search-cancel".into())
                            .label(crate::appearance::text("common.cancel", cx))
                            .on_click(|_, window, cx| window.close_all_dialogs(cx)),
                    )
                    .child(
                        crate::styles::button("lyric-search-export", cx)
                            .debug_selector(|| "lyric-search-export".into())
                            .label(crate::appearance::text("form.lyricsExport.export", cx))
                            .disabled(self.selected.is_none() || self.preview.is_none())
                            .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
                    )
                    .child(
                        crate::styles::button("lyric-search-confirm", cx)
                            .debug_selector(|| "lyric-search-confirm".into())
                            .primary()
                            .label(crate::appearance::text("common.confirm", cx))
                            .disabled(self.selected.is_none())
                            .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
                    ),
            )
    }
}
fn open(
    parent: Entity<Feishin>,
    item: Item,
    zoom: f32,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Search> {
    let search = cx.new(|cx| Search::new(parent, item, window, cx));
    let pending = search.clone();
    window.defer(cx, move |_, cx| {
        pending.update(cx, |search, cx| search.search(cx))
    });
    let focus = search.read(cx).name.read(cx).focus_handle(cx);
    let content = search.clone();
    let title = crate::appearance::text("form.lyricSearch.title", cx);
    window.open_dialog(cx, move |dialog, _, _| {
        dialog
            .title(title.clone())
            .aria_label(title.clone())
            .width(rems(48.75))
            .body_height(px(600. * zoom))
            .child(content.clone())
    });
    focus.focus(window);
    search
}
impl Feishin {
    fn lyric_search_preview(
        &self,
        record: &Record,
        matched: &Match,
        cx: &Context<Self>,
    ) -> AnyElement {
        let lines = record.lines();
        let synchronized = lines.iter().any(|line| line.time.is_some());
        let karaoke = self.state.settings.lyric_words
            && lines.iter().any(|line| {
                !line.words.is_empty() || line.cue_lines.iter().any(|cue| !cue.words.is_empty())
            });
        let position = self.lyric_position() - self.lyric_offset();
        let display = self.state.settings.lyric_display_for("default");
        let agents = record
            .variants
            .get(record.selected)
            .map(|variant| variant.agents.as_slice())
            .unwrap_or_default();
        let mut information = vec![];
        if self.state.settings.lyric_show_provider {
            information.push(matched.provider.name().to_owned());
        }
        if self.state.settings.lyric_show_match {
            information.push(format!("{} - {}", matched.name, matched.artist));
        }
        let mut rows = Vec::new();
        for (index, text) in information.into_iter().enumerate() {
            rows.push(crate::lyrics_row::Row {
                index,
                information: true,
                sidebar: false,
                synchronized: false,
                karaoke: false,
                bright: true,
                font_size: 20. * self.state.settings.zoom,
                rem: f32::from(cx.theme().font_size),
                gap_after: 20. * self.state.settings.zoom,
                padding_left: 0.,
                padding_right: 0.,
                opacity: 1.,
                scale: 1.,
                alignment: self.state.settings.lyric_alignment.clone(),
                content: crate::lyrics_markup::Text(text).into_any_element(),
                click: None,
            });
        }
        for (index, line) in lines.iter().enumerate() {
            let end = lines.get(index + 1).and_then(|line| line.time).or_else(|| {
                self.current_entry()
                    .map(|entry| entry.item.duration)
                    .filter(|duration| *duration > 0.)
            });
            rows.push(crate::lyrics_row::Row {
                index,
                information: false,
                sidebar: false,
                synchronized,
                karaoke,
                bright: !synchronized
                    || crate::lyrics_row::is_bright(
                        &lines,
                        index,
                        position,
                        self.playing,
                        karaoke,
                        self.state.settings.lyric_lead_time,
                    ),
                font_size: 20. * self.state.settings.zoom,
                rem: f32::from(cx.theme().font_size),
                gap_after: if index + 1 == lines.len() {
                    0.
                } else {
                    20. * self.state.settings.zoom
                },
                padding_left: 0.,
                padding_right: 0.,
                opacity: display.opacity_non_active,
                scale: display.scale_non_active,
                alignment: self.state.settings.lyric_alignment.clone(),
                content: self.lyric_cues(
                    line,
                    (index, false, "preview"),
                    agents,
                    (end, position),
                    None,
                    cx,
                ),
                click: line.time.map(|time| {
                    std::rc::Rc::new(cx.listener(move |this, _, _, cx| this.seek_lyric(time, cx)))
                        as _
                }),
            });
        }
        div()
            .v_flex()
            .py(rems(1.))
            .children(rows)
            .into_any_element()
    }
}
impl Feishin {
    pub fn open_lyric_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.current_entry().map(|entry| entry.item.clone()) else {
            return;
        };
        open(cx.entity(), item, self.state.settings.zoom, window, cx);
    }
}

#[cfg(test)]
#[path = "lyric_search_tests.rs"]
mod tests;
