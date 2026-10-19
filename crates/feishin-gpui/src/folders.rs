use crate::app::{Feishin, Route};
use feishin_core::model::{Item, ItemKind, Sort};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IconName, Selectable, Sizable, StyledExt,
    button::ButtonVariants,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
};
use std::collections::{HashMap, HashSet};

actions!(folders, [ChooseFolder]);

#[derive(Default)]
pub struct Browser {
    server: Option<uuid::Uuid>,
    generation: u64,
    libraries: Vec<String>,
    roots: Option<Vec<Item>>,
    children: HashMap<String, Vec<Item>>,
    paths: HashMap<String, Vec<Item>>,
    expanded: HashSet<String>,
    pending: HashSet<String>,
    errors: HashMap<String, String>,
    selected: Option<String>,
    focus: Option<FocusHandle>,
    scroll: UniformListScrollHandle,
    width: f32,
}

impl Browser {
    fn server(&mut self, server: uuid::Uuid) {
        if self.server != Some(server) {
            *self = Self {
                server: Some(server),
                generation: self.generation.wrapping_add(1),
                ..Default::default()
            };
        }
    }
    fn rows(&self) -> Vec<(Item, usize)> {
        let mut stack = self
            .roots
            .iter()
            .flatten()
            .rev()
            .map(|item| (item, 0))
            .collect::<Vec<_>>();
        let mut seen = HashSet::new();
        let mut rows = Vec::new();
        while let Some((item, depth)) = stack.pop() {
            if !seen.insert(item.id.as_str()) {
                continue;
            }
            rows.push((item.clone(), depth));
            if self.expanded.contains(&item.id) {
                stack.extend(
                    self.children
                        .get(&item.id)
                        .into_iter()
                        .flatten()
                        .rev()
                        .map(|child| (child, depth + 1)),
                );
            }
        }
        rows
    }
}

fn breadcrumb_gap(depth: usize, width: f32) -> (usize, usize) {
    let max_items = match width {
        width if width >= 1152. => 8,
        width if width >= 960. => 6,
        width if width >= 768. => 4,
        width if width >= 600. => 3,
        width if width >= 360. => 2,
        _ => 1,
    };
    if depth < max_items {
        (depth, depth)
    } else if max_items == 1 {
        (0, depth)
    } else {
        let middle = max_items - 2;
        (middle / 2, depth - 1 - (middle - middle / 2))
    }
}

fn breadcrumb_label(name: &str) -> String {
    let text = name.encode_utf16().collect::<Vec<_>>();
    if text.len() > 26 {
        format!(
            "{}…{}",
            String::from_utf16_lossy(&text[..12]),
            String::from_utf16_lossy(&text[text.len() - 12..])
        )
    } else {
        name.to_owned()
    }
}

impl Feishin {
    pub fn folder_table_click(&mut self, index: usize, event: &ClickEvent, cx: &mut Context<Self>) {
        let Some(item) = self.items.get(index).cloned() else {
            return;
        };
        if event.click_count() == 2 {
            if item.kind == ItemKind::Folder {
                self.navigate(Route::Detail(Box::new(item)), cx);
            } else if item.kind == ItemKind::Song {
                let tracks = self
                    .items
                    .iter()
                    .filter(|item| item.kind == ItemKind::Song)
                    .cloned()
                    .collect();
                self.add_tracks_from(tracks, feishin_core::queue::Enqueue::Now, Some(&item), cx);
            }
        } else {
            self.select_library_index(index, event.modifiers(), cx);
        }
        cx.notify();
    }

    pub fn refresh_folder_browser(&mut self) {
        if self.folder_route() {
            self.folder_browser.generation = self.folder_browser.generation.wrapping_add(1);
            self.folder_browser.roots = None;
            self.folder_browser.children.clear();
            self.folder_browser.pending.clear();
            self.folder_browser.errors.clear();
        }
    }
    pub fn folder_route(&self) -> bool {
        matches!(self.route, Route::Library(ItemKind::Folder))
            || matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Folder)
    }
    pub fn remember_folder_route(&mut self, route: &Route) {
        if *route == Route::Library(ItemKind::Folder) {
            self.folder_browser.selected = None;
        }
        let Route::Detail(folder) = route else {
            return;
        };
        if folder.kind != ItemKind::Folder {
            return;
        }
        self.folder_browser.server(folder.server_id);
        if !self.folder_browser.paths.contains_key(&folder.id) {
            let mut path = match &self.route {
                Route::Detail(parent)
                    if parent.kind == ItemKind::Folder && parent.server_id == folder.server_id =>
                {
                    self.folder_browser
                        .paths
                        .get(&parent.id)
                        .cloned()
                        .unwrap_or_else(|| vec![parent.as_ref().clone()])
                }
                _ => Vec::new(),
            };
            if let Some(index) = path.iter().position(|item| item.id == folder.id) {
                path.truncate(index);
            }
            path.push(folder.as_ref().clone());
            self.folder_browser.paths.insert(folder.id.clone(), path);
        }
        if let Some(path) = self.folder_browser.paths.get(&folder.id) {
            self.folder_browser.expanded.extend(
                path.iter()
                    .take(path.len().saturating_sub(1))
                    .map(|item| item.id.clone()),
            );
        }
        self.folder_browser.selected = Some(folder.id.clone());
    }
    pub fn prepare_folder_browser(&mut self, cx: &mut Context<Self>) {
        if !self.folder_route() {
            return;
        }
        let api = match &self.route {
            Route::Detail(item) => self.sessions.get(&item.server_id).cloned(),
            _ => self.api(),
        };
        let Some(api) = api else {
            return;
        };
        self.folder_browser.server(api.server.id);
        if self.folder_browser.libraries != api.server.options.music_folder_ids {
            self.folder_browser = Browser {
                server: Some(api.server.id),
                libraries: api.server.options.music_folder_ids.clone(),
                generation: self.folder_browser.generation.wrapping_add(1),
                ..Default::default()
            };
        }
        if self.folder_browser.focus.is_none() {
            self.folder_browser.focus = Some(cx.focus_handle());
        }
        if !cx
            .key_bindings()
            .borrow()
            .bindings()
            .any(|binding| binding.action().as_any().is::<ChooseFolder>())
        {
            cx.bind_keys([
                KeyBinding::new("enter", ChooseFolder, Some("FolderTree")),
                KeyBinding::new("space", ChooseFolder, Some("FolderTree")),
            ]);
        }
        self.load_folder_branch(None, cx);
        if let Route::Detail(item) = &self.route {
            let item = item.as_ref().clone();
            self.folder_browser.selected = Some(item.id.clone());
            self.folder_browser
                .paths
                .entry(item.id.clone())
                .or_insert_with(|| vec![item.clone()]);
            let path = self.folder_browser.paths[&item.id].clone();
            for ancestor in path {
                if self.folder_browser.expanded.contains(&ancestor.id) {
                    self.load_folder_branch(Some(ancestor), cx);
                }
            }
        }
    }
    fn load_folder_branch(&mut self, parent: Option<Item>, cx: &mut Context<Self>) {
        let key = parent
            .as_ref()
            .map_or(String::new(), |item| item.id.clone());
        if self.folder_browser.pending.contains(&key)
            || self.folder_browser.errors.contains_key(&key)
            || if parent.is_none() {
                self.folder_browser.roots.is_some()
            } else {
                self.folder_browser.children.contains_key(&key)
            }
        {
            return;
        }
        let Some(api) = self
            .folder_browser
            .server
            .and_then(|id| self.sessions.get(&id))
            .cloned()
        else {
            return;
        };
        let generation = self.folder_browser.generation;
        self.folder_browser.pending.insert(key.clone());
        let task = cx.background_executor().spawn(async move {
            let mut items = api.folder_children(parent.as_ref())?;
            items.retain(|item| item.kind == ItemKind::Folder);
            feishin_core::lists::sort_items(&mut items, Sort::Name, false);
            anyhow::Ok(items)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                let browser = &mut this.folder_browser;
                if browser.generation != generation {
                    return;
                }
                browser.pending.remove(&key);
                match result {
                    Ok(items) => {
                        let path = browser.paths.get(&key).cloned().unwrap_or_default();
                        for item in &items {
                            let mut path = path.clone();
                            if path.iter().any(|ancestor| ancestor.id == item.id) {
                                continue;
                            }
                            path.push(item.clone());
                            browser.paths.entry(item.id.clone()).or_insert(path);
                        }
                        if key.is_empty() {
                            browser.roots = Some(items);
                        } else {
                            browser.children.insert(key, items);
                        }
                    }
                    Err(error) => {
                        log::warn!("Folder tree request failed: {error}");
                        browser.errors.insert(key, error.to_string());
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn toggle_folder_branch(&mut self, item: Item, cx: &mut Context<Self>) {
        if !self.folder_browser.expanded.remove(&item.id) {
            self.folder_browser.expanded.insert(item.id.clone());
            self.folder_browser.errors.remove(&item.id);
            self.load_folder_branch(Some(item), cx);
        }
        cx.notify();
    }
    fn open_tree_folder(&mut self, item: Item, cx: &mut Context<Self>) {
        let current = matches!(&self.route, Route::Detail(active) if active.id == item.id && active.server_id == item.server_id);
        if current && self.folder_browser.expanded.contains(&item.id) {
            self.folder_browser.expanded.remove(&item.id);
        } else {
            self.folder_browser.expanded.insert(item.id.clone());
            self.load_folder_branch(Some(item.clone()), cx);
        }
        self.folder_browser.selected = Some(item.id.clone());
        if !current {
            self.navigate(Route::Detail(Box::new(item)), cx);
        }
        cx.notify();
    }
    pub fn folder_browser_view(
        &self,
        content: AnyElement,
        available: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = match &self.route {
            Route::Detail(item) => self
                .folder_browser
                .paths
                .get(&item.id)
                .cloned()
                .unwrap_or_else(|| vec![item.as_ref().clone()]),
            _ => Vec::new(),
        };
        let available = if self.folder_browser.width > 0. {
            self.folder_browser.width
        } else {
            available
        };
        let (start, end) = breadcrumb_gap(path.len(), available / self.state.settings.zoom);
        let mut ancestors = Vec::new();
        for (index, item) in path.iter().enumerate() {
            if index == start && start < end {
                let collapsed = path[start..end].to_vec();
                let view = cx.entity().downgrade();
                ancestors.push(
                    crate::styles::button("folder-overflow", cx)
                        .debug_selector(|| "folder-overflow".into())
                        .small()
                        .ghost()
                        .label("…")
                        .tooltip(self.tr("Folders"))
                        .dropdown_menu(move |mut menu, _, _| {
                            for item in &collapsed {
                                let item = item.clone();
                                let view = view.clone();
                                menu = menu.item(PopupMenuItem::new(item.name.clone()).on_click(
                                    move |_, _, cx| {
                                        view.update(cx, |view, cx| {
                                            view.navigate(Route::Detail(Box::new(item.clone())), cx)
                                        })
                                        .ok();
                                    },
                                ));
                            }
                            menu
                        })
                        .into_any_element(),
                );
            }
            if (start..end).contains(&index) {
                continue;
            }
            let item = item.clone();
            let id = item.id.clone();
            let label = breadcrumb_label(&item.name);
            ancestors.push(
                crate::styles::button(SharedString::from(format!("folder-ancestor-{id}")), cx)
                    .small()
                    .ghost()
                    .debug_selector(move || format!("folder-ancestor-{id}"))
                    .label(label)
                    .tooltip(item.name.clone())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.navigate(Route::Detail(Box::new(item.clone())), cx)
                    }))
                    .into_any_element(),
            );
        }
        let breadcrumbs = div()
            .h_flex()
            .flex_wrap()
            .gap_1()
            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
            .child(
                crate::styles::button("folder-root", cx)
                    .small()
                    .ghost()
                    .label(self.tr("common.home"))
                    .debug_selector(|| "folder-root".into())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.navigate(Route::Library(ItemKind::Folder), cx)
                    })),
            )
            .children(ancestors);
        let rows = self.folder_browser.rows();
        let tree = div()
            .id("folder-tree")
            .role(Role::Tree)
            .aria_label(self.tr("Folders"))
            .key_context("FolderTree")
            .on_action(cx.listener(|view, _: &ChooseFolder, _, cx| {
                let rows = view.folder_browser.rows();
                if let Some((item, _)) = rows
                    .iter()
                    .find(|(item, _)| Some(&item.id) == view.folder_browser.selected.as_ref())
                    .or_else(|| rows.first())
                {
                    view.open_tree_folder(item.clone(), cx);
                }
            }))
            .debug_selector(|| "folder-tree".into())
            .relative()
            .v_flex()
            .w(px(300. * self.state.settings.zoom))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .border_r_1()
            .border_color(cx.theme().border)
            .when_some(self.folder_browser.focus.as_ref(), |div, focus| {
                div.track_focus(focus)
            })
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, _, cx| {
                let rows = view.folder_browser.rows();
                if rows.is_empty() {
                    return;
                }
                let current = rows
                    .iter()
                    .position(|(item, _)| Some(&item.id) == view.folder_browser.selected.as_ref());
                match event.keystroke.key.as_str() {
                    "up" | "down" => {
                        let index = if event.keystroke.key == "up" {
                            current.map_or(rows.len() - 1, |index| index.saturating_sub(1))
                        } else {
                            current.map_or(0, |index| (index + 1).min(rows.len() - 1))
                        };
                        view.folder_browser.selected = Some(rows[index].0.id.clone());
                        view.folder_browser
                            .scroll
                            .scroll_to_item(index, ScrollStrategy::Center);
                    }
                    "right" => {
                        let item = rows[current.unwrap_or(0)].0.clone();
                        if !view.folder_browser.expanded.contains(&item.id) {
                            view.toggle_folder_branch(item, cx);
                        }
                    }
                    "left" => {
                        let current = current.unwrap_or(0);
                        let item = &rows[current].0;
                        if !view.folder_browser.expanded.remove(&item.id)
                            && rows[current].1 > 0
                            && let Some(index) = (0..current)
                                .rev()
                                .find(|&index| rows[index].1 < rows[current].1)
                        {
                            view.folder_browser.selected = Some(rows[index].0.id.clone());
                            view.folder_browser
                                .scroll
                                .scroll_to_item(index, ScrollStrategy::Center);
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
                cx.notify();
            }))
            .when(!self.folder_browser.errors.is_empty(), |div| {
                div.child(
                    crate::styles::button("folder-tree-retry", cx)
                        .label(self.tr("Refresh"))
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.folder_browser.errors.clear();
                            view.folder_browser.roots = None;
                            view.prepare_folder_browser(cx);
                            cx.notify();
                        })),
                )
            })
            .child(
                uniform_list(
                    "folder-tree-rows",
                    rows.len(),
                    cx.processor(move |view, range: std::ops::Range<usize>, _, cx| {
                        range
                            .map(|index| {
                                let (item, depth) = &rows[index];
                                let open = item.clone();
                                let toggle = item.clone();
                                let id = item.id.clone();
                                let expanded = view.folder_browser.expanded.contains(&item.id);
                                let selected =
                                    view.folder_browser.selected.as_ref() == Some(&item.id);
                                let expandable = view
                                    .folder_browser
                                    .children
                                    .get(&item.id)
                                    .is_none_or(|children| !children.is_empty());
                                let mut row = div()
                                    .id(SharedString::from(format!("tree-row-{id}")))
                                    .role(Role::TreeItem)
                                    .aria_label(item.name.clone())
                                    .aria_selected(selected)
                                    .aria_level(depth + 1)
                                    .when(expandable, |row| row.aria_expanded(expanded))
                                    .h_flex()
                                    .debug_selector(move || format!("folder-tree-row-{id}"))
                                    .h(px(32. * view.state.settings.zoom))
                                    .pl(px(*depth as f32 * 16. * view.state.settings.zoom));
                                if expandable {
                                    row = row.child(
                                        crate::styles::button(("folder-expand", index), cx)
                                            .ghost()
                                            .small()
                                            .icon(if expanded {
                                                IconName::ChevronDown
                                            } else {
                                                IconName::ChevronRight
                                            })
                                            .tooltip(view.tr(if expanded {
                                                "Collapse"
                                            } else {
                                                "Expand"
                                            }))
                                            .on_click(cx.listener(move |view, _, _, cx| {
                                                view.toggle_folder_branch(toggle.clone(), cx)
                                            })),
                                    );
                                } else {
                                    row = row.child(div().w(px(28.)));
                                }
                                row.child(
                                    crate::styles::button(("folder-open", index), cx)
                                        .ghost()
                                        .small()
                                        .flex_1()
                                        .min_w_0()
                                        .justify_start()
                                        .icon(IconName::Folder)
                                        .label(item.name.clone())
                                        .selected(selected)
                                        .on_click(cx.listener(move |view, _, window, cx| {
                                            if let Some(focus) = &view.folder_browser.focus {
                                                window.focus(focus);
                                            }
                                            view.open_tree_folder(open.clone(), cx);
                                        })),
                                )
                                .on_drag(
                                    crate::queue::Dragged {
                                        items: vec![item.clone()],
                                        keys: vec![],
                                        sidebar_playlist: false,
                                        playlist_source: None,
                                    },
                                    crate::context_preview::drag_constructor(cx),
                                )
                                .context_menu(
                                    view.item_menu_builder(item, vec![item.clone()], false, cx),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .track_scroll(self.folder_browser.scroll.clone())
                .flex_1()
                .min_h_0(),
            )
            .child(crate::scroll::Scrollbar::vertical(
                "folder-tree-scrollbar",
                &self.folder_browser.scroll,
            ));
        let measure = cx.entity().downgrade();
        div()
            .relative()
            .v_flex()
            .size_full()
            .min_h_0()
            .gap_2()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let measure = measure.clone();
                        let width = f32::from(bounds.size.width);
                        window.defer(cx, move |_, cx| {
                            measure
                                .update(cx, |view, cx| {
                                    if view.folder_browser.width != width {
                                        view.folder_browser.width = width;
                                        cx.notify();
                                    }
                                })
                                .ok();
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(breadcrumbs)
            .child(
                div()
                    .h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .when(available >= 1200. * self.state.settings.zoom, |div| {
                        div.child(tree)
                    })
                    .child(div().flex_1().min_w_0().h_full().child(content)),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{ItemKind, Server, ServerType, Session},
        storage::State,
    };
    use gpui::{AppContext, Modifiers, TestAppContext, VisualTestContext, px, size};
    use gpui_component::Root;
    use std::{
        cell::RefCell,
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        rc::Rc,
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    #[test]
    fn breadcrumbs_match_original_at_every_container_breakpoint() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/folder-breadcrumbs-reference.json"))
                .unwrap();
        for case in reference["cases"].as_array().unwrap() {
            let path = case["path"].as_array().unwrap();
            let (start, end) =
                super::breadcrumb_gap(path.len(), case["width"].as_f64().unwrap() as f32);
            let visible = std::iter::once("root".to_owned())
                .chain(
                    path.iter()
                        .enumerate()
                        .filter(|(index, _)| !(start..end).contains(index))
                        .map(|(_, item)| item["id"].as_str().unwrap().to_owned()),
                )
                .collect::<Vec<_>>();
            let collapsed = path[start..end]
                .iter()
                .map(|item| item["id"].as_str().unwrap())
                .collect::<Vec<_>>();
            let labels = path
                .iter()
                .map(|item| super::breadcrumb_label(item["name"].as_str().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(
                serde_json::to_value(visible).unwrap(),
                case["visible"],
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(collapsed).unwrap(),
                case["collapsed"],
                "{case}"
            );
            assert_eq!(
                serde_json::to_value(labels).unwrap(),
                case["labels"],
                "{case}"
            );
        }
    }

    #[gpui::test]
    fn folder_table_selection_and_double_click_only_queue_displayed_songs(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-folder-table-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.confirm_queue_changes = false;
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            state.settings.play_button_behavior = feishin_core::queue::PlayBehavior::Next;
            let mut config = feishin_core::lists::default_for("song");
            config.display = "poster".into();
            config.pagination = "paginated".into();
            config.items_per_page = 1;
            state.settings.lists.insert("song".into(), config);
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1300.), px(900.)));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw_next_frame_for_test(cx));
        cx.run_until_parked();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.route = Route::Library(ItemKind::Folder);
                view.items = [ItemKind::Folder, ItemKind::Song, ItemKind::Song]
                    .into_iter()
                    .enumerate()
                    .map(|(index, kind)| feishin_core::model::Item {
                        id: index.to_string(),
                        name: format!("Item {index}"),
                        kind,
                        ..Default::default()
                    })
                    .collect();
                cx.notify();
            });
        });
        cx.run_until_parked();
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/list-selection-reference.json")).unwrap();
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                for case in reference["cases"].as_array().unwrap() {
                    view.grid_selection_order = case["initial"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|index| index.as_u64().unwrap() as usize)
                        .collect();
                    view.selected = view.grid_selection_order.iter().copied().collect();
                    view.select_library_index(
                        case["index"].as_u64().unwrap() as usize,
                        Modifiers {
                            control: case["ctrl"].as_bool().unwrap(),
                            shift: case["shift"].as_bool().unwrap(),
                            platform: case["meta"].as_bool().unwrap(),
                            ..Modifiers::none()
                        },
                        cx,
                    );
                    assert_eq!(
                        serde_json::to_value(&view.grid_selection_order).unwrap(),
                        case["selected"],
                        "{case}"
                    );
                    assert_eq!(
                        view.selected,
                        view.grid_selection_order
                            .iter()
                            .copied()
                            .collect::<std::collections::HashSet<_>>()
                    );
                }
                view.selected.clear();
                view.grid_selection_order.clear();
            })
        });
        app.read_with(cx, |view, _| {
            assert_eq!(view.list_config().display, "table");
            assert_eq!(view.list_config().pagination, "infinite");
        });
        let point = cx
            .debug_bounds("metadata-cell-2-titleCombined")
            .unwrap()
            .center();
        cx.simulate_click(point, Modifiers::none());
        assert!(
            app.read_with(cx, |view, _| view.state.queue.entries.is_empty()
                && view.selected.contains(&2)),
            "point {point:?}, state {:?}",
            app.read_with(cx, |view, _| (
                view.route.clone(),
                view.selected.clone(),
                view.state.queue.entries.len()
            ))
        );
        cx.update(|_, cx| {
            app.update(cx, |view, cx| {
                view.play_library_item(view.items[2].clone(), cx)
            })
        });
        assert_eq!(
            app.read_with(cx, |view, _| view
                .state
                .queue
                .entries
                .iter()
                .map(|entry| entry.item.id.clone())
                .collect::<Vec<_>>()),
            ["1", "2"]
        );
        cx.simulate_event(gpui::MouseDownEvent {
            button: gpui::MouseButton::Left,
            position: point,
            modifiers: Modifiers::none(),
            click_count: 2,
            first_mouse: false,
        });
        cx.simulate_event(gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            position: point,
            modifiers: Modifiers::none(),
            click_count: 2,
        });
        cx.run_until_parked();
        app.read_with(cx, |view, _| {
            assert_eq!(
                view.state
                    .queue
                    .entries
                    .iter()
                    .map(|entry| entry.item.id.as_str())
                    .collect::<Vec<_>>(),
                ["1", "2"]
            );
            assert_eq!(view.state.queue.current().unwrap().item.id, "2");
        });
        let point = cx
            .debug_bounds("metadata-cell-0-titleCombined")
            .unwrap()
            .center();
        cx.simulate_click(point, Modifiers::none());
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::Library(ItemKind::Folder)
        );
        cx.simulate_event(gpui::MouseDownEvent {
            button: gpui::MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.simulate_event(gpui::MouseUpEvent {
            button: gpui::MouseButton::Left,
            position: point,
            click_count: 2,
            ..Default::default()
        });
        cx.run_until_parked();
        assert!(app.read_with(
            cx,
            |view, _| matches!(&view.route, Route::Detail(item) if item.id == "0")
                && view.state.queue.entries.len() == 2
        ));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[gpui::test]
    fn lazy_folder_tree_breadcrumbs_keyboard_and_history_preserve_queue(cx: &mut TestAppContext) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Folders".into(),
            url: format!("http://{}", listener.local_addr().unwrap()),
            username: "fixture".into(),
            kind: ServerType::Subsonic,
            legacy_auth: false,
            options: Default::default(),
        };
        let requests = Arc::new(Mutex::new(Vec::new()));
        let calls = requests.clone();
        let (stop, done) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            while matches!(done.try_recv(), Err(std::sync::mpsc::TryRecvError::Empty)) {
                let Ok((mut socket, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(2));
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
                    if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap();
                calls.lock().unwrap().push(path.to_owned());
                let body = if path.contains("getIndexes") {
                    serde_json::json!({"indexes":{"index":[{"artist":[{"id":"a","name":"Alpha"},{"id":"z","name":"Zeta"}]}]}})
                } else if path.contains("getMusicDirectory") && path.contains("id=a") {
                    serde_json::json!({"directory":{"child":[{"id":"b","title":"Nested","isDir":true},{"id":"track-a","title":"Alpha track"}]}})
                } else if path.contains("getMusicDirectory") {
                    serde_json::json!({"directory":{"child":[{"id":"track-b","title":"Nested track"}]}})
                } else {
                    serde_json::json!({})
                };
                let mut body = body.as_object().unwrap().clone();
                body.insert("status".into(), "ok".into());
                let body = serde_json::json!({"subsonic-response":body}).to_string();
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            }
        });
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-folder-tree-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.home.feature = false;
            state.settings.home.items.clear();
            let mut config = feishin_core::lists::default_for("song");
            config.items_per_page = 1;
            state.settings.lists.insert("song".into(), config);
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |view, cx| {
                view.state.active_server = Some(server.id);
                view.sessions
                    .insert(server.id, Api::new(server, Session::default()).unwrap());
                view.navigate(Route::Library(ItemKind::Folder), cx);
            });
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.simulate_resize(size(px(1800.), px(1000.)));
        let wait = |cx: &mut VisualTestContext| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                cx.run_until_parked();
                if app.read_with(cx, |view, _| {
                    !view.loading && view.folder_browser.pending.is_empty()
                }) {
                    break;
                }
                assert!(Instant::now() < deadline, "Folder request did not finish");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        let click = |selector, cx: &mut VisualTestContext| {
            let point = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("Missing {selector}"))
                .center();
            cx.simulate_click(point, Modifiers::none());
            wait(cx);
        };
        wait(cx);
        let queue = app.read_with(cx, |view, _| {
            serde_json::to_value(&view.state.queue).unwrap()
        });
        assert!(cx.debug_bounds("folder-tree-row-a").is_some());
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .all(|path| !path.contains("getMusicDirectory"))
        );
        app.read_with(cx, |view, _| {
            assert_eq!(view.items.len(), 2);
            assert_eq!(view.list_key(), "song");
        });
        click("folder-tree-row-a", cx);
        assert!(cx.debug_bounds("folder-tree-row-b").is_some());
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .any(|(_, node)| {
                    node.role() == gpui::Role::TreeItem
                        && node.label() == Some("Nested")
                        && node.level() == Some(2)
                })
        }));
        assert_eq!(
            app.read_with(cx, |view, _| view.folder_browser.paths["b"]
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>()),
            ["a", "b"]
        );
        cx.simulate_keystrokes("down");
        assert_eq!(
            app.read_with(cx, |view, _| view.folder_browser.selected.clone()),
            Some("b".into())
        );
        cx.simulate_keystrokes("enter");
        wait(cx);
        app.read_with(cx, |view, _| {
            assert!(
                matches!(&view.route,Route::Detail(item) if item.id=="b"),
                "Unexpected route: {:?}",
                view.route
            );
            assert_eq!(view.items[0].id, "track-b");
        });
        assert!(cx.debug_bounds("folder-ancestor-a").is_some());
        cx.simulate_keystrokes("left");
        cx.simulate_keystrokes("left");
        assert_eq!(
            app.read_with(cx, |view, _| view.folder_browser.selected.clone()),
            Some("a".into())
        );
        cx.simulate_resize(size(px(750.), px(1000.)));
        wait(cx);
        cx.update(|window, cx| window.draw_next_frame_for_test(cx));
        cx.run_until_parked();
        assert!(cx.update(|window, _| {
            window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .all(|(_, node)| node.role() != gpui::Role::Tree)
        }));
        assert!(cx.debug_bounds("folder-overflow").is_some());
        click("folder-overflow", cx);
        click("popup-menu-item-0", cx);
        assert!(app.read_with(
            cx,
            |view, _| matches!(&view.route, Route::Detail(item) if item.id == "a")
        ));
        cx.simulate_resize(size(px(1800.), px(1000.)));
        wait(cx);
        click("folder-root", cx);
        assert_eq!(
            app.read_with(cx, |view, _| view.route.clone()),
            Route::Library(ItemKind::Folder)
        );
        cx.update(|_, cx| app.update(cx, |view, cx| view.back(cx)));
        wait(cx);
        assert!(app.read_with(
            cx,
            |view, _| matches!(&view.route,Route::Detail(item) if item.id=="a")
        ));
        assert_eq!(
            app.read_with(cx, |view, _| serde_json::to_value(&view.state.queue)
                .unwrap()),
            queue
        );
        stop.send(()).unwrap();
        worker.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }
}
