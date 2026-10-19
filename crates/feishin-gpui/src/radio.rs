use crate::app::{Feishin, Route};
use feishin_core::{
    model::{Item, ItemKind},
    queue::{Enqueue, Entry},
};
use gpui::*;
pub enum QueueChange {
    Replace(Vec<Item>, Option<Box<Item>>),
    Clear,
    Restore(Vec<Item>, Option<usize>, f64),
}

pub struct PendingQueueChange {
    pub generation: u64,
    pub change: QueueChange,
}

pub struct PendingLibraryFetch {
    pub generation: u64,
    pub route: Route,
    pub mode: Enqueue,
    pub start: Option<Item>,
    pub shuffle: bool,
}

impl Feishin {
    pub(crate) fn can_edit_radio(&self, item: &Item) -> bool {
        item.kind == ItemKind::Radio
            && self.sessions.get(&item.server_id).is_some_and(|api| {
                api.server.kind == feishin_core::model::ServerType::Jellyfin
                    || api.session().is_admin
            })
    }
    pub fn cancel_library_fetches(&mut self, cx: &mut Context<Self>) {
        for flag in self.library_fetches.values() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.library_fetches.clear();
        cx.notify();
    }
    pub fn needs_library_fetch_confirmation(&self, has_start: bool) -> bool {
        self.state.settings.confirm_large_fetch
            && (has_start || !self.has_library_selection())
            && match &self.route {
                Route::Home
                | Route::Library(_)
                | Route::Collection(_)
                | Route::Favorites(_)
                | Route::Search(..) => true,
                Route::Detail(item) => matches!(
                    item.kind,
                    ItemKind::Artist | ItemKind::AlbumArtist | ItemKind::Genre
                ),
                _ => false,
            }
    }
    pub fn sync_library_fetch_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library_fetch_prompt_open || self.queue_prompt_open {
            return;
        }
        let Some(pending) = self.pending_library_fetch.take() else {
            return;
        };
        self.library_fetch_prompt_open = true;
        let title = feishin_core::i18n::translate(
            &self.state.settings.language,
            "form.largeFetchConfirmation.title",
        )
        .unwrap_or_else(|| "Fetch entire list".into());
        let question = feishin_core::i18n::translate(
            &self.state.settings.language,
            "form.largeFetchConfirmation.description",
        )
        .unwrap_or_else(|| "Fetch all tracks in this library list?".into());
        let buttons = [
            self.tr("Cancel"),
            self.tr("Confirm"),
            self.tr("Confirm and do not show again"),
        ];
        let prompt = window.prompt(
            PromptLevel::Warning,
            &title,
            Some(&question),
            &buttons.iter().map(String::as_str).collect::<Vec<_>>(),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let answer = prompt.await.ok();
            this.update(cx, |this, cx| {
                this.library_fetch_prompt_open = false;
                if pending.generation == this.play_request_generation
                    && pending.route == this.route
                    && matches!(answer, Some(1 | 2))
                {
                    if answer == Some(2) {
                        this.state.settings.confirm_large_fetch = false;
                        this.save();
                    }
                    this.enqueue_visible_confirmed(
                        pending.mode,
                        pending.start,
                        pending.shuffle,
                        cx,
                    );
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn current_entry(&self) -> Option<&Entry> {
        self.radio_item
            .as_ref()
            .or_else(|| self.state.queue.current())
    }
    pub fn current_position(&self) -> f64 {
        if self.radio_item.is_some() {
            self.radio_position
        } else {
            self.state.queue.position
        }
    }
    pub fn radio_state(&self) -> Option<(&Entry, &feishin_core::playback::RadioMetadata, f64)> {
        self.radio_item
            .as_ref()
            .map(|entry| (entry, &self.radio_metadata, self.radio_position))
    }
    pub fn add_tracks(&mut self, items: Vec<Item>, mode: Enqueue, cx: &mut Context<Self>) {
        self.add_tracks_from(items, mode, None, cx);
    }
    pub fn add_tracks_from(
        &mut self,
        items: Vec<Item>,
        mode: Enqueue,
        start: Option<&Item>,
        cx: &mut Context<Self>,
    ) {
        let context = self.inferred_playlist_context(mode);
        self.add_tracks_in_context(items, mode, start, context.as_deref(), cx);
    }
    pub(crate) fn inferred_playlist_context(&self, mode: Enqueue) -> Option<String> {
        if matches!(mode, Enqueue::Now)
            && let crate::app::Route::Detail(item) = &self.route
            && item.kind == ItemKind::Playlist
        {
            return Some(item.id.clone());
        }
        None
    }
    pub(crate) fn add_tracks_in_context(
        &mut self,
        mut items: Vec<Item>,
        mode: Enqueue,
        start: Option<&Item>,
        context: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        if items
            .first()
            .is_some_and(|item| item.kind == ItemKind::Radio)
        {
            if matches!(mode, Enqueue::Now) {
                self.start_radio(items.remove(0), cx);
            } else {
                self.message = "Radio stations play live and cannot be queued".into();
                cx.notify();
            }
            return;
        }
        items.retain(|item| item.kind != ItemKind::Radio);
        self.filter_playback_tracks(&mut items);
        if let Some(context) = context.filter(|context| !context.is_empty()) {
            for item in &mut items {
                item.context_playlist_id = Some(context.to_owned());
            }
        }
        if matches!(mode, Enqueue::Now) && self.needs_queue_confirmation() {
            self.request_queue_change(
                QueueChange::Replace(items, start.cloned().map(Box::new)),
                cx,
            );
            return;
        }
        self.commit_tracks_from(items, mode, start, cx);
    }
    pub(crate) fn filter_playback_tracks(&self, items: &mut Vec<Item>) {
        let excluded = feishin_core::playback_filters::apply(items, &self.state.settings);
        if excluded > 0 {
            log::info!(
                "Playback filters excluded {excluded} tracks; {} remain",
                items.len()
            );
        }
    }
    fn commit_tracks_from(
        &mut self,
        items: Vec<Item>,
        mode: Enqueue,
        start: Option<&Item>,
        cx: &mut Context<Self>,
    ) {
        let index = start.and_then(|start| {
            items.iter().position(|item| {
                item.server_id == start.server_id
                    && item.id == start.id
                    && item.playlist_entry_id == start.playlist_entry_id
                    && item.playlist_position == start.playlist_position
            })
        });
        self.state.queue.up_next_enabled = self.state.settings.up_next_queue;
        self.state.queue.add(items, mode);
        if matches!(mode, Enqueue::Now)
            && let Some(index) = index
        {
            self.state.queue.current = self
                .state
                .queue
                .order
                .iter()
                .filter(|id| !self.state.queue.up_next.contains(id))
                .nth(index)
                .copied();
            self.state.queue.position = 0.;
        }
        self.save();
        if matches!(mode, Enqueue::Now) {
            self.play_current(cx);
        }
    }
    pub fn clear_queue(&mut self, cx: &mut Context<Self>) {
        if self.needs_queue_confirmation() {
            self.request_queue_change(QueueChange::Clear, cx);
        } else {
            self.clear_queue_confirmed(cx);
        }
    }
    fn needs_queue_confirmation(&self) -> bool {
        self.state.settings.confirm_queue_changes && !self.state.queue.order.is_empty()
    }
    fn request_queue_change(&mut self, change: QueueChange, cx: &mut Context<Self>) {
        self.play_request_generation += 1;
        self.pending_queue_change = Some(PendingQueueChange {
            generation: self.play_request_generation,
            change,
        });
        cx.notify();
    }
    pub fn restore_server_queue(
        &mut self,
        songs: Vec<Item>,
        current: Option<usize>,
        position: f64,
        cx: &mut Context<Self>,
    ) {
        if self.needs_queue_confirmation() {
            self.request_queue_change(QueueChange::Restore(songs, current, position), cx);
        } else {
            self.commit_queue_restore(songs, current, position, cx);
        }
    }

    fn commit_queue_restore(
        &mut self,
        songs: Vec<Item>,
        current: Option<usize>,
        position: f64,
        cx: &mut Context<Self>,
    ) {
        self.state.queue.up_next_enabled = self.state.settings.up_next_queue;
        self.state.queue.add(songs, Enqueue::Now);
        self.state.queue.current = current
            .and_then(|index| {
                self.state
                    .queue
                    .order
                    .iter()
                    .filter(|id| !self.state.queue.up_next.contains(id))
                    .nth(index)
                    .copied()
            })
            .or(self.state.queue.current);
        self.state.queue.position = position;
        self.save();
        self.play_current(cx);
    }
    pub fn sync_queue_confirmation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_prompt_open {
            return;
        }
        let Some(pending) = self.pending_queue_change.take() else {
            return;
        };
        self.queue_prompt_open = true;
        let clear = matches!(pending.change, QueueChange::Clear);
        let title = self.tr(if clear {
            "Clear queue"
        } else {
            "Replace queue"
        });
        let buttons = [
            self.tr("Cancel"),
            self.tr("Confirm"),
            self.tr("Confirm and do not show again"),
        ];
        let question = self.tr("This will remove all items from the current queue.");
        let prompt = window.prompt(
            PromptLevel::Warning,
            &title,
            Some(&question),
            &buttons.iter().map(String::as_str).collect::<Vec<_>>(),
            cx,
        );
        cx.spawn(async move |this, cx| {
            let answer = prompt.await.ok();
            this.update(cx, |this, cx| {
                this.queue_prompt_open = false;
                if pending.generation != this.play_request_generation {
                    cx.notify();
                    return;
                }
                if matches!(answer, Some(1 | 2)) {
                    if answer == Some(2) {
                        this.state.settings.confirm_queue_changes = false;
                        this.save();
                    }
                    match pending.change {
                        QueueChange::Replace(items, start) => {
                            this.commit_tracks_from(items, Enqueue::Now, start.as_deref(), cx)
                        }
                        QueueChange::Clear => this.clear_queue_confirmed(cx),
                        QueueChange::Restore(songs, current, position) => {
                            this.commit_queue_restore(songs, current, position, cx)
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn clear_queue_confirmed(&mut self, cx: &mut Context<Self>) {
        self.play_request_generation += 1;
        self.queue_selected.clear();
        self.queue_selection_order.clear();
        self.side_queue_selected.clear();
        self.side_queue_selection_order.clear();
        if self.radio_item.is_none() {
            self.report("stop", cx);
        }
        self.state.queue = Default::default();
        if self.radio_item.is_none() {
            self.play_current(cx);
        }
        self.save();
        cx.notify();
    }
    pub fn start_radio(&mut self, item: Item, cx: &mut Context<Self>) {
        self.report("stop", cx);
        self.play_request_generation += 1;
        self.reset_auto_dj();
        if self
            .sleep_timer
            .as_ref()
            .is_some_and(|timer| !matches!(timer.mode, feishin_core::sleep_timer::Mode::Timed(_)))
        {
            self.sleep_timer = None;
        }
        self.radio_item = Some(Entry {
            key: uuid::Uuid::new_v4(),
            item,
        });
        self.radio_position = 0.0;
        self.play_active(cx);
        self.save();
    }
}
