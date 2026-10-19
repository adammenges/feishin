use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::ItemKind,
    playback::Control,
    queue::{Enqueue, Repeat},
    shortcuts::{Action, Binding, Bindings},
};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey},
};
use gpui::{prelude::*, *};
use gpui_component::{ActiveTheme, Disableable, Selectable, StyledExt};
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "shortcut_settings_tests.rs"]
mod settings_tests;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, gpui::Action)]
#[action(namespace = feishin, no_json)]
pub struct RunShortcut {
    pub action: Action,
}

actions!(list, [SelectListAll, RemoveQueueSelection]);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ListScope {
    Library,
    Related,
    ArtistTop,
    ArtistFavorites,
    Queue,
    SideQueue,
}

impl ListScope {
    fn artist_section(self) -> Option<&'static str> {
        match self {
            Self::ArtistTop => Some("topSongs"),
            Self::ArtistFavorites => Some("favoriteSongs"),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct Globals {
    manager: Option<GlobalHotKeyManager>,
    registered: HashMap<u32, (HotKey, Action)>,
}
impl Globals {
    fn requests(bindings: &Bindings, media_keys: bool) -> (Vec<(HotKey, Action)>, Vec<String>) {
        let mut requested = vec![];
        let mut errors = vec![];
        let mut used = HashSet::new();
        for (action, binding) in bindings.iter().filter(|(action, binding)| {
            binding.global && !binding.key.is_empty() && action.allow_global()
        }) {
            match global_key(&binding.key) {
                Ok(key) if used.insert(key.id()) => requested.push((key, *action)),
                Ok(_) => {}
                Err(error) => errors.push(format!("{}: {error}", action.label())),
            }
        }
        if media_keys {
            for (code, action) in [
                (Code::MediaStop, Action::Stop),
                // X11 exposes the combined play/pause key as XF86_AudioPlay.
                (
                    if cfg!(target_os = "linux") {
                        Code::MediaPlay
                    } else {
                        Code::MediaPlayPause
                    },
                    Action::PlayPause,
                ),
                (Code::MediaTrackNext, Action::Next),
                (Code::MediaTrackPrevious, Action::Previous),
            ] {
                // macOS has no system-defined Stop key; its media-key API accepts the other three.
                if cfg!(target_os = "macos") && code == Code::MediaStop {
                    continue;
                }
                let key = HotKey::new(None, code);
                if used.insert(key.id()) {
                    requested.push((key, action));
                }
            }
        }
        (requested, errors)
    }
    fn handles_binding(&self, action: Action, binding: &Binding) -> bool {
        binding.global
            && global_key(&binding.key).ok().is_some_and(|key| {
                self.registered
                    .get(&key.id())
                    .is_some_and(|(_, registered)| *registered == action)
            })
    }
    fn replace(&mut self, bindings: &Bindings, media_keys: bool) -> Vec<String> {
        let (requested, mut errors) = Self::requests(bindings, media_keys);
        if let Some(manager) = &self.manager {
            for (_, (key, _)) in self.registered.drain() {
                if let Err(error) = manager.unregister(key) {
                    log::warn!("Cannot unregister global shortcut: {error}");
                }
            }
        }
        if requested.is_empty() {
            return errors;
        }
        #[cfg(not(test))]
        if self.manager.is_none() {
            match GlobalHotKeyManager::new() {
                Ok(manager) => self.manager = Some(manager),
                Err(error) => errors.push(format!("Global shortcuts unavailable: {error}")),
            }
        }
        let Some(manager) = &self.manager else {
            return errors;
        };
        for (key, action) in requested {
            match manager.register(key) {
                Ok(()) => {
                    self.registered.insert(key.id(), (key, action));
                }
                Err(error) => {
                    // Some backends retain a partial registration after an error.
                    let _ = manager.unregister(key);
                    log::warn!(
                        "Cannot register global shortcut {} ({}): {error}",
                        action.label(),
                        key.key
                    );
                    errors.push(format!("{} ({}): {error}", action.label(), key.key));
                }
            }
        }
        errors
    }
    pub fn poll(&self) -> Vec<Action> {
        GlobalHotKeyEvent::receiver()
            .try_iter()
            .filter(|event| event.state == HotKeyState::Pressed)
            .filter_map(|event| self.registered.get(&event.id).map(|(_, action)| *action))
            .collect()
    }
}
impl Drop for Globals {
    fn drop(&mut self) {
        if let Some(manager) = &self.manager {
            for (key, _) in self.registered.values() {
                let _ = manager.unregister(*key);
            }
        }
    }
}

fn global_key(value: &str) -> anyhow::Result<HotKey> {
    let key = Keystroke::parse(value).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let mut parts = vec![];
    if key.modifiers.control {
        parts.push("ctrl");
    }
    if key.modifiers.alt {
        parts.push("alt");
    }
    if key.modifiers.shift {
        parts.push("shift");
    }
    if key.modifiers.platform {
        parts.push("super");
    }
    anyhow::ensure!(
        !key.modifiers.function,
        "Fn is unavailable for global shortcuts"
    );
    parts.push(&key.key);
    Ok(parts.join("+").parse()?)
}
fn validate(bindings: &Bindings) -> anyhow::Result<()> {
    for (action, binding) in bindings {
        if binding.key.is_empty() {
            continue;
        }
        anyhow::ensure!(
            binding.key.len() <= 128 && !binding.key.contains(char::is_whitespace),
            "Use a single key combination"
        );
        let key =
            Keystroke::parse(&binding.key).map_err(|error| anyhow::anyhow!(error.to_string()))?;
        anyhow::ensure!(
            key.key != "escape",
            "Escape closes the expanded player and cancels recording"
        );
        if binding.global {
            anyhow::ensure!(
                action.allow_global(),
                "This action is available only in Feishin"
            );
            global_key(&binding.key)?;
        }
    }
    Ok(())
}
impl Feishin {
    pub fn observe_duplicate_shortcuts(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        let owner = window.window_handle();
        let view = cx.weak_entity();
        cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != owner || window.has_pending_keystrokes() {
                return;
            }
            let (bindings, pending) = cx
                .key_bindings()
                .borrow()
                .bindings_for_input(std::slice::from_ref(&event.keystroke), &event.context_stack);
            // Keep component precedence and the normal single-action/menu path.
            if pending
                || !bindings
                    .first()
                    .is_some_and(|binding| binding.action().as_any().is::<RunShortcut>())
            {
                return;
            }
            let mut actions = bindings
                .iter()
                .filter_map(|binding| {
                    binding
                        .action()
                        .as_any()
                        .downcast_ref::<RunShortcut>()
                        .map(|shortcut| shortcut.action)
                })
                .collect::<Vec<_>>();
            actions.sort();
            actions.dedup();
            if actions.len() < 2 {
                return;
            }
            view.update(cx, |view, cx| {
                view.record_activity();
                for action in actions {
                    view.run_shortcut(action, window, cx);
                }
                cx.stop_propagation();
            })
            .ok();
        })
    }

    pub fn focused_list(&self, window: &Window, cx: &App) -> Option<ListScope> {
        [
            (ListScope::ArtistTop, &self.artist_song_focus[0]),
            (ListScope::ArtistFavorites, &self.artist_song_focus[1]),
            (ListScope::Library, &self.library_focus),
            (ListScope::Related, &self.related_focus),
            (ListScope::Queue, &self.queue_focus),
            (ListScope::SideQueue, &self.side_queue_focus),
        ]
        .into_iter()
        .find_map(|(scope, focus)| focus.contains_focused(window, cx).then_some(scope))
    }

    pub fn toggle_list_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.focused_list(window, cx) {
            Some(ListScope::Library)
                if self.list_config().display == "detail"
                    && self.items.iter().all(|item| item.kind == ItemKind::Album) =>
            {
                let rows = self.album_track_rows();
                self.selected.clear();
                self.grid_selection_order.clear();
                crate::lists::toggle_all_rows(
                    &mut self.selected_album_tracks,
                    &mut self.album_selection_order,
                    rows,
                );
            }
            Some(ListScope::Library) => {
                crate::lists::toggle_all_rows(
                    &mut self.selected,
                    &mut self.grid_selection_order,
                    (0..self.items.len()).collect(),
                );
            }
            Some(ListScope::Related) => {
                let state = &mut self.player_related;
                crate::lists::toggle_all_rows(
                    &mut state.selected,
                    &mut state.selection_order,
                    (0..state.items.len()).collect(),
                );
            }
            Some(scope) if scope.artist_section().is_some() => {
                self.select_artist_section(scope.artist_section().unwrap(), cx);
            }
            Some(scope) => {
                let rows = self.visible_queue_keys(scope == ListScope::SideQueue, cx);
                let (selected, order) = self.queue_selection_mut(scope == ListScope::SideQueue);
                crate::lists::toggle_all_rows(selected, order, rows);
            }
            None => return,
        }
        cx.notify();
    }

    fn first_list_selection(
        &self,
        scope: ListScope,
        cx: &App,
    ) -> Option<feishin_core::model::Item> {
        if let Some(section) = scope.artist_section() {
            return self
                .artist_sections
                .get(section)?
                .items
                .get(self.first_artist_selection(section, cx)?)
                .cloned();
        }
        if scope == ListScope::Related {
            return self
                .player_related
                .items
                .get(*self.player_related.selected.iter().min()?)
                .cloned();
        }
        if scope != ListScope::Library {
            let key = self.first_queue_selection(scope == ListScope::SideQueue, cx)?;
            return self
                .state
                .queue
                .entries
                .iter()
                .find(|entry| entry.key == key)
                .map(|entry| entry.item.clone());
        }
        if let Some((album, index)) = self
            .album_track_rows()
            .into_iter()
            .find(|key| self.selected_album_tracks.contains(key))
        {
            return self.album_tracks.get(&album)?.items.get(index).cloned();
        }
        self.items.get(*self.selected.iter().min()?).cloned()
    }

    pub fn install_shortcuts(&mut self, cx: &mut Context<Self>) {
        let errors = self.global_shortcuts.replace(
            &self.state.settings.shortcuts,
            self.state.settings.global_media_hotkeys
                && (cfg!(target_os = "linux") || !self.state.settings.media_session),
        );
        // Preserve all component bindings while replacing only our own action.
        let mut bindings = cx
            .key_bindings()
            .borrow()
            .bindings()
            .filter(|binding| {
                let action = binding.action().as_any();
                !action.is::<RunShortcut>()
                    && !action.is::<SelectListAll>()
                    && !action.is::<RemoveQueueSelection>()
            })
            .cloned()
            .collect::<Vec<_>>();
        // Original hotkeys ignore form inputs, including checkbox and radio fields.
        let predicate = KeyBindingContextPredicate::parse(
            "Feishin && !Input && !NumberInput && !FormControl && !Select && !RecordingShortcut",
        )
        .unwrap();
        for (action, binding) in &self.state.settings.shortcuts {
            if binding.key.is_empty() || self.global_shortcuts.handles_binding(*action, binding) {
                continue;
            }
            match KeyBinding::load(
                &binding.key,
                Box::new(RunShortcut { action: *action }),
                Some(
                    if matches!(
                        action,
                        Action::ListPlayDefault
                            | Action::ListPlayNow
                            | Action::ListPlayNext
                            | Action::ListPlayLast
                            | Action::ListNavigateToPage
                    ) {
                        KeyBindingContextPredicate::parse("(Feishin > (LibraryList || RelatedList || ArtistSongList || MainQueueList || SideQueueList)) && !Input && !NumberInput && !FormControl && !Select && !RecordingShortcut").unwrap().into()
                    } else if *action == Action::GlobalSearch {
                        KeyBindingContextPredicate::parse("Feishin && !RecordingShortcut")
                            .unwrap()
                            .into()
                    } else {
                        predicate.clone().into()
                    },
                ),
                false,
                None,
                &DummyKeyboardMapper,
            ) {
                Ok(keybinding) => bindings.push(if binding.physical {
                    keybinding.with_physical_keys()
                } else {
                    keybinding
                }),
                Err(error) => log::warn!("Invalid shortcut for {}: {error}", action.label()),
            }
        }
        bindings.push(KeyBinding::new(if cfg!(target_os = "macos") { "cmd-a" } else { "ctrl-a" }, SelectListAll, Some("(Feishin > (LibraryList || RelatedList || ArtistSongList || MainQueueList || SideQueueList)) && !Input && !NumberInput && !FormControl && !Select && !RecordingShortcut")));
        bindings.push(KeyBinding::new("delete", RemoveQueueSelection, Some("(Feishin > (MainQueueList || SideQueueList)) && !Input && !NumberInput && !FormControl && !Select && !RecordingShortcut")));
        cx.clear_key_bindings();
        cx.bind_keys(bindings);
        if !errors.is_empty() {
            self.message = errors.join("\n");
        }
    }
    pub fn change_shortcut(&mut self, action: Action, binding: Binding, cx: &mut Context<Self>) {
        let mut candidate = self.state.settings.shortcuts.clone();
        candidate.insert(action, binding);
        if let Err(error) = validate(&candidate) {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        self.state.settings.shortcuts = candidate;
        self.install_shortcuts(cx);
        self.save();
        cx.notify();
    }
    pub fn capture_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(action) = self.record_shortcut else {
            return;
        };
        cx.stop_propagation();

        if event.keystroke.key == "escape" {
            self.record_shortcut = None;
            cx.notify();
            return;
        }
        if event.keystroke.key.is_empty() || event.is_held {
            return;
        }
        let mut key = event.keystroke.clone();
        let Some((name, modifiers)) = key.physical_keystroke().and_then(|physical| {
            physical
                .unshifted_key()
                .map(|name| (name, physical.modifiers))
        }) else {
            return;
        };
        key.key = name;
        key.modifiers = modifiers;
        let mut parts = vec![];
        if key.modifiers.control {
            parts.push("ctrl");
        }
        if key.modifiers.alt {
            parts.push("alt");
        }
        if key.modifiers.shift {
            parts.push("shift");
        }
        if key.modifiers.platform {
            parts.push("cmd");
        }
        if key.modifiers.function {
            parts.push("fn");
        }
        parts.push(&key.key);
        let global = self
            .state
            .settings
            .shortcuts
            .get(&action)
            .is_some_and(|binding| binding.global);
        self.change_shortcut(
            action,
            Binding {
                key: parts.join("-"),
                global,
                physical: true,
            },
            cx,
        );
        self.record_shortcut = None;
        cx.notify();
    }
    pub fn shortcut_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.settings_search.read(cx).value();
        let description = self.tr("setting.applicationHotkeys_description");
        let edit = self.tr("common.edit");
        let clear = self.tr("common.clear");
        let actions = Action::ALL
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, action)| {
                feishin_core::settings_search::matches(
                    &self.state.settings.language,
                    &query,
                    &[action.caption()],
                )
            })
            .collect::<Vec<_>>();
        div()
            .id("shortcut-settings")
            .v_flex()
            .gap_2()
            .size_full()
            .child(div().text_sm().child(description.clone()))
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap_3()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .child(
                        div()
                            .debug_selector(|| "shortcut-search".into())
                            .flex_1()
                            .min_w_40()
                            .child(crate::styles::input(&self.settings_search, cx)),
                    )
                    .child(
                        crate::styles::button("shortcut-defaults", cx)
                            .label(self.tr("common.resetToDefault"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.settings.shortcuts = feishin_core::shortcuts::defaults();
                                this.install_shortcuts(cx);
                                this.save();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("shortcut-list")
                    .v_flex()
                    .gap_2()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .native_vertical_scroll()
                    .when(actions.is_empty(), |div| {
                        div.child(self.tr("common.noResultsFromQuery"))
                    })
                    .children(actions.into_iter().map(|(index, action)| {
                        let binding = self
                            .state
                            .settings
                            .shortcuts
                            .get(&action)
                            .cloned()
                            .unwrap_or_default();
                        let global = binding.clone();
                        let recording = self.record_shortcut == Some(action);
                        let label = self.tr(action.caption());
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_3()
                            .items_center()
                            .p_2()
                            .rounded_md()
                            .bg(cx.theme().secondary)
                            .child(div().flex_1().child(label.clone()))
                            .child(
                                div()
                                    .w(px(200.0))
                                    .text_sm()
                                    .when(
                                        !binding.key.is_empty()
                                            && self
                                                .state
                                                .settings
                                                .shortcuts
                                                .values()
                                                .filter(|other| other.key == binding.key)
                                                .count()
                                                > 1,
                                        |div| {
                                            div.border_1()
                                                .border_dashed()
                                                .border_color(rgb(0xff0000))
                                        },
                                    )
                                    .child(if recording {
                                        "…".into()
                                    } else {
                                        binding.key.clone()
                                    }),
                            )
                            .child(
                                crate::styles::button(("record-shortcut", index), cx)
                                    .label(edit.clone())
                                    .aria_label(format!("{label}: {edit}"))
                                    .selected(recording)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.record_shortcut = Some(action);
                                        window.focus(&this.focus);
                                        cx.notify();
                                    })),
                            )
                            .child(div().w(px(24.)).flex_shrink_0().when(
                                action.allow_global(),
                                |div| {
                                    div.child(
                                        gpui_component::checkbox::Checkbox::new((
                                            "global-shortcut",
                                            index,
                                        ))
                                        .aria_label(format!("{label}: {description}"))
                                        .checked(binding.global)
                                        .disabled(binding.key.is_empty())
                                        .on_click(
                                            cx.listener(move |this, checked: &bool, _, cx| {
                                                let mut binding = global.clone();
                                                binding.global = *checked;
                                                this.change_shortcut(action, binding, cx);
                                            }),
                                        ),
                                    )
                                },
                            ))
                            .child(div().w(px(90.)).when(!binding.key.is_empty(), |div| {
                                div.child(
                                    crate::styles::button(("clear-shortcut", index), cx)
                                        .label(clear.clone())
                                        .aria_label(format!("{label}: {clear}"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.change_shortcut(action, Binding::default(), cx)
                                        })),
                                )
                            }))
                    })),
            )
            .into_any_element()
    }
    pub fn run_shortcut(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        use Action::*;
        match action {
            PlayPause => self.toggle_play(cx),
            Play if !self.playing => self.toggle_play(cx),
            Pause if self.playing => self.toggle_play(cx),
            Play | Pause => {}
            Next => self.next(false, cx),
            Previous => self.previous(cx),
            NextAlbum | PreviousAlbum if self.radio_item.is_none() => {
                self.play_request_generation += 1;
                self.state.queue.jump_album(action == NextAlbum);
                self.play_current(cx);
            }
            Stop => self.stop_playback(cx),
            NextAlbum | PreviousAlbum => {}
            SkipForward | SkipBackward => {
                let seconds = if action == SkipForward {
                    self.state.settings.playerbar.skip_forward_seconds
                } else {
                    -self.state.settings.playerbar.skip_backward_seconds
                };
                self.seek_position(self.current_position() + seconds, window, cx);
            }
            ToggleRepeat if self.radio_item.is_none() => {
                self.state.queue.repeat = match self.state.queue.repeat {
                    Repeat::Off => Repeat::All,
                    Repeat::All => Repeat::One,
                    Repeat::One => Repeat::Off,
                };
                self.arm_next();
            }
            ToggleShuffle if self.radio_item.is_none() => {
                self.state.queue.set_shuffle(!self.state.queue.shuffle);
                self.arm_next();
            }
            ToggleRepeat | ToggleShuffle => {}
            VolumeUp | VolumeDown => {
                self.state.settings.volume = (self.state.settings.volume
                    + if action == VolumeUp {
                        self.state.settings.volume_wheel_step.clamp(1., 20.)
                    } else {
                        -self.state.settings.volume_wheel_step.clamp(1., 20.)
                    })
                .clamp(0.0, 100.0);
                self.state.settings.muted = false;
                self.volume.update(cx, |slider, cx| {
                    slider.set_value(self.state.settings.volume, window, cx)
                });
                self.player
                    .send(Control::Volume(self.state.settings.volume));
            }
            VolumeMute => {
                self.state.settings.muted = !self.state.settings.muted;
                self.apply_audio_settings(cx);
            }
            FavoriteCurrentAdd
            | FavoriteCurrentRemove
            | FavoriteCurrentToggle
            | FavoritePreviousAdd
            | FavoritePreviousRemove
            | FavoritePreviousToggle => {
                let previous = matches!(
                    action,
                    FavoritePreviousAdd | FavoritePreviousRemove | FavoritePreviousToggle
                );
                let item = if previous {
                    self.previous_played.clone()
                } else {
                    self.current_entry().map(|entry| entry.item.clone())
                };
                if let Some(item) = item.filter(|item| item.kind == ItemKind::Song) {
                    let enabled = match action {
                        FavoriteCurrentAdd | FavoritePreviousAdd => true,
                        FavoriteCurrentRemove | FavoritePreviousRemove => false,
                        _ => !item.favorite,
                    };
                    self.set_library_values(vec![item], Some(enabled), None, cx);
                }
            }
            Rate0 | Rate1 | Rate2 | Rate3 | Rate4 | Rate5 => {
                let rating = match action {
                    Rate1 => 1,
                    Rate2 => 2,
                    Rate3 => 3,
                    Rate4 => 4,
                    Rate5 => 5,
                    _ => 0,
                };
                if let Some(item) = self
                    .current_entry()
                    .filter(|entry| entry.item.kind == ItemKind::Song)
                    .map(|entry| entry.item.clone())
                {
                    self.set_library_values(vec![item], None, Some(rating), cx);
                }
            }
            GlobalSearch => self.open_command_palette(window, cx),
            LocalSearch => {
                let target = if self.full_player {
                    None
                } else if self.artist_overview_active() {
                    self.state
                        .settings
                        .artist_items
                        .iter()
                        .any(|field| {
                            !field.disabled
                                && matches!(field.id.as_str(), "recentAlbums" | "compilations")
                        })
                        .then(|| (self.artist_searches[2].clone(), false))
                } else if self.has_local_filter() {
                    self.queue_search_open = true;
                    Some((self.local_search.clone(), !self.direct_detail_search()))
                } else {
                    match self.route {
                        Route::Search(..) => Some((self.search.clone(), true)),
                        Route::Settings => {
                            self.settings_search_open = true;
                            Some((self.settings_search.clone(), true))
                        }
                        Route::Shortcuts => Some((self.settings_search.clone(), true)),
                        _ => None,
                    }
                };
                if let Some((input, select)) = target {
                    window.defer(cx, move |window, cx| {
                        input.update(cx, |input, cx| input.focus(window, cx));
                        if select {
                            window.dispatch_action(Box::new(gpui_component::input::SelectAll), cx);
                        }
                    });
                    cx.notify();
                }
            }
            NavigateHome => self.navigate(Route::Home, cx),
            BrowserBack => self.back(cx),
            BrowserForward => self.forward(cx),
            ListNavigateToPage => {
                if let Some(scope) = self.focused_list(window, cx)
                    && let Some(item) = self.first_list_selection(scope, cx)
                    && matches!(
                        item.kind,
                        ItemKind::Album
                            | ItemKind::AlbumArtist
                            | ItemKind::Artist
                            | ItemKind::Genre
                            | ItemKind::Playlist
                    )
                {
                    self.navigate(Route::Detail(Box::new(item)), cx);
                }
            }
            ListPlayDefault | ListPlayNow | ListPlayNext | ListPlayLast => {
                if let Some(scope) = self.focused_list(window, cx)
                    && let Some(item) = self.first_list_selection(scope, cx)
                {
                    let mode = match action {
                        ListPlayDefault => self.state.settings.play_button_behavior.mode(),
                        ListPlayNext => Enqueue::Next,
                        ListPlayLast => Enqueue::End,
                        _ => Enqueue::Now,
                    };
                    let shuffle = action == ListPlayDefault
                        && self.state.settings.play_button_behavior
                            == feishin_core::queue::PlayBehavior::Shuffle;
                    if scope.artist_section().is_some() {
                        self.add_tracks_from(vec![item.clone()], mode, Some(&item), cx);
                    } else if scope == ListScope::Related {
                        if let Some(index) = self.player_related.selected.iter().min().copied() {
                            self.play_related(index, mode, shuffle, cx);
                        }
                    } else if scope != ListScope::Library && matches!(mode, Enqueue::Now) {
                        if let Some(key) =
                            self.first_queue_selection(scope == ListScope::SideQueue, cx)
                        {
                            self.play_queue_entry(key, cx);
                        }
                    } else if item.kind == ItemKind::Song {
                        self.add_tracks_from(vec![item.clone()], mode, Some(&item), cx);
                    } else {
                        self.enqueue_items(vec![item], mode, shuffle, cx);
                    }
                }
            }
            ListShowPlayingSong => self.show_playing_song(cx),
            ToggleQueue => self.toggle_playback_panels(cx),
            ToggleFullscreenPlayer => self.toggle_full_player(cx),
            ToggleFullscreenWindow => window.toggle_fullscreen(),
            ZoomIn | ZoomOut => {
                self.state.settings.zoom = (self.state.settings.zoom
                    + if action == ZoomIn { 0.1 } else { -0.1 })
                .clamp(0.5, 2.0);
                self.theme_signature.clear();
                self.apply_theme(window, cx);
            }
        }
        self.save();
        cx.notify();
    }
    pub fn record_played(&mut self) {
        if let Some(entry) = self
            .current_entry()
            .filter(|entry| entry.item.kind == ItemKind::Song)
            .cloned()
            && self
                .last_played
                .as_ref()
                .is_none_or(|(key, _)| *key != entry.key)
        {
            self.previous_played = self.last_played.take().map(|(_, item)| item);
            self.last_played = Some((entry.key, entry.item.clone()));
        }
    }
}

#[cfg(test)]
mod media_tests {
    use super::*;
    use ::core::prelude::v1::test;

    #[test]
    fn media_key_requests_match_original_commands_and_keep_custom_local_bindings() {
        let reference: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("../assets/media-keys-reference.json")).unwrap();
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "windows"
        };
        let original = reference
            .iter()
            .find(|case| {
                case["platform"] == platform
                    && case["mediaSession"] == false
                    && case["playbackType"] == "web"
            })
            .unwrap();
        let bindings = feishin_core::shortcuts::defaults();
        let (requested, errors) = Globals::requests(&bindings, true);
        assert!(errors.is_empty());
        let expected = original["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|binding| !(cfg!(target_os = "macos") && binding["key"] == "MediaStop"))
            .map(|binding| {
                let code = match binding["key"].as_str().unwrap() {
                    "MediaStop" => Code::MediaStop,
                    "MediaPlayPause" if cfg!(target_os = "linux") => Code::MediaPlay,
                    "MediaPlayPause" => Code::MediaPlayPause,
                    "MediaNextTrack" => Code::MediaTrackNext,
                    "MediaPreviousTrack" => Code::MediaTrackPrevious,
                    key => panic!("Unmapped original media key {key}"),
                };
                let action = match binding["command"].as_str().unwrap() {
                    "renderer-player-stop" => Action::Stop,
                    "renderer-player-play-pause" => Action::PlayPause,
                    "renderer-player-next" => Action::Next,
                    "renderer-player-previous" => Action::Previous,
                    command => panic!("Unmapped original command {command}"),
                };
                (HotKey::new(None, code), action)
            })
            .collect::<Vec<_>>();
        assert_eq!(requested, expected);
        assert!(Globals::requests(&bindings, false).0.is_empty());
        let mut registered = Globals {
            manager: None,
            registered: requested
                .iter()
                .map(|(key, action)| (key.id(), (*key, *action)))
                .collect(),
        };
        assert!(!registered.handles_binding(Action::PlayPause, &bindings[&Action::PlayPause]));
        let custom = Binding {
            key: "ctrl-p".into(),
            global: true,
            ..Binding::default()
        };
        assert!(
            !registered.handles_binding(Action::PlayPause, &custom),
            "An unrelated media key cannot suppress a failed custom shortcut's local fallback"
        );
        let custom_key = global_key(&custom.key).unwrap();
        registered
            .registered
            .insert(custom_key.id(), (custom_key, Action::PlayPause));
        assert!(registered.handles_binding(Action::PlayPause, &custom));
        let mut bindings = bindings;
        bindings.insert(Action::PlayPause, custom);
        let media_next = "MediaTrackNext";
        bindings.insert(
            Action::Stop,
            Binding {
                key: media_next.into(),
                global: true,
                ..Binding::default()
            },
        );
        let (requested, errors) = Globals::requests(&bindings, true);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(requested.contains(&(custom_key, Action::PlayPause)));
        assert_eq!(
            requested
                .iter()
                .filter(|(key, _)| key.key == Code::MediaTrackNext)
                .copied()
                .collect::<Vec<_>>(),
            [(HotKey::new(None, Code::MediaTrackNext), Action::Stop)],
            "Explicit user bindings take precedence over built-in media keys"
        );
        bindings.insert(Action::VolumeUp, bindings[&Action::PlayPause].clone());
        assert!(validate(&bindings).is_ok());
        let (requested, errors) = Globals::requests(&bindings, false);
        assert!(errors.is_empty());
        assert_eq!(
            requested
                .iter()
                .filter(|(key, _)| *key == custom_key)
                .count(),
            1,
            "Duplicate globals must register once, as Electron does"
        );
    }
}
