use crate::app::Feishin;
use feishin_core::{
    api::Api,
    model::{Item, ItemKind, ListQuery},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, IconName, StyledExt, WindowExt,
    button::ButtonVariants,
    checkbox::Checkbox,
    input::{InputEvent, InputState},
};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

fn language(cx: &App) -> &str {
    cx.try_global::<crate::appearance::Language>()
        .map_or("en", |value| value.0.as_str())
}

actions!(add_to_playlist, [ToggleRow, Submit]);

pub struct Picker {
    parent: WeakEntity<Feishin>,
    api: Api,
    pub(crate) sources: Vec<Item>,
    input: Entity<InputState>,
    playlists: Vec<Item>,
    selected: Vec<String>,
    new_names: Vec<String>,
    skip_duplicates: bool,
    loading: bool,
    busy: bool,
    message: String,
    focused: Option<String>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    cancelled: Arc<AtomicBool>,
    _subscriptions: Vec<Subscription>,
}
impl Drop for Picker {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Added {
    tracks: usize,
    completed: Vec<String>,
    created: Vec<(String, Item)>,
    errors: Vec<String>,
}

// The original skips tracks already in each destination, retaining repeated source tracks.
fn add(
    api: &Api,
    sources: &[Item],
    selected: &[Item],
    names: &[String],
    skip: bool,
) -> anyhow::Result<Added> {
    anyhow::ensure!(
        sources.iter().all(|item| item.server_id == api.server.id),
        "Choose items from the current server"
    );
    anyhow::ensure!(
        selected.iter().all(|item| item.server_id == api.server.id
            && item.kind == ItemKind::Playlist
            && item.rules.is_none()),
        "Choose regular playlists on the current server"
    );
    let mut songs = vec![];
    for source in sources {
        songs.extend(api.all_tracks(source)?);
    }
    let mut result = Added::default();
    if songs.is_empty() {
        return Ok(result);
    }
    let mut targets = selected.to_vec();
    for name in names {
        match api.create_playlist(name, false, None, &[]) {
            Ok(id) => {
                let item = Item {
                    id,
                    name: name.clone(),
                    server_id: api.server.id,
                    kind: ItemKind::Playlist,
                    ..Default::default()
                };
                targets.push(item.clone());
                result.created.push((name.clone(), item));
            }
            Err(error) => result.errors.push(format!("[{name}] {error}")),
        }
    }
    for target in targets {
        let operation = (|| -> anyhow::Result<usize> {
            let existing = if skip {
                api.all_tracks(&target)?
                    .into_iter()
                    .map(|song| song.id)
                    .collect::<HashSet<_>>()
            } else {
                HashSet::new()
            };
            let tracks = songs
                .iter()
                .filter(|song| !existing.contains(&song.id))
                .cloned()
                .collect::<Vec<_>>();
            if !tracks.is_empty() {
                api.add_to_playlist_checked(&target.id, &tracks)?;
            }
            Ok(tracks.len())
        })();
        match operation {
            Ok(count) => {
                result.tracks += count;
                result.completed.push(target.id);
            }
            Err(error) => result.errors.push(format!("[{}] {error}", target.name)),
        }
    }
    Ok(result)
}

impl Feishin {
    pub fn open_add_to_playlist(
        &mut self,
        sources: Vec<Item>,
        selected: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if sources.is_empty() || self.add_to_playlist.is_some() {
            return;
        }
        let Some(api) = self.api() else {
            return;
        };
        if sources
            .iter()
            .any(|item| item.server_id != api.server.id || item.kind == ItemKind::Radio)
        {
            return;
        }
        if !cx
            .key_bindings()
            .borrow()
            .bindings()
            .any(|binding| binding.action().as_any().is::<ToggleRow>())
        {
            cx.bind_keys([
                KeyBinding::new("space", ToggleRow, Some("AddToPlaylist")),
                KeyBinding::new("enter", Submit, Some("AddToPlaylist")),
            ]);
        }
        let parent = cx.weak_entity();
        let skip = self.state.settings.playlist_skip_duplicates;
        let mut playlists = self.playlist_options.clone();
        for target in &selected {
            if !playlists.iter().any(|item| item.id == target.id) {
                playlists.push(target.clone());
            }
        }
        let picker = cx.new(|cx| {
            let input = cx.new(|cx| {
                crate::appearance::input_state(window, cx)
                    .placeholder("form.addToPlaylist.searchOrCreate")
            });
            let subscription = cx.subscribe_in(
                &input,
                window,
                |this: &mut Picker, _, event: &InputEvent, window, cx| match event {
                    InputEvent::PressEnter { .. } => this.submit(window, cx),
                    InputEvent::Change => {
                        this.focused = None;
                        this.scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    }
                    _ => {}
                },
            );
            let observation = cx.observe(&parent.upgrade().unwrap(), |_, _, cx| cx.notify());
            let cancelled = Arc::new(AtomicBool::new(false));
            Picker {
                parent: parent.clone(),
                api: api.with_cancellation(cancelled.clone()),
                sources,
                input,
                playlists,
                selected: selected.into_iter().map(|item| item.id).collect(),
                new_names: vec![],
                skip_duplicates: skip,
                loading: false,
                busy: false,
                message: String::new(),
                focused: None,
                focus: cx.focus_handle(),
                scroll: Default::default(),
                cancelled,
                _subscriptions: vec![subscription, observation],
            }
        });
        self.add_to_playlist = Some(picker.clone());
        picker.update(cx, |picker, cx| picker.load(cx));
        let title = self.tr("form.addToPlaylist.title");
        window.open_dialog(cx, move |dialog, _, cx| {
            let parent = parent.clone();
            let busy = picker.read(cx).busy;
            dialog
                .title(title.clone())
                .aria_label(title.clone())
                .width(rems(38.75))
                .close_button(!busy)
                .keyboard(!busy)
                .overlay_closable(!busy)
                .child(picker.clone())
                .on_close(move |_, _, cx| {
                    parent
                        .update(cx, |parent, cx| {
                            parent.add_to_playlist = None;
                            cx.notify();
                        })
                        .ok();
                })
        });
        if let Some(picker) = &self.add_to_playlist {
            let input = picker.read(cx).input.clone();
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }
}

impl Picker {
    fn text(&self, key: &str, cx: &App) -> String {
        crate::appearance::text(key, cx).to_string()
    }
    fn load(&mut self, cx: &mut Context<Self>) {
        self.playlists
            .retain(|item| item.rules.is_none() && item.server_id == self.api.server.id);
        self.playlists
            .sort_by_cached_key(|item| item.name.to_lowercase());
        let api = self.api.clone();
        self.loading = true;
        let task = cx.background_executor().spawn(async move {
            api.all_items(ListQuery {
                kind: ItemKind::Playlist,
                ..Default::default()
            })
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(mut items) => {
                        items.retain(|item| item.rules.is_none());
                        this.playlists = items;
                    }
                    Err(error) => {
                        log::warn!("Could not load add-to-playlist choices: {error}");
                        this.message = error.to_string();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn visible(&self, cx: &App) -> Vec<Item> {
        let query = self.input.read(cx).value().to_lowercase();
        self.playlists
            .iter()
            .filter(|item| item.name.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }
    fn toggle(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        if self.selected.iter().any(|value| value == id) {
            self.selected.retain(|value| value != id);
        } else {
            self.selected.push(id.into());
        }
        cx.notify();
    }
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.input.read(cx).value().to_string();
        if self.busy || name.trim().is_empty() {
            return;
        }
        self.new_names.push(name);
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        cx.notify();
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.cancelled.store(true, Ordering::Relaxed);
        self.parent
            .update(cx, |parent, cx| {
                parent.add_to_playlist = None;
                cx.notify();
            })
            .ok();
        window.close_dialog(cx);
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.loading || self.selected.is_empty() && self.new_names.is_empty() {
            return;
        }
        if self
            .parent
            .upgrade()
            .is_none_or(|parent| parent.read(cx).state.active_server != Some(self.api.server.id))
        {
            self.message = "The active server changed; reopen the playlist dialog".into();
            cx.notify();
            return;
        }
        let targets = self
            .selected
            .iter()
            .filter_map(|id| self.playlists.iter().find(|item| &item.id == id).cloned())
            .collect::<Vec<_>>();
        if targets.len() != self.selected.len() {
            self.message = "A selected playlist is no longer available".into();
            cx.notify();
            return;
        }
        self.busy = true;
        self.message.clear();
        cx.notify();
        let (api, sources, names, skip) = (
            self.api.clone(),
            self.sources.clone(),
            self.new_names.clone(),
            self.skip_duplicates,
        );
        let count = targets.len() + names.len();
        let task = cx
            .background_executor()
            .spawn(async move { add(&api, &sources, &targets, &names, skip) });
        cx.spawn_in(window,async move|this,cx|{
            let result=task.await;
            this.update_in(cx,|this,window,cx|{
                this.busy=false;
                match result {
                    Ok(result)=>{
                        for (name,item) in result.created {
                            if let Some(index)=this.new_names.iter().position(|value|*value==name){this.new_names.remove(index);}
                            this.selected.push(item.id.clone());this.playlists.push(item);
                        }
                        if result.errors.is_empty() {
                            if result.tracks>0 {
                                let message=feishin_core::i18n::translate_with(language(cx),"form.addToPlaylist.success",&serde_json::json!({"message":result.tracks,"numOfPlaylists":count})).unwrap_or_else(|| format!("Added {} tracks to {count} playlists",result.tracks));
                                this.parent.update(cx,|parent,cx|{parent.message=message;parent.refresh_playlists(cx);}).ok();
                                this.close(window,cx);
                            } else {this.message=this.text("form.addToPlaylist.noneAdded",cx);}
                        } else {
                            // Retrying must not append again to destinations that already succeeded.
                            this.selected.retain(|id|!result.completed.contains(id));
                            this.message=result.errors.join("\n");log::warn!("Add to playlist partly failed: {}",this.message);
                            this.parent.update(cx,|parent,cx|parent.refresh_playlists(cx)).ok();
                        }
                    }
                    Err(error)=>{log::warn!("Could not add to playlists: {error}");this.message=error.to_string();}
                }
                cx.notify();
            }).ok();
        }).detach();
    }
}

impl Render for Picker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.visible(cx);
        let font = cx.theme().font_size;
        let disabled = self.busy || self.loading;
        let parent = self.parent.upgrade();
        if let Some(parent) = &parent {
            let top = self.scroll.top_item();
            parent.update(cx, |parent, cx| {
                for item in visible.iter().skip(top).take(16) {
                    parent.fetch_item_cover(item, cx);
                }
            });
        }
        let mut rows = div()
            .id("add-playlist-rows")
            .debug_selector(|| "add-playlist-rows".into())
            .v_flex()
            .max_h(font * 18.)
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .track_focus(&self.focus)
            .key_context("AddToPlaylist")
            .on_action(cx.listener(|this, _: &ToggleRow, _, cx| {
                if this.busy {
                    return;
                }
                let rows = this.visible(cx);
                let selected = this
                    .focused
                    .as_ref()
                    .and_then(|id| rows.iter().find(|row| &row.id == id))
                    .or_else(|| rows.first());
                if let Some(item) = selected {
                    this.focused = Some(item.id.clone());
                    this.toggle(&item.id, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Submit, window, cx| this.submit(window, cx)))
            .on_scroll_wheel(cx.listener(|_, _, _, cx| cx.notify()))
            .tab_index(0)
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.busy {
                    return;
                }
                let rows = this.visible(cx);
                let index = this
                    .focused
                    .as_ref()
                    .and_then(|id| rows.iter().position(|row| &row.id == id));
                let selected = match event.keystroke.key.as_str() {
                    "down" => {
                        Some(index.map_or(0, |index| (index + 1).min(rows.len().saturating_sub(1))))
                    }
                    "up" => Some(index.unwrap_or(0).saturating_sub(1)),
                    "space" => {
                        if let Some(item) = rows.get(index.unwrap_or(0)) {
                            this.toggle(&item.id, cx);
                        }
                        None
                    }
                    "enter" => {
                        this.submit(window, cx);
                        None
                    }
                    "tab" => {
                        this.focused = None;
                        return;
                    }
                    _ => return,
                };
                if let Some(index) = selected.filter(|index| *index < rows.len()) {
                    this.focused = Some(rows[index].id.clone());
                    this.scroll.scroll_to_item(index);
                }
                window.prevent_default();
                cx.stop_propagation();
                cx.notify();
            }));
        for item in visible {
            let id = item.id.clone();
            let click_id = id.clone();
            let checkbox_id = id.clone();
            let selected = self.selected.contains(&id);
            let row = div()
                .id(SharedString::from(format!("add-playlist-row-{id}")))
                .debug_selector({
                    let id = id.clone();
                    move || format!("add-playlist-row-{id}")
                })
                .h_flex()
                .w_full()
                .gap_3()
                .p_2()
                .cursor_pointer()
                .role(Role::Button)
                .aria_label(item.name.clone())
                .aria_selected(selected)
                .when(self.focused.as_ref() == Some(&id), |row| {
                    row.bg(cx.theme().secondary)
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    window.focus(&this.focus);
                    this.focused = Some(click_id.clone());
                    this.toggle(&click_id, cx);
                }))
                .child(
                    Checkbox::new(SharedString::from(format!("add-playlist-check-{id}")))
                        .checked(selected)
                        .disabled(disabled)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            window.focus(&this.focus);
                            this.focused = Some(checkbox_id.clone());
                            this.toggle(&checkbox_id, cx);
                            cx.stop_propagation();
                        })),
                )
                .when_some(parent.clone(), |row, parent| {
                    row.child(parent.read(cx).artwork(&item, f32::from(font * 3.), cx))
                })
                .child(
                    div()
                        .v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .child(item.name.clone()),
                        )
                        .child(
                            div()
                                .h_flex()
                                .justify_between()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "{} · {}",
                                    item.column_data
                                        .get("songCount")
                                        .map_or("0", String::as_str),
                                    feishin_core::metadata_format::duration_text(
                                        item.duration,
                                        language(cx),
                                        false
                                    )
                                ))
                                .child(self.text(
                                    if item.public {
                                        "common.public"
                                    } else {
                                        "common.private"
                                    },
                                    cx,
                                )),
                        ),
                );
            rows = rows.child(row);
        }
        let mut selected = div().h_flex().flex_wrap().gap_2();
        for id in self.selected.clone() {
            let name = self
                .playlists
                .iter()
                .find(|item| item.id == id)
                .map_or(id.clone(), |item| item.name.clone());
            selected = selected.child(
                crate::styles::button(SharedString::from(format!("add-playlist-remove-{id}")), cx)
                    .label(name)
                    .icon(IconName::Close)
                    .disabled(self.busy)
                    .on_click(cx.listener(move |this, _, _, cx| this.toggle(&id, cx))),
            );
        }
        for (index, name) in self.new_names.iter().enumerate() {
            selected = selected.child(
                crate::styles::button(("add-playlist-new", index), cx)
                    .label(format!("+ {name}"))
                    .icon(IconName::Close)
                    .disabled(self.busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.busy && index < this.new_names.len() {
                            this.new_names.remove(index);
                            cx.notify();
                        }
                    })),
            );
        }
        let query = self.input.read(cx).value().to_string();
        div()
            .v_flex()
            .w_full()
            .gap_3()
            .child(crate::styles::input(&self.input, cx).disabled(self.busy))
            .child(rows)
            .when(!query.is_empty(), |div| {
                div.child(
                    crate::styles::button("add-playlist-create", cx)
                        .debug_selector(|| "add-playlist-create".into())
                        .ghost()
                        .icon(IconName::Plus)
                        .disabled(self.busy)
                        .label(
                            feishin_core::i18n::translate_with(
                                language(cx),
                                "form.addToPlaylist.create",
                                &serde_json::json!({"playlist": query}),
                            )
                            .unwrap_or(query),
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.create(window, cx))),
                )
            })
            .child(selected)
            .child(
                Checkbox::new("add-playlist-skip-duplicates")
                    .debug_selector(|| "add-playlist-skip-duplicates".into())
                    .label(self.text("form.addToPlaylist.input_skipDuplicates", cx))
                    .checked(self.skip_duplicates)
                    .disabled(self.busy)
                    .on_click(cx.listener(|this, value: &bool, _, cx| {
                        this.skip_duplicates = *value;
                        this.parent
                            .update(cx, |parent, cx| {
                                parent.state.settings.playlist_skip_duplicates = *value;
                                parent.save();
                                cx.notify();
                            })
                            .ok();
                        cx.notify();
                    })),
            )
            .when(!self.message.is_empty(), |element| {
                element.child(div().text_sm().child(self.message.clone()))
            })
            .child(
                div()
                    .h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        crate::styles::button("add-playlist-cancel", cx)
                            .debug_selector(|| "add-playlist-cancel".into())
                            .ghost()
                            .label(self.text("common.cancel", cx).to_uppercase())
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    )
                    .child(
                        crate::styles::button("add-playlist-submit", cx)
                            .debug_selector(|| "add-playlist-submit".into())
                            .primary()
                            .label(self.text("common.add", cx).to_uppercase())
                            .disabled(
                                disabled || (self.selected.is_empty() && self.new_names.is_empty()),
                            )
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{Picker, add};
    use crate::app::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{Item, ItemKind, Server, ServerType, Session},
        queue::Enqueue,
        storage::State,
    };
    use gpui::{AppContext, Entity, Modifiers, TestAppContext, VisualTestContext, point, px, size};
    use gpui_component::Root;
    use serde_json::{Value, json};
    use std::{
        cell::RefCell,
        collections::BTreeMap,
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    #[derive(Default)]
    struct Recorded {
        existing: BTreeMap<String, Vec<String>>,
        calls: Vec<Value>,
        created: Vec<String>,
        fail: bool,
    }
    struct Fixture {
        api: Api,
        recorded: Arc<Mutex<Recorded>>,
        done: Arc<AtomicBool>,
        worker: Option<thread::JoinHandle<()>>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.done.store(true, Ordering::Relaxed);
            self.worker.take().unwrap().join().unwrap();
        }
    }
    impl Fixture {
        fn new() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let server = Server {
                id: uuid::Uuid::new_v4(),
                name: "Fixture".into(),
                url: format!("http://{}", listener.local_addr().unwrap()),
                username: "me".into(),
                kind: ServerType::Navidrome,
                legacy_auth: false,
                options: Default::default(),
            };
            let api = Api::new(
                server,
                Session {
                    user_id: "me".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            let recorded = Arc::new(Mutex::new(Recorded::default()));
            let data = recorded.clone();
            let done = Arc::new(AtomicBool::new(false));
            let stop = done.clone();
            let worker = thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let Ok((mut stream, _)) = listener.accept() else {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut first = String::new();
                    if reader.read_line(&mut first).is_err() || first.is_empty() {
                        continue;
                    }
                    let mut length = 0;
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty()
                        {
                            break;
                        }
                        if let Some(raw) = line.to_ascii_lowercase().strip_prefix("content-length:")
                        {
                            length = raw.trim().parse::<usize>().unwrap();
                        }
                    }
                    let mut bytes = vec![0; length];
                    if reader.read_exact(&mut bytes).is_err() {
                        continue;
                    }
                    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                    let method = first.split_whitespace().next().unwrap();
                    let url = gpui::http_client::Url::parse(&format!(
                        "http://localhost{}",
                        first.split_whitespace().nth(1).unwrap()
                    ))
                    .unwrap();
                    let path = url.path();
                    let mut state = data.lock().unwrap();
                    let mut status = "200 OK";
                    let response = if method == "POST" && path == "/api/playlist" {
                        let name = body["name"].as_str().unwrap();
                        assert_eq!(body["public"], false);
                        state.created.push(name.into());
                        json!({"id":format!("new-{name}")})
                    } else if method == "POST" && path.ends_with("/tracks") {
                        let id = path.split('/').nth(3).unwrap();
                        if state.fail && id == "q" {
                            status = "500 Error";
                            json!({"error":"fixture failure"})
                        } else {
                            state.calls.push(json!({"id":id,"songs":body["ids"]}));
                            state.existing.entry(id.into()).or_default().extend(
                                body["ids"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .map(|id| id.as_str().unwrap().into()),
                            );
                            json!({})
                        }
                    } else if path == "/api/playlist" {
                        json!([{"id":"p","name":"First","songCount":3,"duration":125,"ownerId":"me"},{"id":"q","name":"Second","songCount":2,"ownerId":"me","public":true},{"id":"smart","name":"Smart","rules":{"all":[]}}])
                    } else if path == "/api/album" {
                        json!([{"id":"album","name":"Album"}])
                    } else if path.starts_with("/rest/") {
                        json!({"subsonic-response":{"status":"ok","directory":{"child":[{"id":"a","title":"a"},{"id":"b","title":"b"},{"id":"b","title":"b"}]}}})
                    } else {
                        let id = path.split('/').nth(3).unwrap_or("");
                        let songs = if path.ends_with("/tracks") && id != "source" {
                            state.existing.get(id).cloned().unwrap_or_default()
                        } else {
                            vec!["a".into(), "b".into(), "b".into()]
                        };
                        Value::Array(
                            songs
                                .iter()
                                .enumerate()
                                .map(|(index, id)| {
                                    if path.ends_with("/tracks") {
                                        json!({"id":index+1,"mediaFileId":id,"title":id})
                                    } else {
                                        json!({"id":id,"title":id})
                                    }
                                })
                                .collect(),
                        )
                    };
                    let count = response.as_array().map_or(0, Vec::len);
                    let body = response.to_string();
                    drop(state);
                    let _ = write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nX-Total-Count: {count}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                }
            });
            Self {
                api,
                recorded,
                done,
                worker: Some(worker),
            }
        }
        fn item(&self, id: &str, kind: ItemKind) -> Item {
            Item {
                id: id.into(),
                name: id.into(),
                kind,
                server_id: self.api.server.id,
                ..Default::default()
            }
        }
    }

    #[test]
    fn submissions_match_original_multi_playlist_duplicate_and_creation_cases() {
        let fixture = Fixture::new();
        let reference: Value =
            serde_json::from_str(include_str!("../assets/add-to-playlist-reference.json")).unwrap();
        for case in reference["cases"].as_array().unwrap() {
            *fixture.recorded.lock().unwrap() = Recorded {
                existing: serde_json::from_value(case["existing"].clone()).unwrap(),
                ..Default::default()
            };
            let sources = match case["sourceKey"].as_str().unwrap() {
                "songId" => ["a", "b", "b"]
                    .iter()
                    .map(|id| fixture.item(id, ItemKind::Song))
                    .collect::<Vec<_>>(),
                kind => vec![fixture.item(
                    "source",
                    match kind {
                        "albumId" => ItemKind::Album,
                        "artistId" => ItemKind::Artist,
                        "genreId" => ItemKind::Genre,
                        "folderId" => ItemKind::Folder,
                        _ => ItemKind::Playlist,
                    },
                )],
            };
            let names = serde_json::from_value::<Vec<String>>(case["names"].clone()).unwrap();
            let result = add(
                &fixture.api,
                &sources,
                &[
                    fixture.item("p", ItemKind::Playlist),
                    fixture.item("q", ItemKind::Playlist),
                ],
                &names,
                case["skip"] == true,
            )
            .unwrap();
            assert!(result.errors.is_empty(), "{case}: {:?}", result.errors);
            assert_eq!(
                fixture.recorded.lock().unwrap().calls,
                case["calls"].as_array().unwrap().clone(),
                "{case}"
            );
            assert_eq!(fixture.recorded.lock().unwrap().created, names);
            assert_eq!(
                result.tracks,
                case["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|call| call["songs"].as_array().unwrap().len())
                    .sum::<usize>()
            );
        }
        fixture.recorded.lock().unwrap().calls.clear();
        let mut invalid = fixture.item("a", ItemKind::Song);
        invalid.server_id = uuid::Uuid::new_v4();
        assert!(
            add(
                &fixture.api,
                &[invalid],
                &[],
                &["Never created".into()],
                false
            )
            .is_err()
        );
        assert!(fixture.recorded.lock().unwrap().calls.is_empty());
    }

    fn click(cx: &mut VisualTestContext, id: &str) {
        let bounds = cx
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("Missing {id}"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    }
    fn settle(cx: &mut VisualTestContext, picker: &Entity<Picker>) {
        for _ in 0..400 {
            cx.run_until_parked();
            if !picker.read_with(cx, |picker, _| picker.loading || picker.busy) {
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("Playlist request did not finish");
    }

    #[gpui::test]
    fn dialog_preserves_navigation_selection_cancel_preferences_and_partial_retry(
        cx: &mut TestAppContext,
    ) {
        let fixture = Fixture::new();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir = std::env::temp_dir().join(format!("feishin-add-dialog-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.playlist_list = false;
            state
                .queue
                .add(vec![fixture.item("kept", ItemKind::Song)], Enqueue::Now);
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                app.route = Route::Settings;
                app.state.active_server = Some(fixture.api.server.id);
                app.sessions
                    .insert(fixture.api.server.id, fixture.api.clone());
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1500.), px(1200.)));
        let queue = app.read_with(cx, |app, _| app.state.queue.order.clone());
        let sources = vec![
            fixture.item("a", ItemKind::Song),
            fixture.item("b", ItemKind::Song),
            fixture.item("b", ItemKind::Song),
        ];
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_add_to_playlist(
                    sources.clone(),
                    vec![fixture.item("p", ItemKind::Playlist)],
                    window,
                    cx,
                )
            })
        });
        let picker = app.read_with(cx, |app, _| app.add_to_playlist.clone().unwrap());
        settle(cx, &picker);
        assert!(cx.debug_bounds("add-playlist-row-smart").is_none());
        assert_eq!(
            picker.read_with(cx, |picker, _| picker.selected.clone()),
            ["p"]
        );
        click(cx, "add-playlist-row-q");
        cx.simulate_keystrokes("up space down space");
        cx.run_until_parked();
        assert!(
            picker.read_with(cx, |picker, _| picker.selected.is_empty()),
            "{:?}",
            picker.read_with(cx, |picker, _| (
                picker.selected.clone(),
                picker.focused.clone()
            ))
        );
        cx.simulate_keystrokes("space up space");
        cx.run_until_parked();
        assert_eq!(
            picker.read_with(cx, |picker, _| picker.selected.clone()),
            ["q", "p"]
        );
        cx.update(|window, cx| {
            picker.update(cx, |picker, cx| {
                picker
                    .input
                    .update(cx, |input, cx| input.set_value("Fresh", window, cx))
            })
        });
        cx.run_until_parked();
        click(cx, "add-playlist-create");
        click(cx, "add-playlist-skip-duplicates");
        assert!(!picker.read_with(cx, |picker, _| picker.skip_duplicates));
        click(cx, "add-playlist-cancel");
        assert!(fixture.recorded.lock().unwrap().calls.is_empty());
        assert!(fixture.recorded.lock().unwrap().created.is_empty());
        assert!(app.read_with(cx, |app, _| app.add_to_playlist.is_none()));
        assert!(
            !feishin_core::storage::load(&dir.join("state.json"))
                .unwrap()
                .settings
                .playlist_skip_duplicates
        );
        drop(picker);
        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_add_to_playlist(
                    sources.clone(),
                    vec![fixture.item("p", ItemKind::Playlist)],
                    window,
                    cx,
                )
            })
        });
        let picker = app.read_with(cx, |app, _| app.add_to_playlist.clone().unwrap());
        settle(cx, &picker);
        assert!(!picker.read_with(cx, |picker, _| picker.skip_duplicates));
        click(cx, "add-playlist-row-q");
        cx.update(|window, cx| {
            picker.update(cx, |picker, cx| {
                picker
                    .input
                    .update(cx, |input, cx| input.set_value("Fresh", window, cx))
            })
        });
        cx.run_until_parked();
        click(cx, "add-playlist-create");
        fixture.recorded.lock().unwrap().fail = true;
        click(cx, "add-playlist-submit");
        settle(cx, &picker);
        picker.read_with(cx, |picker, _| {
            assert_eq!(picker.selected, ["q"]);
            assert!(picker.new_names.is_empty());
            assert!(picker.message.contains("500"));
        });
        assert_eq!(fixture.recorded.lock().unwrap().created, ["Fresh"]);
        fixture.recorded.lock().unwrap().fail = false;
        click(cx, "add-playlist-submit");
        settle(cx, &picker);
        app.read_with(cx, |app, _| {
            assert!(app.add_to_playlist.is_none());
            assert_eq!(app.route, Route::Settings);
            assert_eq!(app.state.queue.order, queue);
        });
        assert_eq!(
            fixture.recorded.lock().unwrap().calls,
            vec![
                json!({"id":"p","songs":["a","b","b"]}),
                json!({"id":"new-Fresh","songs":["a","b","b"]}),
                json!({"id":"q","songs":["a","b","b"]})
            ]
        );
        cx.simulate_mouse_move(point(px(1400.), px(1000.)), None, Modifiers::none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[gpui::test]
    fn sidebar_drops_open_preselected_dialog_and_reject_smart_playlists(cx: &mut TestAppContext) {
        use gpui::MouseButton;
        let fixture = Fixture::new();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-sidebar-add-drop-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.image_shown = false;
            state.queue.add(
                vec![
                    fixture.item("a", ItemKind::Song),
                    fixture.item("b", ItemKind::Song),
                    fixture.item("b", ItemKind::Song),
                ],
                Enqueue::Now,
            );
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                app.route = Route::Queue;
                app.state.active_server = Some(fixture.api.server.id);
                app.sessions
                    .insert(fixture.api.server.id, fixture.api.clone());
                app.playlist_options = vec![
                    fixture.item("p", ItemKind::Playlist),
                    Item {
                        rules: Some(json!({"all":[]})),
                        ..fixture.item("smart", ItemKind::Playlist)
                    },
                ];
                app.queue_selected
                    .extend(app.state.queue.order[1..].iter().copied());
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1800.), px(2200.)));
        cx.run_until_parked();
        let queue = app.read_with(cx, |app, _| app.state.queue.order.clone());
        for target in ["smart", "p"] {
            let bounds = cx.debug_bounds("queue-drag-row-1").unwrap();
            let source = point(bounds.origin.x + px(62.), bounds.origin.y + px(2.));
            let destination = cx
                .debug_bounds(&format!("sidebar-playlist-{target}"))
                .unwrap()
                .center();
            cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                source + point(px(12.), px(5.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            cx.simulate_mouse_move(destination, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_up(destination, MouseButton::Left, Modifiers::none());
            cx.run_until_parked();
            if target == "smart" {
                assert!(app.read_with(cx, |app, _| app.add_to_playlist.is_none()));
            } else {
                let picker = app.read_with(cx, |app, _| app.add_to_playlist.clone().unwrap());
                settle(cx, &picker);
                picker.read_with(cx, |picker, _| {
                    assert_eq!(picker.selected, ["p"]);
                    assert_eq!(
                        picker
                            .sources
                            .iter()
                            .map(|item| item.id.as_str())
                            .collect::<Vec<_>>(),
                        ["b", "b"]
                    );
                });
                click(cx, "add-playlist-cancel");
            }
        }
        app.read_with(cx, |app, _| {
            assert_eq!(app.route, Route::Queue);
            assert_eq!(app.state.queue.order, queue);
            assert_eq!(app.queue_selected.len(), 2);
        });
        assert!(fixture.recorded.lock().unwrap().calls.is_empty());
        if dir.exists() {
            std::fs::remove_dir_all(dir).unwrap();
        }
    }
}
