use crate::app::{Feishin, Route};
use crate::scroll::VerticalScroll;
use feishin_core::{
    model::{Item, ItemKind},
    tags::{self, Artwork, FieldType, Metadata, Patch},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Selectable, StyledExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    input::{InputEvent, InputState, NumberInputEvent, StepAction},
    menu::{DropdownMenu, PopupMenuItem},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

pub struct Field {
    pub name: String,
    pub input: Entity<InputState>,
    pub original: String,
    pub multi: bool,
    pub removed: bool,
    pub mixed: bool,
    pub original_mixed: bool,
    pub edited: bool,
}

fn tag_input(
    name: &str,
    multi: bool,
    mixed: bool,
    window: &mut Window,
    cx: &mut Context<InputState>,
) -> InputState {
    InputState::new(window, cx)
        .aria_label(name.to_owned())
        .label_transform(|label, cx| {
            if label.as_str() == "page.itemDetail.multipleValues" {
                crate::appearance::text(label, cx).into()
            } else {
                label.clone()
            }
        })
        .multi_line(multi || tags::field_type(name) == FieldType::Textarea)
        .placeholder(if mixed {
            "page.itemDetail.multipleValues"
        } else {
            ""
        })
}

impl Feishin {
    pub fn tag_value_input(
        &self,
        index: usize,
        field: &Field,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.tag_busy || field.removed;
        let kind = if field.multi {
            FieldType::Textarea
        } else {
            tags::field_type(&field.name)
        };
        match kind {
            FieldType::Boolean => div()
                .debug_selector(move || format!("tag-boolean-{index}"))
                .h_flex()
                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                .gap_2()
                .child(
                    Checkbox::new(("tag-boolean", index))
                        .aria_label(field.name.clone())
                        .indeterminate(field.mixed)
                        .checked(!field.mixed && field.input.read(cx).value().as_str() == "1")
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, value: &bool, window, cx| {
                            let field = &mut this.tag_fields[index];
                            field.input.update(cx, |input, cx| {
                                input.set_value(if *value { "1" } else { "0" }, window, cx)
                            });
                            field.mixed = false;
                            field.edited = true;
                            cx.notify();
                        })),
                )
                .when(field.mixed, |div| {
                    div.child(self.tr("page.itemDetail.multipleValues"))
                })
                .into_any_element(),
            FieldType::Number => div()
                .debug_selector(move || format!("tag-number-{index}"))
                .child(crate::styles::number_input(&field.input, cx).disabled(disabled))
                .into_any_element(),
            FieldType::Textarea => crate::styles::input(&field.input, cx)
                .h(px(64.0))
                .disabled(disabled)
                .into_any_element(),
            FieldType::Text => crate::styles::input(&field.input, cx)
                .disabled(disabled)
                .into_any_element(),
        }
    }
    pub fn choose_tag_files(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tag_busy {
            return;
        }
        self.tag_targets.clear();
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select audio files to edit".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picker.await {
                this.update_in(cx, |this, window, cx| this.load_tags(paths, window, cx))
                    .ok();
            }
        })
        .detach();
    }

    pub fn edit_selected_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = if self.has_library_selection() {
            self.selected_library_items()
        } else {
            self.items.clone()
        };
        self.edit_tag_items(items, window, cx);
    }
    pub fn edit_tag_items(
        &mut self,
        items: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tag_busy {
            return;
        }
        if items.iter().any(|item| item.kind == ItemKind::Album) {
            self.tag_busy = true;
            self.tag_targets.clear();
            self.tag_files.clear();
            self.tag_fields.clear();
            self.navigate(Route::TagEditor, cx);
            self.message = "Loading album tracks...".into();
            let sessions = self.sessions.clone();
            let task = cx.background_executor().spawn(async move {
                let mut songs = Vec::new();
                for item in items {
                    if item.kind == ItemKind::Song {
                        songs.push(item);
                    } else if item.kind == ItemKind::Album {
                        songs.extend(
                            sessions
                                .get(&item.server_id)
                                .ok_or_else(|| anyhow::anyhow!("Sign in to this album's server"))?
                                .all_tracks(&item)?,
                        );
                    }
                }
                anyhow::ensure!(!songs.is_empty(), "No album tracks found");
                Ok::<_, anyhow::Error>(songs)
            });
            cx.spawn_in(window, async move |this, cx| {
                let result = task.await;
                this.update_in(cx, |this, window, cx| {
                    this.tag_busy = false;
                    if this.route != Route::TagEditor {
                        cx.notify();
                        return;
                    }
                    match result {
                        Ok(songs) => this.edit_tag_items(songs, window, cx),
                        Err(error) => {
                            log::warn!("Could not load album tags: {error}");
                            this.message = error.to_string();
                        }
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
            cx.notify();
            return;
        }
        self.tag_targets = items
            .iter()
            .filter(|item| item.kind == ItemKind::Song && !item.path.is_empty())
            .map(|item| {
                let settings = &self.state.settings;
                let path = settings.resolve_song_path(&item.path);
                (PathBuf::from(path), item.clone())
            })
            .collect();
        let paths = self
            .tag_targets
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            self.choose_tag_files(window, cx);
        } else {
            self.load_tags(paths, window, cx);
        }
    }

    pub fn load_tags(
        &mut self,
        mut paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tag_busy {
            return;
        }
        paths.sort();
        paths.dedup();
        self.tag_busy = true;
        self.tag_progress = Some((0, paths.len()));
        let (progress, events) = std::sync::mpsc::channel();
        self.tag_progress_events = Some(events);
        self.form_name
            .update(cx, |input, cx| input.set_value("", window, cx));
        if self.route != Route::TagEditor {
            self.navigate(Route::TagEditor, cx);
        }
        self.message = "Reading audio tags...".into();
        let task = cx.background_executor().spawn(async move {
            let mut files = vec![];
            let mut errors = vec![];
            let total = paths.len();
            for (index, path) in paths.into_iter().enumerate() {
                match tags::read(&path) {
                    Ok(file) => files.push(file),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
                let _ = progress.send((index + 1, total));
            }
            (files, errors)
        });
        cx.spawn_in(window, async move |this, cx| {
            let (files, errors) = task.await;
            this.update_in(cx, |this, window, cx| {
                this.tag_files = files;
                this.tag_scope = None;
                this.tag_busy = false;
                this.tag_progress_events = None;
                this.tag_progress = None;
                this.message = errors.join("\n");
                this.populate_tag_fields(window, cx);
            })
            .ok();
        })
        .detach();
    }

    pub fn populate_tag_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.load_tag_sources(cx);
        self.tag_suggestion_generation += 1;
        self.tag_suggestions = None;
        self.tag_config_edit = None;
        self.tag_subscriptions.clear();
        let files = self
            .tag_scope
            .and_then(|index| self.tag_files.get(index))
            .map(std::slice::from_ref)
            .unwrap_or(&self.tag_files);
        let fields = tags::merged(files);
        self.tag_fields = fields
            .into_iter()
            .map(|(name, values)| {
                let multi = name != "LYRICS"
                    && (self
                        .state
                        .settings
                        .tag_configs
                        .get(&tags::canonical_name(&name))
                        .is_some_and(|config| config.multi_value)
                        || files.iter().any(|file| {
                            file.fields
                                .get(&name)
                                .is_some_and(|values| values.len() > 1)
                        }));
                let original = values
                    .as_ref()
                    .map(|values| values.join(if name == "LYRICS" { "\n\n" } else { "\n" }))
                    .unwrap_or_default();
                let input = cx.new(|cx| tag_input(&name, multi, values.is_none(), window, cx));
                input.update(cx, |input, cx| {
                    input.set_value(original.clone(), window, cx)
                });
                Field {
                    name,
                    input,
                    original,
                    multi,
                    removed: false,
                    mixed: values.is_none(),
                    original_mixed: values.is_none(),
                    edited: false,
                }
            })
            .collect();
        let priority = |name: &str| {
            [
                "TITLE",
                "ARTIST",
                "ALBUM",
                "ALBUMARTIST",
                "TRACKNUMBER",
                "DISCNUMBER",
                "DATE",
            ]
            .iter()
            .position(|field| *field == name)
            .unwrap_or(7)
        };
        self.tag_fields.sort_by(|a, b| {
            priority(&a.name)
                .cmp(&priority(&b.name))
                .then_with(|| feishin_core::i18n::compare(&a.name, &b.name, false))
        });
        self.tag_artwork = Artwork::Keep;
        self.tag_artwork_path = None;
        let art = files
            .first()
            .and_then(|file| file.artwork.as_ref())
            .filter(|art| files.iter().all(|file| file.artwork.as_ref() == Some(*art)));
        if let Some(art) = art {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(art, &mut hash);
            let path = self.data_dir.join("cache").join(format!(
                "tag-artwork-{:016x}.img",
                std::hash::Hasher::finish(&hash)
            ));
            if feishin_core::storage::atomic_write(&path, art).is_ok() {
                self.tag_artwork_path = Some(path);
            }
        }
        for (name, input) in self
            .tag_fields
            .iter()
            .map(|field| (field.name.clone(), field.input.clone()))
            .collect::<Vec<_>>()
        {
            self.bind_tag_input(name, input, window, cx);
        }
        cx.notify();
    }

    fn bind_tag_input(
        &mut self,
        name: String,
        input: Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let field_name = name.clone();
        self.tag_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, input, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    if let Some(field) = this
                        .tag_fields
                        .iter_mut()
                        .find(|field| field.name == field_name && field.input == *input)
                    {
                        field.mixed = false;
                        field.edited = true;
                    }
                    this.suggest_tag_values(&field_name, window, cx);
                }
            },
        ));

        self.tag_subscriptions.push(cx.subscribe_in(
            &input,
            window,
            move |this, _, event: &NumberInputEvent, window, cx| {
                if this.tag_busy {
                    return;
                }
                let NumberInputEvent::Step(action) = event;
                if let Some(field) = this
                    .tag_fields
                    .iter_mut()
                    .find(|field| field.name == name && !field.removed)
                {
                    let value = field.input.read(cx).value().parse::<f64>().unwrap_or(0.0);
                    let value = value
                        + if *action == StepAction::Increment {
                            1.0
                        } else {
                            -1.0
                        };
                    if value.is_finite() {
                        field.input.update(cx, |input, cx| {
                            input.set_value(value.to_string(), window, cx)
                        });
                        field.mixed = false;
                        field.edited = true;
                        cx.notify();
                    }
                }
            },
        ));
    }
    pub fn save_tags(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.save_tags_with_close(false, window, cx);
    }

    fn save_tags_with_close(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.tag_busy {
            return;
        }
        let mut patch = Patch {
            artwork: self.tag_artwork.clone(),
            ..Patch::default()
        };
        let mut empty_fields = Vec::new();
        for field in &self.tag_fields {
            let value = field.input.read(cx).value().to_string();
            if field.removed {
                patch.fields.insert(field.name.clone(), vec![]);
            } else if field.edited || value != field.original {
                let values = if field.multi {
                    value.split('\n').map(str::to_owned).collect()
                } else {
                    vec![value.clone()]
                };
                if tags::empty_edit(&field.name, &values) {
                    empty_fields.push(field.name.clone());
                    continue;
                }
                if let Err(error) = tags::validate_field_value(&field.name, &value, field.multi) {
                    self.message = error.to_string();
                    cx.notify();
                    return;
                }
                patch.fields.insert(field.name.clone(), values);
            }
        }
        if !empty_fields.is_empty() {
            self.message = format!(
                "{}: {}",
                self.tr("page.itemDetail.emptyFields"),
                empty_fields.join(", ")
            );
            cx.notify();
            return;
        }
        let paths = self
            .tag_scope
            .and_then(|index| self.tag_files.get(index))
            .map(std::slice::from_ref)
            .unwrap_or(&self.tag_files)
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        if patch.fields.is_empty() && matches!(patch.artwork, Artwork::Keep) {
            if close {
                self.close_tag_editor(cx);
                return;
            }
            self.message = "No tag changes to save".into();
            cx.notify();
            return;
        }
        self.tag_busy = true;
        self.tag_progress = Some((0, paths.len()));
        let (progress, events) = std::sync::mpsc::channel();
        self.tag_progress_events = Some(events);
        let targets = self.tag_targets.clone();
        let sessions = self.sessions.clone();
        let rescan = self.state.settings.tag_rescan;
        let artwork_changed = !matches!(patch.artwork, Artwork::Keep);
        let task = cx.background_executor().spawn(async move {
            let mut changed: Vec<Metadata> = vec![];
            let mut errors = vec![];
            let total = paths.len();
            for (index, path) in paths.into_iter().enumerate() {
                match tags::write(&path, &patch).and_then(|_| tags::read(&path)) {
                    Ok(file) => changed.push(file),
                    Err(error) => errors.push(format!("{}: {error}", path.display())),
                }
                let _ = progress.send((index + 1, total));
            }
            let write_failed = !errors.is_empty();
            let changed_paths = changed
                .iter()
                .map(|file| &file.path)
                .collect::<HashSet<_>>();
            let mut refresh: HashMap<uuid::Uuid, Vec<String>> = HashMap::new();
            let mut cover_keys = HashSet::new();
            let mut refreshed = Vec::new();
            for (path, item) in targets {
                if changed_paths.contains(&path) {
                    if artwork_changed {
                        cover_keys.insert(Self::cover_key(&item));
                    }
                    refresh.entry(item.server_id).or_default().push(item.id);
                }
            }
            if rescan {
                for (server, mut ids) in refresh {
                    if let Some(api) = sessions.get(&server).filter(|api| api.session().is_admin) {
                        ids.sort();
                        ids.dedup();
                        if let Err(error) = api.refresh_items(&ids) {
                            log::warn!("Metadata refresh after tag editing failed: {error}");
                            errors.push(format!(
                                "Files saved; server metadata refresh failed: {error}"
                            ));
                        } else {
                            refreshed.push(server);
                        }
                    }
                }
            }
            (changed, errors, cover_keys, refreshed, write_failed)
        });
        cx.spawn_in(window, async move |this, cx| {
            let (changed, errors, cover_keys, refreshed, write_failed) = task.await;
            this.update_in(cx, |this, window, cx| {
                this.tag_busy = false;
                this.tag_progress_events = None;
                this.tag_progress = None;
                for key in cover_keys {
                    this.invalidate_artwork(&key, false);
                }
                this.message = format!(
                    "Saved {} audio files{}",
                    changed.len(),
                    if errors.is_empty() {
                        String::new()
                    } else {
                        format!("\n{}", errors.join("\n"))
                    }
                );
                for file in changed {
                    if let Some(previous) =
                        this.tag_files.iter_mut().find(|old| old.path == file.path)
                    {
                        *previous = file;
                    }
                }
                if !write_failed {
                    this.populate_tag_fields(window, cx);
                }
                cx.notify();
                if this
                    .state
                    .active_server
                    .is_some_and(|server| refreshed.contains(&server))
                    && let Some(api) = this.api()
                {
                    this.watch_library_scan(api, None, cx);
                }
                if close && !write_failed && this.route == Route::TagEditor {
                    this.close_tag_editor(cx);
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn close_tag_editor(&mut self, cx: &mut Context<Self>) {
        if self.tag_busy {
            return;
        }
        self.tag_subscriptions.clear();
        self.tag_files.clear();
        self.tag_fields.clear();
        self.tag_targets.clear();
        self.tag_source_names.clear();
        self.tag_suggestion_generation += 1;
        self.tag_suggestions = None;
        self.tag_config_edit = None;
        self.tag_artwork = Artwork::Keep;
        self.tag_artwork_path = None;
        while self
            .history
            .last()
            .is_some_and(|visit| visit.route == Route::TagEditor)
        {
            self.history.pop();
        }
        if self.history.is_empty() {
            self.navigate(Route::Home, cx);
        } else {
            self.back(cx);
        }
        cx.notify();
    }

    pub fn add_tag_field(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.tag_busy {
            return;
        }
        if let Err(error) = tags::validate_field_name(name) {
            self.message = error.to_string();
            cx.notify();
            return;
        }
        let name = tags::canonical_name(name);
        if self.tag_fields.iter().any(|field| field.name == name) {
            self.message = "Field already exists".into();
            cx.notify();
            return;
        }
        let multi = name != "LYRICS"
            && self
                .state
                .settings
                .tag_configs
                .get(&name)
                .is_some_and(|config| config.multi_value);
        let input = cx.new(|cx| tag_input(&name, multi, false, window, cx));
        self.tag_fields.push(Field {
            name: name.clone(),
            input: input.clone(),
            original: String::new(),
            multi,
            removed: false,
            mixed: false,
            original_mixed: false,
            edited: true,
        });
        self.bind_tag_input(name, input.clone(), window, cx);
        self.form_name
            .update(cx, |input, cx| input.set_value("", window, cx));
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub fn revert_tag(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let name = &self.tag_fields[index].name;
        let files = self
            .tag_scope
            .and_then(|index| self.tag_files.get(index))
            .map(std::slice::from_ref)
            .unwrap_or(&self.tag_files);
        let exists = tags::merged(files).contains_key(name);
        if !exists {
            self.tag_fields.remove(index);
            cx.notify();
            return;
        }
        let field = &mut self.tag_fields[index];
        let input =
            cx.new(|cx| tag_input(&field.name, field.multi, field.original_mixed, window, cx));
        input.update(cx, |input, cx| {
            input.set_value(field.original.clone(), window, cx)
        });
        field.input = input.clone();
        field.mixed = field.original_mixed;
        field.removed = false;
        field.edited = false;
        let name = field.name.clone();
        self.bind_tag_input(name, input, window, cx);
        cx.notify();
    }

    pub fn set_tag_source(
        &mut self,
        index: usize,
        source: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.tag_fields.get(index) else {
            return;
        };
        let name = field.name.clone();
        self.state
            .settings
            .tag_configs
            .entry(tags::canonical_name(&name))
            .or_default()
            .autocomplete_source = source.into();
        self.save();
        self.suggest_tag_values(&name, window, cx);
    }

    pub fn toggle_tag_multi(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.tag_fields.get_mut(index) else {
            return;
        };
        if field.name == "LYRICS" {
            return;
        }
        let multi = !field.multi;
        field.multi = multi
            || self
                .tag_scope
                .and_then(|index| self.tag_files.get(index))
                .map(std::slice::from_ref)
                .unwrap_or(&self.tag_files)
                .iter()
                .any(|file| {
                    file.fields
                        .get(&field.name)
                        .is_some_and(|values| values.len() > 1)
                });
        let value = field.input.read(cx).value().to_string();
        let input = cx.new(|cx| tag_input(&field.name, field.multi, field.mixed, window, cx));
        input.update(cx, |input, cx| input.set_value(value, window, cx));
        field.input = input.clone();
        let name = field.name.clone();
        self.state
            .settings
            .tag_configs
            .entry(tags::canonical_name(&field.name))
            .or_default()
            .multi_value = multi;
        self.bind_tag_input(name, input, window, cx);
        self.save();
        cx.notify();
    }

    pub fn suggest_tag_values(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.tag_suggestion_generation += 1;
        let generation = self.tag_suggestion_generation;
        let Some(field) = self
            .tag_fields
            .iter()
            .find(|field| field.name == name && !field.removed)
        else {
            return;
        };
        let text = field.input.read(cx).value().to_string();
        let search = if field.multi {
            text.rsplit('\n').next().unwrap_or_default().trim()
        } else {
            text.trim()
        }
        .to_owned();
        let config = self
            .state
            .settings
            .tag_configs
            .get(&tags::canonical_name(name))
            .cloned()
            .unwrap_or_default();
        let values = config
            .custom_values
            .iter()
            .filter(|value| value.to_lowercase().contains(&search.to_lowercase()))
            .cloned()
            .collect::<Vec<_>>();
        let values = tags::suggestions(&values, &[]);
        self.tag_suggestions = Some((name.into(), values.clone()));
        cx.notify();
        let Some(api) = self.api().filter(|_| {
            !search.is_empty()
                && (config.autocomplete_source.starts_with("tag:")
                    || matches!(
                        config.autocomplete_source.as_str(),
                        "serverArtists" | "serverGenres"
                    ))
        }) else {
            return;
        };
        let name = name.to_owned();
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            if !this
                .read_with(cx, |this, _| this.tag_suggestion_generation == generation)
                .unwrap_or(false)
            {
                return;
            }
            let task = cx
                .background_executor()
                .spawn(async move { api.tag_suggestions(&config.autocomplete_source, &search) });
            let result = task.await;
            this.update_in(cx, |this, _, cx| {
                if this.tag_suggestion_generation != generation {
                    return;
                }
                match result {
                    Ok(server_values) => {
                        this.tag_suggestions =
                            Some((name, tags::suggestions(&values.custom, &server_values)));
                    }
                    Err(error) => {
                        log::debug!("Tag suggestions unavailable: {error}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn use_tag_suggestion(
        &mut self,
        name: &str,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(field) = self.tag_fields.iter_mut().find(|field| field.name == name) {
            let text = field.input.read(cx).value().to_string();
            let value = if field.multi {
                text.rsplit_once('\n').map_or_else(
                    || value.to_owned(),
                    |(prefix, _)| format!("{prefix}\n{value}"),
                )
            } else {
                value.to_owned()
            };
            field
                .input
                .update(cx, |input, cx| input.set_value(value, window, cx));
            field.edited = true;
            field.mixed = false;
        }
        self.tag_suggestion_generation += 1;
        self.tag_suggestions = None;
        cx.notify();
    }

    fn load_tag_sources(&mut self, cx: &mut Context<Self>) {
        self.tag_source_names.clear();
        let Some(api) = self.api() else {
            return;
        };
        let server = api.server.id;
        let task = cx
            .background_executor()
            .spawn(async move { api.tag_editor_sources() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.route != Route::TagEditor || this.state.active_server != Some(server) {
                    return;
                }
                match result {
                    Ok(names) => this.tag_source_names = names,
                    Err(error) => log::debug!("Tag sources unavailable: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn edit_tag_config(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = self.tag_fields.get(index) else {
            return;
        };
        let config = self
            .state
            .settings
            .tag_configs
            .get(&tags::canonical_name(&field.name))
            .cloned()
            .unwrap_or_default();
        self.tag_config_edit = Some(field.name.clone());
        self.form_rules.update(cx, |input, cx| {
            input.set_value(config.custom_values.join("\n"), window, cx)
        });
        self.form_comment.update(cx, |input, cx| {
            input.set_value(
                config
                    .autocomplete_source
                    .strip_prefix("tag:")
                    .unwrap_or_default()
                    .to_owned(),
                window,
                cx,
            )
        });
        cx.notify();
    }

    pub fn remove_tag_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.tag_config_edit.take() else {
            return;
        };
        self.state
            .settings
            .tag_configs
            .remove(&tags::canonical_name(&name));
        if let Some(field) = self.tag_fields.iter_mut().find(|field| field.name == name) {
            let value = field.input.read(cx).value().to_string();
            let files = self
                .tag_scope
                .and_then(|index| self.tag_files.get(index))
                .map(std::slice::from_ref)
                .unwrap_or(&self.tag_files);
            // Stored arrays and pending multiple values remain editable as arrays.
            field.multi = name != "LYRICS"
                && (files.iter().any(|file| {
                    file.fields
                        .get(&name)
                        .is_some_and(|values| values.len() > 1)
                }) || field.multi
                    && (field.edited || value != field.original)
                    && !value.is_empty());
            let input = cx.new(|cx| tag_input(&name, field.multi, field.mixed, window, cx));
            input.update(cx, |input, cx| input.set_value(value, window, cx));
            field.input = input.clone();
            self.bind_tag_input(name.clone(), input, window, cx);
        }
        self.save();
        self.suggest_tag_values(&name, window, cx);
        cx.notify();
    }

    pub fn save_tag_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self.tag_config_edit.clone() else {
            return;
        };
        let mut configs = self.state.settings.tag_configs.clone();
        let config = configs.entry(tags::canonical_name(&name)).or_default();
        config.custom_values = self
            .form_rules
            .read(cx)
            .value()
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect();
        config
            .custom_values
            .sort_by(|a, b| feishin_core::i18n::compare(a, b, false));
        config.custom_values.dedup();
        let tag = self.form_comment.read(cx).value().trim().to_owned();
        if !tag.is_empty() {
            config.autocomplete_source = format!("tag:{tag}");
        } else if config.autocomplete_source.starts_with("tag:") {
            config.autocomplete_source = "none".into();
        }
        match tags::validate_configs(&configs) {
            Ok(()) => {
                self.state.settings.tag_configs = configs;
                self.tag_config_edit = None;
                self.save();
                self.suggest_tag_values(&name, window, cx);
            }
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
            }
        }
    }

    pub fn load_tag_artwork(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.tag_busy {
            return;
        }
        self.tag_busy = true;
        let job = cx
            .background_executor()
            .spawn(async move { tags::artwork_file(&path).map(|data| (path, data)) });
        cx.spawn(async move |this, cx| {
            let result = job.await;
            this.update(cx, |this, cx| {
                this.tag_busy = false;
                if matches!(this.route, Route::TagEditor) {
                    match result {
                        Ok((path, data)) => {
                            this.tag_artwork = Artwork::Replace(data);
                            this.tag_artwork_path = Some(path);
                        }
                        Err(error) => this.message = error.to_string(),
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn drop_tag_artwork(&mut self, paths: &[PathBuf], cx: &mut Context<Self>) {
        if let Some(path) = crate::images::first_image_path(paths) {
            self.load_tag_artwork(path.clone(), cx);
        }
    }

    pub fn tag_editor_view(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.tag_busy
            && self.tag_files.is_empty()
            && !self.tag_targets.is_empty()
            && self
                .tag_targets
                .iter()
                .all(|(path, _)| path.try_exists().is_ok_and(|exists| !exists))
        {
            return div()
                .id("tag-path-recovery")
                .debug_selector(|| "tag-path-recovery".into())
                .v_flex()
                .gap_4()
                .size_full()
                .native_vertical_scroll()
                .child(self.message.clone())
                .child(crate::path_settings::view(
                    cx.weak_entity(),
                    self.tag_targets.first().map(|(_, item)| item.path.clone()),
                ))
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            crate::styles::button("tag-path-reload", cx)
                                .debug_selector(|| "tag-path-reload".into())
                                .label(self.tr("common.reload"))
                                .on_click(cx.listener(|view, _, window, cx| {
                                    let items = view
                                        .tag_targets
                                        .iter()
                                        .map(|(_, item)| item.clone())
                                        .collect();
                                    view.edit_tag_items(items, window, cx);
                                })),
                        )
                        .child(
                            crate::styles::button("tag-path-cancel", cx)
                                .label(self.tr("common.cancel"))
                                .on_click(cx.listener(|view, _, _, cx| view.close_tag_editor(cx))),
                        ),
                )
                .into_any_element();
        }
        let files = self
            .tag_scope
            .and_then(|index| self.tag_files.get(index))
            .map(std::slice::from_ref)
            .unwrap_or(&self.tag_files);
        let has_artwork = match &self.tag_artwork {
            Artwork::Replace(_) => true,
            Artwork::Remove => false,
            Artwork::Keep => files.iter().any(|file| file.artwork.is_some()),
        };
        let mixed_artwork = matches!(self.tag_artwork, Artwork::Keep)
            && has_artwork
            && files
                .first()
                .is_some_and(|first| files.iter().any(|file| file.artwork != first.artwork));
        div()
            .id("tag-editor")
            .debug_selector(|| "tag-editor".into())
            .v_flex()
            .gap_4()
            .size_full()
            .native_vertical_scroll()
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .child(
                        crate::styles::button("choose-tags", cx)
                            .label(crate::appearance::text("Choose audio files", cx))
                            .disabled(self.tag_busy)
                            .on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.choose_tag_files(window, cx)
                                }),
                            ),
                    )
                    .child(
                        crate::styles::button("save-tags", cx)
                            .primary()
                            .label(self.tr(if self.tag_busy {
                                "dlna.speakerProperties.loading"
                            } else {
                                "common.save"
                            }))
                            .disabled(self.tag_busy || self.tag_files.is_empty())
                            .on_click(
                                cx.listener(|this, _, window, cx| this.save_tags(window, cx)),
                            ),
                    )
                    .child(
                        crate::styles::button("save-close-tags", cx)
                            .debug_selector(|| "tag-save-close".into())
                            .label(self.tr("common.saveAndClose"))
                            .disabled(self.tag_busy || self.tag_files.is_empty())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.save_tags_with_close(true, window, cx)
                            })),
                    )
                    .child(
                        crate::styles::button("cancel-tags", cx)
                            .debug_selector(|| "tag-cancel".into())
                            .label(self.tr("common.cancel"))
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(|this, _, _, cx| this.close_tag_editor(cx))),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_2()
                    .flex_wrap()
                    .child(
                        crate::styles::button("all-tag-files", cx)
                            .label(
                                feishin_core::i18n::translate_with(
                                    &self.state.settings.language,
                                    "common.countSelected",
                                    &serde_json::json!({"count":self.tag_files.len()}),
                                )
                                .unwrap_or_else(|| format!("{} selected", self.tag_files.len())),
                            )
                            .selected(self.tag_scope.is_none())
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.tag_scope = None;
                                this.populate_tag_fields(window, cx);
                            })),
                    )
                    .children(self.tag_files.iter().enumerate().map(|(i, file)| {
                        crate::styles::button(("tag-file", i), cx)
                            .selected(self.tag_scope == Some(i))
                            .label(
                                file.path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .into_owned(),
                            )
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.tag_scope = Some(i);
                                this.populate_tag_fields(window, cx);
                            }))
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Only changed fields are saved. Multi-value fields use one value per line.",
                    ),
            )
            .children(self.tag_fields.iter().enumerate().map(|(i, field)| {
                div()
                    .h_flex()
                    .flex_wrap()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .items_start()
                    .max_w(px(900.0))
                    .child(div().w(px(180.0)).pt_2().child(field.name.clone()))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(120.))
                            .child(self.tag_value_input(i, field, cx)),
                    )
                    .when(
                        field.edited || field.input.read(cx).value().as_str() != field.original,
                        |div| {
                            div.child(
                                crate::styles::button(("revert-tag", i), cx)
                                    .label(crate::appearance::text("Undo changes", cx))
                                    .disabled(self.tag_busy)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.revert_tag(i, window, cx);
                                    })),
                            )
                        },
                    )
                    .child(
                        crate::styles::button(("tag-multi", i), cx)
                            .label(self.tr(if field.multi {
                                "common.filter_multiple"
                            } else {
                                "common.filter_single"
                            }))
                            .disabled(self.tag_busy || field.name == "LYRICS")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_tag_multi(i, window, cx);
                            })),
                    )
                    .child(
                        crate::styles::button(("tag-suggestions", i), cx)
                            .label(crate::appearance::text("Suggestions", cx))
                            .disabled(self.tag_busy || field.removed)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                let name = this.tag_fields[i].name.clone();
                                this.suggest_tag_values(&name, window, cx);
                            })),
                    )
                    .child(
                        crate::styles::button(("tag-source", i), cx)
                            .debug_selector(move || format!("tag-source-{i}"))
                            .label(self.tr("page.itemDetail.autocompleteSource"))
                            .disabled(self.tag_busy)
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                let names = self.tag_source_names.clone();
                                let selected = self
                                    .state
                                    .settings
                                    .tag_configs
                                    .get(&field.name)
                                    .map(|config| config.autocomplete_source.clone())
                                    .unwrap_or_else(|| "none".into());
                                move |menu, _, cx| {
                                    let mut menu = menu.scrollable(true).max_h(px(360.));
                                    let options = [
                                        ("none", "common.none"),
                                        ("serverArtists", "entity.artist_other"),
                                        ("serverGenres", "entity.genre_other"),
                                    ]
                                    .into_iter()
                                    .map(|(source, label)| {
                                        (source.to_owned(), crate::appearance::text(label, cx))
                                    })
                                    .chain(names.iter().map(|name| {
                                        (format!("tag:{name}"), tags::source_label(name))
                                    }));
                                    for (source, label) in options {
                                        let view = view.clone();
                                        let checked = source == selected;
                                        menu = menu.item(
                                            PopupMenuItem::new(label).checked(checked).on_click(
                                                move |_, window, cx| {
                                                    view.update(cx, |this, cx| {
                                                        this.set_tag_source(i, &source, window, cx)
                                                    })
                                                    .ok();
                                                },
                                            ),
                                        );
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        crate::styles::button(("tag-config", i), cx)
                            .label(self.tr("page.itemDetail.tagConfiguration"))
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.edit_tag_config(i, window, cx)
                            })),
                    )
                    .child(
                        crate::styles::button(("remove-tag", i), cx)
                            .debug_selector(move || format!("tag-remove-{i}"))
                            .label(self.tr(if field.removed {
                                "common.undo"
                            } else {
                                "common.delete"
                            }))
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tag_fields[i].removed = !this.tag_fields[i].removed;
                                cx.notify();
                            })),
                    )
            }))
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .max_w(px(900.0))
                    .child(
                        div()
                            .w(px(180.0))
                            .child(crate::appearance::text("Custom tag name", cx)),
                    )
                    .child(crate::styles::input(&self.form_name, cx))
                    .child(
                        crate::styles::button("choose-tag-field", cx)
                            .debug_selector(|| "tag-field-choices".into())
                            .label(self.tr("page.itemDetail.addField"))
                            .disabled(self.tag_busy)
                            .dropdown_menu({
                                let view = cx.weak_entity();
                                move |menu, _, cx| {
                                    let mut menu = menu.scrollable(true).max_h(px(360.));
                                    let Some(app) = view.upgrade() else {
                                        return menu;
                                    };
                                    let app = app.read(cx);
                                    let query =
                                        tags::canonical_name(&app.form_name.read(cx).value());
                                    let mut names = tags::KNOWN_FIELDS
                                        .iter()
                                        .map(|name| (*name).to_owned())
                                        .chain(app.state.settings.tag_configs.keys().cloned())
                                        .filter(|name| {
                                            name.contains(&query)
                                                && !app
                                                    .tag_fields
                                                    .iter()
                                                    .any(|field| &field.name == name)
                                        })
                                        .collect::<Vec<_>>();
                                    names.sort_by(|a, b| feishin_core::i18n::compare(a, b, false));
                                    names.dedup();
                                    for name in names {
                                        let view = view.clone();
                                        menu =
                                            menu.item(PopupMenuItem::new(name.clone()).on_click(
                                                move |_, window, cx| {
                                                    view.update(cx, |app, cx| {
                                                        app.add_tag_field(&name, window, cx)
                                                    })
                                                    .ok();
                                                },
                                            ));
                                    }
                                    menu
                                }
                            }),
                    )
                    .child(
                        crate::styles::button("add-custom-tag", cx)
                            .debug_selector(|| "tag-add".into())
                            .label(crate::appearance::text("Add field", cx))
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let name = this.form_name.read(cx).value().to_string();
                                this.add_tag_field(&name, window, cx);
                            })),
                    ),
            )
            .when_some(
                self.tag_suggestions.as_ref(),
                |container, (name, values)| {
                    container.child(
                        div().v_flex().gap_2().child(name.clone()).children(
                            [
                                ("page.itemDetail.customValues", &values.custom),
                                ("page.itemDetail.serverSuggestions", &values.server),
                            ]
                            .into_iter()
                            .filter(|(_, values)| !values.is_empty())
                            .map(|(group, values)| {
                                div().v_flex().gap_2().child(self.tr(group)).child(
                                    div()
                                        .h_flex()
                                        .when(crate::appearance::rtl(cx), |div| {
                                            div.flex_row_reverse()
                                        })
                                        .gap_2()
                                        .flex_wrap()
                                        .children(values.iter().enumerate().map(
                                            |(index, value)| {
                                                let name = name.clone();
                                                let value = value.clone();
                                                crate::styles::button((group, index), cx)
                                                    .label(value.clone())
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.use_tag_suggestion(
                                                                &name, &value, window, cx,
                                                            )
                                                        },
                                                    ))
                                            },
                                        )),
                                )
                            }),
                        ),
                    )
                },
            )
            .when_some(self.tag_config_edit.as_ref(), |container, name| {
                container.child(
                    div()
                        .v_flex()
                        .gap_3()
                        .max_w(px(900.0))
                        .child(format!(
                            "{}: {name}",
                            self.tr("page.itemDetail.tagConfiguration")
                        ))
                        .child(self.tr("page.itemDetail.customValues"))
                        .child(crate::styles::input(&self.form_rules, cx).h(px(120.0)))
                        .child(self.tr("page.itemDetail.autocompleteSource"))
                        .child(crate::styles::input(&self.form_comment, cx))
                        .when(!self.tag_source_names.is_empty(), |div| {
                            div.child(
                                crate::styles::button("tag-config-source", cx)
                                    .label(self.tr("page.itemDetail.serverSuggestions"))
                                    .dropdown_menu({
                                        let names = self.tag_source_names.clone();
                                        let view = cx.weak_entity();
                                        move |menu, _, _| {
                                            let mut menu = menu.scrollable(true).max_h(px(360.));
                                            for name in &names {
                                                let view = view.clone();
                                                let name = name.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(tags::source_label(&name))
                                                        .on_click(move |_, window, cx| {
                                                            view.update(cx, |this, cx| {
                                                                this.form_comment.update(
                                                                    cx,
                                                                    |input, cx| {
                                                                        input.set_value(
                                                                            name.clone(),
                                                                            window,
                                                                            cx,
                                                                        )
                                                                    },
                                                                )
                                                            })
                                                            .ok();
                                                        }),
                                                );
                                            }
                                            menu
                                        }
                                    }),
                            )
                        })
                        .child(
                            crate::styles::button("save-tag-config", cx)
                                .label(crate::appearance::text("Save field settings", cx))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.save_tag_config(window, cx)
                                })),
                        )
                        .child(
                            crate::styles::button("remove-tag-config", cx)
                                .debug_selector(|| "tag-remove-config".into())
                                .label(self.tr("common.delete"))
                                .disabled(self.tag_busy)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.remove_tag_config(window, cx)
                                })),
                        )
                        .child(
                            crate::styles::button("cancel-tag-config", cx)
                                .label(crate::appearance::text("Cancel", cx))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.tag_config_edit = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                crate::styles::button("tag-rescan", cx)
                    .label(self.tr("page.itemDetail.triggerRescan"))
                    .selected(self.state.settings.tag_rescan)
                    .disabled(self.tag_busy)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.settings.tag_rescan = !this.state.settings.tag_rescan;
                        this.save();
                        cx.notify();
                    })),
            )
            .when_some(self.tag_progress, |div, (done, total)| {
                div.child(format!("{done} / {total} files"))
            })
            .child(div().text_lg().child(self.tr("page.itemDetail.artworkTab")))
            .child(
                div()
                    .id("tag-artwork-drop")
                    .debug_selector(|| "tag-artwork-drop".into())
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .p_4()
                    .child(crate::appearance::text(
                        "Drop an image here to replace the front cover",
                        cx,
                    ))
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                        this.drop_tag_artwork(paths.paths(), cx);
                        cx.stop_propagation();
                    })),
            )
            .when_some(self.tag_artwork_path.as_ref(), |div, path| {
                div.child(
                    img(path.clone())
                        .size(px(180.0))
                        .object_fit(ObjectFit::Contain),
                )
            })
            .when(self.tag_artwork_path.is_none(), |div| {
                div.child(self.tr(if mixed_artwork {
                    "page.itemDetail.multipleArtworks"
                } else {
                    "page.itemDetail.noArtwork"
                }))
            })
            .child(
                div()
                    .h_flex()
                    .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                    .gap_3()
                    .child(
                        crate::styles::button("choose-artwork", cx)
                            .label(crate::appearance::text("Replace front cover", cx))
                            .disabled(self.tag_busy)
                            .on_click(cx.listener(|_, _, _, cx| {
                                let picker = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: false,
                                    multiple: false,
                                    prompt: Some("Choose cover artwork".into()),
                                });
                                cx.spawn(async move |this, cx| {
                                    if let Ok(Ok(Some(paths))) = picker.await
                                        && let Some(path) = paths.first()
                                    {
                                        this.update(cx, |this, cx| {
                                            this.load_tag_artwork(path.clone(), cx);
                                        })
                                        .ok();
                                    }
                                })
                                .detach();
                            })),
                    )
                    .child(
                        crate::styles::button("remove-artwork", cx)
                            .label(self.tr("page.itemDetail.removeArtwork"))
                            .disabled(self.tag_busy || !has_artwork)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tag_artwork = Artwork::Remove;
                                this.tag_artwork_path = None;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}
