use crate::app::{Feishin, Route};
use feishin_core::{
    model::{Item, ItemKind, ListQuery},
    queue::Enqueue,
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, Disableable, Icon, Sizable, StyledExt, WindowExt,
    button::ButtonVariants,
    input::{InputEvent, InputState},
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const KINDS: [ItemKind; 3] = [ItemKind::Album, ItemKind::AlbumArtist, ItemKind::Song];
const SECTION_KEYS: [&str; 3] = ["albums", "artists", "tracks"];
const PAGE_SIZE: usize = 4;
actions!(command_palette, [Choose, Back]);

#[derive(Clone, Copy, Default, PartialEq)]
enum Page {
    #[default]
    Home,
    Navigation,
    Servers,
}
#[derive(Default)]
struct Section {
    items: Vec<Item>,
    more: bool,
    busy: bool,
    fetched: bool,
    error: Option<String>,
}
#[derive(Clone)]
enum Operation {
    Search,
    Create,
    Page(Page),
    Navigate(Route),
    Server(uuid::Uuid),
    Manage,
    Item(Item),
    More(usize),
    All(usize),
    Playlist(Item),
}
struct Command {
    id: String,
    label: String,
    subtitle: String,
    operation: Operation,
    group: Option<usize>,
    disabled: bool,
}
impl Command {
    fn selectable(&self) -> bool {
        !self.disabled && !matches!(self.operation, Operation::All(_))
    }
}

#[derive(Clone, Copy)]
enum Movement {
    Previous,
    Next,
    First,
    Last,
    PreviousGroup,
    NextGroup,
}

impl Movement {
    fn from_key(key: &str, modifiers: Modifiers) -> Option<Self> {
        let next = matches!(key, "down" | "j" | "n");
        Some(match key {
            "home" => Self::First,
            "end" => Self::Last,
            "up" | "down" => {
                if modifiers.platform {
                    if next { Self::Last } else { Self::First }
                } else if modifiers.alt {
                    if next {
                        Self::NextGroup
                    } else {
                        Self::PreviousGroup
                    }
                } else if next {
                    Self::Next
                } else {
                    Self::Previous
                }
            }
            "j" | "n" | "p" | "k" if modifiers.control => {
                if next {
                    Self::Next
                } else {
                    Self::Previous
                }
            }
            _ => return None,
        })
    }
}

fn selection(commands: &[Command], selected: Option<&str>, movement: Movement) -> Option<String> {
    let items = commands
        .iter()
        .filter(|item| item.selectable())
        .collect::<Vec<_>>();
    let index = items
        .iter()
        .position(|item| Some(item.id.as_str()) == selected);
    let adjacent = |next: bool| {
        if next {
            index.map_or(0, |i| (i + 1).min(items.len().saturating_sub(1)))
        } else {
            index.unwrap_or(0).saturating_sub(1)
        }
    };
    let target = match movement {
        Movement::First => 0,
        Movement::Last => items.len().saturating_sub(1),
        Movement::Next => adjacent(true),
        Movement::Previous => adjacent(false),
        Movement::NextGroup | Movement::PreviousGroup => {
            let next = matches!(movement, Movement::NextGroup);
            index
                .and_then(|index| {
                    let group = items[index].group?;
                    let group = if next {
                        items[index + 1..]
                            .iter()
                            .find_map(|item| item.group.filter(|g| *g != group))
                    } else {
                        items[..index]
                            .iter()
                            .rev()
                            .find_map(|item| item.group.filter(|g| *g != group))
                    }?;
                    items.iter().position(|item| item.group == Some(group))
                })
                .unwrap_or_else(|| adjacent(next))
        }
    };
    items.get(target).map(|item| item.id.clone())
}

pub struct Palette {
    parent: WeakEntity<Feishin>,
    input: Entity<InputState>,
    page: Page,
    query: String,
    sections: [Section; 3],
    generation: u64,
    cancelled: Arc<AtomicBool>,
    server: Option<uuid::Uuid>,
    selected: Option<String>,
    hovered: Option<String>,
    action_focus: HashMap<String, [FocusHandle; 4]>,
    row_focus: HashMap<String, FocusHandle>,
    scroll: ScrollHandle,
    scroll_target: Option<String>,
    heading_focus: [FocusHandle; 3],
    subscriptions: Vec<Subscription>,
}
impl Drop for Palette {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Feishin {
    pub fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.command_palette.is_some() {
            self.close_command_palette(window, cx);
            return;
        }
        if window.has_active_dialog(cx) {
            return;
        }
        if !cx
            .key_bindings()
            .borrow()
            .bindings()
            .any(|binding| binding.action().as_any().is::<Choose>())
        {
            cx.bind_keys([
                KeyBinding::new("enter", Choose, Some("CommandPalette")),
                KeyBinding::new("enter", Choose, Some("CommandPalette > Input")),
                KeyBinding::new("escape", Back, Some("CommandPalette")),
            ]);
        }
        let parent = cx.entity();
        let palette = cx.new(|cx: &mut Context<Palette>| {
            let input = cx.new(|cx| {
                crate::appearance::input_state(window, cx).placeholder("page.globalSearch.title")
            });
            let mut palette = Palette {
                parent: parent.downgrade(),
                input,
                page: Page::Home,
                query: String::new(),
                sections: Default::default(),
                generation: 0,
                cancelled: Default::default(),
                server: self.state.active_server,
                selected: None,
                hovered: None,
                action_focus: HashMap::new(),
                row_focus: HashMap::new(),
                scroll: Default::default(),
                scroll_target: None,
                heading_focus: std::array::from_fn(|_| cx.focus_handle()),
                subscriptions: vec![],
            };
            let weak = cx.weak_entity();
            let handle = window.window_handle();
            // Composite navigation must run before the input's Home/End bindings.
            palette
                .subscriptions
                .push(cx.intercept_keystrokes(move |event, window, cx| {
                    if window.window_handle() == handle {
                        weak.update(cx, |palette, cx| {
                            let focused = palette.input.focus_handle(cx).is_focused(window)
                                || palette
                                    .row_focus
                                    .values()
                                    .any(|handle| handle.is_focused(window))
                                || palette
                                    .action_focus
                                    .values()
                                    .flatten()
                                    .any(|handle| handle.is_focused(window))
                                || palette
                                    .heading_focus
                                    .iter()
                                    .any(|handle| handle.is_focused(window));
                            if !palette.cancelled.load(Ordering::Relaxed)
                                && (focused
                                    || event
                                        .context_stack
                                        .iter()
                                        .any(|context| context.contains("CommandPalette")))
                            {
                                palette.key_down(event, window, cx);
                            }
                        })
                        .ok();
                    }
                }));
            palette.subscriptions.push(cx.subscribe_in(
                &palette.input,
                window,
                |this, _, event, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.query_changed(cx);
                    }
                },
            ));
            palette
                .subscriptions
                .push(cx.observe(&parent, |this, parent, cx| {
                    let server = parent.read(cx).state.active_server;
                    if this.server != server {
                        this.server = server;
                        this.query_changed(cx);
                    }
                    cx.notify();
                }));
            palette
        });
        self.command_palette = Some(palette.clone());
        let weak = parent.downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let weak = weak.clone();
            let cancel_palette = palette.clone();
            dialog
                .width(rems(38.75))
                .body_scroll_handle(palette.read(cx).scroll.clone())
                .close_button(false)
                .on_cancel(move |_, window, cx| {
                    cancel_palette.update(cx, |palette, cx| {
                        if palette.page == Page::Home {
                            true
                        } else {
                            palette.change_page(Page::Home, window, cx);
                            false
                        }
                    })
                })
                .child(palette.clone())
                .on_close(move |_, _, cx| {
                    weak.update(cx, |this, cx| {
                        if let Some(palette) = this.command_palette.take() {
                            palette.read(cx).cancelled.store(true, Ordering::Relaxed);
                        }
                        cx.notify();
                    })
                    .ok();
                })
        });
        if let Some(palette) = &self.command_palette {
            let input = palette.read(cx).input.clone();
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        cx.notify();
    }
    pub fn close_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(palette) = self.command_palette.take() {
            if self.add_to_playlist.take().is_some() {
                window.close_dialog(cx);
            }
            palette.read(cx).cancelled.store(true, Ordering::Relaxed);
            window.close_dialog(cx);
            cx.notify();
        }
    }
}

impl Palette {
    fn query_changed(&mut self, cx: &mut Context<Self>) {
        self.cancelled.store(true, Ordering::Relaxed);
        self.cancelled = Default::default();
        self.generation += 1;
        self.query = self.input.read(cx).value().trim().to_string();
        self.sections = Default::default();
        self.hovered = None;
        self.scroll_target = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
        if self.page != Page::Home || self.query.is_empty() {
            return;
        }
        let generation = self.generation;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            this.update(cx, |this, cx| {
                if this.page == Page::Home
                    && this.generation == generation
                    && !this.cancelled.load(Ordering::Relaxed)
                {
                    for index in 0..3 {
                        if !this.sections[index].fetched {
                            this.load_section(index, cx);
                        }
                    }
                }
            })
            .ok();
        })
        .detach();
    }
    fn load_section(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.sections[index].busy {
            return;
        }
        let Some(api) = self
            .parent
            .upgrade()
            .and_then(|parent| parent.read(cx).api())
        else {
            return;
        };
        let api = api.with_cancellation(self.cancelled.clone());
        let server = api.server.id;
        let generation = self.generation;
        let query = ListQuery {
            kind: KINDS[index],
            search: self.query.clone(),
            offset: self.sections[index].items.len(),
            limit: PAGE_SIZE,
            ..Default::default()
        };
        self.sections[index].busy = true;
        self.sections[index].error = None;
        let task = cx
            .background_executor()
            .spawn(async move { api.list(&query) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.generation != generation
                    || this.server != Some(server)
                    || this.cancelled.load(Ordering::Relaxed)
                {
                    return;
                }
                let section = &mut this.sections[index];
                section.busy = false;
                section.fetched = true;
                match result {
                    Ok(page) => {
                        section.more = page.has_more;
                        let covers = page.items.clone();
                        section.items.extend(page.items);
                        if let Some(parent) = this.parent.upgrade() {
                            parent.update(cx, |parent, cx| {
                                for item in &covers {
                                    parent.fetch_item_cover(item, cx);
                                }
                            });
                        }
                    }
                    Err(error) => {
                        log::debug!("Command palette search failed: {error}");
                        section.error = Some(error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn commands(&self, cx: &App) -> Vec<Command> {
        let Some(parent) = self.parent.upgrade() else {
            return vec![];
        };
        let parent = parent.read(cx);
        let mut commands = vec![];
        let mut add = |id: &str, label: String, operation: Operation| {
            let group = match (&self.page, &operation) {
                (Page::Navigation, Operation::Navigate(Route::Library(_)))
                | (Page::Servers, Operation::Manage) => 1,
                _ => 0,
            };
            commands.push(Command {
                id: id.into(),
                label,
                subtitle: String::new(),
                operation,
                group: Some(group),
                disabled: false,
            });
        };
        match self.page {
            Page::Home => {
                let label = if self.query.is_empty() {
                    format!("{}...", parent.tr("Search"))
                } else {
                    feishin_core::i18n::translate_with(
                        &parent.state.settings.language,
                        "page.globalSearch.commands.searchFor",
                        &serde_json::json!({"query":self.query}),
                    )
                    .unwrap_or_else(|| format!("Search for {}", self.query))
                };
                add("search", label, Operation::Search);
                if parent.api().is_some() {
                    add(
                        "create",
                        format!("{}...", parent.tr("action.createPlaylist")),
                        Operation::Create,
                    );
                }
                add(
                    "navigation",
                    format!("{}...", parent.tr("page.globalSearch.commands.goToPage")),
                    Operation::Page(Page::Navigation),
                );
                add(
                    "servers",
                    format!(
                        "{}...",
                        parent.tr("page.globalSearch.commands.serverCommands")
                    ),
                    Operation::Page(Page::Servers),
                );
            }
            Page::Navigation => {
                for (id, label, route) in [
                    ("home", "Home", Route::Home),
                    (
                        "search",
                        "Search",
                        Route::Search(self.query.clone(), ItemKind::Song),
                    ),
                    ("settings", "Settings", Route::Settings),
                    ("albums", "Albums", Route::Library(ItemKind::Album)),
                    ("tracks", "Songs", Route::Library(ItemKind::Song)),
                    (
                        "artists",
                        "Album artists",
                        Route::Library(ItemKind::AlbumArtist),
                    ),
                    ("genres", "Genres", Route::Library(ItemKind::Genre)),
                    ("folders", "Folders", Route::Library(ItemKind::Folder)),
                    ("playlists", "Playlists", Route::Library(ItemKind::Playlist)),
                ] {
                    add(id, parent.tr(label), Operation::Navigate(route));
                }
            }
            Page::Servers => {
                for server in &parent.state.servers {
                    add(
                        &server.id.to_string(),
                        format!("{}...", server.name),
                        Operation::Server(server.id),
                    );
                }
                add(
                    "manage",
                    format!("{}...", parent.tr("Manage servers")),
                    Operation::Manage,
                );
            }
        }
        // The original search field is external to cmdk: it queries the library
        // without filtering the general commands on any palette page.
        if self.page == Page::Home {
            let mut results = vec![];
            for (index, section) in self.sections.iter().enumerate() {
                let key = SECTION_KEYS[index];
                results.push(Command {
                    id: format!("all-{key}"),
                    label: parent.tr(["Albums", "Album artists", "Songs"][index]),
                    subtitle: String::new(),
                    operation: Operation::All(index),
                    group: None,
                    disabled: false,
                });
                if parent
                    .state
                    .command_palette_sections
                    .get(key)
                    .copied()
                    .unwrap_or(true)
                {
                    for item in &section.items {
                        results.push(Command {
                            id: format!("{key}-{}", item.id),
                            label: item.name.clone(),
                            subtitle: if item.kind == ItemKind::AlbumArtist {
                                item.column_data
                                    .get("albumCount")
                                    .and_then(|count| count.parse::<usize>().ok())
                                    .and_then(|count| {
                                        feishin_core::i18n::translate_with(
                                            &parent.state.settings.language,
                                            "entity.albumWithCount",
                                            &serde_json::json!({"count":count}),
                                        )
                                    })
                                    .unwrap_or_default()
                            } else {
                                item.artist.clone()
                            },
                            operation: Operation::Item(item.clone()),
                            group: None,
                            disabled: false,
                        });
                    }
                    if section.more || section.error.is_some() {
                        results.push(Command {
                            id: format!("more-{key}"),
                            label: parent.tr(if section.error.is_some() {
                                "common.retry"
                            } else {
                                "View more"
                            }),
                            subtitle: section.error.clone().unwrap_or_default(),
                            operation: Operation::More(index),
                            group: None,
                            disabled: section.busy,
                        });
                    }
                }
            }
            results.extend(commands);
            commands = results;
        }
        commands
    }
    fn change_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.selected = None;
        self.hovered = None;
        self.scroll_target = None;
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.input.update(cx, |input, cx| input.focus(window, cx));
        if page == Page::Home && !self.query.is_empty() {
            for index in 0..3 {
                if !self.sections[index].fetched {
                    self.load_section(index, cx);
                }
            }
        }
        cx.notify();
    }

    fn activate(&mut self, operation: Operation, window: &mut Window, cx: &mut Context<Self>) {
        if let Operation::Page(page) = operation {
            self.change_page(page, window, cx);
            return;
        }
        if let Operation::More(index) = operation {
            self.load_section(index, cx);
            return;
        }
        if let Operation::Playlist(item) = operation {
            self.parent
                .update(cx, |parent, cx| {
                    parent.open_add_to_playlist(vec![item], vec![], window, cx)
                })
                .ok();
            return;
        }
        self.cancelled.store(true, Ordering::Relaxed);
        let query = self.query.clone();
        self.parent
            .update(cx, |parent, cx| {
                parent.command_palette = None;
                window.close_dialog(cx);
                match operation {
                    Operation::Search => parent.navigate(Route::Search(query, ItemKind::Song), cx),
                    Operation::Create => parent.edit_playlist(None, vec![], window, cx),
                    Operation::Navigate(route) => parent.navigate(route, cx),
                    Operation::Server(id) => {
                        if parent.sessions.contains_key(&id) {
                            parent.choose_server(id, cx);
                        } else {
                            parent.navigate(Route::Servers, cx);
                            parent.edit_server(Some(id), window, cx);
                        }
                    }
                    Operation::Manage => parent.navigate(Route::Servers, cx),
                    Operation::Item(item) => {
                        let item = if item.kind == ItemKind::Song && !item.album_id.is_empty() {
                            Item {
                                id: item.album_id,
                                name: item.album,
                                kind: ItemKind::Album,
                                server_id: item.server_id,
                                ..Default::default()
                            }
                        } else {
                            item
                        };
                        parent.navigate(Route::Detail(Box::new(item)), cx);
                    }
                    Operation::All(index) => {
                        parent.navigate(Route::Library(KINDS[index]), cx);
                        parent.local_filter = query.clone();
                        parent
                            .local_search
                            .update(cx, |input, cx| input.set_value(query, window, cx));
                        parent.reload(false, cx);
                    }
                    Operation::Page(_) | Operation::More(_) | Operation::Playlist(_) => {}
                }
                cx.notify();
            })
            .ok();
    }
    fn toggle_section(&mut self, index: usize, cx: &mut Context<Self>) {
        self.parent
            .update(cx, |parent, cx| {
                let expanded = parent
                    .state
                    .command_palette_sections
                    .entry(SECTION_KEYS[index].into())
                    .or_insert(true);
                *expanded = !*expanded;
                parent.save();
                cx.notify();
            })
            .ok();
        cx.notify();
    }
    fn move_selection(&mut self, movement: Movement, cx: &mut Context<Self>) {
        let commands = self.commands(cx);
        self.selected = selection(&commands, self.selected.as_deref(), movement);
        if let Some(index) = commands
            .iter()
            .position(|item| Some(&item.id) == self.selected.as_ref())
        {
            self.scroll_target = Some(commands[index].id.clone());
        }
        cx.notify();
    }
    fn key_down(&mut self, event: &KeystrokeEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if self
            .input
            .update(cx, |input, cx| input.marked_text_range(window, cx))
            .is_some()
        {
            return;
        }
        if matches!(key, "enter" | "space") {
            let action = self.action_focus.iter().find_map(|(id, handles)| {
                handles
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .map(|index| (id.clone(), index))
            });
            if let Some((id, index)) = action {
                if let Some(Command {
                    operation: Operation::Item(item),
                    ..
                }) = self
                    .commands(cx)
                    .into_iter()
                    .find(|command| command.id == id)
                    && item
                        .column_data
                        .get("albumCount")
                        .is_none_or(|count| count != "0")
                {
                    if index == 3 {
                        self.activate(Operation::Playlist(item), window, cx);
                    } else {
                        self.parent
                            .update(cx, |parent, cx| {
                                parent.enqueue(
                                    item,
                                    [Enqueue::Now, Enqueue::Next, Enqueue::End][index],
                                    cx,
                                )
                            })
                            .ok();
                    }
                }
                window.prevent_default();
                cx.stop_propagation();
                return;
            }
        }
        if key == "tab" && !modifiers.shift {
            let inside_actions = self
                .action_focus
                .values()
                .flatten()
                .any(|handle| handle.is_focused(window));
            if !inside_actions {
                let selected = self
                    .commands(cx)
                    .into_iter()
                    .find(|command| Some(&command.id) == self.selected.as_ref());
                if let Some(Command {
                    id,
                    operation: Operation::Item(item),
                    ..
                }) = selected
                {
                    let first = if item
                        .column_data
                        .get("albumCount")
                        .is_some_and(|count| count == "0")
                    {
                        3
                    } else {
                        0
                    };
                    if let Some(handles) = self.action_focus.get(&id) {
                        window.focus(&handles[first]);
                        window.prevent_default();
                        cx.stop_propagation();
                    }
                }
            }
            return;
        }
        let Some(movement) = Movement::from_key(key, modifiers) else {
            return;
        };
        if matches!(key, "up" | "down") {
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
        self.move_selection(movement, cx);
        window.prevent_default();
        cx.stop_propagation();
    }
}

impl Palette {
    fn divider(&self, cx: &App) -> Div {
        div()
            .h(px(1.))
            .my(rems(0.5))
            .flex_shrink_0()
            .bg(cx.theme().border)
    }

    fn search_input(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        // The source uses a small TextInput with overlaid 34px left/right sections.
        div()
            .relative()
            .h(rems(2.25))
            .flex_shrink_0()
            .debug_selector(|| "palette-input".into())
            .child(
                crate::styles::input(&self.input, cx)
                    .h(rems(2.25))
                    .py_0()
                    .pl(rems(2.125))
                    .pr(rems(if self.query.is_empty() { 0.75 } else { 2.125 }))
                    .text_size(rems(0.8125))
                    .rounded(rems(0.1875))
                    .bg(cx.theme().secondary)
                    .text_color(cx.theme().secondary_foreground),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .bottom_0()
                    .w(rems(2.125))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        Icon::default()
                            .path("native-icons/search.svg")
                            .with_size(window.rem_size()),
                    ),
            )
            .when(!self.query.is_empty(), |input| {
                input.child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .w(rems(2.125))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            gpui_component::button::Button::new("palette-clear")
                                .ghost()
                                .p_0()
                                .with_size(window.rem_size() * 1.75)
                                .size(rems(1.75))
                                .icon(
                                    Icon::default()
                                        .path("native-icons/close.svg")
                                        .with_size(window.rem_size()),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.input.update(cx, |input, cx| {
                                        input.set_value("", window, cx);
                                        input.focus(window, cx);
                                    });
                                })),
                        ),
                )
            })
    }

    fn results(
        &self,
        commands: Vec<Command>,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mut list = div()
            .id("palette-results")
            .debug_selector(|| "palette-results".into())
            .role(Role::ListBox)
            .aria_label("Global Command Menu")
            .v_flex()
            .flex_shrink_0();
        let mut previous_group = None;
        for (index, command) in commands.into_iter().enumerate() {
            let heading = if command.group != previous_group {
                previous_group = command.group;
                match (self.page, command.group) {
                    (Page::Home, Some(_)) => {
                        Some(crate::appearance::text("page.globalSearch.title", cx))
                    }
                    (Page::Navigation, Some(1)) => Some("Library".into()),
                    (Page::Servers, Some(0)) => {
                        Some(crate::appearance::text("page.appMenu.selectServer", cx))
                    }
                    (Page::Servers, Some(1)) => Some(crate::appearance::text("common.manage", cx)),
                    _ => None,
                }
            } else {
                None
            };
            let heading_height = if heading.is_some() {
                window.rem_size() * 2.625
            } else {
                px(0.)
            };
            if let Some(heading) = heading {
                list = list.child(
                    div()
                        .my(rems(0.75))
                        .text_size(rems(0.8125))
                        .opacity(0.8)
                        .debug_selector(move || format!("palette-group-{index}"))
                        .child(heading),
                );
            }
            if let Operation::All(section) = command.operation {
                list = list.child(self.section_header(section, command.label, window, cx));
            } else {
                list = list.child(self.result_row(index, command, heading_height, window, cx));
            }
        }
        list.when(self.page != Page::Home, |list| {
            list.child(div().h(px(1.)).bg(cx.theme().border))
        })
    }

    fn section_header(
        &self,
        section: usize,
        label: String,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let expanded = self.parent.upgrade().is_none_or(|parent| {
            parent
                .read(cx)
                .state
                .command_palette_sections
                .get(SECTION_KEYS[section])
                .copied()
                .unwrap_or(true)
        });
        let mut heading = div()
            .id(("palette-heading", section))
            .role(Role::Button)
            .aria_label(label.clone())
            .track_focus(&self.heading_focus[section])
            .tab_index(0)
            .debug_selector(move || format!("palette-all-{}", SECTION_KEYS[section]))
            .h_flex()
            .gap(rems(0.25))
            .items_center()
            .text_size(rems(0.8125))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_section(section, cx)))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    this.toggle_section(section, cx);
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .size(rems(1.))
                    .flex_shrink_0()
                    .opacity(0.9)
                    .debug_selector(move || format!("palette-collapse-{}", SECTION_KEYS[section]))
                    .child(
                        Icon::default()
                            .path(if expanded {
                                "native-icons/arrow-down.svg"
                            } else {
                                "native-icons/arrow-right.svg"
                            })
                            .with_size(window.rem_size()),
                    ),
            )
            .child(div().flex_1().child(label));
        if self.sections[section].fetched && !self.query.is_empty() {
            let count = format!(
                "{}{}",
                self.sections[section].items.len(),
                if self.sections[section].more { "+" } else { "" }
            );
            let language = self
                .parent
                .upgrade()
                .map(|parent| parent.read(cx).state.settings.language.clone())
                .unwrap_or_default();
            let label = feishin_core::i18n::translate_with(
                &language,
                "common.numberOfResults",
                &serde_json::json!({"numberOfResults":count}),
            )
            .unwrap_or(count);
            heading = heading.child(
                crate::styles::button(("palette-count", section), cx)
                    .debug_selector(move || format!("palette-count-{}", SECTION_KEYS[section]))
                    .primary()
                    .xsmall()
                    .compact()
                    .w(rems(8.))
                    .label(label)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.activate(Operation::All(section), window, cx);
                    })),
            );
        }
        div()
            .when(section > 0, |row| row.mt(rems(0.25)))
            .p(rems(0.5))
            .border_1()
            .border_color(cx.theme().border)
            .rounded(rems(0.1875))
            .bg(cx.theme().secondary)
            .text_color(cx.theme().secondary_foreground)
            .child(heading)
    }

    fn result_row(
        &self,
        index: usize,
        command: Command,
        heading_height: Pixels,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let id = command.id.clone();
        let hover_id = id.clone();
        let pointer_id = id.clone();
        let selectable = command.selectable();
        let selected = self.selected.as_ref() == Some(&id);
        let show_actions = selected || self.hovered.as_ref() == Some(&id);
        let operation = command.operation.clone();
        let mut row = div()
            .id(("palette-row", index))
            .debug_selector(move || format!("palette-{id}"))
            .role(Role::ListBoxOption)
            .track_focus(&self.row_focus[&command.id])
            .aria_label(command.label.clone())
            .aria_selected(selected)
            .aria_disabled(command.disabled)
            .relative()
            .h_flex()
            .items_center()
            .gap(rems(0.5))
            .p(rems(0.5))
            .rounded(rems(0.1875))
            .text_color(cx.theme().foreground)
            .cursor_pointer()
            .when(selected, |row| row.bg(cx.theme().secondary))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hovered = Some(hover_id.clone());
                } else if this.hovered.as_ref() == Some(&hover_id) {
                    this.hovered = None;
                }
                cx.notify();
            }))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if selectable && this.selected.as_ref() != Some(&pointer_id) {
                    this.selected = Some(pointer_id.clone());
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                if selectable {
                    this.activate(operation.clone(), window, cx);
                }
            }));
        if let Operation::Item(item) = command.operation {
            let zoom = cx
                .try_global::<crate::styles::NativeStyles>()
                .map_or(1., |styles| styles.zoom());
            let metadata = div()
                .letter_spacing(px(0.5 * zoom))
                .flex_1()
                .min_w_0()
                .h_flex()
                .gap(rems(0.5))
                .items_center()
                .child(
                    div()
                        .size(px(40. * zoom))
                        .flex_shrink_0()
                        .rounded(px(4. * zoom))
                        .overflow_hidden()
                        .debug_selector(move || format!("palette-artwork-{index}"))
                        .children(
                            self.parent
                                .upgrade()
                                .map(|parent| parent.read(cx).artwork(&item, 40. * zoom, cx)),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .v_flex()
                        .justify_center()
                        .child(
                            div()
                                .debug_selector(move || format!("palette-title-{index}"))
                                .truncate()
                                .child(command.label),
                        )
                        .child(
                            div()
                                .debug_selector(move || format!("palette-subtitle-{index}"))
                                .text_size(rems(0.8125))
                                .line_height(rems(1.))
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(command.subtitle),
                        ),
                );
            let mut content = div()
                .h(px(40. * zoom))
                .w_full()
                .h_flex()
                .items_start()
                .gap(rems(1.5))
                .child(metadata);
            if show_actions {
                let handles = self
                    .action_focus
                    .get(&command.id)
                    .expect("Library command focus handles");
                let mut controls = div()
                    .h(px(40. * zoom))
                    .h_flex()
                    .items_center()
                    .flex_shrink_0();
                for (offset, icon, mode) in [
                    (0, "play", Enqueue::Now),
                    (1, "play-next", Enqueue::Next),
                    (2, "play-last", Enqueue::End),
                ] {
                    let item = item.clone();
                    let held = item.clone();
                    controls = controls.child(
                        self.inline_action(index, offset, icon, window, cx)
                            .debug_selector(move || format!("palette-play-{index}-{offset}"))
                            .focus_handle(&handles[offset])
                            .disabled(
                                item.column_data
                                    .get("albumCount")
                                    .is_some_and(|count| count == "0"),
                            )
                            .tooltip(crate::appearance::text(
                                match mode {
                                    Enqueue::Now => "Play now",
                                    Enqueue::Next => "Play next",
                                    _ => "Add last",
                                },
                                cx,
                            ))
                            .on_long_press(cx.listener(move |this, _, _, cx| {
                                this.parent
                                    .update(cx, |parent, cx| {
                                        parent.enqueue_items(vec![held.clone()], mode, true, cx)
                                    })
                                    .ok();
                                cx.stop_propagation();
                            }))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.parent
                                    .update(cx, |parent, cx| parent.enqueue(item.clone(), mode, cx))
                                    .ok();
                            })),
                    );
                }
                controls = controls.child(
                    self.inline_action(index, 3, "playlist-add", window, cx)
                        .debug_selector(move || format!("palette-playlist-{index}"))
                        .focus_handle(&handles[3])
                        .tooltip(crate::appearance::text("Add to playlist", cx))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.activate(Operation::Playlist(item.clone()), window, cx);
                        })),
                );
                content = content.child(controls);
            }
            row = row.child(content);
        } else {
            row = row.child(command.label);
        }
        if self.scroll_target.as_ref() == Some(&command.id) {
            let handle = self.scroll.clone();
            let id = command.id;
            let view = cx.weak_entity();
            row = row.child(
                canvas(
                    move |bounds, window, cx| {
                        window.defer(cx, move |_, cx| {
                            view.update(cx, |view, cx| {
                                if view.scroll_target.as_ref() != Some(&id) {
                                    return;
                                }
                                let viewport = handle.bounds();
                                let top = (bounds.top() - heading_height)
                                    .max(bounds.bottom() - viewport.size.height);
                                let delta = if top < viewport.top() {
                                    viewport.top() - top
                                } else if bounds.bottom() > viewport.bottom() {
                                    viewport.bottom() - bounds.bottom()
                                } else {
                                    px(0.)
                                };
                                handle.set_offset(handle.offset() + point(px(0.), delta));
                                view.scroll_target = None;
                                cx.notify();
                            })
                            .ok();
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        }
        row
    }

    fn inline_action(
        &self,
        index: usize,
        offset: usize,
        icon: &'static str,
        window: &Window,
        cx: &App,
    ) -> gpui_component::button::Button {
        use gpui_component::button::{Button, ButtonCustomVariant};
        let background = cx.theme().secondary;
        let rgb = background.to_rgb();
        let hover = Rgba {
            r: rgb.r + (1. - rgb.r) * 0.05,
            g: rgb.g + (1. - rgb.g) * 0.05,
            b: rgb.b + (1. - rgb.b) * 0.05,
            a: rgb.a,
        }
        .into();
        Button::new(("palette-action", index * 4 + offset))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(background)
                    .foreground(cx.theme().foreground)
                    .hover(hover)
                    .active(hover),
            )
            .action_icon(
                cx.try_global::<crate::styles::NativeStyles>()
                    .map_or(1., |styles| styles.zoom()),
            )
            .size(rems(1.875))
            .with_size(window.rem_size() * 1.875)
            .p_0()
            .border_1()
            .border_color(transparent_black())
            .rounded(px(0.))
            .when(offset == 0, |button| button.rounded_l(rems(0.1875)))
            .when(offset == 3, |button| button.rounded_r(rems(0.1875)))
            .child(
                Icon::default()
                    .path(format!("native-icons/{icon}.svg"))
                    .with_size(window.rem_size() * 0.8125),
            )
    }

    fn footer(&self, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        let mut breadcrumbs = div()
            .h_flex()
            .items_center()
            .debug_selector(|| "palette-breadcrumbs".into())
            .child(
                crate::styles::button("palette-home", cx)
                    .ghost()
                    .xsmall()
                    .compact()
                    .label("HOME")
                    .on_click(
                        cx.listener(|this, _, window, cx| this.change_page(Page::Home, window, cx)),
                    ),
            );
        if self.page != Page::Home {
            breadcrumbs = breadcrumbs
                .child(
                    div().mx(rems(0.25)).child(
                        Icon::default()
                            .path("native-icons/arrow-forward.svg")
                            .with_size(window.rem_size() * 0.875),
                    ),
                )
                .child(
                    crate::styles::button("palette-page", cx)
                        .ghost()
                        .xsmall()
                        .compact()
                        .label(if self.page == Page::Navigation {
                            "GO"
                        } else {
                            "SERVERS"
                        }),
                );
        }
        div()
            .h_flex()
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .child(breadcrumbs)
            .child(
                div().h_flex().gap(rems(0.5)).children(
                    ["ESC", "↑", "↓", "⏎"]
                        .into_iter()
                        .enumerate()
                        .map(|(index, label)| {
                            div()
                                .debug_selector(move || format!("palette-kbd-{index}"))
                                .font_family(cx.theme().mono_font_family.clone())
                                .font_weight(FontWeight::BOLD)
                                .text_size(rems(0.875))
                                .line_height(rems(1.125))
                                .py(rems(0.105))
                                .px(rems(0.39375))
                                .border_1()
                                .border_b(window.rem_size() * 0.1875)
                                .border_color(if cx.theme().is_dark() {
                                    rgb(0x424242)
                                } else {
                                    rgb(0xdee2e6)
                                })
                                .bg(if cx.theme().is_dark() {
                                    rgb(0x242424)
                                } else {
                                    rgb(0xf8f9fa)
                                })
                                .text_color(if cx.theme().is_dark() {
                                    rgb(0xc9c9c9)
                                } else {
                                    rgb(0x495057)
                                })
                                .rounded(rems(0.1875))
                                .child(label)
                        }),
                ),
            )
    }
}

impl Render for Palette {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let commands = self.commands(cx);
        if !commands.iter().any(|command| {
            !matches!(command.operation, Operation::All(_))
                && Some(&command.id) == self.selected.as_ref()
        }) {
            self.selected = selection(&commands, None, Movement::First);
        }
        if self.row_focus.iter().any(|(id, handle)| {
            handle.is_focused(window) && !commands.iter().any(|command| &command.id == id)
        }) || self.action_focus.iter().any(|(id, handles)| {
            handles.iter().any(|handle| handle.is_focused(window))
                && (!commands.iter().any(|command| &command.id == id)
                    || (self.selected.as_ref() != Some(id) && self.hovered.as_ref() != Some(id)))
        }) {
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
        self.action_focus
            .retain(|id, _| commands.iter().any(|command| &command.id == id));
        self.row_focus
            .retain(|id, _| commands.iter().any(|command| &command.id == id));
        for command in &commands {
            if !matches!(command.operation, Operation::All(_)) {
                self.row_focus
                    .entry(command.id.clone())
                    .or_insert_with(|| cx.focus_handle().tab_index(-1).tab_stop(false));
            }
            if matches!(command.operation, Operation::Item(_)) {
                self.action_focus
                    .entry(command.id.clone())
                    .or_insert_with(|| std::array::from_fn(|_| cx.focus_handle()));
            }
        }
        div()
            .id("command-palette")
            .debug_selector(|| "command-palette".into())
            .key_context("CommandPalette")
            .v_flex()
            .w_full()
            .flex_shrink_0()
            .text_size(rems(0.875))
            .line_height(rems(1.125))
            .font_weight(FontWeight::NORMAL)
            .capture_key_up(cx.listener(|this, event: &KeyUpEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && this
                        .action_focus
                        .values()
                        .flatten()
                        .any(|handle| handle.is_focused(window))
                {
                    // The source inline handlers activate on key down, suppressing the browser click.
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &Choose, window, cx| {
                if this
                    .input
                    .update(cx, |input, cx| input.marked_text_range(window, cx))
                    .is_some()
                {
                    cx.propagate();
                    return;
                }
                if let Some(command) = this.commands(cx).into_iter().find(|command| {
                    command.selectable() && Some(&command.id) == this.selected.as_ref()
                }) {
                    this.activate(command.operation.clone(), window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Back, window, cx| {
                if this.page != Page::Home {
                    this.change_page(Page::Home, window, cx);
                } else {
                    this.cancelled.store(true, Ordering::Relaxed);
                    this.parent
                        .update(cx, |parent, cx| {
                            parent.command_palette = None;
                            window.close_dialog(cx);
                            cx.notify();
                        })
                        .ok();
                }
            }))
            .child(self.search_input(window, cx))
            .child(self.divider(cx))
            .child(self.results(commands, window, cx))
            .child(self.divider(cx))
            .child(self.footer(window, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::Page;
    use crate::app::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{Item, Server, ServerType, Session},
        queue::Enqueue,
        storage,
    };
    use gpui::{AppContext, Focusable, Modifiers, TestAppContext};
    use gpui_component::Root;
    use gpui_component::WindowExt;
    use serde_json::json;
    use std::{
        cell::RefCell,
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::Duration,
    };

    #[test]
    fn command_navigation_matches_original_sequences() {
        use super::{Command, Movement, Operation, selection};
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../assets/command-keyboard-reference.json"))
                .unwrap();
        for case in cases.as_array().unwrap() {
            let disabled = case["config"]["disabled"].as_array().unwrap();
            let commands = case["config"]["groups"]
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .flat_map(|(group, items)| {
                    items.as_array().unwrap().iter().map(move |item| Command {
                        id: item.to_string(),
                        label: String::new(),
                        subtitle: String::new(),
                        operation: Operation::Search,
                        group: Some(group),
                        disabled: disabled.contains(item),
                    })
                })
                .collect::<Vec<_>>();
            let mut selected = selection(&commands, None, Movement::First);
            assert_eq!(selected.as_deref(), case["initial"]["selected"].as_str());
            for step in case["steps"].as_array().unwrap() {
                let operation = &step["operation"];
                if let Some(id) = operation["pointer"].as_str() {
                    if commands
                        .iter()
                        .any(|command| command.id == id && command.selectable())
                    {
                        selected = Some(id.into());
                    }
                } else {
                    let key = match operation["key"].as_str().unwrap() {
                        "ArrowUp" => "up",
                        "ArrowDown" => "down",
                        "Home" => "home",
                        "End" => "end",
                        other => other,
                    };
                    if let Some(movement) = Movement::from_key(
                        key,
                        Modifiers {
                            control: operation["ctrlKey"].as_bool().unwrap_or(false),
                            alt: operation["altKey"].as_bool().unwrap_or(false),
                            platform: operation["metaKey"].as_bool().unwrap_or(false),
                            ..Default::default()
                        },
                    ) {
                        selected = selection(&commands, selected.as_deref(), movement);
                    }
                }
                assert_eq!(
                    selected.as_deref(),
                    step["selected"].as_str(),
                    "{operation}"
                );
            }
        }
    }

    #[gpui::test]
    fn navigation_keeps_shortcuts_after_a_focused_search_is_removed(cx: &mut TestAppContext) {
        use feishin_core::model::ItemKind;
        cx.update(gpui_component::init);
        let directory =
            std::env::temp_dir().join(format!("feishin-navigation-focus-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = storage::State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let app = cx.new(|cx| Feishin::new(directory.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        for destination in [
            Route::Home,
            Route::Library(ItemKind::Album),
            Route::Library(ItemKind::Song),
        ] {
            cx.update(|_, cx| {
                app.update(cx, |view, cx| {
                    view.navigate(Route::Search(String::new(), ItemKind::Song), cx);
                })
            });
            cx.run_until_parked();
            cx.update(|window, cx| {
                app.update(cx, |view, cx| {
                    view.search.update(cx, |input, cx| input.focus(window, cx));
                })
            });
            cx.run_until_parked();
            cx.update(|_, cx| app.update(cx, |view, cx| view.navigate(destination.clone(), cx)));
            cx.run_until_parked();
            cx.simulate_keystrokes("secondary-k");
            cx.run_until_parked();
            assert!(
                app.read_with(cx, |view, _| view.command_palette.is_some()),
                "{destination:?}"
            );
            cx.update(|window, cx| {
                app.update(cx, |view, cx| view.close_command_palette(window, cx))
            });
            cx.run_until_parked();
        }
        let _ = std::fs::remove_dir_all(directory);
    }

    #[gpui::test]
    fn command_palette_keyboard_debounce_mouse_paging_collapse_queue_and_server_actions(
        cx: &mut TestAppContext,
    ) {
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        cx.update(gpui_component::init);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Music server".into(),
            url: format!("http://{}", listener.local_addr().unwrap()),
            username: "fixture".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let unsigned = Server {
            id: uuid::Uuid::new_v4(),
            name: "Unsigned".into(),
            ..server.clone()
        };
        let done = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(vec![]));
        let stop = done.clone();
        let calls = requests.clone();
        let worker = thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(2));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap();
                let (resource_path, parameters) = path.split_once('?').unwrap_or((path, ""));
                let query = parameters
                    .split('&')
                    .filter_map(|pair| pair.split_once('='))
                    .collect::<std::collections::HashMap<_, _>>();
                let offset = query
                    .get("_start")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                let limit = query
                    .get("_end")
                    .and_then(|value| value.parse::<usize>().ok())
                    .map_or(6, |end| end.saturating_sub(offset));
                let resource = resource_path.strip_prefix("/api/").unwrap_or("");
                let response = if matches!(resource, "album" | "artist" | "song") {
                    calls.lock().unwrap().push(path.to_string());
                    serde_json::Value::Array((offset..(offset + limit).min(6)).map(|index| json!({
                        "id":format!("{resource}-{index}"), "name":format!("Music {resource} {index}"),
                        "title":format!("Music {resource} {index}"), "artist":"Music artist", "albumArtist":"Music artist",
                        "album":"Music album", "albumId":"album-0", "artistId":"artist-0", "albumCount":1
                    })).collect())
                } else if resource == "playlist" {
                    json!([])
                } else {
                    json!({"id":"album-0","name":"Music album", "subsonic-response":{"status":"ok"}})
                };
                let body = response.to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Total-Count: 6\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
            }
        });
        let dir =
            std::env::temp_dir().join(format!("feishin-command-palette-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = storage::State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.sidebar.items.clear();
            state.settings.confirm_large_fetch = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.run_until_parked();
        cx.update(|window, cx| {
            app.update(cx, |view, cx| {
                view.state.servers = vec![server.clone(), unsigned.clone()];
                view.state.active_server = Some(server.id);
                view.sessions.insert(
                    server.id,
                    Api::new(
                        server.clone(),
                        Session {
                            token: "fixture-token".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                view.route = Route::Home;
                let song = Item {
                    id: "existing".into(),
                    server_id: server.id,
                    ..Default::default()
                };
                view.state.queue.add(vec![song.clone(), song], Enqueue::Now);
                view.state.queue.position = 24.;
                window.focus(&view.sidebar_search_focus);
                cx.notify();
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("secondary-k");
        cx.run_until_parked();
        assert!(app.read_with(cx, |view, _| view.command_palette.is_some()));
        cx.executor().advance_clock(Duration::from_millis(310));
        cx.refresh().unwrap();
        cx.run_until_parked();
        let palette = app.read_with(cx, |view, _| view.command_palette.clone().unwrap());
        cx.update(|window, cx| {
            palette.read(cx).input.clone().update(cx, |input, cx| {
                gpui::EntityInputHandler::replace_and_mark_text_in_range(
                    input, None, "m", None, window, cx,
                );
            });
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(
            app.read_with(cx, |view, _| view.command_palette.is_some()),
            "Composition Enter must not select a command"
        );
        cx.update(|window, cx| {
            palette.read(cx).input.clone().update(cx, |input, cx| {
                gpui::EntityInputHandler::unmark_text(input, window, cx);
                input.set_value("", window, cx);
            });
        });
        cx.run_until_parked();
        let ids = palette.read_with(cx, |view, cx| {
            view.commands(cx)
                .iter()
                .filter(|command| command.selectable())
                .take(2)
                .map(|command| format!("palette-{}", command.id))
                .collect::<Vec<_>>()
        });
        for selected in [0, 1] {
            for (index, id) in ids.iter().enumerate() {
                let bounds = cx.debug_bounds(id).unwrap();
                let highlighted = cx
                    .update(|window, _| window.painted_quad_opacities_for_test())
                    .iter()
                    .any(|(paint, color)| *paint == bounds && color.a > 0.);
                assert_eq!(
                    highlighted,
                    index == selected,
                    "Keyboard selection must remain visible: {id}"
                );
            }
            let nodes =
                cx.update(|window, _| window.accessibility_tree_for_test().unwrap().nodes.clone());
            let options = nodes
                .iter()
                .filter(|(_, node)| node.role() == gpui::Role::ListBoxOption)
                .collect::<Vec<_>>();
            assert_eq!(options.len(), 4);
            assert_eq!(
                options
                    .iter()
                    .filter(|(_, node)| node.is_selected() == Some(true))
                    .count(),
                1
            );
            cx.simulate_keystrokes(if selected == 0 { "down" } else { "up" });
            cx.run_until_parked();
        }
        cx.simulate_keystrokes("down down enter");
        cx.run_until_parked();
        let palette = app.read_with(cx, |view, _| view.command_palette.clone().unwrap());
        assert!(
            palette.read_with(cx, |view, _| view.page == Page::Navigation),
            "selection and commands: {:?}; query focus: {}",
            palette.read_with(cx, |view, cx| (
                view.selected.clone(),
                view.commands(cx)
                    .iter()
                    .map(|command| command.id.clone())
                    .collect::<Vec<_>>()
            )),
            cx.update(|window, cx| palette.read(cx).input.focus_handle(cx).is_focused(window))
        );
        for (keys, id) in [
            ("up", "home"),
            ("end", "playlists"),
            ("down", "playlists"),
            ("home", "home"),
            ("ctrl-j", "search"),
            ("ctrl-n", "settings"),
            ("ctrl-p", "search"),
            ("ctrl-k", "home"),
            ("alt-down", "albums"),
            ("alt-up", "home"),
        ] {
            cx.simulate_keystrokes(keys);
            cx.run_until_parked();
            assert_eq!(
                palette.read_with(cx, |view, _| view.selected.clone()),
                Some(id.into()),
                "{keys}"
            );
        }
        cx.simulate_keystrokes("escape");
        assert!(palette.read_with(cx, |view, _| view.page == Page::Home));
        let input = palette.read_with(cx, |view, _| view.input.clone());
        let home: serde_json::Value =
            serde_json::from_str(include_str!("../assets/command-home-reference.json")).unwrap();
        for case in home.as_array().unwrap() {
            cx.update(|window, cx| {
                input.update(cx, |input, cx| {
                    input.set_value(case["query"].as_str().unwrap().to_owned(), window, cx)
                })
            });
            cx.run_until_parked();
            palette.read_with(cx, |view, cx| {
                let commands = view.commands(cx);
                assert_eq!(
                    commands
                        .iter()
                        .filter(|command| command.group.is_some())
                        .count(),
                    case["items"].as_array().unwrap().len()
                );
                assert_eq!(
                    commands
                        .iter()
                        .filter(|command| command.group.is_some())
                        .map(|command| command.id.as_str())
                        .collect::<Vec<_>>(),
                    ["search", "create", "navigation", "servers"]
                );
            });
        }
        for query in ["stale", "music"] {
            cx.update(|window, cx| {
                input.update(cx, |input, cx| input.set_value(query, window, cx))
            });
        }
        cx.run_until_parked();
        assert!(requests.lock().unwrap().is_empty());
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        palette.read_with(cx, |view, _| {
            assert!(view.sections.iter().all(|section| section.items.len() == 4
                && section.more
                && !section.busy
                && section.error.is_none()));
        });
        assert_eq!(
            palette
                .read_with(cx, |view, _| view.selected.clone())
                .as_deref(),
            Some("search"),
            "New results must not change the selected command"
        );
        let calls = requests.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert!(
            calls
                .iter()
                .all(|call| call.contains("music") && !call.contains("stale"))
        );
        cx.simulate_keystrokes("end up enter");
        cx.run_until_parked();
        palette.read_with(cx, |view, cx| {
            assert!(view.page == Page::Navigation);
            assert_eq!(view.query, "music");
            assert_eq!(view.commands(cx).len(), 9);
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(palette.read_with(cx, |view, _| view.query.clone()), "music");
        assert_eq!(
            requests.lock().unwrap().len(),
            3,
            "Back should retain loaded results"
        );
        let collapsed = cx.debug_bounds("palette-all-albums").unwrap().center();
        cx.simulate_click(collapsed, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            storage::load(&dir.join("state.json"))
                .unwrap()
                .command_palette_sections
                .get("albums"),
            Some(&false)
        );
        assert!(!palette.read_with(cx, |view, cx| {
            view.commands(cx)
                .iter()
                .any(|command| command.id == "albums-album-0")
        }));
        let point = cx.debug_bounds("palette-collapse-albums").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, cx| {
            palette.update(cx, |view, cx| {
                view.selected = Some("more-albums".into());
                view.sections[0].busy = true;
                view.input.update(cx, |input, cx| input.focus(window, cx));
                cx.notify();
            })
        });
        cx.run_until_parked();
        assert_eq!(
            palette
                .read_with(cx, |view, _| view.selected.clone())
                .as_deref(),
            Some("more-albums")
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            requests.lock().unwrap().len(),
            3,
            "A loading command must not activate again"
        );
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        assert_eq!(
            palette
                .read_with(cx, |view, _| view.selected.clone())
                .as_deref(),
            Some("albums-album-0")
        );
        cx.update(|_, cx| {
            palette.update(cx, |view, cx| {
                view.sections[0].busy = false;
                cx.notify();
            })
        });
        let more = palette.read_with(cx, |view, cx| {
            view.commands(cx)
                .iter()
                .position(|command| command.id == "more-albums")
                .unwrap()
        });
        cx.update(|_, cx| {
            palette.update(cx, |view, cx| {
                view.scroll_target = Some(view.commands(cx)[more].id.clone());
                cx.notify();
            })
        });
        cx.run_until_parked();
        let point = cx.debug_bounds("palette-more-albums").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        assert!(
            palette.read_with(cx, |view, _| view.sections[0].items.len() == 6
                && !view.sections[0].more)
        );
        assert!(
            requests
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .contains("_start=4")
        );
        let song_index = palette.read_with(cx, |view, cx| {
            view.commands(cx)
                .iter()
                .position(|command| command.id == "tracks-song-0")
                .unwrap()
        });
        cx.update(|_, cx| {
            palette.update(cx, |view, cx| {
                view.scroll_target = Some(view.commands(cx)[song_index].id.clone());
                cx.notify();
            })
        });
        cx.run_until_parked();
        let song_point = cx.debug_bounds("palette-tracks-song-0").unwrap().center();
        cx.simulate_mouse_move(song_point, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            palette
                .read_with(cx, |view, _| view.selected.clone())
                .as_deref(),
            Some("tracks-song-0")
        );
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert!(cx.update(|window, cx| {
            palette.read(cx).action_focus["tracks-song-0"][0].is_focused(window)
        }));
        cx.simulate_keystrokes("tab");
        cx.run_until_parked();
        assert!(cx.update(|window, cx| {
            palette.read(cx).action_focus["tracks-song-0"][1].is_focused(window)
        }));
        cx.simulate_keystrokes("shift-tab");
        cx.run_until_parked();
        assert!(cx.update(|window, cx| {
            palette.read(cx).action_focus["tracks-song-0"][0].is_focused(window)
        }));
        cx.simulate_keystrokes("down");
        cx.run_until_parked();
        assert!(cx.update(|window, cx| palette.read(cx).input.focus_handle(cx).is_focused(window)));
        cx.update(|_, cx| {
            palette.update(cx, |view, cx| {
                view.scroll_target = Some(view.commands(cx)[song_index].id.clone());
                cx.notify();
            })
        });
        cx.run_until_parked();
        let song_point = cx.debug_bounds("palette-tracks-song-0").unwrap().center();
        cx.simulate_mouse_move(song_point, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            palette
                .read_with(cx, |view, _| view.selected.clone())
                .as_deref(),
            Some("tracks-song-0")
        );
        let before = app.read_with(cx, |view, _| view.state.queue.order.clone());
        let add = cx
            .debug_bounds(Box::leak(
                format!("palette-play-{song_index}-2").into_boxed_str(),
            ))
            .unwrap()
            .center();
        cx.simulate_click(add, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.state.queue.order[..2], before);
            assert_eq!(view.state.queue.entries.len(), 3);
            assert_eq!(view.state.queue.entries.last().unwrap().item.id, "song-0");
            assert_eq!(view.state.queue.position, 24.);
            assert!(!view.playing);
            assert!(view.command_palette.is_some());
        });
        // Enter and Space activate only the focused inline action, preserving the palette.
        for (key, expected) in [("enter", 4), ("space", 5)] {
            cx.update(|window, cx| {
                window.focus(&palette.read(cx).action_focus["tracks-song-0"][2])
            });
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            assert_eq!(
                app.read_with(cx, |view, _| view.state.queue.entries.len()),
                expected,
                "{key} must activate on key down"
            );
            cx.simulate_event(gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse(key).unwrap(),
            });
            cx.run_until_parked();
            app.read_with(cx, |view, _| {
                assert_eq!(view.state.queue.entries.len(), expected, "{key}");
                assert!(view.command_palette.is_some());
                assert!(!view.playing);
                assert_eq!(view.state.queue.position, 24.);
            });
        }
        cx.simulate_mouse_down(add, gpui::MouseButton::Left, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(499));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            5
        );
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            6
        );
        cx.simulate_mouse_up(add, gpui::MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            app.read_with(cx, |view, _| view.state.queue.entries.len()),
            6,
            "A held action must not also click on release"
        );
        cx.update(|window, cx| window.focus(&palette.read(cx).action_focus["tracks-song-0"][3]));
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(view.add_to_playlist.is_some());
            assert!(
                view.command_palette.is_some(),
                "The playlist picker must preserve the search underneath"
            );
        });
        cx.executor().advance_clock(Duration::from_millis(310));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(310));
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(view.add_to_playlist.is_none());
            assert!(view.command_palette.is_some());
        });
        assert!(
            cx.update(
                |window, cx| palette.read(cx).action_focus["tracks-song-0"][3].is_focused(window)
            ),
            "Closing the nested picker must restore its initiating control"
        );
        assert_eq!(palette.read_with(cx, |view, _| view.query.clone()), "music");
        cx.update(|window, cx| {
            input.update(cx, |input, cx| input.set_value("cancelled", window, cx))
        });
        cx.run_until_parked();
        let count = requests.lock().unwrap().len();
        cx.simulate_keystrokes("escape");
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        assert_eq!(requests.lock().unwrap().len(), count);
        assert!(app.read_with(cx, |view, _| view.command_palette.is_none()));
        assert!(cx.update(|window, cx| !window.has_active_dialog(cx)));
        cx.update(|window, cx| app.update(cx, |view, cx| view.open_command_palette(window, cx)));
        cx.run_until_parked();
        let point = cx.debug_bounds("palette-servers").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        let point = cx
            .debug_bounds(Box::leak(
                format!("palette-{}", unsigned.id).into_boxed_str(),
            ))
            .unwrap()
            .center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(view.route, Route::Servers);
            assert_eq!(view.editing_server, Some(unsigned.id));
            assert!(view.command_palette.is_none());
            assert!(view.sessions.contains_key(&server.id));
        });
        cx.update(|window, cx| app.update(cx, |view, cx| view.open_command_palette(window, cx)));
        cx.run_until_parked();
        let point = cx.debug_bounds("palette-create").unwrap().center();
        cx.simulate_click(point, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(
                view.route == Route::PlaylistEditor
                    && view.form_target.is_none()
                    && view.command_palette.is_none(),
                "Create click at {point:?}: route {:?}, target {:?}, palette open {}",
                view.route,
                view.form_target.as_ref().map(|item| &item.id),
                view.command_palette.is_some()
            );
        });
        cx.update(|window, cx| app.update(cx, |view, cx| view.open_command_palette(window, cx)));
        cx.run_until_parked();
        cx.simulate_keystrokes("home enter");
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.route,
                Route::Search(String::new(), feishin_core::model::ItemKind::Song)
            );
            assert!(view.command_palette.is_none());
        });
        cx.update(|window, cx| {
            app.update(cx, |view, cx| view.open_command_palette(window, cx));
            let input = app
                .read(cx)
                .command_palette
                .as_ref()
                .unwrap()
                .read(cx)
                .input
                .clone();
            input.update(cx, |input, cx| input.set_value("music", window, cx));
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        let count = cx.debug_bounds("palette-count-albums").unwrap().center();
        cx.simulate_click(count, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert!(view.command_palette.is_none());
            assert_eq!(
                view.route,
                Route::Library(feishin_core::model::ItemKind::Album)
            );
            assert_eq!(view.local_filter, "music");
            assert_eq!(view.state.queue.entries.len(), 6);
            assert!(!view.playing);
        });
        done.store(true, Ordering::Relaxed);
        worker.join().unwrap();
        std::fs::remove_dir_all(dir).ok();
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "command_palette_layout_tests.rs"]
mod layout_tests;
