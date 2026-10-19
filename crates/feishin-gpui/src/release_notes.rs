use crate::{app::Feishin, scroll::VerticalScroll};
use feishin_core::{
    i18n,
    release_notes::{self, Document, Release},
};
use gpui::{prelude::*, *};
use gpui_component::{
    StyledExt, WindowExt,
    button::ButtonVariants,
    menu::{DropdownMenu, PopupMenuItem},
    text::{TextView, TextViewStyle},
};
use std::{collections::BTreeMap, time::Instant};

pub struct State {
    pub open: bool,
    pub automatic: bool,
    pub automatic_checked: bool,
    pub started: Instant,
    pub selected: String,
    pub history: Vec<Release>,
    pub history_loaded: bool,
    pub documents: BTreeMap<String, Document>,
    pub document: Option<Document>,
    pub busy: bool,
    pub error: Option<String>,
    pub generation: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            automatic: false,
            automatic_checked: false,
            started: Instant::now(),
            selected: env!("CARGO_PKG_VERSION").into(),
            history: Vec::new(),
            history_loaded: false,
            documents: BTreeMap::new(),
            document: None,
            busy: false,
            error: None,
            generation: 0,
        }
    }
}

struct Content {
    view: WeakEntity<Feishin>,
    _subscription: Subscription,
}

impl Render for Content {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.view
            .update(cx, |this, cx| this.release_notes_view(window, cx))
            .unwrap_or_else(|_| div().into_any_element())
    }
}

fn date(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

impl Feishin {
    pub fn poll_release_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if cfg!(test)
            || self.release_notes.automatic_checked
            || self.release_notes.started.elapsed().as_secs() < 2
        {
            return;
        }
        if self.state.settings.last_release_notes_version == env!("CARGO_PKG_VERSION") {
            self.release_notes.automatic_checked = true;
        } else if !self.queue_prompt_open
            && !self.library_fetch_prompt_open
            && !window.has_active_dialog(cx)
        {
            self.release_notes.automatic_checked = true;
            self.release_notes.automatic = true;
            self.open_release_notes(window, cx);
        }
    }

    pub fn open_release_notes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.release_notes.open {
            return;
        }
        self.release_notes.open = true;
        let view = cx.entity();
        let weak = view.downgrade();
        let content = cx.new(|cx| Content {
            view: weak.clone(),
            _subscription: cx.observe(&view, |_, _, cx| cx.notify()),
        });
        let title = if self.release_notes.automatic {
            i18n::translate_with(
                &self.state.settings.language,
                "common.newVersion",
                &serde_json::json!({"version":env!("CARGO_PKG_VERSION")}),
            )
            .unwrap_or_else(|| format!("Feishin {}", env!("CARGO_PKG_VERSION")))
        } else {
            self.tr("View release notes")
        };
        window.open_dialog(cx, move |dialog, _, _| {
            let weak = weak.clone();
            dialog
                .title(title.clone())
                .aria_label(title.clone())
                .width(rems(48.75))
                .child(content.clone())
                .on_close(move |_, _, cx| {
                    weak.update(cx, |this, cx| this.dismiss_release_notes(cx))
                        .ok();
                })
        });
        self.select_release_notes(env!("CARGO_PKG_VERSION").into(), cx);
    }

    pub fn dismiss_release_notes(&mut self, cx: &mut Context<Self>) {
        self.release_notes.open = false;
        self.release_notes.automatic = false;
        self.release_notes.automatic_checked = true;
        self.release_notes.generation += 1;
        self.release_notes.busy = false;
        self.state.settings.last_release_notes_version = env!("CARGO_PKG_VERSION").into();
        self.save();
        cx.notify();
    }

    pub fn select_release_notes(&mut self, version: String, cx: &mut Context<Self>) {
        self.release_notes.generation += 1;
        self.release_notes.selected = version.clone();
        self.release_notes.error = None;
        self.release_notes.document = self.release_notes.documents.get(&version).cloned();
        self.release_notes.busy = self.release_notes.document.is_none();
        cx.notify();
        if !self.release_notes.busy {
            return;
        }
        let generation = self.release_notes.generation;
        let mut history = self.release_notes.history.clone();
        let loaded = self.release_notes.history_loaded;
        let task = cx.background_executor().spawn(async move {
            let client = release_notes::Client::new()?;
            let mut history_loaded = loaded;
            if !loaded {
                match client.history() {
                    Ok(releases) => {
                        history = releases;
                        history_loaded = true;
                    }
                    Err(error) => log::warn!("Cannot load release history: {error}"),
                }
            }
            let document = client.document(&version, &history);
            anyhow::Ok((history, history_loaded, document))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if generation != this.release_notes.generation || !this.release_notes.open {
                    return;
                }
                this.release_notes.busy = false;
                let document = match result {
                    Ok((history, loaded, document)) => {
                        this.release_notes.history = history;
                        this.release_notes.history_loaded = loaded;
                        document
                    }
                    Err(error) => Err(error),
                };
                match document {
                    Ok(document) => {
                        this.release_notes
                            .documents
                            .insert(this.release_notes.selected.clone(), document.clone());
                        this.release_notes.document = Some(document);
                    }
                    Err(error) => {
                        log::warn!("Cannot load release notes: {error}");
                        this.release_notes.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn release_notes_view(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.release_notes;
        let version = state.selected.clone();
        let mut options = state
            .history
            .iter()
            .map(|release| {
                let date = date(&release.published_at);
                (
                    release.version.clone(),
                    if date.is_empty() {
                        release.version.clone()
                    } else {
                        format!("{} - {date}", release.version)
                    },
                )
            })
            .collect::<Vec<_>>();
        if !options
            .iter()
            .any(|(version, _)| version == env!("CARGO_PKG_VERSION"))
        {
            options.insert(
                0,
                (
                    env!("CARGO_PKG_VERSION").into(),
                    env!("CARGO_PKG_VERSION").into(),
                ),
            );
        }
        let page = state
            .document
            .as_ref()
            .map(|document| document.page().to_owned())
            .or_else(|| release_notes::fallback_page(&version, &state.history).ok());
        let content = match state.document.as_ref() {
            Some(Document::Markdown { body, .. }) => div()
                .size_full()
                .child(
                    TextView::markdown("release-markdown", body.clone(), window, cx)
                        .style(TextViewStyle {
                            safe_urls_only: true,
                            ..Default::default()
                        })
                        .selectable(true)
                        .scrollable(true),
                )
                .into_any_element(),
            Some(Document::Commits {
                stable, commits, ..
            }) => {
                let heading = i18n::translate_with(
                    &self.state.settings.language,
                    "page.releasenotes.commitsSinceStable",
                    &serde_json::json!({"stable":stable}),
                )
                .unwrap_or_else(|| format!("Commits since {stable}"));
                div()
                    .v_flex()
                    .size_full()
                    .gap_3()
                    .child(heading)
                    .child(
                        div()
                            .id("release-commit-list")
                            .v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_2()
                            .native_vertical_scroll()
                            .when(commits.is_empty(), |div| {
                                div.child(self.tr("No new commits in this range"))
                            })
                            .children(commits.iter().enumerate().map(|(index, commit)| {
                                let page = commit.page.clone();
                                let message = commit.message.clone();
                                div()
                                    .id(("release-commit", index))
                                    .h_flex()
                                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                    .gap_2()
                                    .debug_selector(move || format!("release-commit-{index}"))
                                    .tooltip(move |window, cx| {
                                        gpui_component::tooltip::Tooltip::new(message.clone())
                                            .build(window, cx)
                                    })
                                    .child(
                                        div().flex_1().min_w_0().truncate().child(
                                            commit
                                                .message
                                                .lines()
                                                .next()
                                                .unwrap_or_default()
                                                .to_owned(),
                                        ),
                                    )
                                    .child(date(&commit.date))
                                    .child(
                                        crate::styles::button(("release-commit-page", index), cx)
                                            .ghost()
                                            .label(self.tr("View"))
                                            .on_click(move |_, _, cx| cx.open_url(&page)),
                                    )
                            })),
                    )
                    .into_any_element()
            }
            Some(Document::NoStable { .. }) => div()
                .child(self.tr("No stable release available to compare with"))
                .into_any_element(),
            None => div()
                .child(if state.busy {
                    self.tr("Loading")
                } else {
                    self.tr("An error occurred")
                })
                .into_any_element(),
        };
        div()
            .id("release-notes-content")
            .debug_selector(|| "release-notes-content".into())
            .v_flex()
            .gap_3()
            .w_full()
            .child(
                crate::styles::button("release-version", cx)
                    .debug_selector(|| "release-version".into())
                    .label(version)
                    .dropdown_menu({
                        let view = cx.weak_entity();
                        move |mut menu, _, _| {
                            for (version, label) in &options {
                                let view = view.clone();
                                let version = version.clone();
                                menu = menu.item(PopupMenuItem::new(label.clone()).on_click(
                                    move |_, _, cx| {
                                        view.update(cx, |this, cx| {
                                            this.select_release_notes(version.clone(), cx)
                                        })
                                        .ok();
                                    },
                                ));
                            }
                            menu
                        }
                    }),
            )
            .child(div().h(px(400.)).w_full().child(content))
            .when_some(state.error.as_ref(), |div, error| {
                div.child(error.clone()).child(
                    crate::styles::button("retry-release-notes", cx)
                        .label(self.tr("Retry"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.select_release_notes(this.release_notes.selected.clone(), cx)
                        })),
                )
            })
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .justify_end()
                    .gap_2()
                    .when_some(page, |div, page| {
                        div.child(
                            crate::styles::button("release-notes-page", cx)
                                .ghost()
                                .label(self.tr("View more"))
                                .on_click(move |_, _, cx| cx.open_url(&page)),
                        )
                    })
                    .child(
                        crate::styles::button("dismiss-release-notes", cx)
                            .debug_selector(|| "dismiss-release-notes".into())
                            .label(self.tr("Dismiss"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.dismiss_release_notes(cx);
                                window.close_dialog(cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}
