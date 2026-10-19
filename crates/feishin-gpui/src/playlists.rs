use crate::{
    app::{Feishin, Route},
    queue::Dragged,
};
use feishin_core::{
    model::{Item, ItemKind, ListQuery, ServerType},
    playlist_folders::{self, Node},
};
use gpui::{prelude::*, *};
use gpui_component::{
    ActiveTheme, IconName, Selectable, Sizable, StyledExt,
    button::ButtonVariants,
    menu::{ContextMenuExt, DropdownMenu, PopupMenuItem},
};

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "playlist_folder_tests.rs"]
mod folder_tests;

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "sidebar_playlist_tests.rs"]
mod section_tests;

#[derive(Clone)]
pub struct FolderDrag {
    pub path: String,
    pub shared: bool,
}

#[derive(IntoElement)]
struct PlaylistFeedback {
    item: Item,
    sorting: bool,
    zoom: f32,
    row: AnyElement,
}
impl RenderOnce for PlaylistFeedback {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_component::animation::CssTransition;
        let drag = cx.active_drag::<Dragged>();
        let adding = drag.is_some_and(|drag| {
            !drag.items.is_empty()
                && drag
                    .items
                    .iter()
                    .all(|item| item.kind != ItemKind::Playlist)
        });
        let dragging = drag.is_some_and(|drag| {
            drag.sidebar_playlist
                && drag
                    .items
                    .iter()
                    .any(|item| item.id == self.item.id && item.server_id == self.item.server_id)
        });
        let target = if dragging || (self.item.rules.is_some() && adding) {
            0.5
        } else {
            1.
        };
        let highlight = self.item.rules.is_none()
            && (adding
                || (self.sorting
                    && drag.is_some_and(|drag| {
                        !drag.items.is_empty()
                            && drag
                                .items
                                .iter()
                                .all(|item| item.kind == ItemKind::Playlist)
                    })));
        let state = window.use_keyed_state(
            SharedString::from(format!(
                "playlist-dim-{}-{}",
                self.item.server_id, self.item.id
            )),
            cx,
            |_, _| {
                CssTransition::new(
                    target,
                    std::time::Duration::from_millis(300),
                    [0., 0., 0.58, 1.],
                )
            },
        );
        let (opacity, animating) = state.update(cx, |state, cx| {
            let now = cx.background_executor().now();
            if state.target() != target {
                // Motion restarts the full tween on interruption, unlike CSS reversal shortening.
                *state = CssTransition::new(
                    state.value(now).0,
                    std::time::Duration::from_millis(300),
                    [0., 0., 0.58, 1.],
                );
                state.retarget(target, now);
            }
            state.value(now)
        });
        if animating {
            window.request_animation_frame();
        }
        let color = cx.theme().primary.opacity(0.8);
        let radius = cx.theme().radius;
        div()
            .relative()
            .w_full()
            .opacity(opacity)
            .child(self.row)
            .when(highlight, |row| {
                row.child(
                    canvas(
                        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                        move |bounds, hitbox, window, _| {
                            if hitbox.is_hovered(window) {
                                window.paint_quad(
                                    outline(bounds, color, BorderStyle::default())
                                        .border_widths(px(2. * self.zoom))
                                        .corner_radii(radius),
                                );
                            }
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                )
            })
    }
}

#[derive(IntoElement)]
struct PlaylistArtwork {
    id: SharedString,
    hovered: bool,
    size: Pixels,
    content: AnyElement,
}
impl RenderOnce for PlaylistArtwork {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, _| {
            gpui_component::animation::CssTransition::new(
                1.,
                std::time::Duration::from_millis(200),
                [0.25, 0.1, 0.25, 1.],
            )
        });
        let (scale, animating) = state.update(cx, |state, cx| {
            let now = cx.background_executor().now();
            state.retarget(if self.hovered { 1.08 } else { 1. }, now);
            state.value(now)
        });
        if animating {
            window.request_animation_frame();
        }
        div().size(self.size).paint_scale(scale).child(self.content)
    }
}
impl Feishin {
    pub fn playlist_album_mode(&self) -> bool {
        self.state.settings.playlist_albums
            && !self.playlist_tracks_editing()
            && matches!(&self.route, Route::Detail(item) if item.kind == ItemKind::Playlist)
    }
    pub fn reset_playlists(&mut self) {
        self.discard_playlist_tracks_edit();
        self.sidebar_playlist_hover = None;
        self.reset_auto_dj();
        self.playlist_generation += 1;
        self.playlist_options.clear();
        self.playlist_users.clear();
        self.playlist_loading = false;
        self.playlist_folder_path.clear();
        self.shared_playlist_folder_path.clear();
    }
    pub fn load_playlist_options(&mut self, cx: &mut Context<Self>) {
        if self.playlist_options.is_empty() && !self.playlist_loading {
            self.refresh_playlists(cx);
        }
    }
    pub fn refresh_playlists(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.api() else {
            return;
        };
        let server = api.server.id;
        self.playlist_generation += 1;
        let generation = self.playlist_generation;
        self.playlist_loading = true;
        let task = cx.background_executor().spawn(async move {
            let items = api.all_items(ListQuery {
                kind: ItemKind::Playlist,
                ..Default::default()
            })?;
            let users = if api.server.kind == ServerType::Navidrome && api.session().is_admin {
                api.playlist_users().unwrap_or_else(|error| {
                    log::warn!("Could not load playlist owners: {error}");
                    vec![]
                })
            } else {
                vec![]
            };
            Ok::<_, anyhow::Error>((items, users))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.active_server != Some(server)
                    || this.playlist_generation != generation
                {
                    return;
                }
                this.playlist_loading = false;
                match result {
                    Ok((items, users)) => {
                        this.playlist_options = items;
                        this.playlist_users = users;
                        this.fetch_covers(cx);
                    }
                    Err(error) => log::warn!("Could not load sidebar playlists: {error}"),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    fn hover_sidebar_folder_drag(
        &mut self,
        path: &str,
        shared: bool,
        inside: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.state.active_server else {
            return;
        };
        let separator = &self.state.settings.playlist_separator;
        if separator.is_empty() || self.state.settings.playlist_folder_view == "navigation" {
            return;
        }
        let before = self.sidebar_folder_drag_expanded.clone();
        if inside {
            // Parent hitboxes include their children. Let the deepest folder keep its chain.
            if before.iter().any(|(id, scope, current)| {
                *id == server
                    && *scope == shared
                    && playlist_folders::in_folder(current, path, separator)
            }) {
                return;
            }
            let mut chain = vec![];
            let mut prefix = String::new();
            for segment in path.split(separator).filter(|segment| !segment.is_empty()) {
                if !prefix.is_empty() {
                    prefix.push_str(separator);
                }
                prefix.push_str(segment);
                chain.push(prefix.clone());
            }
            self.sidebar_folder_drag_expanded
                .retain(|(id, scope, current)| {
                    *id == server && *scope == shared && chain.contains(current)
                });
            for path in chain {
                if !self.state.playlist_folder_expanded(server, &path, shared) {
                    self.sidebar_folder_drag_expanded
                        .insert((server, shared, path));
                }
            }
        } else {
            self.sidebar_folder_drag_expanded
                .retain(|(id, scope, current)| {
                    *id != server
                        || *scope != shared
                        || (current != path
                            && !playlist_folders::in_folder(current, path, separator))
                });
        }
        if before != self.sidebar_folder_drag_expanded {
            cx.notify();
        }
    }
    fn keep_sidebar_folder_drag_expansion(&mut self, cx: &mut Context<Self>) {
        if self.sidebar_folder_drag_expanded.is_empty() {
            return;
        }
        for (server, shared, path) in self.sidebar_folder_drag_expanded.drain() {
            if self.state.active_server == Some(server)
                && !self.state.playlist_folder_expanded(server, &path, shared)
            {
                self.state.toggle_playlist_folder(server, &path, shared);
            }
        }
        self.save();
        cx.notify();
    }
    pub fn toggle_playlist_folder(&mut self, path: &str, shared: bool, cx: &mut Context<Self>) {
        if self.state.settings.playlist_folder_view == "navigation" {
            *if shared {
                &mut self.shared_playlist_folder_path
            } else {
                &mut self.playlist_folder_path
            } = path.into();
        } else if let Some(server) = self.state.active_server {
            self.state.toggle_playlist_folder(server, path, shared);
            self.save();
        }
        cx.notify();
    }
    fn is_shared_playlist(&self, item: &Item) -> bool {
        self.api().is_some_and(|api| {
            if !item.owner_name.is_empty() {
                item.owner_name != api.server.username
            } else {
                let user = api.session().user_id;
                !item.owner_id.is_empty()
                    && !user.is_empty()
                    && item.owner_id != user
                    && item.owner_id != api.server.username
            }
        })
    }
    pub(crate) fn move_sidebar_playlist_folder(
        &mut self,
        source: &FolderDrag,
        target: &str,
        cx: &mut Context<Self>,
    ) {
        let items = self
            .playlist_options
            .iter()
            .filter(|item| self.is_shared_playlist(item) == source.shared)
            .cloned()
            .collect::<Vec<_>>();
        if items.is_empty() {
            return;
        }
        self.move_sidebar_playlists(
            &Dragged {
                sidebar_playlist: false,
                playlist_source: None,
                keys: vec![],
                items,
            },
            Some(&source.path),
            target,
            cx,
        );
    }
    pub fn move_sidebar_playlists(
        &mut self,
        dragged: &Dragged,
        source: Option<&str>,
        target: &str,
        cx: &mut Context<Self>,
    ) {
        let separator = &self.state.settings.playlist_separator;
        let result: anyhow::Result<Vec<(Item, String)>> = if let Some(source) = source {
            (if dragged.items.is_empty() {
                &self.playlist_options
            } else {
                &dragged.items
            })
            .iter()
            .filter_map(|item| {
                match playlist_folders::move_folder(&item.name, source, target, separator) {
                    Ok(Some(name)) => Some(Ok((item.clone(), name))),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
        } else {
            Ok(dragged
                .items
                .iter()
                .filter(|item| item.kind == ItemKind::Playlist)
                .map(|item| {
                    (
                        item.clone(),
                        playlist_folders::move_playlist(&item.name, target, separator),
                    )
                })
                .collect())
        };
        match result {
            Ok(updates) => self.rename_sidebar_playlists(updates, cx),
            Err(error) => {
                self.message = error.to_string();
                cx.notify();
            }
        }
    }
    fn rename_sidebar_playlists(&mut self, updates: Vec<(Item, String)>, cx: &mut Context<Self>) {
        let Some(api) = self.api() else {
            return;
        };
        let server = api.server.id;
        if updates.is_empty() {
            return;
        }
        if updates.iter().any(|(item, name)| {
            item.server_id != server || name.trim().is_empty() || name.len() > 4096
        }) {
            self.message = "Choose playlists on the current server and a valid folder name".into();
            cx.notify();
            return;
        }
        let task = cx.background_executor().spawn(async move {
            let mut saved = 0;
            for (item, name) in updates {
                if item.name == name {
                    continue;
                }
                if let Err(error) = api.update_playlist_checked(
                    &item,
                    &name,
                    &item.comment,
                    item.public,
                    item.rules.clone(),
                ) {
                    anyhow::bail!("Updated {saved} playlists before the server returned: {error}");
                }
                saved += 1;
            }
            Ok(saved)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if this.state.active_server != Some(server) {
                    return;
                }
                this.refresh_playlists(cx);
                if this.route == Route::Library(ItemKind::Playlist) {
                    this.reload(false, cx);
                }
                this.message = match result {
                    Ok(count) => format!("Updated {count} playlists"),
                    Err(error) => {
                        log::warn!("Could not move playlist folder: {error}");
                        error.to_string()
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
    pub fn edit_playlist_folder(
        &mut self,
        path: String,
        shared: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playlist_folder_shared = shared;
        self.form_name
            .update(cx, |input, cx| input.set_value(path.clone(), window, cx));
        self.navigate(Route::PlaylistFolderEditor(path), cx);
    }
    pub fn playlist_folder_editor(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .v_flex()
            .gap_3()
            .max_w(px(650.0))
            .child(crate::appearance::text("Folder path", cx))
            .child(crate::styles::input(&self.form_name, cx))
            .child(
                crate::styles::button("save-playlist-folder", cx)
                    .primary()
                    .label(crate::appearance::text("Rename folder", cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let Route::PlaylistFolderEditor(source) = &this.route else {
                            return;
                        };
                        let target = this.form_name.read(cx).value().trim().to_owned();
                        if target.is_empty() {
                            this.message = "Enter a folder name".into();
                            cx.notify();
                            return;
                        }
                        let prefix = format!("{source}{}", this.state.settings.playlist_separator);
                        let updates = this
                            .playlist_options
                            .iter()
                            .filter(|item| {
                                this.is_shared_playlist(item) == this.playlist_folder_shared
                            })
                            .filter_map(|item| {
                                item.name.strip_prefix(&prefix).map(|rest| {
                                    (
                                        item.clone(),
                                        format!(
                                            "{target}{}{rest}",
                                            this.state.settings.playlist_separator
                                        ),
                                    )
                                })
                            })
                            .collect();
                        this.rename_sidebar_playlists(updates, cx);
                        this.navigate(Route::Library(ItemKind::Playlist), cx);
                    })),
            )
            .into_any_element()
    }
    pub fn sidebar_playlists(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.settings.sidebar.collapsed {
            return self.sidebar_playlist_group(false, cx);
        }
        div()
            .v_flex()
            .flex_shrink_0()
            .child(
                div()
                    .debug_selector(|| "sidebar-playlist-separator".into())
                    .h(px(self.state.settings.zoom))
                    .my(cx.theme().font_size * 0.25)
                    .bg(cx.theme().border),
            )
            .child(self.sidebar_playlist_group(false, cx))
            .child(self.sidebar_playlist_group(true, cx))
            .into_any_element()
    }
    fn playlist_heading_action(
        &self,
        id: &'static str,
        icon: &str,
        label: &str,
        cx: &App,
    ) -> gpui_component::button::Button {
        crate::styles::button(id, cx)
            .debug_selector(move || id.into())
            .ghost()
            .action_icon(self.state.settings.zoom)
            .size(cx.theme().font_size * 1.875)
            .p_0()
            .border_0()
            .icon(
                gpui_component::Icon::default()
                    .path(format!("native-icons/{icon}.svg"))
                    .with_size(cx.theme().font_size),
            )
            .tooltip(self.tr(label))
    }
    fn sidebar_playlist_group(&self, shared_group: bool, cx: &mut Context<Self>) -> AnyElement {
        let settings = &self.state.settings;
        let order = self
            .state
            .active_server
            .and_then(|server| self.state.sidebar_playlist_order.get(&server))
            .map_or(&[][..], Vec::as_slice);
        let mut playlists = settings.sidebar.playlists(&self.playlist_options, order);
        let shared = |item: &Item| self.is_shared_playlist(item);
        let shared_count = playlists.iter().filter(|item| shared(item)).count();
        let collapsed = settings.sidebar.collapsed;
        if collapsed {
            // The original compact list is flat and groups owned entries before shared ones.
            playlists.sort_by_key(shared);
        }
        if !collapsed {
            playlists.retain(|item| shared(item) == shared_group);
        }
        if shared_group && playlists.is_empty() {
            return div().into_any_element();
        }
        let section = if shared_group {
            "shared-playlists"
        } else {
            "playlists"
        };
        let open = if collapsed {
            self.sidebar_playlists_open
        } else {
            !self.sidebar_closed_sections.contains(section)
        };
        let mut nodes = if !collapsed && settings.playlist_folders {
            playlist_folders::tree(
                &playlists,
                &settings.playlist_separator,
                settings.playlist_folder_view == "single",
            )
        } else {
            playlists
                .iter()
                .map(|item| Node::Playlist {
                    name: item.name.clone(),
                    item: Box::new(item.clone()),
                })
                .collect()
        };
        if !collapsed && settings.sidebar.playlist_sorting {
            playlist_folders::order(&mut nodes, order);
        }
        let path = if shared_group {
            &self.shared_playlist_folder_path
        } else {
            &self.playlist_folder_path
        }
        .clone();
        let display = if !collapsed
            && settings.playlist_folders
            && settings.playlist_folder_view == "navigation"
            && !path.is_empty()
        {
            fn find<'a>(nodes: &'a [Node], path: &str) -> Option<&'a [Node]> {
                nodes.iter().find_map(|node| {
                    if let Node::Folder {
                        path: candidate,
                        children,
                        ..
                    } = node
                    {
                        if candidate == path {
                            Some(children.as_slice())
                        } else {
                            find(children, path)
                        }
                    } else {
                        None
                    }
                })
            }
            find(&nodes, &path).unwrap_or(&nodes)
        } else {
            &nodes
        };
        div()
            .id(if shared_group {
                "sidebar-shared-playlists"
            } else {
                "sidebar-playlists"
            })
            .v_flex()
            .when(collapsed, |div| div.gap_1())
            .flex_shrink_0()
            .child(
                div()
                    .id(if shared_group {
                        "shared-playlist-root-drop"
                    } else {
                        "playlist-root-drop"
                    })
                    .on_drop(cx.listener(|this, dragged: &Dragged, _, cx| {
                        this.move_sidebar_playlists(dragged, None, "", cx);
                        cx.stop_propagation();
                    }))
                    .on_drop(cx.listener(|this, dragged: &FolderDrag, _, cx| {
                        this.move_sidebar_playlist_folder(dragged, "", cx);
                        cx.stop_propagation();
                    }))
                    .child(if collapsed {
                        div()
                            .v_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                crate::styles::button("compact-playlists-library", cx)
                                    .ghost()
                                    .small()
                                    .w_full()
                                    .label(self.tr("Playlists"))
                                    .text_xs()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate(Route::Library(ItemKind::Playlist), cx)
                                    })),
                            )
                            .child(
                                div()
                                    .h_flex()
                                    .gap_1()
                                    .child(
                                        crate::styles::button("compact-playlists-toggle", cx)
                                            .debug_selector(|| "compact-playlists-toggle".into())
                                            .ghost()
                                            .xsmall()
                                            .icon(if self.sidebar_playlists_open {
                                                IconName::ChevronUp
                                            } else {
                                                IconName::ChevronDown
                                            })
                                            .tooltip(self.tr(if self.sidebar_playlists_open {
                                                "common.collapse"
                                            } else {
                                                "common.expand"
                                            }))
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.sidebar_playlists_open =
                                                    !this.sidebar_playlists_open;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        crate::styles::button("compact-playlists-create", cx)
                                            .debug_selector(|| "compact-playlists-create".into())
                                            .ghost()
                                            .xsmall()
                                            .icon(IconName::Plus)
                                            .tooltip(self.tr("action.createPlaylist"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.edit_playlist(None, vec![], window, cx)
                                            })),
                                    ),
                            )
                            .into_any_element()
                    } else {
                        let in_navigation = settings.playlist_folders
                            && settings.playlist_folder_view == "navigation"
                            && !path.is_empty();
                        let leading = in_navigation.then(|| {
                            let button = self
                                .playlist_heading_action(
                                    if shared_group {
                                        "shared-playlist-folder-back"
                                    } else {
                                        "playlist-folder-back"
                                    },
                                    "arrow-left",
                                    "common.back",
                                    cx,
                                )
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    let path = if shared_group {
                                        &mut view.shared_playlist_folder_path
                                    } else {
                                        &mut view.playlist_folder_path
                                    };
                                    *path = path
                                        .rsplit_once(&view.state.settings.playlist_separator)
                                        .map_or(String::new(), |(parent, _)| parent.into());
                                    cx.stop_propagation();
                                    cx.notify();
                                }));
                            div()
                                .when(crate::appearance::rtl(cx), |div| {
                                    div.ml(cx.theme().font_size * 0.25)
                                })
                                .when(!crate::appearance::rtl(cx), |div| {
                                    div.mr(cx.theme().font_size * 0.25)
                                })
                                .child(button)
                                .into_any_element()
                        });
                        let trailing = (!shared_group).then(|| {
                            fn paths(nodes: &[Node], result: &mut Vec<String>) {
                                for node in nodes {
                                    if let Node::Folder { path, children, .. } = node {
                                        result.push(path.clone());
                                        paths(children, result);
                                    }
                                }
                            }
                            let mut folder_paths = vec![];
                            paths(&nodes, &mut folder_paths);
                            let all_expanded = self.state.active_server.is_some_and(|server| {
                                folder_paths.iter().all(|path| {
                                    self.state.playlist_folder_expanded(server, path, false)
                                })
                            });
                            div()
                                .h_flex()
                                .gap(cx.theme().font_size * 0.25)
                                .flex_shrink_0()
                                .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                                .child(
                                    self.playlist_heading_action(
                                        "sidebar-create-playlist",
                                        "plus",
                                        "action.createPlaylist",
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |view, _, window, cx| {
                                            view.edit_playlist(None, vec![], window, cx);
                                            cx.stop_propagation();
                                        },
                                    )),
                                )
                                .when(
                                    settings.playlist_folder_view != "navigation"
                                        && !folder_paths.is_empty(),
                                    |div| {
                                        div.child(
                                            self.playlist_heading_action(
                                                "sidebar-toggle-all-folders",
                                                if all_expanded {
                                                    "folders-collapse"
                                                } else {
                                                    "folders-expand"
                                                },
                                                if all_expanded {
                                                    "action.collapseAllFolders"
                                                } else {
                                                    "action.expandAllFolders"
                                                },
                                                cx,
                                            )
                                            .on_click(
                                                cx.listener(move |view, _, _, cx| {
                                                    if let Some(server) = view.state.active_server {
                                                        for path in &folder_paths {
                                                            if view.state.playlist_folder_expanded(
                                                                server, path, false,
                                                            ) == all_expanded
                                                            {
                                                                view.state.toggle_playlist_folder(
                                                                    server, path, false,
                                                                );
                                                            }
                                                        }
                                                        view.save();
                                                        cx.notify();
                                                    }
                                                    cx.stop_propagation();
                                                }),
                                            ),
                                        )
                                    },
                                )
                                .child(
                                    self.playlist_heading_action(
                                        "sidebar-view-playlists",
                                        "queue",
                                        "action.viewPlaylists",
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |view, _, _, cx| {
                                            view.navigate(Route::Library(ItemKind::Playlist), cx);
                                            cx.stop_propagation();
                                        },
                                    )),
                                )
                                .into_any_element()
                        });
                        self.sidebar_section_toggle_with_controls(
                            section,
                            if in_navigation {
                                playlist_folders::leaf(&path, &settings.playlist_separator)
                            } else if shared_group {
                                "page.sidebar.shared"
                            } else {
                                "page.sidebar.playlists"
                            },
                            leading,
                            trailing,
                            cx,
                        )
                        .into_any_element()
                    }),
            )
            .when(!collapsed || open, |container| {
                container.child({
                    let content = div()
                        .v_flex()
                        .flex_shrink_0()
                        .when(collapsed, |div| div.gap_1())
                        .when(!collapsed, |div| div.pb(cx.theme().font_size * 0.75))
                        .children(display.iter().enumerate().map(|(index, node)| {
                            let entry = self.playlist_node(node, shared_group, cx);
                            if collapsed
                                && shared_count > 0
                                && index == playlists.len() - shared_count
                            {
                                gpui::div()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .pt_1()
                                    .child(entry)
                                    .into_any_element()
                            } else {
                                entry
                            }
                        }));
                    if collapsed {
                        content.into_any_element()
                    } else {
                        crate::sidebar_motion::SectionCollapse {
                            id: format!("sidebar-panel-{section}").into(),
                            open,
                            content: content.into_any_element(),
                        }
                        .into_any_element()
                    }
                })
            })
            .into_any_element()
    }
    pub fn reorder_sidebar_playlists_at(
        &mut self,
        items: &[Item],
        target: Option<&str>,
        after: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.state.settings.sidebar.playlist_sorting {
            return;
        }
        let Some(server) = self.state.active_server else {
            return;
        };
        if items
            .iter()
            .any(|item| item.kind != ItemKind::Playlist || item.server_id != server)
        {
            return;
        }
        let anchor = target
            .or_else(|| items.first().map(|item| item.id.as_str()))
            .and_then(|id| self.playlist_options.iter().find(|item| item.id == id));
        let Some(anchor) = anchor else {
            return;
        };
        if items.iter().any(|item| {
            self.playlist_options
                .iter()
                .find(|current| current.id == item.id)
                .is_none_or(|current| {
                    current.owner_id != anchor.owner_id
                        || (current.owner_id.is_empty() && current.owner_name != anchor.owner_name)
                })
        }) {
            return;
        }
        let shared = self.is_shared_playlist(anchor);
        let scope = self
            .playlist_options
            .iter()
            .filter(|item| self.is_shared_playlist(item) == shared)
            .map(|item| item.id.clone())
            .collect::<std::collections::HashSet<_>>();
        let previous = self
            .state
            .sidebar_playlist_order
            .get(&server)
            .cloned()
            .unwrap_or_default();
        let visible = self
            .state
            .settings
            .sidebar
            .playlists(&self.playlist_options, &previous)
            .into_iter()
            .filter(|item| scope.contains(&item.id))
            .collect::<Vec<_>>();
        let moving: Vec<_> = items.iter().map(|item| item.id.clone()).collect();
        if moving.is_empty()
            || moving
                .iter()
                .any(|id| !visible.iter().any(|item| &item.id == id))
            || target.is_some_and(|id| !visible.iter().any(|item| item.id == id))
        {
            return;
        }
        // The original saves only the filtered rows in this owned/shared scope.
        let mut scoped_order = visible.iter().map(|item| item.id.clone()).collect();
        feishin_core::sidebar::reorder(&mut scoped_order, &visible, &moving, target, after);
        let mut order = previous
            .iter()
            .filter(|id| !scope.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        order.extend(scoped_order);
        if order != previous {
            self.state.sidebar_playlist_order.insert(server, order);
            self.save();
            cx.notify();
        }
    }
    pub fn move_sidebar_playlist(&mut self, item: Item, up: bool, cx: &mut Context<Self>) {
        let order = self
            .state
            .active_server
            .and_then(|server| self.state.sidebar_playlist_order.get(&server))
            .map_or(&[][..], Vec::as_slice);
        let visible = self
            .state
            .settings
            .sidebar
            .playlists(&self.playlist_options, order)
            .into_iter()
            .filter(|candidate| {
                self.is_shared_playlist(candidate) == self.is_shared_playlist(&item)
            })
            .collect::<Vec<_>>();
        let Some(index) = visible.iter().position(|value| value.id == item.id) else {
            return;
        };
        let target = if up {
            index.checked_sub(1)
        } else {
            index.checked_add(1)
        };
        if let Some(target) = target.and_then(|index| visible.get(index)) {
            self.reorder_sidebar_playlists_at(&[item], Some(&target.id), !up, cx);
        }
    }
    fn compact_playlist(&self, item: &Item, cx: &mut Context<Self>) -> AnyElement {
        let cover = cx.theme().font_size * 3.;
        let open = item.clone();
        let content = crate::styles::button(SharedString::from(format!("sidebar-playlist-open-{}", item.id)), cx)
            .debug_selector({let id=item.id.clone();move || format!("compact-playlist-artwork-{id}")})
            .ghost().tooltip(item.name.clone()).p_0().size(cover).relative()
            .selected(matches!(&self.route, Route::Detail(current) if current.id == item.id && current.server_id == item.server_id))
            .child(crate::row_play::ArtworkPlay {
                parent_hovered: Some(self.sidebar_playlist_hover.as_deref() == Some(item.id.as_str())),
                id: format!("sidebar-artwork-{}-{}",item.server_id,item.id).into(),
                selector: format!("compact-playlist-cover-{}",item.id).into(),
                button_selector: format!("compact-playlist-play-{}",item.id).into(),
                item:item.clone(),row:crate::lists::TableRow::Library,compact:false,
                width:Some(cover),height:cover,content:self.artwork(item,f32::from(cover),cx),app:cx.weak_entity(),
            })
            .when(crate::row_play::active(item, &crate::lists::TableRow::Library, self.state.queue.current()), |button| button.child(
                div().absolute().inset(px(-4. * self.state.settings.zoom))
                    .border(px(2. * self.state.settings.zoom)).border_color(cx.theme().primary).rounded_md()
            ))
            .on_click(cx.listener(move |this, _, _, cx| {this.navigate(Route::Detail(Box::new(open.clone())),cx);cx.stop_propagation();}))
            .into_any_element();
        PlaylistArtwork {
            id: format!("playlist-scale-{}-{}", item.server_id, item.id).into(),
            hovered: self.sidebar_playlist_hover.as_deref() == Some(item.id.as_str()),
            size: cover,
            content,
        }
        .into_any_element()
    }
    fn sidebar_playlist_label(
        &self,
        item: &Item,
        name: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let font = cx.theme().font_size;
        let compact = self.state.settings.sidebar.playlist_mode == "compact";
        let hovered = self.sidebar_playlist_hover.as_deref() == Some(item.id.as_str());
        let active = crate::row_play::active(
            item,
            &crate::lists::TableRow::Library,
            self.state.queue.current(),
        );
        let own = self
            .sessions
            .get(&item.server_id)
            .is_some_and(|api| api.session().user_id == item.owner_id);
        let open = item.clone();
        let height = font * if compact { 2.125 } else { 3.5 };
        let title = div()
            .debug_selector({
                let id = item.id.clone();
                move || format!("sidebar-playlist-title-{id}")
            })
            .text_size(font * 0.875)
            .line_height(font * 1.125)
            .font_weight(FontWeight::MEDIUM)
            .truncate()
            .text_color(if active {
                cx.theme().primary
            } else {
                cx.theme().foreground
            })
            .child(name.to_owned());
        let metadata = div()
            .h_flex()
            .gap(font * 0.75)
            .text_size(font * 0.8125)
            .line_height(font)
            .text_color(cx.theme().muted_foreground)
            .min_w_0()
            .overflow_hidden()
            .child(
                div()
                    .h_flex()
                    .gap(font * 0.25)
                    .flex_shrink_0()
                    .child(
                        gpui_component::Icon::default()
                            .path("native-icons/song.svg")
                            .with_size(font * 0.8125),
                    )
                    .child(
                        item.column_data
                            .get("songCount")
                            .cloned()
                            .unwrap_or_else(|| "0".into()),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .gap(font * 0.25)
                    .min_w_0()
                    .truncate()
                    .child(
                        gpui_component::Icon::default()
                            .path("native-icons/duration.svg")
                            .with_size(font * 0.8125),
                    )
                    .child(feishin_core::metadata_format::duration_text(
                        item.duration,
                        &self.state.settings.language,
                        false,
                    )),
            )
            .when(own && item.public, |div| {
                div.child(self.tr("common.public"))
            })
            .when(!own, |div| {
                div.child(
                    gpui::div()
                        .h_flex()
                        .gap(font * 0.25)
                        .min_w_0()
                        .truncate()
                        .child(
                            gpui_component::Icon::default()
                                .path("native-icons/user.svg")
                                .with_size(font * 0.8125),
                        )
                        .child(item.owner_name.clone()),
                )
            });
        div()
            .relative()
            .h(height)
            .flex_1()
            .min_w_0()
            .child(
                crate::styles::button(
                    SharedString::from(format!("sidebar-playlist-open-{}", item.id)),
                    cx,
                )
                .debug_selector({
                    let id = item.id.clone();
                    move || format!("sidebar-playlist-open-{id}")
                })
                .ghost()
                .tooltip(name.to_owned())
                .size_full()
                .p_0()
                .justify_start()
                .child(
                    div()
                        .h_flex()
                        .w_full()
                        .gap(font * 0.75)
                        .px(font * 0.75)
                        .py(font * if compact { 0.5 } else { 0.25 })
                        .when(!compact, |div| {
                            div.child(self.artwork(item, f32::from(font * 3.), cx))
                        })
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(font * 0.25)
                                .when(hovered, |div| div.mr(px(100. * self.state.settings.zoom)))
                                .child(title)
                                .when(!compact, |div| div.child(metadata)),
                        ),
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.navigate(Route::Detail(Box::new(open.clone())), cx);
                    cx.stop_propagation();
                })),
            )
            .when(hovered, |div| {
                div.child(
                    gpui::div()
                        .absolute()
                        .right(font * 0.25)
                        .top((height - font * 1.875) / 2.)
                        .px(font * if compact { 0.5 } else { 0.75 })
                        .bg(cx.theme().secondary)
                        .child(crate::row_play::RowControls {
                            id: format!("sidebar-play-{}", item.id).into(),
                            item: item.clone(),
                            enabled: self.library_fetches.is_empty(),
                            app: cx.weak_entity(),
                        }),
                )
            })
            .into_any_element()
    }
    fn playlist_folder_header(
        &self,
        node: &Node,
        open: bool,
        shared: bool,
        cx: &mut Context<Self>,
    ) -> gpui_component::button::Button {
        use gpui_component::{
            Icon,
            button::{Button, ButtonCustomVariant},
        };
        let Node::Folder {
            name,
            path,
            children,
        } = node
        else {
            unreachable!("Folder headers need a folder");
        };
        fn count(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|node| match node {
                    Node::Playlist { .. } => 1,
                    Node::Folder { children, .. } => count(children),
                })
                .sum()
        }
        let navigation = self.state.settings.playlist_folder_view == "navigation";
        let rem = cx.theme().font_size;
        let rtl = crate::appearance::rtl(cx);
        let id = if shared {
            format!("shared:{path}")
        } else {
            path.clone()
        };
        let selector = |part: &str| {
            let selector = format!("playlist-folder-{part}-{id}");
            move || selector.clone()
        };
        let toggle = path.clone();
        let icon = div()
            .size(rem * 0.875)
            .flex_shrink_0()
            .debug_selector(selector("icon"))
            .child(
                Icon::default()
                    .path(if open {
                        "native-icons/folder.svg"
                    } else {
                        "native-icons/folder-closed.svg"
                    })
                    .with_size(rem * 0.875)
                    .text_color(cx.theme().muted_foreground),
            );
        Button::new(SharedString::from(format!("playlist-folder-toggle-{id}")))
            .debug_selector(selector("header"))
            .custom(ButtonCustomVariant::new(cx).hover(if navigation {
                cx.theme().secondary
            } else {
                cx.theme().background
            }))
            .label_full_width()
            .aria_label(name.clone())
            .when(!navigation, |button| button.aria_expanded(open))
            .w_full()
            .h(rem * if navigation { 3.5 } else { 2.125 })
            .border_0()
            .p_0()
            .px(rem * 0.75)
            .rounded(cx.theme().radius)
            .child(
                div()
                    .h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(rem * 0.75)
                    .when(rtl, |div| div.flex_row_reverse())
                    .child(if navigation {
                        div()
                            .size(rem * 3.)
                            .flex_shrink_0()
                            .h_flex()
                            .justify_center()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().secondary)
                            .debug_selector(selector("icon-frame"))
                            .child(icon)
                            .into_any_element()
                    } else {
                        icon.into_any_element()
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_left()
                            .text_size(rem * 0.875)
                            .line_height(rem * 1.125)
                            .font_weight(FontWeight::MEDIUM)
                            .debug_selector(selector("name"))
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(rem * 0.8125)
                            .line_height(rem)
                            .font_weight(FontWeight::NORMAL)
                            .text_color(cx.theme().muted_foreground)
                            .debug_selector(selector("count"))
                            .child(count(children).to_string()),
                    )
                    .child(
                        div()
                            .size(rem * if navigation { 0.8125 } else { 0.875 })
                            .flex_shrink_0()
                            .when(navigation, |div| div.opacity(0.6))
                            .debug_selector(selector("chevron"))
                            .child(if navigation {
                                Icon::default()
                                    .path("native-icons/arrow-right.svg")
                                    .with_size(rem * 0.8125)
                                    .into_any_element()
                            } else {
                                crate::sidebar_motion::FolderChevron {
                                    id: format!(
                                        "folder-chevron-{:?}-{id}",
                                        self.state.active_server
                                    )
                                    .into(),
                                    open,
                                    size: rem * 0.875,
                                }
                                .into_any_element()
                            }),
                    ),
            )
            .on_click(
                cx.listener(move |view, _, _, cx| view.toggle_playlist_folder(&toggle, shared, cx)),
            )
    }
    fn playlist_node(&self, node: &Node, shared: bool, cx: &mut Context<Self>) -> AnyElement {
        let settings = &self.state.settings;
        match node {
            Node::Playlist { name, item } => {
                let item = item.as_ref().clone();
                let target = item.clone();
                let up = item.clone();
                let down = item.clone();
                let collapsed = settings.sidebar.collapsed;
                let hover_id = item.id.clone();
                let row_open = item.clone();

                let row = div()
                    .id(SharedString::from(format!("sidebar-playlist-{}", item.id)))
                    .debug_selector({
                        let id = item.id.clone();
                        move || format!("sidebar-playlist-{id}")
                    })
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if !hovered
                            && this.sidebar_playlist_hover.as_deref() != Some(hover_id.as_str())
                        {
                            return;
                        }
                        let value = hovered.then(|| hover_id.clone());
                        if this.sidebar_playlist_hover != value {
                            this.sidebar_playlist_hover = value;
                            cx.notify();
                        }
                    }))
                    .when(collapsed, |div| {
                        div.h_flex()
                            .justify_center()
                            .py(cx.theme().font_size * 0.25)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.navigate(Route::Detail(Box::new(row_open.clone())), cx)
                            }))
                    })
                    .relative()
                    .on_drag(
                        Dragged {
                            sidebar_playlist: true,
                            playlist_source: None,
                            keys: vec![],
                            items: vec![item.clone()],
                        },
                        crate::context_preview::drag_constructor(cx),
                    )
                    .on_drop_with_bounds(crate::queue::row_drop_listener(
                        cx,
                        move |this, dragged, after, window, cx| {
                            if dragged.items.is_empty() {
                                return;
                            }
                            this.keep_sidebar_folder_drag_expansion(cx);
                            if dragged
                                .items
                                .iter()
                                .all(|item| item.kind == ItemKind::Playlist)
                            {
                                this.reorder_sidebar_playlists_at(
                                    &dragged.items,
                                    Some(&target.id),
                                    after,
                                    cx,
                                );
                                cx.stop_propagation();
                                return;
                            }
                            if target.rules.is_some() {
                                cx.stop_propagation();
                                return;
                            }
                            this.open_add_to_playlist(
                                dragged.items.clone(),
                                vec![target.clone()],
                                window,
                                cx,
                            );
                            cx.stop_propagation();
                        },
                    ))
                    .child(if collapsed {
                        self.compact_playlist(&item, cx)
                    } else {
                        div()
                            .h_flex()
                            .when(crate::appearance::rtl(cx), |div| div.flex_row_reverse())
                            .gap_1()
                            .items_center()
                            .child(self.sidebar_playlist_label(&item, name, cx))
                            .when(settings.sidebar.playlist_sorting, |div| {
                                div.child(
                                    crate::styles::button(
                                        SharedString::from(format!(
                                            "sidebar-playlist-up-{}",
                                            item.id
                                        )),
                                        cx,
                                    )
                                    .ghost()
                                    .small()
                                    .label(crate::appearance::text("↑", cx))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.move_sidebar_playlist(up.clone(), true, cx)
                                        },
                                    )),
                                )
                                .child(
                                    crate::styles::button(
                                        SharedString::from(format!(
                                            "sidebar-playlist-down-{}",
                                            item.id
                                        )),
                                        cx,
                                    )
                                    .ghost()
                                    .small()
                                    .label(crate::appearance::text("↓", cx))
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.move_sidebar_playlist(down.clone(), false, cx)
                                        },
                                    )),
                                )
                            })
                            .into_any_element()
                    })
                    .context_menu(self.item_menu_builder(&item, item.action_items(), false, cx))
                    .into_any_element();
                PlaylistFeedback {
                    item,
                    sorting: settings.sidebar.playlist_sorting,
                    zoom: settings.zoom,
                    row,
                }
                .into_any_element()
            }
            Node::Folder { path, children, .. } => {
                let closed = self.state.active_server.is_none_or(|server| {
                    !self.state.playlist_folder_expanded(server, path, shared)
                        && !self.sidebar_folder_drag_expanded.contains(&(
                            server,
                            shared,
                            path.clone(),
                        ))
                });
                let drag_path = path.clone();
                let folder_drag_path = path.clone();
                let destination = path.clone();
                let folder_destination = path.clone();
                let rename = path.clone();
                let root_move = path.clone();
                let view = cx.weak_entity();
                let line_color = feishin_core::appearance::color(&settings.playlist_folder_color)
                    .ok()
                    .map(|[r, g, b, a]| Hsla::from(Rgba { r, g, b, a }))
                    .unwrap_or(cx.theme().border);
                div()
                    .id(SharedString::from(format!("playlist-folder-{path}")))
                    .v_flex()
                    .on_drag_move(cx.listener(
                        move |view, event: &DragMoveEvent<Dragged>, _, cx| {
                            let valid = !event.drag(cx).items.is_empty();
                            view.hover_sidebar_folder_drag(
                                &drag_path,
                                shared,
                                valid && event.bounds.contains(&event.event.position),
                                cx,
                            );
                        },
                    ))
                    .on_drag_move(cx.listener(
                        move |view, event: &DragMoveEvent<FolderDrag>, _, cx| {
                            let drag = event.drag(cx);
                            let valid = drag.shared == shared
                                && drag.path != folder_drag_path
                                && !playlist_folders::in_folder(
                                    &folder_drag_path,
                                    &drag.path,
                                    &view.state.settings.playlist_separator,
                                );
                            view.hover_sidebar_folder_drag(
                                &folder_drag_path,
                                shared,
                                valid && event.bounds.contains(&event.event.position),
                                cx,
                            );
                        },
                    ))
                    .on_drop(cx.listener(|view, _: &Dragged, _, cx| {
                        view.keep_sidebar_folder_drag_expansion(cx)
                    }))
                    .on_drop(cx.listener(|view, _: &FolderDrag, _, cx| {
                        view.keep_sidebar_folder_drag_expansion(cx)
                    }))
                    .child(
                        div()
                            .id(SharedString::from(format!("playlist-folder-drop-{path}")))
                            .debug_selector({
                                let path = path.clone();
                                move || {
                                    if shared {
                                        format!("playlist-folder-drop-shared:{path}")
                                    } else {
                                        format!("playlist-folder-drop-{path}")
                                    }
                                }
                            })
                            .on_drag(
                                FolderDrag {
                                    path: path.clone(),
                                    shared,
                                },
                                crate::context_preview::folder_drag_constructor(cx),
                            )
                            .on_drop(cx.listener(move |this, dragged: &Dragged, _, cx| {
                                this.keep_sidebar_folder_drag_expansion(cx);
                                this.move_sidebar_playlists(dragged, None, &destination, cx);
                                cx.stop_propagation();
                            }))
                            .on_drop(cx.listener(move |this, dragged: &FolderDrag, _, cx| {
                                this.keep_sidebar_folder_drag_expansion(cx);
                                this.move_sidebar_playlist_folder(dragged, &folder_destination, cx);
                                cx.stop_propagation();
                            }))
                            .child(self.playlist_folder_header(
                                node,
                                !closed && settings.playlist_folder_view != "navigation",
                                shared,
                                cx,
                            ))
                            .context_menu(move |menu, _, cx| {
                                let rename_view = view.clone();
                                let root_view = view.clone();
                                let rename = rename.clone();
                                let source = root_move.clone();
                                menu.item(
                                    PopupMenuItem::new(crate::appearance::text(
                                        "Rename folder",
                                        cx,
                                    ))
                                    .on_click(
                                        move |_, window, cx| {
                                            rename_view
                                                .update(cx, |this, cx| {
                                                    this.edit_playlist_folder(
                                                        rename.clone(),
                                                        shared,
                                                        window,
                                                        cx,
                                                    )
                                                })
                                                .ok();
                                        },
                                    ),
                                )
                                .item(
                                    PopupMenuItem::new(crate::appearance::text("Move to root", cx))
                                        .on_click(move |_, _, cx| {
                                            root_view
                                                .update(cx, |this, cx| {
                                                    this.move_sidebar_playlist_folder(
                                                        &FolderDrag {
                                                            path: source.clone(),
                                                            shared,
                                                        },
                                                        "",
                                                        cx,
                                                    )
                                                })
                                                .ok();
                                        }),
                                )
                            }),
                    )
                    .when(settings.playlist_folder_view != "navigation", |div| {
                        div.child(crate::sidebar_motion::FolderCollapse {
                            id: format!(
                                "playlist-folder-collapse-{:?}-{shared}-{path}",
                                self.state.active_server
                            )
                            .into(),
                            open: !closed,
                            content: gpui::div()
                                .v_flex()
                                .pl(if settings.playlist_folder_view == "single" {
                                    cx.theme().font_size * 0.75
                                } else {
                                    px(settings.playlist_folder_indent as f32 * settings.zoom)
                                })
                                .children(children.iter().enumerate().map(|(index, child)| {
                                    let row = self.playlist_node(child, shared, cx);
                                    if settings.playlist_folder_view == "single" {
                                        return row;
                                    }
                                    let indent = settings.playlist_folder_indent as f32;
                                    gpui::div()
                                        .relative()
                                        .pl(px((indent - 4.).max(0.) * settings.zoom))
                                        .child(
                                            gpui::div()
                                                .absolute()
                                                .left_0()
                                                .top_0()
                                                .h(relative(0.5))
                                                .w(px((indent - 8.).max(0.) * settings.zoom))
                                                .border_l(px(settings.zoom))
                                                .border_b(px(settings.zoom))
                                                .border_color(line_color),
                                        )
                                        .when(index + 1 < children.len(), |div| {
                                            div.child(
                                                gpui::div()
                                                    .absolute()
                                                    .left_0()
                                                    .top(relative(0.5))
                                                    .bottom_0()
                                                    .border_l(px(settings.zoom))
                                                    .border_color(line_color),
                                            )
                                        })
                                        .child(row)
                                        .into_any_element()
                                }))
                                .into_any_element(),
                        })
                    })
                    .into_any_element()
            }
        }
    }
    pub fn playlist_owner_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let owner = self
            .playlist_users
            .iter()
            .find(|(id, _)| id == &self.form_owner)
            .map_or(self.form_owner.clone(), |(_, name)| name.clone());
        let users = self.playlist_users.clone();
        let view = cx.weak_entity();
        div()
            .v_flex()
            .gap_2()
            .child(
                crate::styles::button("playlist-sync", cx)
                    .label(format!(
                        "Sync with server playlist file: {}",
                        self.form_sync.unwrap_or(false)
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.form_sync = Some(!this.form_sync.unwrap_or(false));
                        cx.notify();
                    })),
            )
            .when(
                self.api().is_some_and(|api| api.session().is_admin),
                |div| {
                    div.child(
                        crate::styles::button("playlist-owner", cx)
                            .label(format!("Owner: {owner}"))
                            .dropdown_menu(move |mut menu, _, _| {
                                for (id, name) in &users {
                                    let id = id.clone();
                                    let view = view.clone();
                                    menu = menu.item(PopupMenuItem::new(name.clone()).on_click(
                                        move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.form_owner = id.clone();
                                                cx.notify();
                                            })
                                            .ok();
                                        },
                                    ));
                                }
                                menu
                            }),
                    )
                },
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod row_tests {
    use crate::app::{Feishin, Route};
    use feishin_core::{
        api::Api,
        model::{Item, ItemKind, Server, ServerType, Session},
        playlist_folders::Node,
        queue::{Enqueue, PlayBehavior},
        storage::State,
    };
    use gpui::{
        AppContext, Context, Entity, Modifiers, MouseButton, Render, Role, TestAppContext, Window,
        div, point, prelude::*, px,
    };
    use gpui_component::{ActiveTheme, Root};
    use std::{cell::RefCell, rc::Rc, time::Duration};

    #[gpui::test]
    fn sidebar_drag_feedback_and_artwork_scale_match_original_frames(cx: &mut TestAppContext) {
        use crate::queue::Dragged;
        use gpui::{Empty, VisualTestContext};
        use gpui_component::StyledExt;
        struct Probe {
            app: Entity<Feishin>,
            items: Vec<Item>,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(480.))
                    .p_4()
                    .v_flex()
                    .gap_4()
                    .child(
                        div()
                            .id("feedback-source")
                            .debug_selector(|| "feedback-source".into())
                            .h(px(32.))
                            .child("Drag a song")
                            .on_drag(
                                Dragged {
                                    sidebar_playlist: false,
                                    playlist_source: None,
                                    keys: vec![],
                                    items: vec![Item::default()],
                                },
                                |_, _, _, cx| cx.new(|_| Empty),
                            ),
                    )
                    .children(self.items.iter().map(|item| {
                        self.app.update(cx, |app, cx| {
                            app.playlist_node(
                                &Node::Playlist {
                                    name: item.name.clone(),
                                    item: Box::new(item.clone()),
                                },
                                false,
                                cx,
                            )
                        })
                    }))
            }
        }
        cx.use_platform_text_system();
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-sidebar-feedback-{}", uuid::Uuid::new_v4()));
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.sidebar.playlist_sorting = true;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            let probe = cx.new(|cx| {
                cx.observe(&app, |_, _, cx| cx.notify()).detach();
                Probe {
                    app: app.clone(),
                    items: [false, true]
                        .map(|smart| Item {
                            id: if smart { "smart" } else { "regular" }.into(),
                            name: if smart { "Smart" } else { "Regular" }.into(),
                            kind: ItemKind::Playlist,
                            rules: smart.then(|| serde_json::json!({"all":[]})),
                            ..Default::default()
                        })
                        .into(),
                }
            });
            captured.replace(Some(app));
            Root::new(probe, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/sidebar-playlist-feedback-reference.json"
        ))
        .unwrap();
        let outside = point(px(650.), px(450.));
        let draw = |cx: &mut VisualTestContext| {
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        let alpha = |cx: &mut VisualTestContext, id: &str| {
            let bounds = cx
                .debug_bounds(&format!("sidebar-playlist-title-{id}"))
                .unwrap();
            let glyphs = cx.update(|window, _| window.painted_glyphs_with_opacity_for_test());
            let values = glyphs
                .iter()
                .filter(|(rect, _, _)| bounds.contains(&rect.center()))
                .map(|(_, _, color)| color.a)
                .collect::<Vec<_>>();
            assert!(!values.is_empty(), "No title glyphs for {id}");
            assert!(
                values
                    .iter()
                    .all(|value| (*value - values[0]).abs() < 0.00001)
            );
            values[0]
        };
        let highlighted = |cx: &mut VisualTestContext, id: &str| {
            let bounds = cx.debug_bounds(&format!("sidebar-playlist-{id}")).unwrap();
            cx.update(|window, cx| {
                window
                    .painted_borders_for_test()
                    .into_iter()
                    .any(|(rect, widths, radii, color)| {
                        rect == bounds
                            && widths.top == px(2.)
                            && radii.top_left == cx.theme().radius
                            && color == cx.theme().primary.opacity(0.8)
                    })
            })
        };
        draw(cx);
        for case in reference["opacity"].as_array().unwrap() {
            cx.simulate_mouse_move(outside, None, Modifiers::none());
            let source = cx.debug_bounds("feedback-source").unwrap().center();
            cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
            cx.simulate_mouse_move(
                source + point(px(12.), px(5.)),
                MouseButton::Left,
                Modifiers::none(),
            );
            assert!(cx.update(|_, cx| cx.has_active_drag()));
            let regular = cx
                .debug_bounds("sidebar-playlist-regular")
                .unwrap()
                .center();
            cx.simulate_mouse_move(regular, MouseButton::Left, Modifiers::none());
            draw(cx);
            assert!(
                highlighted(cx, "regular"),
                "bounds {:?}, borders {:?}",
                cx.debug_bounds("sidebar-playlist-regular"),
                cx.update(|window, _| window.painted_borders_for_test())
            );
            assert!(!highlighted(cx, "smart"));
            let mut previous = 0;
            for frame in case["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                cx.executor()
                    .advance_clock(Duration::from_millis(milliseconds - previous));
                previous = milliseconds;
                if case["reverseAt"].as_u64() == Some(milliseconds) {
                    cx.update(|window, cx| {
                        cx.stop_active_drag(window);
                    });
                }
                draw(cx);
                let actual = alpha(cx, "smart");
                assert!(
                    (actual - frame["value"].as_f64().unwrap() as f32).abs() < 0.0003,
                    "{case}, frame {frame}, opacity {actual}"
                );
                assert_eq!(alpha(cx, "regular"), 1.);
            }
            cx.update(|window, cx| {
                cx.stop_active_drag(window);
            });
            cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
            draw(cx);
            cx.executor().advance_clock(Duration::from_millis(300));
            draw(cx);
            assert_eq!(alpha(cx, "smart"), 1.);
            assert!(!highlighted(cx, "regular"));
        }
        // Start actual sidebar drags, including a smart source and disabled sorting.
        for sorting in [true, false] {
            app.update(cx, |app, cx| {
                app.state.settings.sidebar.playlist_sorting = sorting;
                cx.notify();
            });
            draw(cx);
            for id in ["regular", "smart"] {
                let bounds = cx.debug_bounds(&format!("sidebar-playlist-{id}")).unwrap();
                let source = bounds.origin + point(px(3.), px(2.));
                cx.simulate_mouse_down(source, MouseButton::Left, Modifiers::none());
                cx.simulate_mouse_move(
                    source + point(px(12.), px(5.)),
                    MouseButton::Left,
                    Modifiers::none(),
                );
                assert!(cx.update(|_, cx| {
                    cx.active_drag::<Dragged>()
                        .is_some_and(|drag| drag.sidebar_playlist)
                }));
                let target = cx
                    .debug_bounds("sidebar-playlist-regular")
                    .unwrap()
                    .center();
                cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
                draw(cx);
                cx.executor().advance_clock(Duration::from_millis(300));
                draw(cx);
                assert_eq!(alpha(cx, id), 0.5);
                assert_eq!(highlighted(cx, "regular"), sorting);
                assert!(!highlighted(cx, "smart"));
                cx.update(|window, cx| {
                    cx.stop_active_drag(window);
                });
                cx.simulate_mouse_up(outside, MouseButton::Left, Modifiers::none());
                draw(cx);
                cx.executor().advance_clock(Duration::from_millis(300));
                draw(cx);
                assert_eq!(alpha(cx, id), 1.);
            }
        }
        app.update(cx, |app, cx| {
            app.state.settings.sidebar.collapsed = true;
            cx.notify();
        });
        draw(cx);
        let bounds = cx.debug_bounds("compact-playlist-artwork-regular").unwrap();
        let row = cx.debug_bounds("sidebar-playlist-regular").unwrap();
        assert_eq!(bounds.center().x, row.center().x);
        assert_eq!(row.size.height, bounds.size.height + px(8.));
        cx.simulate_mouse_move(row.origin + point(px(2.), px(2.)), None, Modifiers::none());
        draw(cx);
        cx.update(|window, _| {
            assert!(
                window
                    .accessibility_tree_for_test()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| {
                        node.role() == Role::Button
                            && node.label().is_some_and(|label| label.contains('\n'))
                    }),
                "Padding hover did not show the original artwork playback control"
            );
        });
        cx.simulate_mouse_move(outside, None, Modifiers::none());
        draw(cx);
        cx.executor().advance_clock(Duration::from_millis(200));
        draw(cx);
        for case in reference["scale"].as_array().unwrap() {
            cx.simulate_mouse_move(
                if case["name"] == "hover" {
                    bounds.center()
                } else {
                    outside
                },
                None,
                Modifiers::none(),
            );
            draw(cx);
            let mut previous = 0;
            for frame in case["frames"].as_array().unwrap() {
                let milliseconds = frame["milliseconds"].as_u64().unwrap();
                cx.executor()
                    .advance_clock(Duration::from_millis(milliseconds - previous));
                previous = milliseconds;
                draw(cx);
                let actual = cx.update(|window, _| {
                    let tree = window.accessibility_tree_for_test().unwrap();
                    let rect = tree
                        .nodes
                        .iter()
                        .find(|(_, node)| {
                            node.role() == Role::Button && node.label() == Some("Regular")
                        })
                        .unwrap()
                        .1
                        .bounds()
                        .unwrap();
                    (rect.x1 - rect.x0) as f32
                        / window.scale_factor()
                        / f32::from(bounds.size.width)
                });
                assert!(
                    (actual - frame["value"].as_f64().unwrap() as f32).abs() < 0.00001,
                    "{case}, frame {frame}: {actual}"
                );
            }
        }
        // The expanded artwork hitbox also accepts a click outside its unscaled bounds.
        cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
        draw(cx);
        cx.executor().advance_clock(Duration::from_millis(200));
        draw(cx);
        let edge = point(bounds.origin.x - px(1.), bounds.origin.y + px(3.));
        cx.simulate_mouse_down(edge, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(edge, MouseButton::Left, Modifiers::none());
        draw(cx);
        app.read_with(cx, |app, _| {
            assert!(matches!(&app.route,Route::Detail(item) if item.id=="regular"));
            assert!(app.state.queue.entries.is_empty());
            assert!(app.add_to_playlist.is_none());
        });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn filtered_playlist_reorders_preserve_independent_owned_and_shared_orders(
        cx: &mut TestAppContext,
    ) {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../assets/sidebar-reorder-reference.json")).unwrap();
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            url: "http://127.0.0.1:9".into(),
            username: "me".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let dir =
            std::env::temp_dir().join(format!("feishin-scoped-order-{}", uuid::Uuid::new_v4()));
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            let mut state = State::default();
            state.settings.sidebar.playlist_list = false;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            captured.replace(Some(app.clone()));
            Root::new(app, window, cx)
        });
        let app = captured.borrow_mut().take().unwrap();
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.route = Route::Settings;
                app.state.active_server = Some(server.id);
                app.sessions.insert(
                    server.id,
                    Api::new(
                        server.clone(),
                        Session {
                            user_id: "me".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
                app.playlist_options = reference["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| Item {
                        id: value["id"].as_str().unwrap().into(),
                        name: value["name"].as_str().unwrap().into(),
                        kind: ItemKind::Playlist,
                        server_id: server.id,
                        owner_id: value["ownerId"].as_str().unwrap().into(),
                        owner_name: value["owner"].as_str().unwrap().into(),
                        ..Default::default()
                    })
                    .collect();
                app.state.settings.sidebar.playlist_sorting = true;
                for case in reference["cases"].as_array().unwrap() {
                    let initial = reference["initial"]["owned"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(reference["initial"]["shared"].as_array().unwrap())
                        .map(|id| id.as_str().unwrap().to_owned())
                        .collect();
                    app.state.sidebar_playlist_order.insert(server.id, initial);
                    app.state.settings.sidebar.playlist_filter_regex =
                        case["filter"].as_str().unwrap().into();
                    let moving = case["moving"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|id| {
                            app.playlist_options
                                .iter()
                                .find(|item| item.id == id.as_str().unwrap())
                                .unwrap()
                                .clone()
                        })
                        .collect::<Vec<_>>();
                    app.reorder_sidebar_playlists_at(
                        &moving,
                        case["target"].as_str(),
                        case["edge"] == "bottom",
                        cx,
                    );
                    let order = &app.state.sidebar_playlist_order[&server.id];
                    let scope = case["scope"] == "shared";
                    let visible = app
                        .state
                        .settings
                        .sidebar
                        .playlists(&app.playlist_options, order)
                        .into_iter()
                        .filter(|item| app.is_shared_playlist(item) == scope)
                        .map(|item| item.id)
                        .collect::<Vec<_>>();
                    let ids = |value: &serde_json::Value| {
                        value
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|id| id.as_str().unwrap().to_owned())
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(visible, ids(&case["visible"]), "{case}");
                    for name in ["owned", "shared"] {
                        let actual = order
                            .iter()
                            .filter(|id| {
                                app.playlist_options.iter().any(|item| {
                                    &item.id == *id
                                        && app.is_shared_playlist(item) == (name == "shared")
                                })
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        assert_eq!(actual, ids(&case["orders"][name]), "{case}");
                    }
                    app.state.settings.sidebar.playlist_filter_regex.clear();
                    let unfiltered = app
                        .state
                        .settings
                        .sidebar
                        .playlists(&app.playlist_options, order)
                        .into_iter()
                        .filter(|item| app.is_shared_playlist(item) == scope)
                        .map(|item| item.id)
                        .collect::<Vec<_>>();
                    assert_eq!(unfiltered, ids(&case["unfiltered"]), "{case}");
                }
                app.state.sidebar_playlist_order.insert(
                    server.id,
                    ["c", "b", "a", "u", "t", "s"].map(str::to_owned).into(),
                );
                let item = app
                    .playlist_options
                    .iter()
                    .find(|item| item.id == "b")
                    .unwrap()
                    .clone();
                app.move_sidebar_playlist(item.clone(), false, cx);
                assert_eq!(
                    app.state.sidebar_playlist_order[&server.id],
                    ["u", "t", "s", "c", "a", "b"]
                );
                app.move_sidebar_playlist(item.clone(), false, cx);
                assert_eq!(
                    app.state.sidebar_playlist_order[&server.id],
                    ["u", "t", "s", "c", "a", "b"]
                );
                app.move_sidebar_playlist(item, true, cx);
                assert_eq!(
                    app.state.sidebar_playlist_order[&server.id],
                    ["u", "t", "s", "c", "b", "a"]
                );
                assert_eq!(app.route, Route::Settings);
                assert!(app.state.queue.entries.is_empty());
            })
        });
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[gpui::test]
    fn sidebar_playlist_metadata_hover_and_queue_actions_match_original(cx: &mut TestAppContext) {
        cx.use_platform_text_system();
        struct Probe {
            app: Entity<Feishin>,
            item: Item,
        }
        impl Render for Probe {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .w(px(480.))
                    .h(px(100.))
                    .p_4()
                    .child(self.app.update(cx, |app, cx| {
                        app.playlist_node(
                            &Node::Playlist {
                                name: self.item.name.clone(),
                                item: Box::new(self.item.clone()),
                            },
                            false,
                            cx,
                        )
                    }))
            }
        }
        cx.set_assets(crate::icons::Assets);
        cx.update(gpui_component::init);
        let dir =
            std::env::temp_dir().join(format!("feishin-sidebar-controls-{}", uuid::Uuid::new_v4()));
        let server = Server {
            id: uuid::Uuid::new_v4(),
            name: "Fixture".into(),
            url: "http://127.0.0.1:9".into(),
            username: "me".into(),
            kind: ServerType::Navidrome,
            legacy_auth: false,
            options: Default::default(),
        };
        let captured = Rc::new(RefCell::new(None));
        let (_, cx) = cx.add_window_view(|window, cx| {
            window.activate_accessibility_for_test();
            let mut state = State::default();
            state.settings.sidebar.playlist_sorting = false;
            state.settings.play_button_behavior = PlayBehavior::Next;
            let app = cx.new(|cx| Feishin::new(dir.clone(), Ok(state), window, cx));
            app.update(cx, |app, _| {
                app.sessions.insert(
                    server.id,
                    Api::new(
                        server.clone(),
                        Session {
                            user_id: "me".into(),
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                );
            });
            let probe = cx.new(|cx| {
                cx.observe(&app, |_, _, cx| cx.notify()).detach();
                Probe {
                    app: app.clone(),
                    item: Item::default(),
                }
            });
            captured.replace(Some((app, probe.clone())));
            Root::new(probe, window, cx)
        });
        let (app, probe) = captured.borrow_mut().take().unwrap();
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../assets/sidebar-playlist-controls-reference.json"
        ))
        .unwrap();
        let known_texts = reference["cases"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|case| case["texts"].as_array().unwrap())
            .map(|value| value.as_str().unwrap())
            .collect::<std::collections::HashSet<_>>();
        for case in reference["cases"].as_array().unwrap() {
            cx.simulate_mouse_move(point(px(800.), px(600.)), None, Modifiers::none());
            let item = Item {
                id: "playlist".into(),
                kind: ItemKind::Playlist,
                server_id: server.id,
                name: "Daily".into(),
                duration: 125.,
                owner_id: if case["own"] == true { "me" } else { "friend" }.into(),
                owner_name: "Alex".into(),
                public: case["isPublic"] == true,
                column_data: std::collections::BTreeMap::from([("songCount".into(), "24".into())]),
                playlist_songs: (0..3)
                    .map(|index| Item {
                        id: format!("track-{index}"),
                        server_id: server.id,
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            };
            cx.update(|window, cx| {
                app.update(cx, |app, cx| {
                    app.route = Route::Settings;
                    app.state.settings.language = case["language"].as_str().unwrap().into();
                    cx.set_global(crate::appearance::Language(
                        app.state.settings.language.clone(),
                    ));
                    app.state.settings.sidebar.collapsed = case["mode"] == "icon";
                    app.state.settings.sidebar.playlist_mode =
                        case["mode"].as_str().unwrap().into();
                    app.sidebar_playlist_hover = None;
                    app.queue_prompt_open = true;
                    app.state.queue = Default::default();
                    app.state.queue.add(
                        vec![Item {
                            id: "kept-a".into(),
                            context_playlist_id: (case["active"] == true)
                                .then(|| "playlist".into()),
                            ..Default::default()
                        }],
                        Enqueue::Now,
                    );
                    cx.notify();
                });
                probe.update(cx, |probe, cx| {
                    probe.item = item;
                    cx.notify();
                });
                window.refresh();
            });
            cx.run_until_parked();
            if case["hovered"] == true {
                let selector = if case["mode"] == "icon" {
                    "compact-playlist-artwork-playlist"
                } else {
                    "sidebar-playlist-playlist"
                };
                let bounds = cx.debug_bounds(selector).unwrap();
                cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
                cx.run_until_parked();
            }
            cx.update(|window, _| {
                let tree = window.accessibility_tree_for_test().unwrap();
                let labels = tree
                    .nodes
                    .iter()
                    .filter(|(_, node)| node.role() == Role::Label)
                    .filter_map(|(_, node)| node.value())
                    .filter(|value| known_texts.contains(value))
                    .collect::<Vec<_>>();
                let expected = case["texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| value.as_str().unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(labels, expected, "{case}");
                let controls = tree
                    .nodes
                    .iter()
                    .filter(|(_, node)| {
                        node.role() == Role::Button
                            && !node.is_hidden()
                            && node.label().is_some_and(|label| label.contains('\n'))
                    })
                    .count();
                assert_eq!(
                    controls,
                    if case["controls"] == true {
                        3
                    } else if case["play"] == true {
                        1
                    } else {
                        0
                    },
                    "{case}"
                );
            });
            if case["mode"] != "icon" {
                let title = cx.debug_bounds("sidebar-playlist-title-playlist").unwrap();
                cx.update(|window, cx| {
                    let expected = if case["highlighted"] == true {
                        cx.theme().primary
                    } else {
                        cx.theme().foreground
                    };
                    let glyphs = window
                        .painted_glyphs_for_test()
                        .into_iter()
                        .filter(|(bounds, _, _)| title.contains(&bounds.center()))
                        .collect::<Vec<_>>();
                    assert!(!glyphs.is_empty(), "Title did not paint: {case}");
                    assert!(
                        glyphs.iter().all(|(_, _, color)| *color == expected),
                        "Wrong active title color: {case}"
                    );
                });
            }
            for call in case["calls"].as_array().unwrap() {
                let mode = call["mode"].as_str().unwrap();
                let replace = matches!(mode, "now" | "shuffle");
                let held = matches!(mode, "shuffle" | "nextShuffle" | "lastShuffle");
                cx.update(|_, cx| {
                    app.update(cx, |app, cx| {
                        app.pending_queue_change = None;
                        app.state.settings.confirm_queue_changes = true;
                        app.state.queue = Default::default();
                        app.state.queue.add(
                            ["kept-a", "kept-b"]
                                .map(|id| Item {
                                    id: id.into(),
                                    ..Default::default()
                                })
                                .into(),
                            Enqueue::Now,
                        );
                        cx.notify();
                    })
                });
                cx.run_until_parked();
                let selector = if case["mode"] == "icon" {
                    "compact-playlist-play-playlist"
                } else if replace {
                    "sidebar-play-playlist-now"
                } else if mode.starts_with("next") {
                    "sidebar-play-playlist-next"
                } else {
                    "sidebar-play-playlist-last"
                };
                let bounds = cx.debug_bounds(selector).unwrap();
                cx.simulate_mouse_move(bounds.center(), None, Modifiers::none());
                cx.simulate_mouse_down(bounds.center(), MouseButton::Left, Modifiers::none());
                cx.run_until_parked();
                if held {
                    cx.executor().advance_clock(Duration::from_millis(501));
                    cx.run_until_parked();
                }
                cx.simulate_mouse_up(bounds.center(), MouseButton::Left, Modifiers::none());
                cx.run_until_parked();
                app.read_with(cx, |app, _| {
                    assert_eq!(app.route, Route::Settings, "Playback navigated: {case}");
                    assert!(app.library_fetches.is_empty());
                    let songs = if replace {
                        let Some(pending) = &app.pending_queue_change else {
                            panic!("Missing confirmation: {call}")
                        };
                        let crate::radio::QueueChange::Replace(songs, _) = &pending.change else {
                            panic!("Wrong confirmation")
                        };
                        songs.clone()
                    } else {
                        assert!(app.pending_queue_change.is_none());
                        let ordered = app.state.queue.ordered();
                        assert_eq!(ordered.len(), 5, "Held release duplicated playback: {call}");
                        let range = if mode.starts_with("next") { 1..4 } else { 2..5 };
                        ordered[range]
                            .iter()
                            .map(|entry| entry.item.clone())
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(songs.len(), 3, "{call}");
                    let mut ids = songs
                        .iter()
                        .map(|song| song.id.as_str())
                        .collect::<Vec<_>>();
                    ids.sort();
                    assert_eq!(ids, ["track-0", "track-1", "track-2"]);
                    assert!(
                        songs
                            .iter()
                            .all(|song| song.context_playlist_id.as_deref() == Some("playlist"))
                    );
                    assert!(app.history.is_empty());
                });
            }
        }
        cx.simulate_mouse_move(point(px(800.), px(600.)), None, Modifiers::none());
        let before = cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.state.settings.sidebar.collapsed = false;
                app.state.settings.sidebar.playlist_mode = "expanded".into();
                app.library_fetches.insert(
                    u64::MAX,
                    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                );
                cx.notify();
                serde_json::to_value(&app.state.queue).unwrap()
            })
        });
        cx.run_until_parked();
        let row = cx.debug_bounds("sidebar-playlist-playlist").unwrap();
        cx.simulate_mouse_move(row.center(), None, Modifiers::none());
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update(|window, _| {
            let controls = window
                .accessibility_tree_for_test()
                .unwrap()
                .nodes
                .iter()
                .filter(|(_, node)| {
                    node.role() == Role::Button
                        && node.label().is_some_and(|label| label.contains('\n'))
                })
                .collect::<Vec<_>>();
            assert_eq!(controls.len(), 3);
            assert!(controls.iter().all(|(_, node)| node.is_disabled()));
        });
        let next = cx
            .debug_bounds("sidebar-play-playlist-next")
            .unwrap()
            .center();
        cx.simulate_mouse_down(next, MouseButton::Left, Modifiers::none());
        cx.executor().advance_clock(Duration::from_millis(501));
        cx.run_until_parked();
        cx.simulate_mouse_up(next, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        app.read_with(cx, |app, _| {
            assert_eq!(serde_json::to_value(&app.state.queue).unwrap(), before);
            assert_eq!(app.route, Route::Settings);
        });
        std::fs::remove_dir_all(dir).unwrap();
    }
}
